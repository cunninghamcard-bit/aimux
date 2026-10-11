#!/usr/bin/env python3
"""Generate the enum-state shared samples and their D25/D26 normalized forms.

Samples are JSONL: one compact JSON value per line, keys in the Rust struct's
serialization order, so `serialize(deserialize(line))` can be compared byte for
byte. Run from this directory; the output is committed, this script only
documents how it was produced (A7 §D25/D26).
"""
import json

def dump(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)

ERR = {"name": "AI_InvalidToolInputError", "toolName": "weather",
       "toolInput": "{\"city\":", "cause": "EOF while parsing"}

def tool_call(dynamic=None, invalid=None, error=None, pe=None, inp=None, meta=False):
    v = {"toolCallId": "call_1", "toolName": "weather",
         "input": {"city": "Tokyo"} if inp is None else inp}
    if pe is not None: v["providerExecuted"] = pe
    if dynamic is not None: v["dynamic"] = dynamic
    if meta: v["providerMetadata"] = {"openai": {"itemId": "fc_1"}}
    if invalid is not None: v["invalid"] = invalid
    if error is not None: v["error"] = error
    return v

def norm_tool_call(v):
    # D25: dynamic absent/false -> Static (not written); true written.
    # invalid absent/false -> Valid (not written); providerExecuted passthrough.
    v = dict(v)
    if v.get("dynamic") is False: del v["dynamic"]
    if v.get("invalid") is False: del v["invalid"]
    return v

tool_calls = []
for pe in (None, True, False):
    for dyn in (None, False, True):
        tool_calls.append(tool_call(dynamic=dyn, pe=pe))
tool_calls += [
    tool_call(dynamic=False, invalid=False),
    tool_call(invalid=False),
    tool_call(dynamic=True, invalid=False),
    tool_call(dynamic=True, invalid=True, error=ERR, inp="{\"city\":"),
    tool_call(dynamic=True, invalid=True, error=ERR, inp="{\"city\":", pe=True, meta=True),
    tool_call(meta=True),
]
# D26: rejected by try_from from E.2 on (invalid without dynamic:true, or without error).
tool_calls_rejected = [
    tool_call(invalid=True, error=ERR, inp="{\"city\":"),
    tool_call(dynamic=False, invalid=True, error=ERR, inp="{\"city\":"),
    tool_call(dynamic=True, invalid=True, inp="{\"city\":"),
]

def tool_result(dynamic=None, is_error=None, preliminary=None, meta=False):
    v = {"toolCallId": "call_1", "toolName": "web_search", "result": {"hits": 3}}
    if is_error is not None: v["isError"] = is_error
    if preliminary is not None: v["preliminary"] = preliminary
    if dynamic is not None: v["dynamic"] = dynamic
    if meta: v["providerMetadata"] = {"xai": {"id": "fs_1"}}
    return v

tool_results = [tool_result(dynamic=d) for d in (None, False, True)] + [
    tool_result(is_error=True), tool_result(is_error=False),
    tool_result(preliminary=True, dynamic=False), tool_result(preliminary=False),
    tool_result(meta=True),
]

def raw_tool_call(inp, dynamic=None, pe=None):
    v = {"toolCallId": "call_1", "toolName": "weather", "input": inp}
    if pe is not None: v["providerExecuted"] = pe
    if dynamic is not None: v["dynamic"] = dynamic
    return v

# RFC-0042:84-91 rows 1-6, reused from 1.3 fixtures/v4/tool-input-vectors.json
# (branch exp/u1.3-tool-input-vectors; not on 8455db9a), as provider input text.
vectors = ["\"Tokyo\"", "\"{\\\"x\\\":1}\"", "123", "true", "null",
           "{\"a\":1,\"a\":2}", " 1e2 ", "{\"a\":", ""]
raw_tool_calls = [raw_tool_call(s) for s in vectors] + [
    raw_tool_call("{}", dynamic=d, pe=p) for p in (None, True, False) for d in (None, False, True)
]

def input_start(dynamic=None, pe=None, title=None):
    v = {"type": "tool-input-start", "id": "call_1", "toolName": "weather"}
    if pe is not None: v["providerExecuted"] = pe
    if dynamic is not None: v["dynamic"] = dynamic
    if title is not None: v["title"] = title
    return v

def norm_input_start(v):
    # D25: ToolInputStart always writes the dynamic boolean
    # (stream-language-model-call.ts:809); providerExecuted passthrough.
    out = {}
    for k in ("type", "id", "toolName", "providerExecuted"):
        if k in v: out[k] = v[k]
    out["dynamic"] = bool(v.get("dynamic", False))
    for k in ("title", "providerMetadata"):
        if k in v: out[k] = v[k]
    return out

input_starts = [input_start(dynamic=d, pe=p) for p in (None, True, False) for d in (None, False, True)]
input_starts.append(input_start(title="Weather"))

files = {
    "tool_call": (tool_calls, norm_tool_call),
    "tool_call_rejected": (tool_calls_rejected, None),
    # ToolResult has no providerExecuted field at 8455db9a (it documents itself
    # as a provider-executed result); E.3 decides client vs provider. Until
    # then its normalized form is the identity (A7 correction: passthrough).
    "tool_result": (tool_results, lambda v: v),
    "raw_tool_call": (raw_tool_calls, lambda v: v),
    "stream_tool_input_start": (input_starts, norm_input_start),
    "text_stream_tool_input_start": (input_starts, norm_input_start),
}
import os
os.makedirs("normalized", exist_ok=True)
for name, (rows, norm) in files.items():
    with open(f"{name}.jsonl", "w") as f:
        f.write("".join(dump(r) + "\n" for r in rows))
    if norm:
        with open(f"normalized/{name}.jsonl", "w") as f:
            f.write("".join(dump(norm(r)) + "\n" for r in rows))
