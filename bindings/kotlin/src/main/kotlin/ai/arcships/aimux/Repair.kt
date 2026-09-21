package ai.arcships.aimux

import kotlinx.serialization.Serializable
import kotlinx.serialization.SerialName
import kotlinx.serialization.json.*

/** A synchronous host function; executed on the calling/iteration thread. */
typealias ToolCallRepair = (ToolCallRepairContext) -> RawToolCall?

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
    /** The prompt of the current step, as wire JSON — a message shape this codec does not model must not fail the repair before it runs. */
    val messages: List<JsonElement> = emptyList(),
    val instructions: String? = null,
)
