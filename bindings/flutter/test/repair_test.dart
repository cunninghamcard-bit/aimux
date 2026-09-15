// e2e tests for `GenerateTextOptions.repairToolCall` (the Dart mirror of the
// Go binding's repair_test.go).
//
// Drives the *real* Rust core via dart:ffi against a mock OpenAI-compatible
// server that answers with a tool call whose arguments lost their closing
// brace. No real API calls are made.
//
// Run:
//   export LD_LIBRARY_PATH="<repo>/target/release:$LD_LIBRARY_PATH"
//   cd bindings/flutter && dart test test/repair_test.dart
//
// The FFI call runs in a worker isolate for the reason spelled out in
// `structured_e2e_test.dart`: `generateText` blocks its isolate until the Rust
// core finishes, so the mock server has to live elsewhere. The repair function
// is constructed *inside* the worker — its `NativeCallable.isolateLocal`
// trampoline may only be invoked on the mutator thread of the isolate that
// created it, which is exactly the thread the blocking FFI call occupies, and
// a [ToolCallRepair] cannot be sent there ready-made.

import 'dart:convert';
import 'dart:io';
import 'dart:isolate';

import 'package:aimux/aimux.dart';
import 'package:aimux/repair.dart';
import 'package:aimux/typed_model.dart';
import 'package:aimux/types.dart';
import 'package:test/test.dart';

/// The model drops the closing brace of the arguments.
const String malformedToolCallResponse = r'''
{"id":"chatcmpl-bad","model":"gpt-4o","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_bad","type":"function","function":{"name":"get_weather","arguments":"{\"location\":\"Tokyo\""}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":20,"completion_tokens":10,"total_tokens":30}}
''';

Future<HttpServer> startMockServer(String body) async {
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
  server.listen((request) async {
    await utf8.decoder.bind(request).join();
    request.response
      ..statusCode = HttpStatus.ok
      ..headers.contentType = ContentType.json
      ..write(body);
    await request.response.close();
  });
  return server;
}

Tool weatherTool() => Tool.function(FunctionTool(
      name: 'get_weather',
      inputSchema: {
        'type': 'object',
        'properties': {
          'location': {'type': 'string'},
        },
        'required': ['location'],
      },
    ));

/// What the Dart repair function does with the invalid call.
enum RepairMode { appendBrace, giveUp, throwing }

class RepairArgs {
  final String baseUrl;
  final RepairMode mode;
  RepairArgs(this.baseUrl, this.mode);
}

/// Runs one `generateText` with a [ToolCallRepair] in a worker isolate and
/// returns a sendable summary: the resulting tool calls as plain JSON and the
/// context the repair function saw.
Future<Map<String, dynamic>> runWithRepair(RepairArgs args) {
  return Isolate.run(() {
    Map<String, dynamic>? seen;
    final repair = ToolCallRepair((context) {
      seen = {
        'tool_name': context.toolCall.toolName,
        'input': context.toolCall.input,
        'error': context.error,
        'input_schema': context.inputSchema,
        'tools': context.tools.length,
        'messages': context.messages.length,
      };
      switch (args.mode) {
        case RepairMode.appendBrace:
          return RawToolCall(
            toolCallId: context.toolCall.toolCallId,
            toolName: context.toolCall.toolName,
            input: '${context.toolCall.input}}',
          );
        case RepairMode.giveUp:
          return null;
        case RepairMode.throwing:
          throw StateError('boom');
      }
    });
    final model =
        TypedModel(Model.openai('sk-test-fake-key', 'gpt-4o', baseUrl: args.baseUrl));
    try {
      final result = model.generateText(
        'What is the weather in Tokyo?',
        GenerateTextOptions(tools: [weatherTool()], repairToolCall: repair),
      );
      return {
        'tool_calls':
            jsonDecode(jsonEncode(result.toolCalls)) as List<dynamic>,
        'seen': seen,
      };
    } finally {
      repair.close();
      model.close();
    }
  });
}

void main() {
  late HttpServer server;
  late String baseUrl;

  setUp(() async {
    server = await startMockServer(malformedToolCallResponse);
    baseUrl = 'http://${server.address.host}:${server.port}';
  });

  tearDown(() => server.close(force: true));

  test('a Dart repair function fixes malformed arguments', () async {
    final out = await runWithRepair(RepairArgs(baseUrl, RepairMode.appendBrace));

    final seen = out['seen'] as Map<String, dynamic>;
    expect(seen['tool_name'], 'get_weather');
    expect(seen['input'], r'{"location":"Tokyo"');
    expect((seen['error'] as Map<String, dynamic>).keys,
        contains('InvalidToolInput'));
    expect((seen['input_schema'] as Map)['properties'], isNotNull);
    expect(seen['tools'], 1);
    expect(seen['messages'], greaterThan(0));

    final calls = out['tool_calls'] as List<dynamic>;
    expect(calls, hasLength(1));
    final call = calls.single as Map<String, dynamic>;
    expect(call['invalid'], isNot(true));
    expect((call['input'] as Map<String, dynamic>)['location'], 'Tokyo');
  });

  test('returning null keeps the original error and the raw text', () async {
    final out = await runWithRepair(RepairArgs(baseUrl, RepairMode.giveUp));

    final call = (out['tool_calls'] as List<dynamic>).single
        as Map<String, dynamic>;
    expect(call['invalid'], isTrue);
    expect(call['input'], r'{"location":"Tokyo"');
  });

  test('a thrown exception is reported as a ToolCallRepair failure', () async {
    final out = await runWithRepair(RepairArgs(baseUrl, RepairMode.throwing));

    final call = (out['tool_calls'] as List<dynamic>).single
        as Map<String, dynamic>;
    expect(call['invalid'], isTrue);
    final failure =
        (call['error'] as Map<String, dynamic>)['ToolCallRepair']
            as Map<String, dynamic>;
    expect((failure['original_error'] as Map<String, dynamic>).keys,
        contains('InvalidToolInput'));
    expect(jsonEncode(failure['cause']), contains('boom'));
  });

  test('a repair cannot ride into another isolate', () async {
    final repair = ToolCallRepair((_) => null);
    try {
      await expectLater(Isolate.run(() => repair.handle), throwsArgumentError);
    } finally {
      repair.close();
    }
  });

  test('a closed repair serializes as null, and closing twice is a no-op', () {
    final repair = ToolCallRepair((_) => null);
    expect(repair.handle, isNotNull);
    expect(GenerateTextOptions(repairToolCall: repair).toJson(),
        {'repair_tool_call': repair.handle});

    repair.close();
    repair.close();
    expect(repair.handle, isNull);
    expect(GenerateTextOptions(repairToolCall: repair).toJson(),
        {'repair_tool_call': null});
  });
}
