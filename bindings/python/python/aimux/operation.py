"""Host-driven operations. Callables never enter the native extension."""
import asyncio
import functools
import inspect
import json
from typing import Any, Awaitable, Callable, Dict, Optional, Union

RepairToolCall = Callable[[Dict[str, Any]], Union[Optional[Dict[str, Any]], Awaitable[Optional[Dict[str, Any]]]]]


def _start(model, mode, prompt_json, options, repair):
    if repair is not None and not callable(repair):
        raise TypeError("repair_tool_call must be callable")
    prompt = json.loads(prompt_json)
    if isinstance(prompt, dict) and "prompt" in prompt:
        prompt = prompt["prompt"]
    data = dict(options or {})
    data.pop("repair_tool_call", None)
    return model.start_operation(json.dumps({
        "protocol_version": 1, "mode": mode, "prompt": prompt,
        "options": data, "repair_tool_call": repair is not None,
    }))


def _reply(value):
    if value is None:
        return {"type": "unchanged"}
    return {"type": "repaired", "tool_call": value}


def events(model, mode, prompt_json, options, repair):
    """Synchronous driver: repair runs on the caller's thread, outside Rust."""
    if inspect.iscoroutinefunction(repair):
        raise TypeError("async repair_tool_call requires an async operation")
    op = _start(model, mode, prompt_json, options, repair)
    try:
        while True:
            wire = op.next(2)
            if wire is None:
                return
            event = json.loads(wire)
            if event["type"] == "repair_request":
                try:
                    value = repair(event["context"])
                    if inspect.isawaitable(value):
                        if inspect.iscoroutine(value):
                            value.close()
                        raise TypeError("async repair_tool_call requires an async operation")
                    status = op.reply(event["request_id"], json.dumps(_reply(value)))
                except Exception as exc:
                    status = op.reply(event["request_id"], json.dumps({"type": "failed", "message": str(exc)}))
                if status not in (0, 3):
                    raise RuntimeError("unexpected operation reply status: %s" % status)
            else:
                yield event
    finally:
        op.close()


def result(model, mode, prompt_json, options, repair):
    iterator = events(model, mode, prompt_json, options, repair)
    try:
        for event in iterator:
            if event["type"] == "result":
                return event["result"]
        raise RuntimeError("operation ended without a result")
    finally:
        iterator.close()


async def _wait(function, *args):
    # The executor transports data only. User code stays on the caller's loop.
    return await asyncio.get_running_loop().run_in_executor(None, functools.partial(function, *args))


async def async_events(model, mode, prompt_json, options, repair):
    op = _start(model, mode, prompt_json, options, repair)
    async def control():
        while True:
            wire = await _wait(op.next, 0)
            if wire is None:
                return
            event = json.loads(wire)
            try:
                value = repair(event["context"])
                if inspect.isawaitable(value):
                    value = await value
                status = op.reply(event["request_id"], json.dumps(_reply(value)))
            except asyncio.CancelledError:
                raise
            except Exception as exc:
                status = op.reply(event["request_id"], json.dumps({"type": "failed", "message": str(exc)}))
            if status == 3:
                return
            if status != 0:
                raise RuntimeError("unexpected operation reply status: %s" % status)
    hook_task = asyncio.create_task(control())
    async def watch():
        await _wait(op.finished)
        hook_task.cancel()
    terminal_task = asyncio.create_task(watch())
    def hook_done(task):
        if not task.cancelled() and task.exception() is not None:
            op.cancel()
    hook_task.add_done_callback(hook_done)
    try:
        while True:
            wire = await _wait(op.next, 1)
            if wire is None:
                break
            event = json.loads(wire)
            yield event
        if hook_task.done() and not hook_task.cancelled() and hook_task.exception():
            raise hook_task.exception()
    finally:
        op.cancel()
        hook_task.cancel()
        await _wait(op.close)
        await asyncio.gather(hook_task, terminal_task, return_exceptions=True)


async def async_result(model, mode, prompt_json, options, repair):
    iterator = async_events(model, mode, prompt_json, options, repair)
    try:
        async for event in iterator:
            if event["type"] == "result":
                return event["result"]
        raise RuntimeError("operation ended without a result")
    finally:
        await iterator.aclose()
