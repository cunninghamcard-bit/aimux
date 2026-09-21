import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';
import 'package:aimux/aimux.dart';
import 'package:aimux/typed_model.dart';
import 'package:aimux/types.dart';
import 'package:test/test.dart';

void main() {
  late HttpServer server;
  late TypedModel model;
  late List<Tool> tools;
  setUp(() async {
    final fixture = jsonDecode(await File(
            Platform.environment['AIMUX_CONTRACT_FIXTURE'] ??
                '../../contract-tests/host-operation.json')
        .readAsString());
    tools = GenerateTextOptions.fromJson(fixture['options']).tools!;
    server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    server.listen((request) async {
      final input = jsonDecode(await utf8.decoder.bind(request).join());
      if (input['stream'] == true) {
        request.response.headers.contentType =
            ContentType('text', 'event-stream');
        final call =
            fixture['response']['choices'][0]['message']['tool_calls'][0];
        for (final delta in [
          {
            'delta': {
              'tool_calls': [
                {'index': 0, ...call}
              ]
            }
          },
          {'delta': {}, 'finish_reason': 'tool_calls'},
        ]) {
          request.response.write('data: ${jsonEncode({
                'id': 'test',
                'model': 'mock',
                'choices': [delta]
              })}\n\n');
        }
        request.response.write('data: [DONE]\n\n');
      } else {
        request.response.headers.contentType = ContentType.json;
        request.response.write(jsonEncode(fixture['response']));
      }
      await request.response.close();
    });
    model = TypedModel(Model.openai('fake', 'mock',
        baseUrl: 'http://127.0.0.1:${server.port}'));
  });
  tearDown(() async {
    model.close();
    await server.close(force: true);
  });

  RawToolCall fixed(ToolCallRepairContext c) => RawToolCall(
      toolCallId: c.toolCall.toolCallId,
      toolName: c.toolCall.toolName,
      input: '{"city":"北京"}');

  test('async repair stays on caller isolate and supports nested generation',
      () async {
    final caller = Isolate.current;
    final options = GenerateTextOptions(
        tools: tools,
        repairToolCall: (c) async {
          expect(Isolate.current, same(caller));
          final nested = await model.generateTextAsync('nested',
              GenerateTextOptions(tools: tools, repairToolCall: fixed));
          expect(nested.toolCalls.single.input, {'city': '北京'});
          return fixed(c);
        });
    expect(options.toJson().containsKey('repair_tool_call'), isFalse);
    final result = await model.generateTextAsync('hello', options);
    expect(result.toolCalls.single.input, {'city': '北京'});
  });

  test('deadline releases workers while the host future remains unresolved',
      () async {
    final entered = Completer<void>();
    final pending = Completer<RawToolCall?>();
    final result = model.generateTextAsync(
        'hello',
        GenerateTextOptions(
          tools: tools,
          timeout: TimeoutConfiguration(totalMs: 300),
          repairToolCall: (_) {
            entered.complete();
            return pending.future;
          },
        ));
    await expectLater(
        result,
        throwsA(isA<AimuxException>()
            .having((e) => e.code, 'code', AimuxErrorCode.timeout)));
    expect(entered.isCompleted, isTrue);
    pending.complete(null);
  });

  test('subscription cancellation closes native waits during repair', () async {
    final entered = Completer<void>();
    final pending = Completer<RawToolCall?>();
    final subscription = model
        .streamText(
            'hello',
            GenerateTextOptions(
              tools: tools,
              repairToolCall: (_) {
                entered.complete();
                return pending.future;
              },
            ))
        .listen((_) {}, onError: (Object _) {});
    await entered.future.timeout(const Duration(seconds: 3));
    await subscription.cancel().timeout(const Duration(seconds: 3));
    pending.complete(null);
  });
}
