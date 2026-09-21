package ai.arcships.aimux

import com.sun.jna.ptr.IntByReference
import com.sun.jna.ptr.PointerByReference
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.*
import java.io.Closeable
import java.util.concurrent.CancellationException
import java.util.concurrent.atomic.AtomicLong

/** Pure data transport; user functions run after each native call returns. */
internal class HostOperation(model: Model, mode: String, prompt: String, options: String?, private val repair: ToolCallRepair) : Closeable {
    private val handle = AtomicLong(model.startOperation(buildJsonObject {
        put("protocol_version", 1); put("mode", mode)
        put("prompt", AimuxJson.parseToJsonElement(prompt))
        put("options", AimuxJson.parseToJsonElement(options ?: "{}"))
        put("repair_tool_call", true)
    }.toString()))

    fun next(): String? {
        while (handle.get() != 0L) {
            if (Thread.currentThread().isInterrupted) { close(); throw CancellationException("operation interrupted") }
            val h = handle.get()
            val out = PointerByReference(); val state = IntByReference()
            FFI.lib.aimux_operation_next(h, 2, 100, out, state)?.let { throw expectAimuxError(it) }
            if (state.value == 1) continue
            if (state.value == 2) return null
            check(state.value == 0) { "concurrent operation reader" }
            val event = AimuxJson.parseToJsonElement(takeString(out.value)!!).jsonObject
            val type = event.getValue("type").jsonPrimitive.content
            if (type != "repair_request") return event.getValue(if (type == "part") "part" else "result").toString()
            val reply = try {
                val context = AimuxJson.decodeFromJsonElement(ToolCallRepairContext.serializer(), event.getValue("context"))
                val value = repair(context)
                buildJsonObject {
                    put("type", if (value == null) "unchanged" else "repaired")
                    if (value != null) put("tool_call", AimuxJson.encodeToJsonElement(RawToolCall.serializer(), value))
                }
            } catch (e: CancellationException) { throw e }
            catch (e: InterruptedException) { Thread.currentThread().interrupt(); throw CancellationException("repair interrupted") }
            catch (e: Exception) { buildJsonObject { put("type", "failed"); put("message", e.toString()) } }
            if (handle.get() == 0L) return null
            val status = IntByReference()
            FFI.lib.aimux_operation_reply(h, event.getValue("request_id").jsonPrimitive.content, reply.toString(), status)?.let { throw expectAimuxError(it) }
            check(status.value == 0 || status.value == 3) { "unexpected operation reply status" }
        }
        return null
    }

    override fun close() { handle.getAndSet(0).takeIf { it != 0L }?.let { FFI.lib.aimux_operation_drop(it) } }

    companion object {
        fun result(model: Model, mode: String, prompt: String, options: String?, repair: ToolCallRepair): String =
            HostOperation(model, mode, prompt, options, repair).use { it.next() ?: error("operation ended without result") }

        fun stream(model: Model, mode: String, prompt: String, options: String?, repair: ToolCallRepair, emit: (String) -> Unit) {
            HostOperation(model, mode, prompt, options, repair).use { op ->
                while (true) emit(op.next() ?: break)
            }
        }
    }
}
