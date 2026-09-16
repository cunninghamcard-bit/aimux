"""``repair_tool_call``: the Python hook core calls for an invalid tool call.

The model returns ``{"location":"Tokyo"`` — one brace short — so core's parse
fails and the hook gets its single attempt.
"""

import json
import threading

from aimux import openai, generate_text
from test_e2e import RecordingMockServer

BROKEN_TOOL_CALL = json.dumps({
    "id": "chatcmpl-tc",
    "model": "gpt-4o",
    "choices": [{
        "message": {
            "role": "assistant",
            "content": None,
            "tool_calls": [{
                "id": "call_abc",
                "type": "function",
                "function": {"name": "get_weather", "arguments": '{"location":"Tokyo"'},
            }],
        },
        "finish_reason": "tool_calls",
    }],
    "usage": {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30},
})

WEATHER_TOOL = {
    "type": "function",
    "name": "get_weather",
    "description": "Get weather for a location",
    "input_schema": {
        "type": "object",
        "properties": {"location": {"type": "string"}},
        "required": ["location"],
    },
}


def _generate(repair):
    with RecordingMockServer(BROKEN_TOOL_CALL) as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        result = generate_text(
            model,
            "What's the weather in Tokyo?",
            {"tools": [WEATHER_TOOL], "repair_tool_call": repair},
        )
    assert len(result["tool_calls"]) == 1
    return result["tool_calls"][0]


class TestRepairToolCall:

    def test_repair_closes_the_missing_brace(self):
        contexts = []

        def repair(context):
            contexts.append((context, threading.get_ident()))
            call = dict(context["tool_call"])
            call["input"] = call["input"] + "}"
            return call

        tool_call = _generate(repair)

        assert tool_call["input"] == {"location": "Tokyo"}
        assert not tool_call.get("invalid")
        assert "error" not in tool_call

        # The hook runs exactly once, with the AI SDK repairToolCall context.
        assert len(contexts) == 1
        context, hook_thread = contexts[0]
        # On a blocking thread of its own: the calling thread has released the
        # GIL for the whole native call, so it cannot be the one running the
        # hook — and reaching this assertion at all means it did release it.
        assert hook_thread != threading.get_ident()
        assert context["tool_call"]["tool_name"] == "get_weather"
        assert context["tool_call"]["tool_call_id"] == "call_abc"
        assert context["tool_call"]["input"] == '{"location":"Tokyo"'
        assert "InvalidToolInput" in context["error"]
        assert context["input_schema"] == WEATHER_TOOL["input_schema"]
        assert [tool["name"] for tool in context["tools"]] == ["get_weather"]
        assert context["messages"][-1]["role"] == "user"

    def test_returning_none_keeps_the_original_error(self):
        tool_call = _generate(lambda context: None)

        assert tool_call["invalid"] is True
        assert "InvalidToolInput" in tool_call["error"]
        # The model's text survives verbatim, malformed as it is.
        assert tool_call["input"] == '{"location":"Tokyo"'

    def test_typed_options_carry_the_callable(self):
        """The pydantic layer excludes the callable from the JSON and passes it on."""
        from aimux.wrapper import GenerateTextOptions
        from aimux.wrapper import generate_text as typed_generate_text

        options = GenerateTextOptions(
            tools=[WEATHER_TOOL],
            repair_tool_call=lambda context: {
                **context["tool_call"],
                "input": context["tool_call"]["input"] + "}",
            },
        )
        assert "repair_tool_call" not in options.model_dump_json()

        with RecordingMockServer(BROKEN_TOOL_CALL) as mock:
            model = openai("test-key", "gpt-4o", mock.url)
            result = typed_generate_text(model, "What's the weather in Tokyo?", options)

        assert result.tool_calls[0].input == {"location": "Tokyo"}
        assert not result.tool_calls[0].invalid

    def test_json_schema_skips_the_callable(self):
        """A Callable field has no JSON schema; it must not break the model's."""
        from aimux.wrapper import GenerateTextOptions

        schema = GenerateTextOptions.model_json_schema()
        assert "repair_tool_call" not in schema["properties"]
        assert "tools" in schema["properties"]

    def test_the_hook_may_call_aimux(self):
        """The hook runs on a blocking thread, so a nested aimux call is fine:
        it drives the runtime from there while the workers keep polling."""
        with RecordingMockServer(BROKEN_TOOL_CALL) as mock:
            model = openai("test-key", "gpt-4o", mock.url)

            def repair(context):
                generate_text(model, "ask the model how to fix it", None)
                call = dict(context["tool_call"])
                call["input"] = call["input"] + "}"
                return call

            result = generate_text(
                model,
                "What's the weather in Tokyo?",
                {"tools": [WEATHER_TOOL], "repair_tool_call": repair},
            )

        assert result["tool_calls"][0]["input"] == {"location": "Tokyo"}

    def test_returning_an_error_dict_is_a_repair_error(self):
        tool_call = _generate(lambda context: {"error": "cannot fix that"})

        assert tool_call["invalid"] is True
        assert "cannot fix that" in json.dumps(tool_call["error"]["ToolCallRepair"]["cause"])

    def test_raising_becomes_a_tool_call_repair_error(self):
        def repair(context):
            raise RuntimeError("no idea how to fix that")

        tool_call = _generate(repair)

        assert tool_call["invalid"] is True
        assert "ToolCallRepair" in tool_call["error"]
        failure = tool_call["error"]["ToolCallRepair"]
        assert "InvalidToolInput" in failure["original_error"]
        assert "no idea how to fix that" in json.dumps(failure["cause"])

    def test_a_reply_that_is_neither_a_call_nor_an_error_is_a_repair_error(self):
        tool_call = _generate(lambda context: {})

        assert tool_call["invalid"] is True
        cause = json.dumps(tool_call["error"]["ToolCallRepair"]["cause"])
        assert "neither a RawToolCall nor" in cause

    def test_returning_a_string_is_a_type_error(self):
        tool_call = _generate(lambda context: '{"tool_call_id":"x"}')

        assert tool_call["invalid"] is True
        cause = json.dumps(tool_call["error"]["ToolCallRepair"]["cause"])
        assert "TypeError" in cause and "dict or None" in cause
