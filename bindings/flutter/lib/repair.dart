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

// aimux_tool_call_repair_fn: (context_json, user_data) → reply JSON | NULL.
// The reply is the repaired call, or `{"error": "<message>"}`. `user_data` is
// NULL here — the [NativeCallable] closure already carries the instance.
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
/// Core parses and validates the returned call from scratch. A function that
/// throws reports the failure instead of hiding it: the tool call stays
/// `invalid`, with a `ToolCallRepair` error whose `cause` is the exception.
///
/// The function runs **synchronously on the isolate that created this
/// object**, while `generateText` / `streamText` is in progress, and inside
/// the FFI re-entrancy guard: it must not call back into aimux — that fails
/// with `AIMUX_E_FFI_REENTRANT_CALL`.
///
/// **Create the repair on the isolate that makes the call.** Its native
/// trampoline may only run on the isolate that made it, so a [ToolCallRepair]
/// is not sendable: handing one — or a [GenerateTextOptions] holding one — to
/// `Isolate.run` throws `ArgumentError` there and then, instead of aborting
/// the VM once Core invokes it in the worker.
///
/// [close] is mandatory: the trampoline holds this object alive, so there is
/// no finalizer to fall back on.
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
// The pragma is what turns "not sendable" into a thrown ArgumentError: a bare
// NativeCallable stopped being unsendable on its own when `Pointer` became
// sendable in Dart 3.6.
@pragma('vm:isolate-unsendable')
class ToolCallRepair {
  final RawToolCall? Function(ToolCallRepairContext) _repair;

  /// `isolateLocal`, not `listener`: Core needs the reply synchronously, and
  /// the closure carries the instance, so `user_data` stays NULL.
  late final NativeCallable<_RepairFnC> _callable;

  int _nativeHandle = 0;

  ToolCallRepair(this._repair) {
    _callable = NativeCallable<_RepairFnC>.isolateLocal(_invoke)
      ..keepIsolateAlive = false;
    _nativeHandle = _repairNew(_callable.nativeFunction, nullptr);
  }

  /// The FFI handle, which is how the function reaches `opts_json`
  /// (`"repair_tool_call": <handle>`). Null once [close]d — the native side
  /// reads a null (or 0) field as "no repair function", so a closed repair
  /// degrades to the default instead of failing the call.
  int? get handle => _nativeHandle == 0 ? null : _nativeHandle;

  /// Release the FFI handle and the trampoline. Idempotent, and safe as long
  /// as no invocation is executing at that instant: the native side disarms
  /// calls already in flight, which then behave as "not repaired".
  void close() {
    if (_nativeHandle == 0) return;
    _repairDrop(_nativeHandle);
    _nativeHandle = 0;
    _callable.close();
  }

  /// Runs the Dart function. `nullptr` means "keep the original error".
  ///
  /// Nothing may escape into the C ABI — the Rust frames below cannot unwind a
  /// Dart exception, and what a [NativeCallable] returns after one is
  /// undefined — so every failure becomes the ABI's error envelope, which Core
  /// records as `ToolCallRepair { original_error, cause }`.
  Pointer<Utf8> _invoke(Pointer<Utf8> contextJson, Pointer<Void> userData) {
    try {
      final repaired = _repair(ToolCallRepairContext.fromJson(
          jsonDecode(contextJson.toDartString()) as Map<String, dynamic>));
      return repaired == null ? nullptr : _reply(repaired.toJson());
    } catch (e) {
      try {
        return _reply({'error': e.toString()});
      } catch (_) {
        // The envelope itself does not marshal — an unpaired surrogate in the
        // message, say. "Not repaired" is all that is left.
        return nullptr;
      }
    }
  }

  /// [value] as a reply string aimux owns, since aimux frees what it is given.
  Pointer<Utf8> _reply(Map<String, Object?> value) {
    final json = toCString(encodeJson(value, 'repair_tool_call reply'));
    try {
      return _stringNew(json);
    } finally {
      calloc.free(json);
    }
  }
}
