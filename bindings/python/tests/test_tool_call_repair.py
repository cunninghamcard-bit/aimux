"""Host-side tool-call repair (RFC-0035).

Two halves: a replay of the shared `contract-tests/fixtures/tool-call-repair.json`
against the three native functions, and end-to-end runs of the
``repair_tool_call`` option through both API layers (dict and typed) against
the mock servers from test_e2e.py.
"""

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
from aimux.wrapper import (
    FunctionTool,
    GenerateTextOptions,
    RawToolCall,
    ToolCallRepairContext,
    _opts_to_json,
)
from aimux.wrapper import generate_text as typed_generate_text

from test_e2e import MockServer, OPENAI_CHAT, OPENAI_TOOL_CALL

_REPO_ROOT = Path(__file__).resolve().parents[3]

# The canned tool call answers with {"location": "Tokyo"}; this schema wants
# {"city": ...} and nothing else, so the call comes back invalid.
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


def _repaired(context):
    """A repair that rewrites the argument text to satisfy the schema."""
    call = dict(context["tool_call"])
    call["input"] = json.dumps({"city": json.loads(call["input"])["location"]})
    return call


# ── Shared fixture replay ───────────────────────────────────────────────────

def _cases():
    path = _REPO_ROOT / "contract-tests" / "fixtures" / "tool-call-repair.json"
    return json.loads(path.read_text(encoding="utf-8"))["cases"]


def _opts(case):
    opts = case["input"]["opts"]
    return None if opts is None else json.dumps(opts)


def _call(case):
    fn = case["function"]
    args = case["input"]
    if fn == "tool_call_repair_context":
        return tool_call_repair_context(
            json.dumps(args["tool_call"]), json.dumps(args["prompt"]), _opts(case)
        )
    if fn == "apply_tool_call_repair":
        return apply_tool_call_repair(
            json.dumps(args["tool_call"]), _opts(case), json.dumps(args["reply"])
        )
    return apply_tool_call_repair_to_result(
        json.dumps(args["result"]),
        _opts(case),
        args["tool_call_id"],
        json.dumps(args["reply"]),
    )


@pytest.mark.parametrize("case", _cases(), ids=lambda c: c["name"])
def test_shared_fixture(case):
    if case.get("expected_error"):
        with pytest.raises(InvalidArgumentError):
            _call(case)
        return
    assert json.loads(_call(case)) == case["expected"]


# ── Non-streaming, dict layer ───────────────────────────────────────────────

def _generate(repair, tools=TOOLS, response=OPENAI_TOOL_CALL):
    opts = {"tools": tools} if tools is not None else {}
    if repair is not None:
        opts["repair_tool_call"] = repair
    with MockServer(response) as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        return generate_text(model, "What's the weather in Tokyo?", opts)


def test_invalid_without_repair():
    """The baseline the repair cases are measured against."""
    call = _generate(None)["tool_calls"][0]
    assert call["invalid"] is True
    assert "InvalidToolInput" in call["error"]


def test_repaired_call_and_transcript():
    result = _generate(_repaired)
    call = result["tool_calls"][0]
    assert not call.get("invalid")
    assert call["input"] == {"city": "Tokyo"}
    assert call.get("error") is None

    # The replayed transcript must carry the repaired arguments, or the next
    # turn sends the model the broken ones again.
    parts = [
        part
        for message in result["response_messages"]
        for part in message["content"]
        if isinstance(part, dict) and part.get("type") == "tool_call"
    ]
    assert [p["input"] for p in parts] == [{"city": "Tokyo"}]


def test_unchanged_keeps_original_error():
    seen = []

    def repair(context):
        seen.append(context)
        return None

    call = _generate(repair)["tool_calls"][0]
    assert call["invalid"] is True
    assert "InvalidToolInput" in call["error"]
    assert len(seen) == 1


def test_context_shape():
    captured = {}

    def repair(context):
        captured.update(context)
        return None

    _generate(repair)
    assert captured["tool_call"]["tool_name"] == "get_weather"
    # The raw argument text the provider emitted, not a parsed object.
    assert captured["tool_call"]["input"] == '{"location":"Tokyo"}'
    assert captured["input_schema"] == WEATHER_SCHEMA
    assert captured["tools"] == TOOLS
    assert captured["messages"] == [
        {"role": "user", "content": "What's the weather in Tokyo?"}
    ]
    assert "InvalidToolInput" in captured["error"]


def test_failed_repair_reports_the_exception_message():
    def repair(context):
        raise RuntimeError("repair model unavailable")

    call = _generate(repair)["tool_calls"][0]
    assert call["invalid"] is True
    cause = call["error"]["ToolCallRepair"]
    assert cause["cause"] == {"Other": "repair model unavailable"}
    assert "InvalidToolInput" in cause["original_error"]


def test_repaired_but_still_invalid():
    def repair(context):
        return dict(context["tool_call"], input='{"town":"Tokyo"}')

    call = _generate(repair)["tool_calls"][0]
    assert call["invalid"] is True
    assert "ToolCallRepair" in call["error"]


def test_no_tools_never_calls_repair():
    """AI SDK rule: a call generated without a tool set is never repaired."""
    calls = []
    result = _generate(lambda ctx: calls.append(ctx), tools=None)
    assert calls == []
    assert result["tool_calls"][0]["invalid"] is True


def test_valid_call_never_calls_repair():
    calls = []
    tools = [{
        "type": "function",
        "name": "get_weather",
        "input_schema": {"type": "object", "properties": {"location": {"type": "string"}}},
    }]
    result = _generate(lambda ctx: calls.append(ctx), tools=tools)
    assert calls == []
    assert not result["tool_calls"][0].get("invalid")


def test_repair_may_call_back_into_aimux():
    """The repair runs outside any native call, so re-entering aimux is fine."""
    with MockServer(OPENAI_CHAT) as helper:
        helper_model = openai("test-key", "gpt-4o", helper.url)

        def repair(context):
            # A real host would ask a model to rewrite the arguments; the
            # point here is only that a nested call does not deadlock.
            generate_text(helper_model, "fix these arguments")
            return _repaired(context)

        result = _generate(repair)

    assert result["tool_calls"][0]["input"] == {"city": "Tokyo"}


def test_repair_function_is_not_serialized():
    opts = {"tools": TOOLS, "repair_tool_call": _repaired}
    from aimux import _opts_to_json as dict_opts_to_json

    assert "repair_tool_call" not in dict_opts_to_json(opts)
    assert json.loads(dict_opts_to_json(opts)) == {"tools": TOOLS}


# ── Streaming ───────────────────────────────────────────────────────────────

def test_stream_replaces_the_tool_call_part_and_leaves_deltas_alone():
    opts = {"tools": TOOLS, "repair_tool_call": _repaired}
    with MockServer(TOOL_CALL_STREAM, content_type="text/event-stream") as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        parts = list(stream_text(model, "What's the weather in Tokyo?", opts))

    calls = [p["ToolCall"] for p in parts if "ToolCall" in p]
    assert len(calls) == 1
    assert not calls[0].get("invalid")
    assert calls[0]["input"] == {"city": "Tokyo"}

    # Input deltas are the provider's own text, forwarded untouched.
    deltas = "".join(p["ToolInputDelta"]["delta"] for p in parts if "ToolInputDelta" in p)
    assert deltas == '{"location":"Tokyo"}'


# ── Typed wrapper layer ─────────────────────────────────────────────────────

def test_typed_layer_repairs():
    captured = {}

    def repair(context: ToolCallRepairContext):
        captured["context"] = context
        return RawToolCall(
            tool_call_id=context.tool_call.tool_call_id,
            tool_name=context.tool_call.tool_name,
            input=json.dumps({"city": "Tokyo"}),
        )

    opts = GenerateTextOptions(
        tools=[FunctionTool(name="get_weather", input_schema=WEATHER_SCHEMA)],
        repair_tool_call=repair,
    )
    with MockServer(OPENAI_TOOL_CALL) as mock:
        model = openai("test-key", "gpt-4o", mock.url)
        result = typed_generate_text(model, "What's the weather in Tokyo?", opts)

    assert result.tool_calls[0].invalid is not True
    assert result.tool_calls[0].input == {"city": "Tokyo"}
    assert captured["context"].tool_call.input == '{"location":"Tokyo"}'
    assert captured["context"].tools[0].name == "get_weather"
    assert captured["context"].messages[0].role == "user"


def test_typed_repair_function_is_not_serialized():
    opts = GenerateTextOptions(temperature=0.5, repair_tool_call=_repaired)
    assert json.loads(_opts_to_json(opts)) == {"temperature": 0.5}
