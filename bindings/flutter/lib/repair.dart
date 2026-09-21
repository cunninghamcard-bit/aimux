import 'dart:async';
import 'types.dart';

/// Runs in the caller's isolate. Async functions require an async entry point.
typedef ToolCallRepair = FutureOr<RawToolCall?> Function(
    ToolCallRepairContext context);

/// A tool call before Core parses its input: [input] is the model's argument
/// text verbatim, possibly malformed.
class RawToolCall {
  final String toolCallId;
  final String toolName;
  final String input;
  final bool? providerExecuted;
  // `dynamic` is a Dart built-in identifier — same rename as [ToolCall].
  final bool? isDynamic;
  final String? thoughtSignature;
  final dynamic providerMetadata;

  RawToolCall({
    required this.toolCallId,
    required this.toolName,
    required this.input,
    this.providerExecuted,
    this.isDynamic,
    this.thoughtSignature,
    this.providerMetadata,
  });

  factory RawToolCall.fromJson(Map<String, dynamic> json) => RawToolCall(
        toolCallId: json['tool_call_id'] as String,
        toolName: json['tool_name'] as String,
        input: json['input'] as String,
        providerExecuted: json['provider_executed'] as bool?,
        isDynamic: json['dynamic'] as bool?,
        thoughtSignature: json['thought_signature'] as String?,
        providerMetadata: json['provider_metadata'],
      );

  Map<String, dynamic> toJson() => {
        'tool_call_id': toolCallId,
        'tool_name': toolName,
        'input': input,
        if (providerExecuted != null) 'provider_executed': providerExecuted,
        if (isDynamic != null) 'dynamic': isDynamic,
        if (thoughtSignature != null) 'thought_signature': thoughtSignature,
        if (providerMetadata != null) 'provider_metadata': providerMetadata,
      };
}

/// What a repair function receives — the AI SDK `repairToolCall` arguments.
class ToolCallRepairContext {
  /// The call that failed lookup, JSON parsing, or schema validation.
  final RawToolCall toolCall;

  /// The typed failure (`NoSuchTool` or `InvalidToolInput`) as wire JSON —
  /// the same shape as [ToolCall.error].
  final dynamic error;

  /// JSON Schema of the called tool; an empty-object schema for an unknown tool.
  final Map<String, Object?> inputSchema;
  final List<Tool> tools;

  /// The prompt of the current step as wire JSON.
  final List<Map<String, dynamic>> messages;
  final String? instructions;

  ToolCallRepairContext({
    required this.toolCall,
    required this.error,
    required this.inputSchema,
    required this.tools,
    required this.messages,
    this.instructions,
  });

  factory ToolCallRepairContext.fromJson(Map<String, dynamic> json) =>
      ToolCallRepairContext(
        toolCall:
            RawToolCall.fromJson(json['tool_call'] as Map<String, dynamic>),
        error: json['error'],
        inputSchema: json['input_schema'] as Map<String, dynamic>,
        tools: (json['tools'] as List<dynamic>)
            .map((e) => Tool.fromJson(e as Map<String, dynamic>))
            .toList(),
        messages:
            (json['messages'] as List<dynamic>).cast<Map<String, dynamic>>(),
        instructions: json['instructions'] as String?,
      );
}
