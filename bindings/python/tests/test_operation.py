"""Host execution and cleanup through the actual native operation transport."""
import asyncio
import json
import threading
import time

import pytest
import aimux
from test_e2e import MockServer

TOOLS = [{"type": "function", "name": "weather", "input_schema": {"type": "object"}}]
CALL = {"id": "c1", "type": "function", "function": {"name": "weather", "arguments": "{"}}
RESPONSE = json.dumps({"id": "completion", "model": "mock", "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}, "choices": [{"message": {"role": "assistant", "tool_calls": [CALL]}, "finish_reason": "tool_calls"}]})


def fixed(context):
    return dict(context["tool_call"], input='{"city":"北京"}')


@pytest.fixture
def model():
    with MockServer(RESPONSE) as server:
        yield aimux.openai("fake", "mock", server.url)


def test_sync_host_thread_and_nested_call(model):
    caller = threading.get_ident()
    def repair(context):
        assert threading.get_ident() == caller
        assert context["input_schema"] == {"type": "object"}
        nested = aimux.generate_text(model, "nested", {"tools": TOOLS, "repair_tool_call": fixed})
        assert nested["tool_calls"][0]["input"] == {"city": "北京"}
        return fixed(context)
    result = aimux.generate_text(model, "hi", {"tools": TOOLS, "repair_tool_call": repair})
    assert result["tool_calls"][0]["input"] == {"city": "北京"}


def test_async_repair_keeps_callers_loop(model):
    async def run():
        loop = asyncio.get_running_loop()
        async def repair(context):
            assert asyncio.get_running_loop() is loop
            await asyncio.sleep(0)
            return fixed(context)
        result = await aimux.generate_text_async(model, "hi", {"tools": TOOLS, "repair_tool_call": repair})
        assert result["tool_calls"][0]["input"] == {"city": "北京"}
    asyncio.run(run())


def test_sync_rejects_coroutine_and_preserves_failure(model):
    async def async_repair(_):
        return None
    with pytest.raises(TypeError, match="async operation"):
        aimux.generate_text(model, "hi", {"tools": TOOLS, "repair_tool_call": async_repair})
    for repair in (lambda _: 42, lambda c: dict(fixed(c), provider_executed="invalid")):
        result = aimux.generate_text(model, "hi", {"tools": TOOLS, "repair_tool_call": repair})
        assert "ToolCallRepair" in result["tool_calls"][0]["error"]
        async def run():
            result = await aimux.generate_text_async(model, "hi", {"tools": TOOLS, "repair_tool_call": repair})
            assert "ToolCallRepair" in result["tool_calls"][0]["error"]
        asyncio.run(run())


def test_timeout_cancels_suspended_host_coroutine(model):
    async def run():
        entered = asyncio.Event()
        exited = asyncio.Event()
        async def repair(_):
            entered.set()
            try:
                await asyncio.Future()
            finally:
                exited.set()
        with pytest.raises(aimux.APITimeoutError):
            await aimux.generate_text_async(model, "hi", {"tools": TOOLS, "repair_tool_call": repair, "timeout": {"total_ms": 200}})
        assert entered.is_set() and exited.is_set()
    asyncio.run(run())


def test_task_cancel_closes_operation(model):
    async def run():
        entered = asyncio.Event()
        async def repair(_):
            entered.set()
            await asyncio.Future()
        task = asyncio.create_task(aimux.generate_text_async(model, "hi", {"tools": TOOLS, "repair_tool_call": repair}))
        await asyncio.wait_for(entered.wait(), 2)
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
    asyncio.run(run())


def test_blocking_host_return_cannot_revive_timed_out_operation(model):
    def repair(context):
        time.sleep(0.2)
        return fixed(context)
    with pytest.raises(aimux.APITimeoutError):
        aimux.generate_text(model, "hi", {"tools": TOOLS, "repair_tool_call": repair, "timeout": {"total_ms": 100}})


def test_typed_options_exclude_callable_and_share_driver(model):
    from aimux.wrapper import GenerateTextOptions, generate_text
    options = GenerateTextOptions(tools=TOOLS, repair_tool_call=fixed)
    assert "repair_tool_call" not in options.model_dump_json()
    result = generate_text(model, "hi", options)
    assert result.tool_calls[0].input == {"city": "北京"}
