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
import java.io.Closeable
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
 * caught, leave the original error on the tool call, and are kept in
 * [lastError].
 *
 * [Closeable]: close it once no call that references it is in flight.
 */
@Serializable(with = ToolCallRepairSerializer::class)
class ToolCallRepair(private val fn: (ToolCallRepairContext) -> RawToolCall?) : Closeable {

    /**
     * The last Throwable the repair function (or the context/result JSON
     * conversion) raised. The C contract carries only "repaired" or "not
     * repaired", so a failing function leaves the original validation error on
     * the tool call; this is where the cause is kept.
     */
    @Volatile
    var lastError: Throwable? = null
        private set

    // JNA collects a Callback that is only reachable from native code, so it is
    // held here for as long as this object — and therefore the handle — lives.
    private val callback = object : Callback {
        @Suppress("unused")
        fun callback(contextJson: Pointer?, @Suppress("UNUSED_PARAMETER") userData: Pointer?): Pointer? =
            invoke(contextJson)
    }

    private val handle = AtomicLong(FFI.lib.aimux_tool_call_repair_new(callback, null))

    /** Release the FFI handle. Idempotent; calls already in flight keep their clone. */
    override fun close() {
        val h = handle.getAndSet(0L)
        if (h != 0L) FFI.lib.aimux_tool_call_repair_drop(h)
    }

    internal fun handleValue(): Long = handle.get()

    // An exception must never unwind through the C frames into Rust, so every
    // Throwable stops here and becomes "not repaired".
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
        lastError = t
        null
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
