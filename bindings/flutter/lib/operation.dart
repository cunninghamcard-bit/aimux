import 'dart:async';
import 'dart:convert';
import 'dart:isolate';
import 'aimux.dart';
import 'operation_transport.dart';

int _start(Model model, String mode, Object prompt,
        Map<String, dynamic>? options, bool repair) =>
    model.startOperation({
      'protocol_version': 1,
      'mode': mode,
      'prompt': prompt,
      'options': options ?? {},
      'repair_tool_call': repair,
    });
Map<String, dynamic> _reply(RawToolCall? value) => value == null
    ? {'type': 'unchanged'}
    : {'type': 'repaired', 'tool_call': value.toJson()};

Map<String, dynamic> hostResultSync(Model model, String mode, Object prompt,
    Map<String, dynamic>? options, ToolCallRepair repair) {
  final handle = _start(model, mode, prompt, options, true);
  try {
    while (true) {
      final wire = nextHostEvent(handle, 2);
      if (wire == null) throw StateError('operation ended without a result');
      final event = jsonDecode(wire) as Map<String, dynamic>;
      if (event['type'] == 'result')
        return event['result'] as Map<String, dynamic>;
      if (event['type'] != 'repair_request') continue;
      String reply;
      try {
        final value = repair(ToolCallRepairContext.fromJson(event['context']));
        if (value is Future<RawToolCall?>) {
          value.then<void>((_) {}, onError: (Object _, StackTrace __) {});
          throw ArgumentError(
              'async repairToolCall requires an async entry point');
        }
        reply = jsonEncode(_reply(value));
      } catch (error) {
        reply = jsonEncode({'type': 'failed', 'message': '$error'});
      }
      replyHostOperation(handle, event['request_id'], reply);
    }
  } finally {
    dropHostOperation(handle);
  }
}

// A worker waits on one native lane and transfers data only. ACK bounds the
// isolate mailbox to one event; no user closure is captured or sent here.
Future<void> _receiveLane(List<Object> args) async {
  final output = args[0] as SendPort;
  final handle = args[1] as int;
  final lane = args[2] as int;
  final commands = ReceivePort();
  final iterator = StreamIterator(commands);
  output.send(commands.sendPort);
  try {
    while (true) {
      final wire = nextHostEvent(handle, lane);
      output.send({'wire': wire});
      if (wire == null ||
          !await iterator.moveNext() ||
          iterator.current != true) return;
    }
  } catch (error, stack) {
    output.send({'error': error, 'stack': '$stack'});
  } finally {
    await iterator.cancel();
    commands.close();
  }
}

class _Lane {
  final ReceivePort port;
  final StreamIterator<dynamic> iterator;
  final SendPort commands;
  final Future<void> exited;
  bool ack = false;
  _Lane(this.port, this.iterator, this.commands, this.exited);
  static Future<_Lane> start(int handle, int lane) async {
    final port = ReceivePort();
    final exit = ReceivePort();
    final iterator = StreamIterator(port);
    try {
      await Isolate.spawn(_receiveLane, <Object>[port.sendPort, handle, lane],
          onExit: exit.sendPort);
      final exited = exit.first.then<void>((_) {
        exit.close();
      });
      if (!await iterator.moveNext())
        throw StateError('operation worker did not start');
      return _Lane(port, iterator, iterator.current as SendPort, exited);
    } catch (_) {
      await iterator.cancel();
      port.close();
      exit.close();
      rethrow;
    }
  }

  Future<String?> next() async {
    if (ack) commands.send(true);
    if (!await iterator.moveNext()) return null;
    ack = true;
    final message = iterator.current as Map;
    if (message.containsKey('error'))
      Error.throwWithStackTrace(
          message['error'], StackTrace.fromString(message['stack']));
    return message['wire'] as String?;
  }

  Future<void> close() async {
    commands.send(false);
    await exited;
    await iterator.cancel();
    port.close();
  }
}

Stream<Map<String, dynamic>> hostEvents(Model model, String mode, Object prompt,
    Map<String, dynamic>? options, ToolCallRepair? repair) {
  int? handle;
  StreamSubscription<Map<String, dynamic>>? subscription;
  late StreamController<Map<String, dynamic>> controller;
  controller = StreamController(
    onListen: () {
      try {
        handle = _start(model, mode, prompt, options, repair != null);
        subscription = _events(handle!, repair, () {
          handle = null;
        }).listen(
          controller.add,
          onError: controller.addError,
          onDone: controller.close,
        );
      } catch (error, stack) {
        controller.addError(error, stack);
        controller.close();
      }
    },
    onPause: () => subscription?.pause(),
    onResume: () => subscription?.resume(),
    onCancel: () async {
      if (handle != null) cancelHostOperation(handle!);
      await subscription?.cancel();
    },
  );
  return controller.stream;
}

Stream<Map<String, dynamic>> _events(
    int handle, ToolCallRepair? repair, void Function() closing) async* {
  final lanes = <_Lane>[];
  Future<void>? control;
  Future<void>? terminal;
  final ended = Completer<void>();
  final stop = Object();
  Object? driverError;
  try {
    for (final lane in [0, 1, 3]) {
      lanes.add(await _Lane.start(handle, lane));
    }
    terminal = () async {
      await lanes[2].next();
      if (!ended.isCompleted) ended.complete();
    }();
    control = () async {
      try {
        while (!ended.isCompleted) {
          final wire = await lanes[0].next();
          if (wire == null) return;
          final event = jsonDecode(wire) as Map<String, dynamic>;
          String reply;
          try {
            final value = await Future.any<Object?>([
              Future.sync(() =>
                  repair!(ToolCallRepairContext.fromJson(event['context']))),
              ended.future.then((_) => stop),
            ]);
            if (identical(value, stop)) return;
            reply = jsonEncode(_reply(value as RawToolCall?));
          } catch (error) {
            reply = jsonEncode({'type': 'failed', 'message': '$error'});
          }
          if (ended.isCompleted) return;
          replyHostOperation(handle, event['request_id'], reply);
        }
      } catch (error) {
        driverError = error;
        cancelHostOperation(handle);
      }
    }();
    while (true) {
      final wire = await lanes[1].next();
      if (wire == null) break;
      final event = jsonDecode(wire) as Map<String, dynamic>;
      yield event;
    }
    if (driverError != null) throw driverError!;
  } finally {
    cancelHostOperation(handle);
    closing();
    if (!ended.isCompleted) ended.complete();
    try {
      await control;
      await terminal;
    } finally {
      try {
        for (final lane in lanes) {
          await lane.close();
        }
      } finally {
        await _dropAsync(handle);
      }
    }
  }
}

Future<Map<String, dynamic>> hostResult(Model model, String mode, Object prompt,
    Map<String, dynamic>? options, ToolCallRepair? repair) async {
  await for (final event in hostEvents(model, mode, prompt, options, repair)) {
    if (event['type'] == 'result')
      return event['result'] as Map<String, dynamic>;
  }
  throw StateError('operation ended without a result');
}

// A dedicated capture scope guarantees that no Model or callable crosses isolates.
Future<void> _dropAsync(int handle) =>
    Isolate.run(() => dropHostOperation(handle));
