// repair.dart — host `repairToolCall` functions (dart:ffi, C ABI path).
//
// A [ToolCallRepair] wraps a Dart function as an aimux handle
// (`aimux_tool_call_repair_new`) and marshals as that handle, so it sits in
// `GenerateTextOptions.repairToolCall` and reaches Core through `opts_json`
// (`"repair_tool_call": <handle>`) like any other option.
//
// Its own lookup block, like `multimodal.dart`: the repair context is handed
// over in the typed vocabulary of `types.dart`, and putting that dependency on
// the raw Map-based `aimux.dart` would invert the layering.

import 'dart:convert';
import 'dart:ffi';

import 'package:ffi/ffi.dart';

import 'errors.dart';
import 'types.dart';

// ─────────────────────────────────────────────────────────────────────────────
// FFI
// ─────────────────────────────────────────────────────────────────────────────

// aimux_tool_call_repair_fn: (context_json, user_data) → repaired JSON | NULL.
typedef _RepairFnC = Pointer<Utf8> Function(
    Pointer<Utf8> contextJson, Pointer<Void> userData);

typedef _RepairNewC = Uint64 Function(
    Pointer<NativeFunction<_RepairFnC>> repair, Pointer<Void> userData);
typedef _RepairNewDart = int Function(
    Pointer<NativeFunction<_RepairFnC>> repair, Pointer<Void> userData);

typedef _RepairDropC = Void Function(Uint64 handle);
typedef _RepairDropDart = void Function(int handle);

typedef _StringNewC = Pointer<Utf8> Function(Pointer<Utf8> s);
typedef _StringNewDart = Pointer<Utf8> Function(Pointer<Utf8> s);

/// Lazily opened, as in `errors.dart`: a pure-Dart test that never constructs
/// a [ToolCallRepair] does not dlopen the library.
final DynamicLibrary _lib = openAimuxLibrary();

final _repairNew = _lib.lookupFunction<_RepairNewC, _RepairNewDart>(
    'aimux_tool_call_repair_new');
final _repairDrop = _lib.lookupFunction<_RepairDropC, _RepairDropDart>(
    'aimux_tool_call_repair_drop');
final _stringNew =
    _lib.lookupFunction<_StringNewC, _StringNewDart>('aimux_string_new');

// ─────────────────────────────────────────────────────────────────────────────
// Callback trampoline
// ─────────────────────────────────────────────────────────────────────────────

/// `user_data` → the [ToolCallRepair] that owns the Dart function.
/// [Pointer.fromFunction] takes a top-level function and cannot capture, so
/// the key travels through `user_data` (the same round trip cgo.Handle makes
/// in the Go binding).
final Map<int, ToolCallRepair> _registered = {};
int _nextKey = 1;

/// The native side reads a NULL return as "not repaired", which is also what
/// [Pointer.fromFunction] returns for a `Pointer`-typed callback that throws —
/// but [ToolCallRepair._invoke] catches first, so the cause is not lost.
Pointer<Utf8> _onRepair(Pointer<Utf8> contextJson, Pointer<Void> userData) {
  final repair = _registered[userData.address];
  if (repair == null || contextJson == nullptr) return nullptr;
  return repair._invoke(contextJson);
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

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
        toolCall: RawToolCall.fromJson(json['tool_call'] as Map<String, dynamic>),
        error: json['error'],
        inputSchema: json['input_schema'] as Map<String, dynamic>,
        tools: (json['tools'] as List<dynamic>)
            .map((e) => Tool.fromJson(e as Map<String, dynamic>))
            .toList(),
        messages: (json['messages'] as List<dynamic>)
            .cast<Map<String, dynamic>>(),
        instructions: json['instructions'] as String?,
      );
}

/// A Dart function registered as `GenerateTextOptions.repairToolCall`
/// (AI SDK `repairToolCall`). It gets one attempt to fix an invalid tool call:
/// return the repaired call, or null to keep the original validation error.
/// Core parses and validates the returned call from scratch.
///
/// The function runs **synchronously on the isolate that called**
/// `generateText` / `streamText`, while that call is in progress, and inside
/// the FFI re-entrancy guard: it must not call back into aimux — that fails
/// with `AIMUX_E_FFI_REENTRANT_CALL`.
///
/// Call [close] once no call referencing it is in flight.
///
/// ```dart
/// final repair = ToolCallRepair((ctx) => RawToolCall(
///       toolCallId: ctx.toolCall.toolCallId,
///       toolName: ctx.toolCall.toolName,
///       input: '${ctx.toolCall.input}}',
///     ));
/// try {
///   typed.generateText(prompt, GenerateTextOptions(
///       tools: tools, repairToolCall: repair));
/// } finally {
///   repair.close();
/// }
/// ```
class ToolCallRepair {
  final RawToolCall? Function(ToolCallRepairContext) _repair;
  final int _key;
  int _nativeHandle;
  Object? _lastError;

  ToolCallRepair(this._repair)
      : _key = _nextKey++,
        _nativeHandle = 0 {
    _registered[_key] = this;
    _nativeHandle = _repairNew(Pointer.fromFunction<_RepairFnC>(_onRepair),
        Pointer<Void>.fromAddress(_key));
  }

  /// The FFI handle, which is how the function reaches `opts_json`
  /// (`"repair_tool_call": <handle>`). Null once [close]d — the native side
  /// reads a null field as "no repair function", so a closed repair degrades
  /// to the default instead of failing the call.
  int? get handle => _nativeHandle == 0 ? null : _nativeHandle;

  /// The last exception the Dart function threw (or a failure decoding the
  /// context / encoding the result). The C contract carries only "repaired"
  /// or "not repaired", so a failing function leaves the original validation
  /// error on the tool call; this is where the cause is kept.
  Object? get lastError => _lastError;

  /// Release the FFI handle. Idempotent. Calls already in flight keep their
  /// clone on the native side.
  void close() {
    if (_nativeHandle == 0) return;
    _repairDrop(_nativeHandle);
    _nativeHandle = 0;
    _registered.remove(_key);
  }

  /// Runs the Dart function. `nullptr` means "keep the original error".
  ///
  /// Nothing may escape into the C ABI (undefined behavior, and the Rust
  /// frames below cannot unwind a Dart exception), so every failure is caught
  /// and kept in [lastError].
  Pointer<Utf8> _invoke(Pointer<Utf8> contextJson) {
    try {
      final context = ToolCallRepairContext.fromJson(
          jsonDecode(contextJson.toDartString()) as Map<String, dynamic>);
      final repaired = _repair(context);
      if (repaired == null) return nullptr;
      // aimux frees the returned string, so it has to come from its allocator.
      final json = toCString(encodeJson(repaired.toJson(), 'repaired tool call'));
      try {
        return _stringNew(json);
      } finally {
        calloc.free(json);
      }
    } catch (e) {
      _lastError = e;
      return nullptr;
    }
  }
}
