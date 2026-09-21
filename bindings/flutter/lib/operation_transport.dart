// Data-only C transport, shared by synchronous and isolate-based drivers.
import 'dart:ffi';
import 'package:ffi/ffi.dart';
import 'errors.dart';

typedef _StartC = Pointer<Void> Function(
    Uint64, Pointer<Utf8>, Pointer<Uint64>);
typedef _StartD = Pointer<Void> Function(int, Pointer<Utf8>, Pointer<Uint64>);
typedef _NextC = Pointer<Void> Function(
    Uint64, Int32, Int64, Pointer<Pointer<Utf8>>, Pointer<Int32>);
typedef _NextD = Pointer<Void> Function(
    int, int, int, Pointer<Pointer<Utf8>>, Pointer<Int32>);
typedef _ReplyC = Pointer<Void> Function(
    Uint64, Pointer<Utf8>, Pointer<Utf8>, Pointer<Int32>);
typedef _ReplyD = Pointer<Void> Function(
    int, Pointer<Utf8>, Pointer<Utf8>, Pointer<Int32>);
typedef _CancelC = Pointer<Void> Function(Uint64);
typedef _CancelD = Pointer<Void> Function(int);
typedef _DropC = Void Function(Uint64);
typedef _DropD = void Function(int);

final _lib = openAimuxLibrary();
final _start = _lib.lookupFunction<_StartC, _StartD>('aimux_operation_start');
final _next = _lib.lookupFunction<_NextC, _NextD>('aimux_operation_next');
final _reply = _lib.lookupFunction<_ReplyC, _ReplyD>('aimux_operation_reply');
final _cancel =
    _lib.lookupFunction<_CancelC, _CancelD>('aimux_operation_cancel');
final _drop = _lib.lookupFunction<_DropC, _DropD>('aimux_operation_drop');

int startHostOperation(int model, String request) {
  final input = toCString(request);
  try {
    return takeHandle((out) => _start(model, input, out), 'operation_start');
  } finally {
    calloc.free(input);
  }
}

String? nextHostEvent(int handle, int lane) {
  final out = calloc<Pointer<Utf8>>();
  final state = calloc<Int32>();
  try {
    expectAimuxError(_next(handle, lane, -1, out, state), 'operation_next');
    if (state.value == 2) return null;
    if (state.value != 0) throw StateError('concurrent operation reader');
    try {
      return out.value.toDartString();
    } finally {
      aimuxFreeString(out.value);
    }
  } finally {
    calloc.free(out);
    calloc.free(state);
  }
}

void replyHostOperation(int handle, String id, String reply) {
  final request = toCString(id);
  final value = toCString(reply);
  final status = calloc<Int32>();
  try {
    expectAimuxError(_reply(handle, request, value, status), 'operation_reply');
    if (status.value != 0 && status.value != 3)
      throw StateError('unexpected operation reply status');
  } finally {
    calloc.free(request);
    calloc.free(value);
    calloc.free(status);
  }
}

void cancelHostOperation(int handle) =>
    expectAimuxError(_cancel(handle), 'operation_cancel');
void dropHostOperation(int handle) => _drop(handle);
