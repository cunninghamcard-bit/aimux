import Foundation

// MARK: - Tool call repair (AI SDK `repairToolCall`)

/// A tool call before Core parses its input: `input` is the model's argument
/// text verbatim, possibly malformed.
public struct RawToolCall: Codable, Equatable {
    public var toolCallId: String
    public var toolName: String
    public var input: String
    public var providerExecuted: Bool?
    public var dynamic: Bool?
    public var thoughtSignature: String?
    public var providerMetadata: JSONValue?

    enum CodingKeys: String, CodingKey {
        case toolCallId = "tool_call_id"
        case toolName = "tool_name"
        case input
        case providerExecuted = "provider_executed"
        case dynamic
        case thoughtSignature = "thought_signature"
        case providerMetadata = "provider_metadata"
    }

    public init(toolCallId: String, toolName: String, input: String,
                providerExecuted: Bool? = nil, dynamic: Bool? = nil,
                thoughtSignature: String? = nil, providerMetadata: JSONValue? = nil) {
        self.toolCallId = toolCallId; self.toolName = toolName; self.input = input
        self.providerExecuted = providerExecuted; self.dynamic = dynamic
        self.thoughtSignature = thoughtSignature; self.providerMetadata = providerMetadata
    }
}

/// What a `ToolCallRepair` closure receives — the AI SDK `repairToolCall`
/// arguments.
public struct ToolCallRepairContext: Decodable {
    /// The call that failed tool lookup, JSON parsing, or schema validation.
    public var toolCall: RawToolCall
    /// The typed failure, same shape as `ToolCall.error`.
    public var error: JSONValue
    /// JSON Schema of the called tool; an empty-object schema when the tool
    /// is unknown.
    public var inputSchema: JSONValue
    public var tools: [Tool]
    /// The prompt of the current step.
    public var messages: [ModelMessage]
    public var instructions: String?

    enum CodingKeys: String, CodingKey {
        case toolCall = "tool_call"
        case error
        case inputSchema = "input_schema"
        case tools, messages, instructions
    }
}

/// Synchronous repair runs on the calling thread, outside a native callback.
public typealias ToolCallRepair = (ToolCallRepairContext) throws -> RawToolCall?

/// Async repair runs on MainActor. The transport waits on worker threads.
public typealias AsyncToolCallRepair = @MainActor (ToolCallRepairContext) async throws -> RawToolCall?
