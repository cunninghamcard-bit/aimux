/**
 * aimux — host repair functions for invalid tool calls (AI SDK `repairToolCall`).
 *
 * [ToolCallRepair] registers a Kotlin function with the C ABI and marshals as
 * the returned handle, so it sits in [GenerateTextOptions.repairToolCall] and
 * reaches Core through `opts_json` like any other option.
 */

package ai.arcships.aimux

import com.sun.jna.Callback
import com.sun.jna.Pointer
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import java.io.Closeable
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * A tool call before Core parses its input: [input] is the model's argument
 * text verbatim, possibly malformed.
 */
@Serializable
data class RawToolCall(
    @SerialName("tool_call_id") val toolCallId: String,
    @SerialName("tool_name") val toolName: String,
    val input: String,
    @SerialName("provider_executed") val providerExecuted: Boolean? = null,
    val dynamic: Boolean? = null,
    @SerialName("thought_signature") val thoughtSignature: String? = null,
    @SerialName("provider_metadata") val providerMetadata: JsonElement? = null,
)

/**
 * What a repair function receives — the AI SDK `repairToolCall` arguments.
 */
@Serializable
data class ToolCallRepairContext(
    /** The call that failed tool lookup, JSON parsing, or schema validation. */
    @SerialName("tool_call") val toolCall: RawToolCall,
    /** The typed failure (`NoSuchTool` / `InvalidToolInput`), same shape as [ToolCall.error]. */
    val error: JsonElement? = null,
    /** JSON Schema of the called tool; an empty-object schema when the tool is unknown. */
    @SerialName("input_schema") val inputSchema: JsonElement = JsonObject(emptyMap()),
    val tools: List<Tool> = emptyList(),
    /** The prompt of the current step. */
    val messages: List<ModelMessage> = emptyList(),
    val instructions: String? = null,
)

/**
 * A registered repair function. Pass it as [GenerateTextOptions.repairToolCall];
 * it serializes as its FFI handle.
 *
 * [fn] gets one attempt to fix an invalid tool call: return the repaired call,
 * or `null` to keep the original validation error. Core parses and validates
 * the returned call from scratch.
 *
 * It runs **synchronously on the thread that called `generateText` /
 * `streamText`**, inside the FFI re-entrancy guard: it must not call back into
 * aimux (that fails with code 204). Throwables never reach Rust — they are
 * caught and reported to Core, which records them on the tool call as
 * `ToolCallRepairError` with the throwable's text as its `cause`.
 *
 * [Closeable]: it must be closed, and stays alive until then.
 */
@Serializable(with = ToolCallRepairSerializer::class)
class ToolCallRepair(private val fn: (ToolCallRepairContext) -> RawToolCall?) : Closeable {

    // JNA collects a Callback that is only reachable from native code, so it is
    // held here for as long as this object — and therefore the handle — lives.
    private val callback = object : Callback {
        @Suppress("unused")
        fun callback(contextJson: Pointer?, @Suppress("UNUSED_PARAMETER") userData: Pointer?): Pointer? =
            invoke(contextJson)
    }

    private val handle = AtomicLong(FFI.lib.aimux_tool_call_repair_new(callback, null))

    init {
        handle.get().let { if (it != 0L) registered[it] = this }
    }

    /**
     * Release the FFI handle and stop keeping this object alive. Idempotent.
     *
     * Calls already in flight are disarmed and behave as if the function had
     * returned `null`, so closing is safe as long as no invocation is executing
     * on another thread at that instant.
     */
    override fun close() {
        val h = handle.getAndSet(0L)
        if (h != 0L) {
            FFI.lib.aimux_tool_call_repair_drop(h)
            registered.remove(h)
        }
    }

    internal fun handleValue(): Long = handle.get()

    // An exception must never unwind through the C frames into Rust, so every
    // Throwable stops here and comes back as the error envelope, which Core
    // records as ToolCallRepair {original_error, cause}.
    private fun invoke(contextJson: Pointer?): Pointer? = try {
        if (contextJson == null) {
            null
        } else {
            val context = AimuxJson.decodeFromString(
                ToolCallRepairContext.serializer(), contextJson.getString(0, "UTF-8")
            )
            fn(context)?.let {
                // aimux owns the returned string, so it has to come from its allocator.
                FFI.lib.aimux_string_new(AimuxJson.encodeToString(RawToolCall.serializer(), it))
            }
        }
    } catch (t: Throwable) {
        FFI.lib.aimux_string_new(JsonObject(mapOf("error" to JsonPrimitive(t.toString()))).toString())
    }

    private companion object {
        // Rust holds only a raw function pointer, and JNA tracks callbacks
        // weakly: the options object is dead once serialized, so without this a
        // GC during the HTTP wait frees the trampoline Rust is about to call.
        // Leaking a forgotten repair beats crashing on one.
        val registered = ConcurrentHashMap<Long, ToolCallRepair>()
    }
}

/** Encodes a [ToolCallRepair] as its FFI handle; a closed one encodes as `null` (= absent). */
object ToolCallRepairSerializer : KSerializer<ToolCallRepair> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("aimux.ToolCallRepair", PrimitiveKind.LONG)

    // encodeNull: a closed repair marshals as JSON null, which Core reads as "no repair".
    @OptIn(ExperimentalSerializationApi::class)
    override fun serialize(encoder: Encoder, value: ToolCallRepair) {
        val handle = value.handleValue()
        if (handle == 0L) encoder.encodeNull() else encoder.encodeLong(handle)
    }

    override fun deserialize(decoder: Decoder): ToolCallRepair =
        throw SerializationException("ToolCallRepair is an FFI handle and cannot be decoded")
}
