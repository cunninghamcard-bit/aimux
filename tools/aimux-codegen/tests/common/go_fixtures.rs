//! Fixtures for every hand-written Go type that has a generated
//! counterpart (unit 2.2 follow-up). Each value is a real aimux-core value
//! built with a struct/enum literal; the caller serializes it with
//! `serde_json::to_value`. Enums get one fixture per variant.
//! Initially produced from the manifest field list, then edited by hand.

#![allow(clippy::too_many_lines, clippy::unreadable_literal)]

use serde_json::Value;

use super::{Back, back};

fn v<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serde serializes the aimux-core value")
}

/// `(label, Go type name, serde output, serde decode of the Go output)`.
pub fn coverage_fixtures() -> Vec<(&'static str, &'static str, Value, Option<Back>)> {
    vec![
        (
            "cov-AudioData-Base64",
            "AudioData",
            {
                let x: aimux_core::speech_model::AudioData =
                    aimux_core::speech_model::AudioData::Base64("AQI=".to_string());
                v(&x)
            },
            Some(back::<aimux_core::speech_model::AudioData>),
        ),
        (
            "cov-AudioData-Binary",
            "AudioData",
            {
                let x: aimux_core::speech_model::AudioData =
                    aimux_core::speech_model::AudioData::Binary(vec![1]);
                v(&x)
            },
            Some(back::<aimux_core::speech_model::AudioData>),
        ),
        (
            "cov-ChatCompletionChoice",
            "ChatCompletionChoice",
            {
                let x: aimux_core::openai_output::ChatCompletionChoice =
                    aimux_core::openai_output::ChatCompletionChoice {
                        index: 1,
                        message: aimux_core::openai_output::ChatCompletionMessage {
                            role: "x".to_string(),
                            content: Some("x".to_string()),
                            reasoning_content: Some("x".to_string()),
                            tool_calls: Some(vec![
                                aimux_core::openai_output::ChatCompletionToolCall {
                                    id: "x".to_string(),
                                    tool_type: "x".to_string(),
                                    function: aimux_core::openai_output::ChatCompletionFunction {
                                        name: "x".to_string(),
                                        arguments: "x".to_string(),
                                    },
                                },
                            ]),
                            annotations: Some(vec![serde_json::json!({"k": 1})]),
                        },
                        finish_reason: Some("x".to_string()),
                        logprobs: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionChunkChoice",
            "ChatCompletionChunkChoice",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkChoice =
                    aimux_core::openai_output::ChatCompletionChunkChoice {
                        index: 1,
                        delta: aimux_core::openai_output::ChatCompletionDelta {
                            role: Some("x".to_string()),
                            content: Some("x".to_string()),
                            reasoning_content: Some("x".to_string()),
                            tool_calls: Some(vec![
                                aimux_core::openai_output::ChatCompletionChunkToolCall {
                                    index: 1,
                                    id: Some("x".to_string()),
                                    tool_type: Some("x".to_string()),
                                    function:
                                        aimux_core::openai_output::ChatCompletionChunkFunction {
                                            name: Some("x".to_string()),
                                            arguments: Some("x".to_string()),
                                        },
                                },
                            ]),
                        },
                        finish_reason: Some("x".to_string()),
                        logprobs: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionChunkFunction",
            "ChatCompletionChunkFunction",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkFunction =
                    aimux_core::openai_output::ChatCompletionChunkFunction {
                        name: Some("x".to_string()),
                        arguments: Some("x".to_string()),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionChunkToolCall",
            "ChatCompletionChunkToolCall",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkToolCall =
                    aimux_core::openai_output::ChatCompletionChunkToolCall {
                        index: 1,
                        id: Some("x".to_string()),
                        tool_type: Some("x".to_string()),
                        function: aimux_core::openai_output::ChatCompletionChunkFunction {
                            name: Some("x".to_string()),
                            arguments: Some("x".to_string()),
                        },
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionDelta",
            "ChatCompletionDelta",
            {
                let x: aimux_core::openai_output::ChatCompletionDelta =
                    aimux_core::openai_output::ChatCompletionDelta {
                        role: Some("x".to_string()),
                        content: Some("x".to_string()),
                        reasoning_content: Some("x".to_string()),
                        tool_calls: Some(vec![
                            aimux_core::openai_output::ChatCompletionChunkToolCall {
                                index: 1,
                                id: Some("x".to_string()),
                                tool_type: Some("x".to_string()),
                                function: aimux_core::openai_output::ChatCompletionChunkFunction {
                                    name: Some("x".to_string()),
                                    arguments: Some("x".to_string()),
                                },
                            },
                        ]),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionFunction",
            "ChatCompletionFunction",
            {
                let x: aimux_core::openai_output::ChatCompletionFunction =
                    aimux_core::openai_output::ChatCompletionFunction {
                        name: "x".to_string(),
                        arguments: "x".to_string(),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionMessage",
            "ChatCompletionMessage",
            {
                let x: aimux_core::openai_output::ChatCompletionMessage =
                    aimux_core::openai_output::ChatCompletionMessage {
                        role: "x".to_string(),
                        content: Some("x".to_string()),
                        reasoning_content: Some("x".to_string()),
                        tool_calls: Some(vec![aimux_core::openai_output::ChatCompletionToolCall {
                            id: "x".to_string(),
                            tool_type: "x".to_string(),
                            function: aimux_core::openai_output::ChatCompletionFunction {
                                name: "x".to_string(),
                                arguments: "x".to_string(),
                            },
                        }]),
                        annotations: Some(vec![serde_json::json!({"k": 1})]),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionToolCall",
            "ChatCompletionToolCall",
            {
                let x: aimux_core::openai_output::ChatCompletionToolCall =
                    aimux_core::openai_output::ChatCompletionToolCall {
                        id: "x".to_string(),
                        tool_type: "x".to_string(),
                        function: aimux_core::openai_output::ChatCompletionFunction {
                            name: "x".to_string(),
                            arguments: "x".to_string(),
                        },
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ChatCompletionUsage",
            "ChatCompletionUsage",
            {
                let x: aimux_core::openai_output::ChatCompletionUsage =
                    aimux_core::openai_output::ChatCompletionUsage {
                        prompt_tokens: 1,
                        completion_tokens: 1,
                        total_tokens: 1,
                        prompt_tokens_details: Some(
                            aimux_core::openai_output::PromptTokensDetails {
                                cached_tokens: 1,
                                cache_write_tokens: Some(1),
                            },
                        ),
                        completion_tokens_details: Some(
                            aimux_core::openai_output::CompletionTokensDetails {
                                reasoning_tokens: Some(1),
                            },
                        ),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-CompletionTokensDetails",
            "CompletionTokensDetails",
            {
                let x: aimux_core::openai_output::CompletionTokensDetails =
                    aimux_core::openai_output::CompletionTokensDetails {
                        reasoning_tokens: Some(1),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ContentPart-Text",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart = aimux_core::content::ContentPart::Text {
                    text: "x".to_string(),
                    provider_options: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-Custom",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::Custom {
                        kind: "x".to_string(),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-ReasoningFile",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::ReasoningFile {
                        data: aimux_core::shared::GeneratedFileData::Data {
                            data: aimux_core::shared::FileBytes::Binary(vec![1]),
                        },
                        media_type: "x".to_string(),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-ToolApprovalRequest",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::ToolApprovalRequest {
                        approval_id: "x".to_string(),
                        tool_call_id: "x".to_string(),
                        reason: Some("x".to_string()),
                        is_automatic: Some(true),
                        signature: Some("x".to_string()),
                        input_schema_input: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-Image",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart = aimux_core::content::ContentPart::Image {
                    image: vec![1],
                    media_type: "x".to_string(),
                    provider_options: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-File",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart = aimux_core::content::ContentPart::File {
                    data: vec![1],
                    media_type: "x".to_string(),
                    filename: Some("x".to_string()),
                    provider_options: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-FileBase64",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::FileBase64 {
                        data: "x".to_string(),
                        media_type: "x".to_string(),
                        filename: Some("x".to_string()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-FileUrl",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::FileUrl {
                        url: "x".to_string(),
                        media_type: "x".to_string(),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-FileReference",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::FileReference {
                        media_type: "x".to_string(),
                        reference: serde_json::json!({"k": 1}),
                        filename: Some("x".to_string()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-Reasoning",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::Reasoning {
                        text: "x".to_string(),
                        signature: Some("x".to_string()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-ToolCall",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::ToolCall {
                        tool_call_id: "x".to_string(),
                        tool_name: "x".to_string(),
                        input: serde_json::json!({"k": 1}),
                        provider_executed: Some(true),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-ContentPart-ToolResult",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart =
                    aimux_core::content::ContentPart::ToolResult {
                        tool_call_id: "x".to_string(),
                        tool_name: "x".to_string(),
                        output: aimux_core::language_model_message::ToolResultOutput::Text {
                            value: "x".to_string(),
                            provider_options: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        },
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "cov-EmbeddingCallOptions",
            "EmbeddingCallOptions",
            {
                let x: aimux_core::embedding_model::EmbeddingCallOptions =
                    aimux_core::embedding_model::EmbeddingCallOptions {
                        values: vec!["x".to_string()],
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingCallOptions>),
        ),
        (
            "cov-EmbeddingResponse",
            "EmbeddingResponse",
            {
                let x: aimux_core::embedding_model::EmbeddingResponse =
                    aimux_core::embedding_model::EmbeddingResponse {
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingResponse>),
        ),
        (
            "cov-EmbeddingResult",
            "EmbeddingResult",
            {
                let x: aimux_core::embedding_model::EmbeddingResult =
                    aimux_core::embedding_model::EmbeddingResult {
                        embeddings: vec![vec![0.5]],
                        usage: Some(aimux_core::embedding_model::EmbeddingUsage { tokens: 1 }),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        response: Some(aimux_core::embedding_model::EmbeddingResponse {
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        }),
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingResult>),
        ),
        (
            "cov-EmbeddingUsage",
            "EmbeddingUsage",
            {
                let x: aimux_core::embedding_model::EmbeddingUsage =
                    aimux_core::embedding_model::EmbeddingUsage { tokens: 1 };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingUsage>),
        ),
        (
            "cov-Fingerprint",
            "Fingerprint",
            {
                let x: aimux_core::trace::fingerprint::Fingerprint =
                    aimux_core::trace::fingerprint::Fingerprint {
                        body_hash: "x".to_string(),
                        len_bytes: 1,
                        block_size: 1,
                        block_hashes: vec!["x".to_string()],
                        token_estimate: 1,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::fingerprint::Fingerprint>),
        ),
        (
            "cov-FinishReasonUnified-Stop",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::Stop;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FinishReasonUnified-Length",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::Length;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FinishReasonUnified-ContentFilter",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::ContentFilter;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FinishReasonUnified-ToolCalls",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::ToolCalls;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FinishReasonUnified-Error",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::Error;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FinishReasonUnified-Other",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::Other;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "cov-FunctionToolInputExample",
            "FunctionToolInputExample",
            {
                let x: aimux_core::tool::FunctionToolInputExample =
                    aimux_core::tool::FunctionToolInputExample {
                        input: [("x".to_string(), serde_json::json!({"k": 1}))]
                            .into_iter()
                            .collect(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::tool::FunctionToolInputExample>),
        ),
        (
            "cov-GenerateObjectResult",
            "GenerateObjectResult",
            {
                let x: aimux_core::generate::GenerateObjectResult =
                    aimux_core::generate::GenerateObjectResult {
                        object: serde_json::json!({"k": 1}),
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: Some("x".to_string()),
                        },
                        raw_finish_reason: Some("x".to_string()),
                        usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        reasoning: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        response: aimux_core::types::ResponseMetadata {
                            id: Some("x".to_string()),
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                        },
                        raw: aimux_core::generate::GenerateTextResult {
                            content: vec![aimux_core::result::GenerateContent::Text {
                                text: "x".to_string(),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            }],
                            text: "x".to_string(),
                            tool_calls: vec![aimux_core::tool::ToolCall {
                                tool_call_id: "x".to_string(),
                                tool_name: "x".to_string(),
                                input: serde_json::json!({"k": 1}),
                                provider_executed: Some(true),
                                dynamic: Some(true),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                                invalid: Some(true),
                                error: Some(
                                    aimux_core::error::AiMuxError::ToolCallNotFoundForApproval {
                                        tool_call_id: "x".to_string(),
                                        approval_id: "y".to_string(),
                                    },
                                ),
                            }],
                            finish_reason: aimux_core::types::FinishReason {
                                unified: aimux_core::types::FinishReasonUnified::Stop,
                                raw: Some("x".to_string()),
                            },
                            usage: aimux_core::types::Usage {
                                input_tokens: aimux_core::types::InputTokenUsage {
                                    total: Some(1),
                                    no_cache: Some(1),
                                    cache_read: Some(1),
                                    cache_write: Some(1),
                                },
                                output_tokens: aimux_core::types::OutputTokenUsage {
                                    total: Some(1),
                                    text: Some(1),
                                    reasoning: Some(1),
                                },
                                raw: Default::default(),
                            },
                            warnings: vec![aimux_core::types::Warning::Unsupported {
                                feature: "x".to_string(),
                                details: Some("x".to_string()),
                            }],
                            raw: aimux_core::result::GenerateResult {
                                content: vec![aimux_core::result::GenerateContent::Text {
                                    text: "x".to_string(),
                                    provider_metadata: Some(
                                        [(
                                            "x".to_string(),
                                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                                .into_iter()
                                                .collect(),
                                        )]
                                        .into_iter()
                                        .collect(),
                                    ),
                                }],
                                finish_reason: aimux_core::types::FinishReason {
                                    unified: aimux_core::types::FinishReasonUnified::Stop,
                                    raw: Some("x".to_string()),
                                },
                                usage: aimux_core::types::Usage {
                                    input_tokens: aimux_core::types::InputTokenUsage {
                                        total: None,
                                        no_cache: None,
                                        cache_read: None,
                                        cache_write: None,
                                    },
                                    output_tokens: aimux_core::types::OutputTokenUsage {
                                        total: None,
                                        text: None,
                                        reasoning: None,
                                    },
                                    raw: Default::default(),
                                },
                                warnings: vec![aimux_core::types::Warning::Unsupported {
                                    feature: "x".to_string(),
                                    details: Some("x".to_string()),
                                }],
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                                request: Some(aimux_core::shared::RequestInfo {
                                    body: Some(serde_json::json!({"k": 1})),
                                }),
                                response: Some(aimux_core::shared::ResponseInfo {
                                    id: Some("x".to_string()),
                                    timestamp: Some("x".to_string()),
                                    model_id: Some("x".to_string()),
                                    headers: Some(
                                        [("x".to_string(), "x".to_string())].into_iter().collect(),
                                    ),
                                    body: Some(serde_json::json!({"k": 1})),
                                }),
                            },
                            reasoning: vec![aimux_core::result::ReasoningPart::Text(
                                aimux_core::result::ReasoningOutput {
                                    text: "x".to_string(),
                                    provider_metadata: Some(
                                        [(
                                            "x".to_string(),
                                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                                .into_iter()
                                                .collect(),
                                        )]
                                        .into_iter()
                                        .collect(),
                                    ),
                                },
                            )],
                            reasoning_text: "x".to_string(),
                            sources: vec![aimux_core::result::Source::Url {
                                id: "x".to_string(),
                                url: "x".to_string(),
                                title: Some("x".to_string()),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            }],
                            files: vec![aimux_core::result::GeneratedFile {
                                data: aimux_core::shared::GeneratedFileData::Data {
                                    data: aimux_core::shared::FileBytes::Binary(vec![1]),
                                },
                                media_type: "x".to_string(),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            }],
                            response_messages: vec![aimux_core::message::ModelMessage {
                                role: aimux_core::message::Role::System,
                                content: aimux_core::message::MessageContent::Text("x".to_string()),
                            }],
                            raw_finish_reason: Some("x".to_string()),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            request: aimux_core::shared::RequestInfo {
                                body: Some(serde_json::json!({"k": 1})),
                            },
                            response: aimux_core::shared::ResponseInfo {
                                id: Some("x".to_string()),
                                timestamp: Some("x".to_string()),
                                model_id: Some("x".to_string()),
                                headers: Some(
                                    [("x".to_string(), "x".to_string())].into_iter().collect(),
                                ),
                                body: Some(serde_json::json!({"k": 1})),
                            },
                            total_usage: aimux_core::types::Usage {
                                input_tokens: aimux_core::types::InputTokenUsage {
                                    total: Some(1),
                                    no_cache: Some(1),
                                    cache_read: Some(1),
                                    cache_write: Some(1),
                                },
                                output_tokens: aimux_core::types::OutputTokenUsage {
                                    total: Some(1),
                                    text: Some(1),
                                    reasoning: Some(1),
                                },
                                raw: Default::default(),
                            },
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateObjectResult>),
        ),
        (
            "cov-GenerateResult",
            "GenerateResult",
            {
                let x: aimux_core::result::GenerateResult = aimux_core::result::GenerateResult {
                    content: vec![aimux_core::result::GenerateContent::Text {
                        text: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    }],
                    finish_reason: aimux_core::types::FinishReason {
                        unified: aimux_core::types::FinishReasonUnified::Stop,
                        raw: Some("x".to_string()),
                    },
                    usage: aimux_core::types::Usage {
                        input_tokens: aimux_core::types::InputTokenUsage {
                            total: Some(1),
                            no_cache: Some(1),
                            cache_read: Some(1),
                            cache_write: Some(1),
                        },
                        output_tokens: aimux_core::types::OutputTokenUsage {
                            total: Some(1),
                            text: Some(1),
                            reasoning: Some(1),
                        },
                        raw: Default::default(),
                    },
                    warnings: vec![aimux_core::types::Warning::Unsupported {
                        feature: "x".to_string(),
                        details: Some("x".to_string()),
                    }],
                    provider_metadata: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                    request: Some(aimux_core::shared::RequestInfo {
                        body: Some(serde_json::json!({"k": 1})),
                    }),
                    response: Some(aimux_core::shared::ResponseInfo {
                        id: Some("x".to_string()),
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    }),
                };
                v(&x)
            },
            Some(back::<aimux_core::result::GenerateResult>),
        ),
        (
            "cov-GenerateTextOptions",
            "GenerateTextOptions",
            {
                let x: aimux_core::generate::GenerateTextOptions =
                    aimux_core::generate::GenerateTextOptions {
                        max_output_tokens: Some(1),
                        temperature: Some(0.5),
                        stop_sequences: Some(vec!["x".to_string()]),
                        top_p: Some(0.5),
                        top_k: Some(0.5),
                        presence_penalty: Some(0.5),
                        frequency_penalty: Some(0.5),
                        response_format: Some(aimux_core::options::ResponseFormat::Text),
                        seed: Default::default(),
                        tools: Some(vec![aimux_core::tool::Tool::Function(
                            aimux_core::tool::FunctionTool {
                                name: "x".to_string(),
                                description: Some("x".to_string()),
                                input_schema: serde_json::json!({"k": 1}),
                                strict: Some(true),
                                provider_options: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                                input_examples: Some(vec![
                                    aimux_core::tool::FunctionToolInputExample {
                                        input: [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    },
                                ]),
                            },
                        )]),
                        tool_choice: Some(Default::default()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        reasoning: Some(aimux_core::types::ReasoningEffort::ProviderDefault),
                        instructions: Some("x".to_string()),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        session_id: Some("x".to_string()),
                        include_raw_chunks: Some(true),
                        abort_signal: Default::default(),
                        repair_tool_call: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateTextOptions>),
        ),
        (
            "cov-GenerateTextResult",
            "GenerateTextResult",
            {
                let x: aimux_core::generate::GenerateTextResult =
                    aimux_core::generate::GenerateTextResult {
                        content: vec![aimux_core::result::GenerateContent::Text {
                            text: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        text: "x".to_string(),
                        tool_calls: vec![aimux_core::tool::ToolCall {
                            tool_call_id: "x".to_string(),
                            tool_name: "x".to_string(),
                            input: serde_json::json!({"k": 1}),
                            provider_executed: Some(true),
                            dynamic: Some(true),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            invalid: Some(true),
                            error: Some(
                                aimux_core::error::AiMuxError::ToolCallNotFoundForApproval {
                                    tool_call_id: "x".to_string(),
                                    approval_id: "y".to_string(),
                                },
                            ),
                        }],
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: Some("x".to_string()),
                        },
                        usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        raw: aimux_core::result::GenerateResult {
                            content: vec![aimux_core::result::GenerateContent::Text {
                                text: "x".to_string(),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            }],
                            finish_reason: aimux_core::types::FinishReason {
                                unified: aimux_core::types::FinishReasonUnified::Stop,
                                raw: Some("x".to_string()),
                            },
                            usage: aimux_core::types::Usage {
                                input_tokens: aimux_core::types::InputTokenUsage {
                                    total: Some(1),
                                    no_cache: Some(1),
                                    cache_read: Some(1),
                                    cache_write: Some(1),
                                },
                                output_tokens: aimux_core::types::OutputTokenUsage {
                                    total: Some(1),
                                    text: Some(1),
                                    reasoning: Some(1),
                                },
                                raw: Default::default(),
                            },
                            warnings: vec![aimux_core::types::Warning::Unsupported {
                                feature: "x".to_string(),
                                details: Some("x".to_string()),
                            }],
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            request: Some(aimux_core::shared::RequestInfo {
                                body: Some(serde_json::json!({"k": 1})),
                            }),
                            response: Some(aimux_core::shared::ResponseInfo {
                                id: Some("x".to_string()),
                                timestamp: Some("x".to_string()),
                                model_id: Some("x".to_string()),
                                headers: Some(
                                    [("x".to_string(), "x".to_string())].into_iter().collect(),
                                ),
                                body: Some(serde_json::json!({"k": 1})),
                            }),
                        },
                        reasoning: vec![aimux_core::result::ReasoningPart::Text(
                            aimux_core::result::ReasoningOutput {
                                text: "x".to_string(),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            },
                        )],
                        reasoning_text: "x".to_string(),
                        sources: vec![aimux_core::result::Source::Url {
                            id: "x".to_string(),
                            url: "x".to_string(),
                            title: Some("x".to_string()),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        files: vec![aimux_core::result::GeneratedFile {
                            data: aimux_core::shared::GeneratedFileData::Data {
                                data: aimux_core::shared::FileBytes::Binary(vec![1]),
                            },
                            media_type: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        response_messages: vec![aimux_core::message::ModelMessage {
                            role: aimux_core::message::Role::System,
                            content: aimux_core::message::MessageContent::Text("x".to_string()),
                        }],
                        raw_finish_reason: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        request: aimux_core::shared::RequestInfo {
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        response: aimux_core::shared::ResponseInfo {
                            id: Some("x".to_string()),
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        total_usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateTextResult>),
        ),
        (
            "cov-ImageCallOptions",
            "ImageCallOptions",
            {
                let x: aimux_core::image_model::ImageCallOptions =
                    aimux_core::image_model::ImageCallOptions {
                        prompt: Some("x".to_string()),
                        n: 1,
                        size: Some(
                            aimux_core::shared::Size::try_from("1024x768".to_string()).unwrap(),
                        ),
                        aspect_ratio: Some(
                            aimux_core::shared::AspectRatio::try_from("16:9".to_string()).unwrap(),
                        ),
                        seed: Default::default(),
                        files: Some(vec![aimux_core::image_model::ImageFile::File {
                            media_type: "x".to_string(),
                            data: aimux_core::image_model::ImageFileData::Base64(
                                "AQI=".to_string(),
                            ),
                        }]),
                        mask: Some(aimux_core::image_model::ImageFile::File {
                            media_type: "x".to_string(),
                            data: aimux_core::image_model::ImageFileData::Base64(
                                "AQI=".to_string(),
                            ),
                        }),
                        provider_options: [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageCallOptions>),
        ),
        (
            "cov-ImageOutputs-Base64",
            "ImageOutputs",
            {
                let x: aimux_core::image_model::ImageOutputs =
                    aimux_core::image_model::ImageOutputs::Base64(vec!["AQI=".to_string()]);
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageOutputs>),
        ),
        (
            "cov-ImageOutputs-Binary",
            "ImageOutputs",
            {
                let x: aimux_core::image_model::ImageOutputs =
                    aimux_core::image_model::ImageOutputs::Binary(vec![vec![1]]);
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageOutputs>),
        ),
        (
            "cov-ImageResponse",
            "ImageResponse",
            {
                let x: aimux_core::image_model::ImageResponse =
                    aimux_core::image_model::ImageResponse {
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                    };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageResponse>),
        ),
        (
            "cov-ImageUsage",
            "ImageUsage",
            {
                let x: aimux_core::image_model::ImageUsage = aimux_core::image_model::ImageUsage {
                    input_tokens: Some(1),
                    output_tokens: Some(1),
                    total_tokens: Some(1),
                };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageUsage>),
        ),
        (
            "cov-InputTokenUsage",
            "InputTokenUsage",
            {
                let x: aimux_core::types::InputTokenUsage = aimux_core::types::InputTokenUsage {
                    total: Some(1),
                    no_cache: Some(1),
                    cache_read: Some(1),
                    cache_write: Some(1),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::InputTokenUsage>),
        ),
        (
            "cov-OutputTokenUsage",
            "OutputTokenUsage",
            {
                let x: aimux_core::types::OutputTokenUsage = aimux_core::types::OutputTokenUsage {
                    total: Some(1),
                    text: Some(1),
                    reasoning: Some(1),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::OutputTokenUsage>),
        ),
        (
            "cov-PrefixBreak",
            "PrefixBreak",
            {
                let x: aimux_core::trace::store::PrefixBreak =
                    aimux_core::trace::store::PrefixBreak {
                        at_record_id: "x".to_string(),
                        prev_record_id: "x".to_string(),
                        lcp_bytes: 1,
                        expected_break: true,
                        kind: aimux_core::trace::store::BreakKind::SystemChanged,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::PrefixBreak>),
        ),
        (
            "cov-PromptTokensDetails",
            "PromptTokensDetails",
            {
                let x: aimux_core::openai_output::PromptTokensDetails =
                    aimux_core::openai_output::PromptTokensDetails {
                        cached_tokens: 1,
                        cache_write_tokens: Some(1),
                    };
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-ProviderDefault",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::ProviderDefault;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-None",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::None;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-Minimal",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::Minimal;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-Low",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort = aimux_core::types::ReasoningEffort::Low;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-Medium",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::Medium;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-High",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::High;
                v(&x)
            },
            None,
        ),
        (
            "cov-ReasoningEffort-Xhigh",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::Xhigh;
                v(&x)
            },
            None,
        ),
        (
            "cov-RequestCacheHints",
            "RequestCacheHints",
            {
                let x: aimux_core::trace::record::RequestCacheHints =
                    aimux_core::trace::record::RequestCacheHints {
                        requested_write: true,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::record::RequestCacheHints>),
        ),
        (
            "cov-RequestInfo",
            "RequestInfo",
            {
                let x: aimux_core::shared::RequestInfo = aimux_core::shared::RequestInfo {
                    body: Some(serde_json::json!({"k": 1})),
                };
                v(&x)
            },
            Some(back::<aimux_core::shared::RequestInfo>),
        ),
        (
            "cov-RerankingCallOptions",
            "RerankingCallOptions",
            {
                let x: aimux_core::reranking_model::RerankingCallOptions =
                    aimux_core::reranking_model::RerankingCallOptions {
                        documents: aimux_core::reranking_model::RerankingDocuments::Text {
                            values: vec!["x".to_string()],
                        },
                        query: "x".to_string(),
                        top_n: Some(1),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingCallOptions>),
        ),
        (
            "cov-RerankingRank",
            "RerankingRank",
            {
                let x: aimux_core::reranking_model::RerankingRank =
                    aimux_core::reranking_model::RerankingRank {
                        index: 1,
                        relevance_score: 0.5,
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingRank>),
        ),
        (
            "cov-RerankingResponse",
            "RerankingResponse",
            {
                let x: aimux_core::reranking_model::RerankingResponse =
                    aimux_core::reranking_model::RerankingResponse {
                        id: Some("x".to_string()),
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingResponse>),
        ),
        (
            "cov-RerankingResult",
            "RerankingResult",
            {
                let x: aimux_core::reranking_model::RerankingResult =
                    aimux_core::reranking_model::RerankingResult {
                        ranking: vec![aimux_core::reranking_model::RerankingRank {
                            index: 1,
                            relevance_score: 0.5,
                        }],
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        warnings: Some(vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }]),
                        response: Some(aimux_core::reranking_model::RerankingResponse {
                            id: Some("x".to_string()),
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        }),
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingResult>),
        ),
        (
            "cov-ResponseInfo",
            "ResponseInfo",
            {
                let x: aimux_core::shared::ResponseInfo = aimux_core::shared::ResponseInfo {
                    id: Some("x".to_string()),
                    timestamp: Some("x".to_string()),
                    model_id: Some("x".to_string()),
                    headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                    body: Some(serde_json::json!({"k": 1})),
                };
                v(&x)
            },
            Some(back::<aimux_core::shared::ResponseInfo>),
        ),
        (
            "cov-Role-System",
            "Role",
            {
                let x: aimux_core::message::Role = aimux_core::message::Role::System;
                v(&x)
            },
            Some(back::<aimux_core::message::Role>),
        ),
        (
            "cov-Role-User",
            "Role",
            {
                let x: aimux_core::message::Role = aimux_core::message::Role::User;
                v(&x)
            },
            Some(back::<aimux_core::message::Role>),
        ),
        (
            "cov-Role-Assistant",
            "Role",
            {
                let x: aimux_core::message::Role = aimux_core::message::Role::Assistant;
                v(&x)
            },
            Some(back::<aimux_core::message::Role>),
        ),
        (
            "cov-Role-Tool",
            "Role",
            {
                let x: aimux_core::message::Role = aimux_core::message::Role::Tool;
                v(&x)
            },
            Some(back::<aimux_core::message::Role>),
        ),
        (
            "cov-SearchCallOptions",
            "SearchCallOptions",
            {
                let x: aimux_core::search_model::SearchCallOptions =
                    aimux_core::search_model::SearchCallOptions {
                        query: "x".to_string(),
                        max_results: Some(1),
                        include_raw_content: Some(true),
                        time_range: Some("x".to_string()),
                        include_domains: Some(vec!["x".to_string()]),
                        exclude_domains: Some(vec!["x".to_string()]),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchCallOptions>),
        ),
        (
            "cov-SearchResponse",
            "SearchResponse",
            {
                let x: aimux_core::search_model::SearchResponse =
                    aimux_core::search_model::SearchResponse {
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResponse>),
        ),
        (
            "cov-SearchResult",
            "SearchResult",
            {
                let x: aimux_core::search_model::SearchResult =
                    aimux_core::search_model::SearchResult {
                        results: vec![aimux_core::search_model::SearchResultItem {
                            title: Some("x".to_string()),
                            url: Some("x".to_string()),
                            content: Some("x".to_string()),
                            raw_content: Some("x".to_string()),
                            score: Some(0.5),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        answer: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        response: Some(aimux_core::search_model::SearchResponse {
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        }),
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResult>),
        ),
        (
            "cov-SearchResultItem",
            "SearchResultItem",
            {
                let x: aimux_core::search_model::SearchResultItem =
                    aimux_core::search_model::SearchResultItem {
                        title: Some("x".to_string()),
                        url: Some("x".to_string()),
                        content: Some("x".to_string()),
                        raw_content: Some("x".to_string()),
                        score: Some(0.5),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResultItem>),
        ),
        (
            "cov-SessionChainView",
            "SessionChainView",
            {
                let x: aimux_core::trace::store::SessionChainView =
                    aimux_core::trace::store::SessionChainView {
                        session_id: "x".to_string(),
                        record_ids: vec!["x".to_string()],
                        prefix_stability: 0.5,
                        breaks: vec![aimux_core::trace::store::PrefixBreak {
                            at_record_id: "x".to_string(),
                            prev_record_id: "x".to_string(),
                            lcp_bytes: 1,
                            expected_break: true,
                            kind: aimux_core::trace::store::BreakKind::SystemChanged,
                        }],
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::SessionChainView>),
        ),
        (
            "cov-Source-Url",
            "Source",
            {
                let x: aimux_core::result::Source = aimux_core::result::Source::Url {
                    id: "x".to_string(),
                    url: "x".to_string(),
                    title: Some("x".to_string()),
                    provider_metadata: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                };
                v(&x)
            },
            Some(back::<aimux_core::result::Source>),
        ),
        (
            "cov-Source-Document",
            "Source",
            {
                let x: aimux_core::result::Source = aimux_core::result::Source::Document {
                    id: "x".to_string(),
                    media_type: "x".to_string(),
                    title: "x".to_string(),
                    filename: Some("x".to_string()),
                    provider_metadata: Some(
                        [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                    ),
                };
                v(&x)
            },
            Some(back::<aimux_core::result::Source>),
        ),
        (
            "cov-SpeechCallOptions",
            "SpeechCallOptions",
            {
                let x: aimux_core::speech_model::SpeechCallOptions =
                    aimux_core::speech_model::SpeechCallOptions {
                        text: "x".to_string(),
                        voice: Some("x".to_string()),
                        output_format: Some("x".to_string()),
                        instructions: Some("x".to_string()),
                        speed: Some(0.5),
                        language: Some("x".to_string()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechCallOptions>),
        ),
        (
            "cov-SpeechRequest",
            "SpeechRequest",
            {
                let x: aimux_core::speech_model::SpeechRequest =
                    aimux_core::speech_model::SpeechRequest {
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechRequest>),
        ),
        (
            "cov-SpeechResponse",
            "SpeechResponse",
            {
                let x: aimux_core::speech_model::SpeechResponse =
                    aimux_core::speech_model::SpeechResponse {
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechResponse>),
        ),
        (
            "cov-SpeechResult",
            "SpeechResult",
            {
                let x: aimux_core::speech_model::SpeechResult =
                    aimux_core::speech_model::SpeechResult {
                        audio: aimux_core::speech_model::AudioData::Base64("AQI=".to_string()),
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        request: Some(aimux_core::speech_model::SpeechRequest {
                            body: Some(serde_json::json!({"k": 1})),
                        }),
                        response: aimux_core::speech_model::SpeechResponse {
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechResult>),
        ),
        (
            "cov-StreamPart-TextStart",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::TextStart {
                        id: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-TextDelta",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::TextDelta {
                        id: "x".to_string(),
                        delta: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-TextEnd",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::TextEnd {
                        id: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-StreamStart",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::StreamStart {
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-Finish",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::Finish {
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: Some("x".to_string()),
                        },
                        usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-Error",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::Error {
                        error: aimux_core::error::AiMuxError::ToolCallNotFoundForApproval {
                            tool_call_id: "x".to_string(),
                            approval_id: "y".to_string(),
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolInputStart",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolInputStart {
                        id: "x".to_string(),
                        tool_name: "x".to_string(),
                        provider_executed: Some(true),
                        dynamic: Some(true),
                        title: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolInputDelta",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolInputDelta {
                        id: "x".to_string(),
                        delta: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolInputEnd",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolInputEnd {
                        id: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolCall",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolCall(aimux_core::tool::RawToolCall {
                        tool_call_id: "x".to_string(),
                        tool_name: "x".to_string(),
                        input: "x".to_string(),
                        provider_executed: Some(true),
                        dynamic: Some(true),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    });
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolResult",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolResult(aimux_core::tool::ToolResult {
                        tool_call_id: "x".to_string(),
                        tool_name: "x".to_string(),
                        result: serde_json::json!({"k": 1}),
                        is_error: Some(true),
                        preliminary: Some(true),
                        dynamic: Some(true),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    });
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-File",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::File(aimux_core::result::GeneratedFile {
                        data: aimux_core::shared::GeneratedFileData::Data {
                            data: aimux_core::shared::FileBytes::Binary(vec![1]),
                        },
                        media_type: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    });
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ReasoningFile",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ReasoningFile(
                        aimux_core::result::GeneratedFile {
                            data: aimux_core::shared::GeneratedFileData::Data {
                                data: aimux_core::shared::FileBytes::Binary(vec![1]),
                            },
                            media_type: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        },
                    );
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-Custom",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::Custom {
                        kind: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ToolApprovalRequest",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ToolApprovalRequest(
                        aimux_core::result::RawToolApprovalRequest {
                            approval_id: "x".to_string(),
                            tool_call_id: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        },
                    );
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ReasoningStart",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ReasoningStart {
                        id: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ReasoningDelta",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ReasoningDelta {
                        id: "x".to_string(),
                        delta: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ReasoningEnd",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ReasoningEnd {
                        id: "x".to_string(),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-ResponseMetadata",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::ResponseMetadata(
                        aimux_core::types::ResponseMetadata {
                            id: Some("x".to_string()),
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                        },
                    );
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-Source",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::Source(aimux_core::result::Source::Url {
                        id: "x".to_string(),
                        url: "x".to_string(),
                        title: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    });
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamPart-Raw",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::Raw {
                        raw_value: serde_json::json!({"k": 1}),
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "cov-StreamTextResultAggregated",
            "StreamTextResultAggregated",
            {
                let x: aimux_core::result::StreamTextResultAggregated =
                    aimux_core::result::StreamTextResultAggregated {
                        content: vec![aimux_core::result::GenerateContent::Text {
                            text: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        text: "x".to_string(),
                        reasoning: vec![aimux_core::result::ReasoningPart::Text(
                            aimux_core::result::ReasoningOutput {
                                text: "x".to_string(),
                                provider_metadata: Some(
                                    [(
                                        "x".to_string(),
                                        [("x".to_string(), serde_json::json!({"k": 1}))]
                                            .into_iter()
                                            .collect(),
                                    )]
                                    .into_iter()
                                    .collect(),
                                ),
                            },
                        )],
                        reasoning_text: "x".to_string(),
                        tool_calls: vec![aimux_core::tool::ToolCall {
                            tool_call_id: "x".to_string(),
                            tool_name: "x".to_string(),
                            input: serde_json::json!({"k": 1}),
                            provider_executed: Some(true),
                            dynamic: Some(true),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            invalid: Some(true),
                            error: Some(
                                aimux_core::error::AiMuxError::ToolCallNotFoundForApproval {
                                    tool_call_id: "x".to_string(),
                                    approval_id: "y".to_string(),
                                },
                            ),
                        }],
                        sources: vec![aimux_core::result::Source::Url {
                            id: "x".to_string(),
                            url: "x".to_string(),
                            title: Some("x".to_string()),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        files: vec![aimux_core::result::GeneratedFile {
                            data: aimux_core::shared::GeneratedFileData::Data {
                                data: aimux_core::shared::FileBytes::Binary(vec![1]),
                            },
                            media_type: "x".to_string(),
                            provider_metadata: Some(
                                [(
                                    "x".to_string(),
                                    [("x".to_string(), serde_json::json!({"k": 1}))]
                                        .into_iter()
                                        .collect(),
                                )]
                                .into_iter()
                                .collect(),
                            ),
                        }],
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: Some("x".to_string()),
                        },
                        raw_finish_reason: Some("x".to_string()),
                        usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                        total_usage: aimux_core::types::Usage {
                            input_tokens: aimux_core::types::InputTokenUsage {
                                total: Some(1),
                                no_cache: Some(1),
                                cache_read: Some(1),
                                cache_write: Some(1),
                            },
                            output_tokens: aimux_core::types::OutputTokenUsage {
                                total: Some(1),
                                text: Some(1),
                                reasoning: Some(1),
                            },
                            raw: Default::default(),
                        },
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        request: aimux_core::shared::RequestInfo {
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        response: aimux_core::shared::ResponseInfo {
                            id: Some("x".to_string()),
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        response_messages: vec![aimux_core::message::ModelMessage {
                            role: aimux_core::message::Role::System,
                            content: aimux_core::message::MessageContent::Text("x".to_string()),
                        }],
                    };
                v(&x)
            },
            Some(back::<aimux_core::result::StreamTextResultAggregated>),
        ),
        (
            "cov-Tool-Function",
            "Tool",
            {
                let x: aimux_core::tool::Tool =
                    aimux_core::tool::Tool::Function(aimux_core::tool::FunctionTool {
                        name: "x".to_string(),
                        description: Some("x".to_string()),
                        input_schema: serde_json::json!({"k": 1}),
                        strict: Some(true),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        input_examples: Some(vec![aimux_core::tool::FunctionToolInputExample {
                            input: [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        }]),
                    });
                v(&x)
            },
            Some(back::<aimux_core::tool::Tool>),
        ),
        (
            "cov-Tool-Provider",
            "Tool",
            {
                let x: aimux_core::tool::Tool =
                    aimux_core::tool::Tool::Provider(aimux_core::tool::ProviderTool {
                        id: "x".to_string(),
                        name: "x".to_string(),
                        args: [("x".to_string(), serde_json::json!({"k": 1}))]
                            .into_iter()
                            .collect(),
                    });
                v(&x)
            },
            Some(back::<aimux_core::tool::Tool>),
        ),
        (
            "cov-ToolChoice",
            "ToolChoice",
            {
                let x: aimux_core::tool::ToolChoice = Default::default();
                v(&x)
            },
            None,
        ),
        (
            "cov-TraceFilter",
            "TraceFilter",
            {
                let x: aimux_core::trace::store::TraceFilter =
                    aimux_core::trace::store::TraceFilter {
                        provider: Some("x".to_string()),
                        model: Some("x".to_string()),
                        session_id: Some("x".to_string()),
                        since_unix_ms: Some(1),
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::TraceFilter>),
        ),
        (
            "cov-TraceStats",
            "TraceStats",
            {
                let x: aimux_core::trace::store::TraceStats =
                    aimux_core::trace::store::TraceStats {
                        provider: "x".to_string(),
                        model: "x".to_string(),
                        requests: 1,
                        input_tokens_total: 1,
                        claimed_cache_read_total: 1,
                        claimed_cache_write_total: 1,
                        reported_hit_rate: Some(0.5),
                        client_upper_bound_hit_rate: Some(0.5),
                        verdict_counts: [("x".to_string(), 1)].into_iter().collect(),
                        ttft_p50_ms: Some(1),
                        ttft_p95_ms: Some(1),
                        errors: 1,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::TraceStats>),
        ),
        (
            "cov-TranscriptionCallOptions",
            "TranscriptionCallOptions",
            {
                let x: aimux_core::transcription_model::TranscriptionCallOptions =
                    aimux_core::transcription_model::TranscriptionCallOptions {
                        audio: aimux_core::transcription_model::AudioInput::Binary(vec![1]),
                        media_type: "x".to_string(),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        max_retries: Some(1),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionCallOptions>),
        ),
        (
            "cov-TranscriptionRequest",
            "TranscriptionRequest",
            {
                let x: aimux_core::transcription_model::TranscriptionRequest =
                    aimux_core::transcription_model::TranscriptionRequest {
                        body: Some("x".to_string()),
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionRequest>),
        ),
        (
            "cov-TranscriptionResponse",
            "TranscriptionResponse",
            {
                let x: aimux_core::transcription_model::TranscriptionResponse =
                    aimux_core::transcription_model::TranscriptionResponse {
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        body: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionResponse>),
        ),
        (
            "cov-TranscriptionResult",
            "TranscriptionResult",
            {
                let x: aimux_core::transcription_model::TranscriptionResult =
                    aimux_core::transcription_model::TranscriptionResult {
                        text: "x".to_string(),
                        segments: vec![aimux_core::transcription_model::TranscriptionSegment {
                            text: "x".to_string(),
                            start_second: 0.5,
                            end_second: 0.5,
                        }],
                        language: Some("x".to_string()),
                        duration_in_seconds: Some(0.5),
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        request: Some(aimux_core::transcription_model::TranscriptionRequest {
                            body: Some("x".to_string()),
                        }),
                        response: aimux_core::transcription_model::TranscriptionResponse {
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                            body: Some(serde_json::json!({"k": 1})),
                        },
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionResult>),
        ),
        (
            "cov-TranscriptionSegment",
            "TranscriptionSegment",
            {
                let x: aimux_core::transcription_model::TranscriptionSegment =
                    aimux_core::transcription_model::TranscriptionSegment {
                        text: "x".to_string(),
                        start_second: 0.5,
                        end_second: 0.5,
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionSegment>),
        ),
        (
            "cov-UploadFileCallOptions",
            "UploadFileCallOptions",
            {
                let x: aimux_core::files_model::UploadFileCallOptions =
                    aimux_core::files_model::UploadFileCallOptions {
                        data: aimux_core::files_model::UploadFileData::Data {
                            data: aimux_core::shared::FileBytes::Binary(vec![1]),
                        },
                        media_type: "x".to_string(),
                        filename: Some("x".to_string()),
                        provider_options: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::files_model::UploadFileCallOptions>),
        ),
        (
            "cov-UploadFileResult",
            "UploadFileResult",
            {
                let x: aimux_core::files_model::UploadFileResult =
                    aimux_core::files_model::UploadFileResult {
                        provider_reference: [("x".to_string(), "x".to_string())]
                            .into_iter()
                            .collect(),
                        media_type: Some("x".to_string()),
                        filename: Some("x".to_string()),
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                    };
                v(&x)
            },
            Some(back::<aimux_core::files_model::UploadFileResult>),
        ),
        (
            "cov-UsageSnapshot",
            "UsageSnapshot",
            {
                let x: aimux_core::trace::record::UsageSnapshot =
                    aimux_core::trace::record::UsageSnapshot {
                        input_total: Some(1),
                        input_no_cache: Some(1),
                        cache_read: Some(1),
                        cache_write: Some(1),
                        output_total: Some(1),
                        output_text: Some(1),
                        output_reasoning: Some(1),
                        raw: Some(serde_json::json!({"k": 1})),
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::record::UsageSnapshot>),
        ),
        (
            "cov-VideoCallOptions",
            "VideoCallOptions",
            {
                let x: aimux_core::video_model::VideoCallOptions =
                    aimux_core::video_model::VideoCallOptions {
                        prompt: Some("x".to_string()),
                        n: 1,
                        aspect_ratio: Some(
                            aimux_core::shared::AspectRatio::try_from("16:9".to_string()).unwrap(),
                        ),
                        resolution: Some(
                            aimux_core::shared::Size::try_from("1024x768".to_string()).unwrap(),
                        ),
                        duration: Some(1),
                        fps: Some(1),
                        seed: Default::default(),
                        image: Some(aimux_core::video_model::VideoFile::File {
                            media_type: "x".to_string(),
                            data: aimux_core::video_model::VideoFileData::Base64(
                                "AQI=".to_string(),
                            ),
                        }),
                        frame_images: Some(vec![aimux_core::video_model::VideoFrameImage {
                            image: aimux_core::video_model::VideoFile::File {
                                media_type: "x".to_string(),
                                data: aimux_core::video_model::VideoFileData::Base64(
                                    "x".to_string(),
                                ),
                            },
                            frame_type: aimux_core::video_model::VideoFrameType::FirstFrame,
                        }]),
                        input_references: Some(vec![aimux_core::video_model::VideoFile::File {
                            media_type: "x".to_string(),
                            data: aimux_core::video_model::VideoFileData::Base64(
                                "AQI=".to_string(),
                            ),
                        }]),
                        generate_audio: Some(true),
                        provider_options: [(
                            "x".to_string(),
                            [("x".to_string(), serde_json::json!({"k": 1}))]
                                .into_iter()
                                .collect(),
                        )]
                        .into_iter()
                        .collect(),
                        max_retries: Some(1),
                        poll: Some(aimux_core::video_model::VideoPollOptions {
                            interval_ms: Default::default(),
                            timeout_ms: Default::default(),
                        }),
                        timeout: Some(aimux_core::options::TimeoutConfiguration {
                            total_ms: Default::default(),
                            step_ms: Default::default(),
                            first_chunk_ms: Default::default(),
                            chunk_ms: Default::default(),
                        }),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoCallOptions>),
        ),
        (
            "cov-VideoData-Url",
            "VideoData",
            {
                let x: aimux_core::video_model::VideoData =
                    aimux_core::video_model::VideoData::Url {
                        url: "x".to_string(),
                        media_type: "x".to_string(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoData>),
        ),
        (
            "cov-VideoData-Base64",
            "VideoData",
            {
                let x: aimux_core::video_model::VideoData =
                    aimux_core::video_model::VideoData::Base64 {
                        data: "AQI=".to_string(),
                        media_type: "video/mp4".to_string(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoData>),
        ),
        (
            "cov-VideoData-Binary",
            "VideoData",
            {
                let x: aimux_core::video_model::VideoData =
                    aimux_core::video_model::VideoData::Binary {
                        data: vec![1],
                        media_type: "x".to_string(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoData>),
        ),
        (
            "cov-VideoPollOptions",
            "VideoPollOptions",
            {
                let x: aimux_core::video_model::VideoPollOptions =
                    aimux_core::video_model::VideoPollOptions {
                        interval_ms: Default::default(),
                        timeout_ms: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoPollOptions>),
        ),
        (
            "cov-VideoResponse",
            "VideoResponse",
            {
                let x: aimux_core::video_model::VideoResponse =
                    aimux_core::video_model::VideoResponse {
                        timestamp: Some("x".to_string()),
                        model_id: Some("x".to_string()),
                        headers: Some([("x".to_string(), "x".to_string())].into_iter().collect()),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoResponse>),
        ),
        (
            "cov-VideoResult",
            "VideoResult",
            {
                let x: aimux_core::video_model::VideoResult =
                    aimux_core::video_model::VideoResult {
                        videos: vec![aimux_core::video_model::VideoData::Url {
                            url: "x".to_string(),
                            media_type: "x".to_string(),
                        }],
                        warnings: vec![aimux_core::types::Warning::Unsupported {
                            feature: "x".to_string(),
                            details: Some("x".to_string()),
                        }],
                        provider_metadata: Some(
                            [(
                                "x".to_string(),
                                [("x".to_string(), serde_json::json!({"k": 1}))]
                                    .into_iter()
                                    .collect(),
                            )]
                            .into_iter()
                            .collect(),
                        ),
                        response: aimux_core::video_model::VideoResponse {
                            timestamp: Some("x".to_string()),
                            model_id: Some("x".to_string()),
                            headers: Some(
                                [("x".to_string(), "x".to_string())].into_iter().collect(),
                            ),
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoResult>),
        ),
        (
            "cov-Warning-Unsupported",
            "Warning",
            {
                let x: aimux_core::types::Warning = aimux_core::types::Warning::Unsupported {
                    feature: "x".to_string(),
                    details: Some("x".to_string()),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Warning>),
        ),
        (
            "cov-Warning-Compatibility",
            "Warning",
            {
                let x: aimux_core::types::Warning = aimux_core::types::Warning::Compatibility {
                    feature: "x".to_string(),
                    details: Some("x".to_string()),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Warning>),
        ),
        (
            "cov-Warning-Deprecated",
            "Warning",
            {
                let x: aimux_core::types::Warning = aimux_core::types::Warning::Deprecated {
                    setting: "x".to_string(),
                    message: "x".to_string(),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Warning>),
        ),
        (
            "cov-Warning-Other",
            "Warning",
            {
                let x: aimux_core::types::Warning = aimux_core::types::Warning::Other {
                    message: "x".to_string(),
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Warning>),
        ),
    ]
}
