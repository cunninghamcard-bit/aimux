// Host-side tool-call repair (RFC-0035).
//
// Drives the *real* Rust core through the three pure native functions, but
// makes no HTTP call: repair is post-processing over a result document, so a
// fixture document is all the input the loop needs. The cases come from
// `contract-tests/fixtures/tool-call-repair.json`, the same file the Rust,
// Node and Python suites replay.
//
// Run:
//   export DYLD_LIBRARY_PATH="$PWD/../../target/debug:$DYLD_LIBRARY_PATH"
//   cd bindings/flutter && dart test test/tool_call_repair_test.dart

import 'dart:convert';
import 'dart:io';

import 'package:aimux/aimux.dart';
import 'package:aimux/tool_call_repair.dart';
import 'package:aimux/types.dart';
import 'package:test/test.dart';

/// The fixture lives at the repo root; the test's working directory depends on
/// how the suite is invoked, so try the usual spots (mirrors contract_test).
File _fixtureFile() {
  const candidates = [
    '../../contract-tests/fixtures/tool-call-repair.json',
    'contract-tests/fixtures/tool-call-repair.json',
    '../contract-tests/fixtures/tool-call-repair.json',
  ];
  for (final path in candidates) {
    final file = File(path);
    if (file.existsSync()) return file;
  }
  throw StateError('cannot find tool-call-repair.json; tried $candidates');
}

final Map<String, Map<String, dynamic>> _cases = {
  for (final c in jsonDecode(_fixtureFile().readAsStringSync())['cases']
      as List<dynamic>)
    (c as Map<String, dynamic>)['name'] as String: c,
};

Map<String, dynamic> _case(String name) =>
    _cases[name] ?? (throw StateError("fixture case '$name' is missing"));

/// The options a fixture case was generated with, plus the hook under test.
/// Rebuilt field by field rather than via `fromJson` because the hook is a
/// host closure with no wire form — the fixture only ever sets these two.
GenerateTextOptions _options(
    Map<String, dynamic>? opts, RepairToolCall? repair) {
  final base = GenerateTextOptions.fromJson(opts ?? const {});
  return GenerateTextOptions(
    tools: base.tools,
    instructions: base.instructions,
    repairToolCall: repair,
  );
}

/// The smallest document `apply_tool_call_repair_to_result` accepts, shaped
/// like the `patch_result` fixture: the call in `tool_calls` and its replay in
/// `response_messages`, which must be patched together.
Map<String, dynamic> _resultWith(Map<String, dynamic> call) => {
      'text': '',
      'tool_calls': [call],
      'response_messages': [
        {
          'role': 'assistant',
          'content': [
            {
              'type': 'tool_call',
              'tool_call_id': call['tool_call_id'],
              'tool_name': call['tool_name'],
              'input': call['input'],
            },
          ],
        },
      ],
    };

/// A failure whose `toString()` is the message itself, so the reply carries
/// exactly the fixture's cause (an `Exception` would prefix "Exception: ").
class _RepairFailed implements Exception {
  final String message;
  _RepairFailed(this.message);
  @override
  String toString() => message;
}

void main() {
  test('the hook is never serialized into the options', () {
    final json = _options(
        _case('reply_unchanged')['input']['opts'] as Map<String, dynamic>,
        (_) => null).toJson();
    expect(json.keys, isNot(contains('repair_tool_call')));
    expect(jsonEncode(json), isNot(contains('repair')));
  });

  group('repair context (fixture replay)', () {
    // The fixture's `context_prompt_wrapper` case is not replayed here: the
    // {"prompt": …} wrapper is what `promptToJson` produces for a non-string
    // prompt, so `context_messages_prompt` below exercises exactly that wire
    // shape. Passing the wrapper as a Dart prompt would wrap it twice.
    for (final name in ['context_string_prompt', 'context_messages_prompt']) {
      test(name, () {
        final fixture = _case(name);
        final input = fixture['input'] as Map<String, dynamic>;
        final expected = fixture['expected'] as Map<String, dynamic>;

        ToolCallRepairContext? seen;
        final options = _options(input['opts'] as Map<String, dynamic>?, (c) {
          seen = c;
          return null; // unchanged — this case only checks the argument
        });

        repairResultToolCalls(
            _resultWith(input['tool_call'] as Map<String, dynamic>),
            input['prompt'] as Object,
            options);

        final context = seen;
        expect(context, isNotNull, reason: 'the hook was never invoked');
        final expectedCall = expected['tool_call'] as Map<String, dynamic>;
        expect(context!.toolCall.toolCallId, expectedCall['tool_call_id']);
        expect(context.toolCall.toolName, expectedCall['tool_name']);
        // The provider's raw argument text, not a decoded object.
        expect(context.toolCall.input, expectedCall['input']);
        expect(context.toolCall.isDynamic, expectedCall['dynamic']);
        expect(context.error, equals(expected['error']));
        expect(context.inputSchema, equals(expected['input_schema']));
        expect(context.instructions, expected['instructions']);
        expect(context.messages.map((m) => m.toJson()).toList(),
            equals(expected['messages']));
        expect(context.tools.single.function!.name,
            (expected['tools'] as List<dynamic>).single['name']);
      });
    }

    test('context_without_tools — the hook is never invoked', () {
      // Core answers JSON null for a call made without a tool set (AI SDK
      // rule), and the loop skips it. `context_null_opts` is the same case
      // reached through default options, which a hook-carrying
      // GenerateTextOptions cannot express in Dart.
      final input = _case('context_without_tools')['input']
          as Map<String, dynamic>;
      final call = input['tool_call'] as Map<String, dynamic>;
      var invoked = false;
      final result = repairResultToolCalls(
        _resultWith(call),
        input['prompt'] as Object,
        _options(input['opts'] as Map<String, dynamic>?, (_) {
          invoked = true;
          return null;
        }),
      );

      expect(invoked, isFalse);
      expect(result['tool_calls'], equals([call]));
    });

    test('a valid tool call is never offered to the hook', () {
      var invoked = false;
      final options = _options(
          _case('reply_repaired_validates')['input']['opts']
              as Map<String, dynamic>, (_) {
        invoked = true;
        return null;
      });
      final valid = {
        'tool_call_id': 'call-1',
        'tool_name': 'weather',
        'input': {'city': 'Singapore'},
      };

      final result = repairResultToolCalls(
          _resultWith(valid), 'weather in Singapore?', options);

      expect(invoked, isFalse);
      expect(result['tool_calls'], equals([valid]));
    });
  });

  group('repair replies (fixture replay)', () {
    /// Run one `apply_tool_call_repair` fixture case through the loop and
    /// return the patched call.
    Map<String, dynamic> replay(String name, RepairToolCall repair) {
      final input = _case(name)['input'] as Map<String, dynamic>;
      final result = repairResultToolCalls(
        _resultWith(input['tool_call'] as Map<String, dynamic>),
        // These cases fix the reply, not the prompt; any prompt the context
        // call accepts does.
        'weather in Singapore?',
        _options(input['opts'] as Map<String, dynamic>?, repair),
      );
      return (result['tool_calls'] as List<dynamic>).single
          as Map<String, dynamic>;
    }

    RawToolCall replacementOf(String name) => RawToolCall.fromJson(
        _case(name)['input']['reply']['tool_call'] as Map<String, dynamic>);

    for (final name in [
      'reply_repaired_validates',
      'reply_repaired_still_invalid',
    ]) {
      test(name, () {
        expect(replay(name, (_) => replacementOf(name)),
            equals(_case(name)['expected']));
      });
    }

    test('reply_unchanged — null keeps the invalid call and its error', () {
      expect(replay('reply_unchanged', (_) => null),
          equals(_case('reply_unchanged')['expected']));
    });

    test('reply_failed — a throwing hook becomes ToolCallRepair', () {
      final message =
          _case('reply_failed')['input']['reply']['message'] as String;
      expect(replay('reply_failed', (_) => throw _RepairFailed(message)),
          equals(_case('reply_failed')['expected']));
    });

    test('a thrown Exception contributes its toString as the cause', () {
      final patched =
          replay('reply_failed', (_) => throw Exception('model down'));
      expect(patched['invalid'], isTrue);
      expect(patched['error']['ToolCallRepair']['cause']['Other'],
          'Exception: model down');
    });

    test('patch_result — tool_calls and response_messages move together', () {
      final input = _case('patch_result')['input'] as Map<String, dynamic>;
      final result = repairResultToolCalls(
        jsonDecode(jsonEncode(input['result'])) as Map<String, dynamic>,
        'weather in Singapore?',
        _options(input['opts'] as Map<String, dynamic>?,
            (_) => replacementOf('patch_result')),
      );
      expect(result, equals(_case('patch_result')['expected']));
    });

    test('patch_result_renamed_call — the transcript follows the new id', () {
      final input =
          _case('patch_result_renamed_call')['input'] as Map<String, dynamic>;
      final result = repairResultToolCalls(
        jsonDecode(jsonEncode(input['result'])) as Map<String, dynamic>,
        'weather in Singapore?',
        _options(input['opts'] as Map<String, dynamic>?,
            (_) => replacementOf('patch_result_renamed_call')),
      );
      expect(result, equals(_case('patch_result_renamed_call')['expected']));
    });

    // The three `expected_error: InvalidArgument` cases (apply_without_tools,
    // reject_valid_tool_call, patch_result_unknown_id) are caller mistakes the
    // loop cannot make: it never applies a reply to a call the context
    // function skipped, never offers a valid call (asserted above), and always
    // passes back the id it just read out of the document.
  });

  group('the hook may call back into aimux', () {
    test('an FFI call from inside the hook works', () {
      // The hook runs on the caller's isolate after the native call returned,
      // and the three repair functions take no runtime, so nothing here is
      // re-entrant. A model construction is the cheapest FFI round trip that
      // needs no network.
      final patched = repairResultToolCalls(
        _resultWith(_case('reply_repaired_validates')['input']['tool_call']
            as Map<String, dynamic>),
        'weather in Singapore?',
        _options(
            _case('reply_repaired_validates')['input']['opts']
                as Map<String, dynamic>, (context) {
          Model.openai('test-key', 'gpt-4o').close();
          return RawToolCall(
            toolCallId: context.toolCall.toolCallId,
            toolName: context.toolCall.toolName,
            input: '{"city":"Singapore"}',
          );
        }),
      );

      expect((patched['tool_calls'] as List<dynamic>).single,
          equals(_case('reply_repaired_validates')['expected']));
    });

    test('an async hook on a synchronous call is a StateError', () {
      expect(
        () => repairResultToolCalls(
          _resultWith(_case('reply_unchanged')['input']['tool_call']
              as Map<String, dynamic>),
          'weather in Singapore?',
          _options(
              _case('reply_unchanged')['input']['opts']
                  as Map<String, dynamic>,
              (_) async => null),
        ),
        throwsA(isA<StateError>()),
      );
    });
  });

  group('streaming', () {
    final input =
        _case('reply_repaired_validates')['input'] as Map<String, dynamic>;

    List<Map<String, dynamic>> partsFor(Map<String, dynamic> call) => [
          {
            'ToolInputStart': {'id': 'call-1', 'tool_name': 'weather'},
          },
          {
            'ToolInputDelta': {'id': 'call-1', 'delta': '{"town":'},
          },
          {
            'ToolInputDelta': {'id': 'call-1', 'delta': '"Singapore"}'},
          },
          {'ToolCall': call},
          {
            'Finish': {'finish_reason': 'tool_calls'},
          },
        ];

    test('the invalid ToolCall part is replaced, deltas are not', () async {
      final call = input['tool_call'] as Map<String, dynamic>;
      final parts = partsFor(call);

      final out = await repairStreamToolCalls(
        Stream.fromIterable(parts),
        'weather in Singapore?',
        _options(input['opts'] as Map<String, dynamic>?,
            // Async on purpose: the streaming path is the one that awaits.
            (_) async => RawToolCall.fromJson(
                input['reply']['tool_call'] as Map<String, dynamic>)),
      ).toList();

      // Same parts, same order; only the ToolCall payload changed.
      expect(out.map((p) => p.keys.single).toList(),
          equals(parts.map((p) => p.keys.single).toList()));
      expect(out[1], equals(parts[1]));
      expect(out[2], equals(parts[2]));
      expect(out[4], equals(parts[4]));
      expect(out[3]['ToolCall'],
          equals(_case('reply_repaired_validates')['expected']));
    });

    test('a null return leaves the part as it arrived', () async {
      final call = input['tool_call'] as Map<String, dynamic>;
      final out = await repairStreamToolCalls(
        Stream.fromIterable(partsFor(call)),
        'weather in Singapore?',
        _options(input['opts'] as Map<String, dynamic>?, (_) => null),
      ).toList();

      expect(out[3]['ToolCall'], equals(_case('reply_unchanged')['expected']));
    });

    test('without a hook the stream is passed straight through', () async {
      final parts = partsFor(input['tool_call'] as Map<String, dynamic>);
      final out = await repairStreamToolCalls(Stream.fromIterable(parts),
              'weather in Singapore?', _options(null, null))
          .toList();
      expect(out, equals(parts));
    });
  });
}
