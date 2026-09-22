import json
from pathlib import Path

import pytest

from aimux import (
    InvalidArgumentError,
    apply_tool_call_repair,
    apply_tool_call_repair_to_result,
    generate_text,
    openai,
    stream_text,
    tool_call_repair_context,
)
from test_e2e import MockServer, OPENAI_TOOL_CALL

WEATHER_SCHEMA = {
    "type": "object",
    "properties": {"city": {"type": "string"}},
    "required": ["city"],
    "additionalProperties": False,
}
TOOLS = [{"type": "function", "name": "get_weather", "input_schema": WEATHER_SCHEMA}]
TOOL_CALL_STREAM = (
    'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"role":"assistant",'
    '"tool_calls":[{"index":0,"id":"call_s1","type":"function",'
    '"function":{"name":"get_weather","arguments":""}}]}}]}\n\n'
    'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{"tool_calls":[{"index":0,'
    '"function":{"arguments":"{\\"location\\":\\"Tokyo\\"}"}}]}}]}\n\n'
    'data: {"id":"1","model":"gpt-4o","choices":[{"delta":{},"finish_reason":"tool_calls"}],'
    '"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}\n\n'
    'data: [DONE]\n\n'
)


def _repair(context):
    return dict(context["tool_call"], input='{"city":"Tokyo"}')


def _cases():
    path = Path(__file__).resolve().parents[3] / "contract-tests/fixtures/tool-call-repair.json"
    return json.loads(path.read_text(encoding="utf-8"))["cases"]


def _call(case):
    args = case["input"]
    opts = None if args["opts"] is None else json.dumps(args["opts"])
    if case["function"] == "tool_call_repair_context":
        return tool_call_repair_context(json.dumps(args["tool_call"]), json.dumps(args["prompt"]), opts)
    if case["function"] == "apply_tool_call_repair":
        return apply_tool_call_repair(json.dumps(args["tool_call"]), opts, json.dumps(args["reply"]))
    if case["function"] == "apply_tool_call_repair_to_result":
        return apply_tool_call_repair_to_result(
            json.dumps(args["result"]), opts, args["tool_call_id"], json.dumps(args["reply"])
        )
    raise AssertionError(f"unknown fixture function {case['function']!r}")


@pytest.mark.parametrize("case", _cases(), ids=lambda case: case["name"])
def test_shared_fixture(case):
    if case.get("expected_error"):
        with pytest.raises(InvalidArgumentError):
            _call(case)
    else:
        assert json.loads(_call(case)) == case["expected"]


def test_generate_repairs_call_and_transcript():
    captured = {}

    def repair(context):
        captured.update(context)
        return _repair(context)

    with MockServer(OPENAI_TOOL_CALL) as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        result = generate_text(model, "What's the weather in Tokyo?", {
            "tools": TOOLS,
            "repair_tool_call": repair,
        })

    assert captured["tool_call"]["input"] == '{"location":"Tokyo"}'
    assert result["tool_calls"][0]["input"] == {"city": "Tokyo"}
    transcript_calls = [
        part
        for message in result["response_messages"]
        for part in message["content"]
        if isinstance(part, dict) and part.get("type") == "tool_call"
    ]
    assert [part["input"] for part in transcript_calls] == [result["tool_calls"][0]["input"]]


def test_stream_repairs_tool_call_and_preserves_deltas():
    with MockServer(TOOL_CALL_STREAM, content_type="text/event-stream") as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        parts = list(stream_text(model, "What's the weather in Tokyo?", {
            "tools": TOOLS,
            "repair_tool_call": _repair,
        }))

    calls = [part["ToolCall"] for part in parts if "ToolCall" in part]
    assert len(calls) == 1
    assert calls[0]["input"] == {"city": "Tokyo"}
    deltas = "".join(part["ToolInputDelta"]["delta"] for part in parts if "ToolInputDelta" in part)
    assert deltas == '{"location":"Tokyo"}'
