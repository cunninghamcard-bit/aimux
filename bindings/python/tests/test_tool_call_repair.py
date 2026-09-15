"""``repair_tool_call``: the Python hook core calls for an invalid tool call.

The model returns ``{"location":"Tokyo"`` — one brace short — so core's parse
fails and the hook gets its single attempt.
"""

import json

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
            contexts.append(context)
            call = dict(context["tool_call"])
            call["input"] = call["input"] + "}"
            return call

        tool_call = _generate(repair)

        assert tool_call["input"] == {"location": "Tokyo"}
        assert not tool_call.get("invalid")
        assert "error" not in tool_call

        # The hook runs exactly once, with the AI SDK repairToolCall context.
        assert len(contexts) == 1
        context = contexts[0]
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

    def test_raising_becomes_a_tool_call_repair_error(self):
        def repair(context):
            raise RuntimeError("no idea how to fix that")

        tool_call = _generate(repair)

        assert tool_call["invalid"] is True
        assert "ToolCallRepair" in tool_call["error"]
        failure = tool_call["error"]["ToolCallRepair"]
        assert "InvalidToolInput" in failure["original_error"]
        assert "no idea how to fix that" in json.dumps(failure["cause"])
