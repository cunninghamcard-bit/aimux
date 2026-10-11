//! Every V4 type round-trips its AI SDK JSON exactly (camelCase fields,
//! kebab-case tags, absent optionals omitted).

use aimux_provider::*;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(wire: Value) -> T {
    let parsed: T = serde_json::from_value(wire.clone()).unwrap_or_else(|e| panic!("{e}: {wire}"));
    assert_eq!(serde_json::to_value(&parsed).unwrap(), wire);
    parsed
}

#[test]
fn prompt_round_trips() {
    let prompt: LanguageModelV4Prompt = round_trip(json!([
        {"role": "system", "content": "be brief"},
        {"role": "user", "content": [
            {"type": "text", "text": "hi", "providerOptions": {"openai": {"x": 1}}},
            {"type": "file", "filename": "a.png", "mediaType": "image/png", "data": {"type": "data", "data": "AQID"}},
            {"type": "file", "mediaType": "application/pdf", "data": {"type": "url", "url": "https://x/a.pdf"}},
            {"type": "file", "mediaType": "application/pdf", "data": {"type": "reference", "reference": {"openai": "file-1"}}},
            {"type": "file", "mediaType": "text/plain", "data": {"type": "text", "text": "notes"}}
        ], "providerOptions": {"anthropic": {"cache": true}}},
        {"role": "assistant", "content": [
            {"type": "reasoning", "text": "think"},
            {"type": "reasoning-file", "mediaType": "image/png", "data": {"type": "data", "data": "AA=="}},
            {"type": "custom", "kind": "openai.compaction"},
            {"type": "tool-call", "toolCallId": "c1", "toolName": "weather", "input": {"city": "Tokyo"}, "providerExecuted": true},
            {"type": "tool-result", "toolCallId": "c0", "toolName": "search", "output": {"type": "json", "value": [1, 2]}}
        ]},
        {"role": "tool", "content": [
            {"type": "tool-result", "toolCallId": "c1", "toolName": "weather", "output": {"type": "content", "value": [
                {"type": "text", "text": "sunny"},
                {"type": "file", "mediaType": "image/png", "data": {"type": "data", "data": "AQ=="}, "filename": "w.png"},
                {"type": "custom"}
            ]}},
            {"type": "tool-result", "toolCallId": "c2", "toolName": "rm", "output": {"type": "execution-denied", "reason": "no"}},
            {"type": "tool-result", "toolCallId": "c3", "toolName": "t", "output": {"type": "error-text", "value": "boom"}},
            {"type": "tool-result", "toolCallId": "c4", "toolName": "t", "output": {"type": "error-json", "value": {"e": 1}}},
            {"type": "tool-result", "toolCallId": "c5", "toolName": "t", "output": {"type": "text", "value": "ok"}},
            {"type": "tool-approval-response", "approvalId": "a1", "approved": false, "reason": "unsafe"}
        ]}
    ]));
    // Input layer: a prompt tool call's input is parsed JSON.
    let LanguageModelV4Message::Assistant { content, .. } = &prompt[2] else {
        panic!()
    };
    let LanguageModelV4AssistantPart::ToolCall(call) = &content[3] else {
        panic!()
    };
    assert_eq!(call.input, json!({"city": "Tokyo"}));
}

#[test]
fn call_options_round_trip() {
    round_trip::<LanguageModelV4CallOptions>(json!({
        "prompt": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
        "maxOutputTokens": 100, "temperature": 0.5, "stopSequences": ["\n"],
        "topP": 0.9, "topK": 40.0, "presencePenalty": 0.1, "frequencyPenalty": 0.2,
        "responseFormat": {"type": "json", "schema": {"type": "object"}, "name": "out"},
        "seed": 7,
        "tools": [
            {"type": "function", "name": "weather", "description": "d", "inputSchema": {"type": "object"},
             "inputExamples": [{"input": {"city": "Paris"}}], "strict": true},
            {"type": "provider", "id": "openai.web_search", "name": "web_search", "args": {"size": "low"}}
        ],
        "toolChoice": {"type": "tool", "toolName": "weather"},
        "includeRawChunks": true,
        "headers": {"x-a": "1", "x-b": null},
        "reasoning": "provider-default",
        "providerOptions": {"openai": {"store": false}}
    }));
    for choice in ["auto", "none", "required"] {
        round_trip::<LanguageModelV4ToolChoice>(json!({"type": choice}));
    }
    round_trip::<LanguageModelV4ResponseFormat>(json!({"type": "text"}));
    for level in [
        "provider-default",
        "none",
        "minimal",
        "low",
        "medium",
        "high",
        "xhigh",
    ] {
        round_trip::<LanguageModelV4Reasoning>(json!(level));
    }
}

#[test]
fn generate_result_round_trips() {
    let result: LanguageModelV4GenerateResult = round_trip(json!({
        "content": [
            {"type": "text", "text": "hi", "providerMetadata": {"openai": {"id": "m"}}},
            {"type": "reasoning", "text": "r"},
            {"type": "custom", "kind": "openai.x"},
            {"type": "reasoning-file", "mediaType": "image/png", "data": {"type": "url", "url": "https://x", "originalUrl": "https://y"}},
            {"type": "file", "mediaType": "image/png", "data": {"type": "data", "data": "AQID"}},
            {"type": "tool-approval-request", "approvalId": "a1", "toolCallId": "c1"},
            {"type": "source", "sourceType": "url", "id": "s1", "url": "https://x", "title": "X"},
            {"type": "source", "sourceType": "document", "id": "s2", "mediaType": "application/pdf", "title": "D", "filename": "d.pdf"},
            {"type": "tool-call", "toolCallId": "c1", "toolName": "weather", "input": "{\"city\":\"Tokyo\"}", "dynamic": true},
            {"type": "tool-result", "toolCallId": "c1", "toolName": "weather", "result": {"t": 20}, "isError": false, "preliminary": true}
        ],
        "finishReason": {"unified": "tool-calls", "raw": "tool_use"},
        "usage": {"inputTokens": {"total": 10, "noCache": 6, "cacheRead": 4}, "outputTokens": {"total": 5, "text": 3, "reasoning": 2}, "raw": {"x": 1}},
        "providerMetadata": {"openai": {"a": 1}},
        "request": {"body": {"model": "m"}},
        "response": {"id": "r1", "timestamp": "2026-10-10T00:00:00Z", "modelId": "m", "headers": {"x": "y"}, "body": {}},
        "warnings": [
            {"type": "unsupported", "feature": "topK", "details": "d"},
            {"type": "compatibility", "feature": "seed"},
            {"type": "deprecated", "setting": "s", "message": "m"},
            {"type": "other", "message": "m"}
        ]
    }));
    // Output layer: a tool call's input is the provider's text, unparsed.
    let LanguageModelV4Content::ToolCall(call) = &result.content[8] else {
        panic!()
    };
    assert_eq!(call.input, "{\"city\":\"Tokyo\"}");
}

#[test]
fn stream_parts_round_trip() {
    for part in [
        json!({"type": "stream-start", "warnings": []}),
        json!({"type": "response-metadata", "id": "r", "modelId": "m"}),
        json!({"type": "text-start", "id": "t"}),
        json!({"type": "text-delta", "id": "t", "delta": "Hel"}),
        json!({"type": "text-end", "id": "t", "providerMetadata": {"p": {}}}),
        json!({"type": "reasoning-start", "id": "r"}),
        json!({"type": "reasoning-delta", "id": "r", "delta": "x"}),
        json!({"type": "reasoning-end", "id": "r"}),
        json!({"type": "tool-input-start", "id": "c", "toolName": "w", "providerExecuted": true, "dynamic": true, "title": "W"}),
        json!({"type": "tool-input-delta", "id": "c", "delta": "{\"ci"}),
        json!({"type": "tool-input-end", "id": "c"}),
        json!({"type": "tool-approval-request", "approvalId": "a", "toolCallId": "c"}),
        json!({"type": "tool-call", "toolCallId": "c", "toolName": "w", "input": "\"Tokyo\""}),
        json!({"type": "tool-result", "toolCallId": "c", "toolName": "w", "result": "ok"}),
        json!({"type": "custom", "kind": "a.b"}),
        json!({"type": "file", "mediaType": "image/png", "data": {"type": "data", "data": "AA=="}}),
        json!({"type": "reasoning-file", "mediaType": "image/png", "data": {"type": "data", "data": "AA=="}}),
        json!({"type": "source", "sourceType": "url", "id": "s", "url": "https://x"}),
        json!({"type": "finish", "usage": {"inputTokens": {}, "outputTokens": {}}, "finishReason": {"unified": "stop"}}),
        json!({"type": "raw", "rawValue": {"any": [1]}}),
        json!({"type": "error", "error": "boom"}),
    ] {
        round_trip::<LanguageModelV4StreamPart>(part);
    }
    round_trip::<LanguageModelV4StreamResultInfo>(
        json!({"request": {"body": "x"}, "response": {"headers": {"a": "b"}}}),
    );
}

#[test]
fn unknown_tags_are_rejected() {
    assert!(
        serde_json::from_value::<LanguageModelV4StreamPart>(json!({"type": "text", "text": "x"}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<LanguageModelV4Message>(
            json!({"role": "developer", "content": "x"})
        )
        .is_err()
    );
}
