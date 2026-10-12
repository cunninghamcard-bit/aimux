//! Shared serde fixtures for the wire diff harnesses (Go and Swift), moved
//! out of wire_diff_go.rs unchanged so both languages run the same values.

#![allow(dead_code, clippy::too_many_lines)]

#[path = "go_fixtures.rs"]
mod go_fixtures;
#[path = "go_fixtures_empty.rs"]
mod go_fixtures_empty;

use serde_json::{Value, json};

use aimux_core::image_model::{ImageOutputs, ImageResponse, ImageResult, ImageUsage};
use aimux_core::message::{ModelMessage, Role};
use aimux_core::openai_output::{
    ChatCompletion, ChatCompletionChoice, ChatCompletionChunk, ChatCompletionChunkChoice,
    ChatCompletionDelta, ChatCompletionFunction, ChatCompletionMessage, ChatCompletionToolCall,
    ChatCompletionUsage, PromptTokensDetails,
};
use aimux_core::options::TimeoutConfiguration;
use aimux_core::tool::ToolCall;
use aimux_core::trace::{Fingerprint, RequestCacheHints, TraceRecord, UsageSnapshot};
use aimux_core::types::{
    FinishReason, FinishReasonUnified, InputTokenUsage, OutputTokenUsage, ResponseMetadata, Usage,
};

/// serde decode of a JSON value as the fixture's Rust type, re-serialized:
/// checks the Go → Rust direction (the generated Go's output is accepted by
/// serde and means the same value).
pub type Back = fn(&Value) -> Result<Value, String>;

/// [`Back`] for a `Deserialize` aimux-core type.
///
/// # Errors
///
/// serde rejects the value.
pub fn back<T: serde::Serialize + serde::de::DeserializeOwned>(v: &Value) -> Result<Value, String> {
    let t: T = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&t).map_err(|e| e.to_string())
}

/// One fixture: label, Go type name (same in both packages), serde output.
pub struct Fixture {
    pub label: &'static str,
    pub ty: &'static str,
    pub json: Value,
    pub back: Option<Back>,
}

pub fn fx<T: serde::Serialize>(label: &'static str, ty: &'static str, v: &T) -> Fixture {
    Fixture {
        label,
        ty,
        json: serde_json::to_value(v).expect("serde serializes the aimux-core value"),
        back: None,
    }
}

/// [`fx`] for a type that also implements `Deserialize`.
pub fn fxd<T: serde::Serialize + serde::de::DeserializeOwned>(
    label: &'static str,
    ty: &'static str,
    v: &T,
) -> Fixture {
    Fixture {
        back: Some(back::<T>),
        ..fx(label, ty, v)
    }
}

/// Real aimux-core values, serialized by serde. Values follow the
/// round-trip samples in aimux-core (recording.rs `recording_round_trips`,
/// openai_output.rs tests, trace/record.rs tests).
pub fn fixtures() -> Vec<Fixture> {
    let usage = Usage {
        input_tokens: InputTokenUsage {
            total: Some(10),
            no_cache: Some(4),
            cache_read: Some(6),
            cache_write: None,
        },
        output_tokens: OutputTokenUsage {
            total: Some(5),
            text: Some(3),
            reasoning: Some(2),
        },
        raw: Some(
            json!({"prompt_tokens": 10, "completion_tokens": 5})
                .as_object()
                .unwrap()
                .clone(),
        ),
    };
    let tool_call = ToolCall {
        tool_call_id: "call_1".into(),
        tool_name: "get_weather".into(),
        input: json!({"city": "Paris"}),
        provider_executed: None,
        dynamic: Some(false),
        provider_metadata: None,
        invalid: None,
        error: None,
    };
    let completion = ChatCompletion {
        id: "chatcmpl-1".into(),
        object: "chat.completion".into(),
        created: 1_700_000_000,
        model: "gpt-4o".into(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: "assistant".into(),
                content: None,
                reasoning_content: None,
                tool_calls: Some(vec![ChatCompletionToolCall {
                    id: "call_1".into(),
                    tool_type: "function".into(),
                    function: ChatCompletionFunction {
                        name: "get_weather".into(),
                        arguments: "{\"city\":\"Paris\"}".into(),
                    },
                }]),
                annotations: None,
            },
            finish_reason: Some("tool_calls".into()),
            logprobs: None,
        }],
        usage: ChatCompletionUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            prompt_tokens_details: Some(PromptTokensDetails {
                cached_tokens: 6,
                cache_write_tokens: None,
            }),
            completion_tokens_details: None,
        },
        system_fingerprint: None,
    };
    let chunk = ChatCompletionChunk {
        id: "chatcmpl-1".into(),
        object: "chat.completion.chunk".into(),
        created: 1_700_000_000,
        model: "gpt-4o".into(),
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionDelta {
                content: Some("Hel".into()),
                ..ChatCompletionDelta::default()
            },
            finish_reason: None,
            logprobs: None,
        }],
        usage: None,
    };
    let trace = TraceRecord {
        provider: "openai".into(),
        model: "gpt-4o".into(),
        request_id: Some("req_1".into()),
        session_id: None,
        call_id: "call-1".into(),
        sent_at_unix_ms: 1_700_000_000_000,
        monotonic_sent_ms: 42,
        lcp_token_upper: Some(128),
        ttft_ms: None,
        fingerprint: Fingerprint {
            body_hash: "ab12".into(),
            len_bytes: 2048,
            block_size: 1024,
            block_hashes: vec!["aa".into(), "bb".into()],
            token_estimate: 512,
        },
        usage: UsageSnapshot {
            input_total: Some(10),
            cache_read: Some(6),
            ..UsageSnapshot::default()
        },
        response_cache_headers: None,
        request_cache_hints: Some(RequestCacheHints {
            requested_write: true,
        }),
        verdict: None,
        error: None,
        scope_key: 7,
    };
    let image = ImageResult {
        // Overlap: `[]` is a valid Base64(Vec<String>) and a valid
        // Binary(Vec<Vec<u8>>); serde's untagged order picks Base64.
        images: ImageOutputs::Base64(vec![]),
        warnings: vec![],
        provider_metadata: None,
        response: ImageResponse::default(),
        usage: Some(ImageUsage {
            input_tokens: None,
            output_tokens: Some(1),
            total_tokens: None,
        }),
    };
    let image_bytes = ImageResult {
        images: ImageOutputs::Binary(vec![vec![1, 2, 255]]),
        warnings: vec![],
        provider_metadata: None,
        response: ImageResponse::default(),
        usage: None,
    };
    let mut trace_streamed = trace.clone();
    trace_streamed.ttft_ms = Some(250);
    trace_streamed.session_id = Some("s-1".into());
    trace_streamed.response_cache_headers = Some(
        [("x-cache".to_string(), "HIT".to_string())]
            .into_iter()
            .collect(),
    );
    trace_streamed.error = Some("timeout".into());
    let timeout = TimeoutConfiguration {
        total_ms: Some(30_000),
        step_ms: None,
        first_chunk_ms: Some(5_000),
        chunk_ms: None,
    };
    let mut answered = completion.clone();
    answered.choices[0].message.content = Some("It is sunny.".into());
    answered.choices[0].message.reasoning_content = Some("check weather".into());
    answered.choices[0].message.tool_calls = None;
    answered.choices[0].finish_reason = Some("stop".into());
    answered.system_fingerprint = Some("fp_1".into());
    vec![
        fxd("usage-full", "Usage", &usage),
        fxd("usage-default", "Usage", &Usage::default()),
        fxd(
            "finish-reason",
            "FinishReason",
            &FinishReason {
                unified: FinishReasonUnified::ToolCalls,
                raw: Some("tool_use".into()),
            },
        ),
        fxd("tool-call", "ToolCall", &tool_call),
        fxd(
            "response-metadata-empty",
            "ResponseMetadata",
            &ResponseMetadata::default(),
        ),
        fx("chat-completion", "ChatCompletion", &completion),
        fx("chat-completion-chunk", "ChatCompletionChunk", &chunk),
        fxd("trace-record", "TraceRecord", &trace),
        fxd("trace-record-streamed", "TraceRecord", &trace_streamed),
        fxd("timeout-configuration", "TimeoutConfiguration", &timeout),
        fx("chat-completion-answer", "ChatCompletion", &answered),
        fxd("image-result-overlap", "ImageResult", &image),
        fxd("image-result-binary", "ImageResult", &image_bytes),
        fxd(
            "model-message-text",
            "ModelMessage",
            &ModelMessage::text(Role::User, "hi"),
        ),
    ]
    .into_iter()
    .chain(
        go_fixtures::coverage_fixtures()
            .into_iter()
            .chain(go_fixtures_empty::empty_fixtures())
            .map(|(label, ty, json, back)| Fixture {
                label,
                ty,
                json,
                back,
            }),
    )
    .chain(gen_only_fixtures())
    .collect()
}

/// Fixtures for generated types with no hand-written counterpart, run
/// through the generated Go only: a tri-state field (absent / null /
/// present), and two proxied types (`from`/`into` = `AiMuxErrorWire`,
/// `into`/`try_from` = `String`).
pub fn gen_only_fixtures() -> Vec<Fixture> {
    use aimux_core::error::AiMuxError;
    use aimux_core::evaluation_model::{BooleanCriteria, EvaluationInput};
    use aimux_core::shared::Size;
    vec![
        fxd(
            "tri-state-absent",
            "BooleanCriteria",
            &BooleanCriteria {
                true_: None,
                false_: None,
            },
        ),
        fxd(
            "tri-state-null",
            "BooleanCriteria",
            &BooleanCriteria {
                true_: Some(None),
                false_: None,
            },
        ),
        fxd(
            "tri-state-present",
            "BooleanCriteria",
            &BooleanCriteria {
                true_: Some(Some(EvaluationInput::Text("yes".into()))),
                false_: Some(None),
            },
        ),
        fxd(
            "proxy-aimux-error",
            "AiMuxError",
            &AiMuxError::NoSuchTool {
                tool_name: "search".into(),
                available_tools: Some(vec!["get_weather".into()]),
                tool_input: None,
            },
        ),
        fxd(
            "proxy-size",
            "Size",
            &Size::try_from("1024x768".to_string()).unwrap(),
        ),
    ]
}
