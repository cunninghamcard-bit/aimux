//! Empty-value fixture for every covered type (reviewer A): a minimal real
//! aimux-core value with every array empty, every Option `None`, every map
//! and string empty. Serde writes `[]` for array fields without
//! `skip_serializing_if`, so a hand-written `omitempty` shows up as a row.
//! Produced from the manifest field list like `go_fixtures.rs`.

#![allow(clippy::too_many_lines, clippy::unreadable_literal)]

use serde_json::Value;

use super::{Back, back};

fn v<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serde serializes the aimux-core value")
}

/// `(label, Go type name, serde output, serde decode of the Go output)`.
pub fn empty_fixtures() -> Vec<(&'static str, &'static str, Value, Option<Back>)> {
    vec![
        (
            "empty-AudioData",
            "AudioData",
            {
                let x: aimux_core::speech_model::AudioData =
                    aimux_core::speech_model::AudioData::Base64(String::new());
                v(&x)
            },
            Some(back::<aimux_core::speech_model::AudioData>),
        ),
        (
            "empty-ChatCompletion",
            "ChatCompletion",
            {
                let x: aimux_core::openai_output::ChatCompletion =
                    aimux_core::openai_output::ChatCompletion {
                        id: String::new(),
                        object: String::new(),
                        created: 1,
                        model: String::new(),
                        choices: vec![],
                        usage: aimux_core::openai_output::ChatCompletionUsage {
                            prompt_tokens: 1,
                            completion_tokens: 1,
                            total_tokens: 1,
                            prompt_tokens_details: None,
                            completion_tokens_details: None,
                        },
                        system_fingerprint: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionChoice",
            "ChatCompletionChoice",
            {
                let x: aimux_core::openai_output::ChatCompletionChoice =
                    aimux_core::openai_output::ChatCompletionChoice {
                        index: 1,
                        message: aimux_core::openai_output::ChatCompletionMessage {
                            role: String::new(),
                            content: None,
                            reasoning_content: None,
                            tool_calls: None,
                            annotations: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionChunk",
            "ChatCompletionChunk",
            {
                let x: aimux_core::openai_output::ChatCompletionChunk =
                    aimux_core::openai_output::ChatCompletionChunk {
                        id: String::new(),
                        object: String::new(),
                        created: 1,
                        model: String::new(),
                        choices: vec![],
                        usage: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionChunkChoice",
            "ChatCompletionChunkChoice",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkChoice =
                    aimux_core::openai_output::ChatCompletionChunkChoice {
                        index: 1,
                        delta: aimux_core::openai_output::ChatCompletionDelta {
                            role: None,
                            content: None,
                            reasoning_content: None,
                            tool_calls: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionChunkFunction",
            "ChatCompletionChunkFunction",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkFunction =
                    aimux_core::openai_output::ChatCompletionChunkFunction {
                        name: None,
                        arguments: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionChunkToolCall",
            "ChatCompletionChunkToolCall",
            {
                let x: aimux_core::openai_output::ChatCompletionChunkToolCall =
                    aimux_core::openai_output::ChatCompletionChunkToolCall {
                        index: 1,
                        id: None,
                        tool_type: None,
                        function: aimux_core::openai_output::ChatCompletionChunkFunction {
                            name: None,
                            arguments: None,
                        },
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionDelta",
            "ChatCompletionDelta",
            {
                let x: aimux_core::openai_output::ChatCompletionDelta =
                    aimux_core::openai_output::ChatCompletionDelta {
                        role: None,
                        content: None,
                        reasoning_content: None,
                        tool_calls: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionFunction",
            "ChatCompletionFunction",
            {
                let x: aimux_core::openai_output::ChatCompletionFunction =
                    aimux_core::openai_output::ChatCompletionFunction {
                        name: String::new(),
                        arguments: String::new(),
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionMessage",
            "ChatCompletionMessage",
            {
                let x: aimux_core::openai_output::ChatCompletionMessage =
                    aimux_core::openai_output::ChatCompletionMessage {
                        role: String::new(),
                        content: None,
                        reasoning_content: None,
                        tool_calls: None,
                        annotations: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionToolCall",
            "ChatCompletionToolCall",
            {
                let x: aimux_core::openai_output::ChatCompletionToolCall =
                    aimux_core::openai_output::ChatCompletionToolCall {
                        id: String::new(),
                        tool_type: String::new(),
                        function: aimux_core::openai_output::ChatCompletionFunction {
                            name: String::new(),
                            arguments: String::new(),
                        },
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ChatCompletionUsage",
            "ChatCompletionUsage",
            {
                let x: aimux_core::openai_output::ChatCompletionUsage =
                    aimux_core::openai_output::ChatCompletionUsage {
                        prompt_tokens: 1,
                        completion_tokens: 1,
                        total_tokens: 1,
                        prompt_tokens_details: None,
                        completion_tokens_details: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-CompletionTokensDetails",
            "CompletionTokensDetails",
            {
                let x: aimux_core::openai_output::CompletionTokensDetails =
                    aimux_core::openai_output::CompletionTokensDetails {
                        reasoning_tokens: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ContentPart",
            "ContentPart",
            {
                let x: aimux_core::content::ContentPart = aimux_core::content::ContentPart::Text {
                    text: String::new(),
                    provider_options: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::content::ContentPart>),
        ),
        (
            "empty-EmbeddingCallOptions",
            "EmbeddingCallOptions",
            {
                let x: aimux_core::embedding_model::EmbeddingCallOptions =
                    aimux_core::embedding_model::EmbeddingCallOptions {
                        values: vec![],
                        max_retries: None,
                        timeout: None,
                        provider_options: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingCallOptions>),
        ),
        (
            "empty-EmbeddingResponse",
            "EmbeddingResponse",
            {
                let x: aimux_core::embedding_model::EmbeddingResponse =
                    aimux_core::embedding_model::EmbeddingResponse {
                        headers: None,
                        body: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingResponse>),
        ),
        (
            "empty-EmbeddingResult",
            "EmbeddingResult",
            {
                let x: aimux_core::embedding_model::EmbeddingResult =
                    aimux_core::embedding_model::EmbeddingResult {
                        embeddings: vec![],
                        usage: None,
                        provider_metadata: None,
                        response: None,
                        warnings: vec![],
                    };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingResult>),
        ),
        (
            "empty-EmbeddingUsage",
            "EmbeddingUsage",
            {
                let x: aimux_core::embedding_model::EmbeddingUsage =
                    aimux_core::embedding_model::EmbeddingUsage { tokens: 1 };
                v(&x)
            },
            Some(back::<aimux_core::embedding_model::EmbeddingUsage>),
        ),
        (
            "empty-Fingerprint",
            "Fingerprint",
            {
                let x: aimux_core::trace::fingerprint::Fingerprint =
                    aimux_core::trace::fingerprint::Fingerprint {
                        body_hash: String::new(),
                        len_bytes: 1,
                        block_size: 1,
                        block_hashes: vec![],
                        token_estimate: 1,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::fingerprint::Fingerprint>),
        ),
        (
            "empty-FinishReason",
            "FinishReason",
            {
                let x: aimux_core::types::FinishReason = aimux_core::types::FinishReason {
                    unified: aimux_core::types::FinishReasonUnified::Stop,
                    raw: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReason>),
        ),
        (
            "empty-FinishReasonUnified",
            "FinishReasonUnified",
            {
                let x: aimux_core::types::FinishReasonUnified =
                    aimux_core::types::FinishReasonUnified::Stop;
                v(&x)
            },
            Some(back::<aimux_core::types::FinishReasonUnified>),
        ),
        (
            "empty-FunctionToolInputExample",
            "FunctionToolInputExample",
            {
                let x: aimux_core::tool::FunctionToolInputExample =
                    aimux_core::tool::FunctionToolInputExample {
                        input: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::tool::FunctionToolInputExample>),
        ),
        (
            "empty-GenerateObjectResult",
            "GenerateObjectResult",
            {
                let x: aimux_core::generate::GenerateObjectResult =
                    aimux_core::generate::GenerateObjectResult {
                        object: serde_json::json!({}),
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: None,
                        },
                        raw_finish_reason: None,
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
                        warnings: vec![],
                        reasoning: None,
                        provider_metadata: None,
                        response: aimux_core::types::ResponseMetadata {
                            id: None,
                            timestamp: None,
                            model_id: None,
                        },
                        raw: aimux_core::generate::GenerateTextResult {
                            content: vec![],
                            text: String::new(),
                            tool_calls: vec![],
                            finish_reason: aimux_core::types::FinishReason {
                                unified: aimux_core::types::FinishReasonUnified::Stop,
                                raw: None,
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
                            warnings: vec![],
                            raw: aimux_core::result::GenerateResult {
                                content: vec![],
                                finish_reason: aimux_core::types::FinishReason {
                                    unified: aimux_core::types::FinishReasonUnified::Stop,
                                    raw: None,
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
                                warnings: vec![],
                                provider_metadata: None,
                                request: None,
                                response: None,
                            },
                            reasoning: vec![],
                            reasoning_text: String::new(),
                            sources: vec![],
                            files: vec![],
                            response_messages: vec![],
                            raw_finish_reason: None,
                            provider_metadata: None,
                            request: aimux_core::shared::RequestInfo { body: None },
                            response: aimux_core::shared::ResponseInfo {
                                id: None,
                                timestamp: None,
                                model_id: None,
                                headers: None,
                                body: None,
                            },
                            total_usage: aimux_core::types::Usage {
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
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateObjectResult>),
        ),
        (
            "empty-GenerateResult",
            "GenerateResult",
            {
                let x: aimux_core::result::GenerateResult = aimux_core::result::GenerateResult {
                    content: vec![],
                    finish_reason: aimux_core::types::FinishReason {
                        unified: aimux_core::types::FinishReasonUnified::Stop,
                        raw: None,
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
                    warnings: vec![],
                    provider_metadata: None,
                    request: None,
                    response: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::result::GenerateResult>),
        ),
        (
            "empty-GenerateTextOptions",
            "GenerateTextOptions",
            {
                let x: aimux_core::generate::GenerateTextOptions =
                    aimux_core::generate::GenerateTextOptions {
                        max_output_tokens: None,
                        temperature: None,
                        stop_sequences: None,
                        top_p: None,
                        top_k: None,
                        presence_penalty: None,
                        frequency_penalty: None,
                        response_format: None,
                        seed: Default::default(),
                        tools: None,
                        tool_choice: None,
                        headers: None,
                        provider_options: None,
                        reasoning: None,
                        instructions: None,
                        max_retries: None,
                        timeout: None,
                        session_id: None,
                        include_raw_chunks: None,
                        abort_signal: Default::default(),
                        repair_tool_call: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateTextOptions>),
        ),
        (
            "empty-GenerateTextResult",
            "GenerateTextResult",
            {
                let x: aimux_core::generate::GenerateTextResult =
                    aimux_core::generate::GenerateTextResult {
                        content: vec![],
                        text: String::new(),
                        tool_calls: vec![],
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: None,
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
                        warnings: vec![],
                        raw: aimux_core::result::GenerateResult {
                            content: vec![],
                            finish_reason: aimux_core::types::FinishReason {
                                unified: aimux_core::types::FinishReasonUnified::Stop,
                                raw: None,
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
                            warnings: vec![],
                            provider_metadata: None,
                            request: None,
                            response: None,
                        },
                        reasoning: vec![],
                        reasoning_text: String::new(),
                        sources: vec![],
                        files: vec![],
                        response_messages: vec![],
                        raw_finish_reason: None,
                        provider_metadata: None,
                        request: aimux_core::shared::RequestInfo { body: None },
                        response: aimux_core::shared::ResponseInfo {
                            id: None,
                            timestamp: None,
                            model_id: None,
                            headers: None,
                            body: None,
                        },
                        total_usage: aimux_core::types::Usage {
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
                    };
                v(&x)
            },
            Some(back::<aimux_core::generate::GenerateTextResult>),
        ),
        (
            "empty-ImageCallOptions",
            "ImageCallOptions",
            {
                let x: aimux_core::image_model::ImageCallOptions =
                    aimux_core::image_model::ImageCallOptions {
                        prompt: None,
                        n: 1,
                        size: None,
                        aspect_ratio: None,
                        seed: Default::default(),
                        files: None,
                        mask: None,
                        provider_options: Default::default(),
                        max_retries: None,
                        timeout: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageCallOptions>),
        ),
        (
            "empty-ImageOutputs",
            "ImageOutputs",
            {
                let x: aimux_core::image_model::ImageOutputs =
                    aimux_core::image_model::ImageOutputs::Base64(vec![]);
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageOutputs>),
        ),
        (
            "empty-ImageResponse",
            "ImageResponse",
            {
                let x: aimux_core::image_model::ImageResponse =
                    aimux_core::image_model::ImageResponse {
                        timestamp: None,
                        model_id: None,
                        headers: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageResponse>),
        ),
        (
            "empty-ImageResult",
            "ImageResult",
            {
                let x: aimux_core::image_model::ImageResult =
                    aimux_core::image_model::ImageResult {
                        images: aimux_core::image_model::ImageOutputs::Base64(vec![]),
                        warnings: vec![],
                        provider_metadata: None,
                        response: aimux_core::image_model::ImageResponse {
                            timestamp: None,
                            model_id: None,
                            headers: None,
                        },
                        usage: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageResult>),
        ),
        (
            "empty-ImageUsage",
            "ImageUsage",
            {
                let x: aimux_core::image_model::ImageUsage = aimux_core::image_model::ImageUsage {
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::image_model::ImageUsage>),
        ),
        (
            "empty-InputTokenUsage",
            "InputTokenUsage",
            {
                let x: aimux_core::types::InputTokenUsage = aimux_core::types::InputTokenUsage {
                    total: None,
                    no_cache: None,
                    cache_read: None,
                    cache_write: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::types::InputTokenUsage>),
        ),
        (
            "empty-ModelMessage",
            "ModelMessage",
            {
                let x: aimux_core::message::ModelMessage = aimux_core::message::ModelMessage {
                    role: aimux_core::message::Role::System,
                    content: aimux_core::message::MessageContent::Text(String::new()),
                };
                v(&x)
            },
            Some(back::<aimux_core::message::ModelMessage>),
        ),
        (
            "empty-OutputTokenUsage",
            "OutputTokenUsage",
            {
                let x: aimux_core::types::OutputTokenUsage = aimux_core::types::OutputTokenUsage {
                    total: None,
                    text: None,
                    reasoning: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::types::OutputTokenUsage>),
        ),
        (
            "empty-PrefixBreak",
            "PrefixBreak",
            {
                let x: aimux_core::trace::store::PrefixBreak =
                    aimux_core::trace::store::PrefixBreak {
                        at_record_id: String::new(),
                        prev_record_id: String::new(),
                        lcp_bytes: 1,
                        expected_break: true,
                        kind: aimux_core::trace::store::BreakKind::SystemChanged,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::PrefixBreak>),
        ),
        (
            "empty-PromptTokensDetails",
            "PromptTokensDetails",
            {
                let x: aimux_core::openai_output::PromptTokensDetails =
                    aimux_core::openai_output::PromptTokensDetails {
                        cached_tokens: 1,
                        cache_write_tokens: None,
                    };
                v(&x)
            },
            None,
        ),
        (
            "empty-ReasoningEffort",
            "ReasoningEffort",
            {
                let x: aimux_core::types::ReasoningEffort =
                    aimux_core::types::ReasoningEffort::ProviderDefault;
                v(&x)
            },
            None,
        ),
        (
            "empty-RequestCacheHints",
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
            "empty-RequestInfo",
            "RequestInfo",
            {
                let x: aimux_core::shared::RequestInfo =
                    aimux_core::shared::RequestInfo { body: None };
                v(&x)
            },
            Some(back::<aimux_core::shared::RequestInfo>),
        ),
        (
            "empty-RerankingCallOptions",
            "RerankingCallOptions",
            {
                let x: aimux_core::reranking_model::RerankingCallOptions =
                    aimux_core::reranking_model::RerankingCallOptions {
                        documents: aimux_core::reranking_model::RerankingDocuments::Text {
                            values: vec![],
                        },
                        query: String::new(),
                        top_n: None,
                        max_retries: None,
                        timeout: None,
                        provider_options: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingCallOptions>),
        ),
        (
            "empty-RerankingRank",
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
            "empty-RerankingResponse",
            "RerankingResponse",
            {
                let x: aimux_core::reranking_model::RerankingResponse =
                    aimux_core::reranking_model::RerankingResponse {
                        id: None,
                        timestamp: None,
                        model_id: None,
                        headers: None,
                        body: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingResponse>),
        ),
        (
            "empty-RerankingResult",
            "RerankingResult",
            {
                let x: aimux_core::reranking_model::RerankingResult =
                    aimux_core::reranking_model::RerankingResult {
                        ranking: vec![],
                        provider_metadata: None,
                        warnings: None,
                        response: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::reranking_model::RerankingResult>),
        ),
        (
            "empty-ResponseInfo",
            "ResponseInfo",
            {
                let x: aimux_core::shared::ResponseInfo = aimux_core::shared::ResponseInfo {
                    id: None,
                    timestamp: None,
                    model_id: None,
                    headers: None,
                    body: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::shared::ResponseInfo>),
        ),
        (
            "empty-ResponseMetadata",
            "ResponseMetadata",
            {
                let x: aimux_core::types::ResponseMetadata = aimux_core::types::ResponseMetadata {
                    id: None,
                    timestamp: None,
                    model_id: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::types::ResponseMetadata>),
        ),
        (
            "empty-Role",
            "Role",
            {
                let x: aimux_core::message::Role = aimux_core::message::Role::System;
                v(&x)
            },
            Some(back::<aimux_core::message::Role>),
        ),
        (
            "empty-SearchCallOptions",
            "SearchCallOptions",
            {
                let x: aimux_core::search_model::SearchCallOptions =
                    aimux_core::search_model::SearchCallOptions {
                        query: String::new(),
                        max_results: None,
                        include_raw_content: None,
                        time_range: None,
                        include_domains: None,
                        exclude_domains: None,
                        max_retries: None,
                        timeout: None,
                        provider_options: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchCallOptions>),
        ),
        (
            "empty-SearchResponse",
            "SearchResponse",
            {
                let x: aimux_core::search_model::SearchResponse =
                    aimux_core::search_model::SearchResponse {
                        headers: None,
                        body: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResponse>),
        ),
        (
            "empty-SearchResult",
            "SearchResult",
            {
                let x: aimux_core::search_model::SearchResult =
                    aimux_core::search_model::SearchResult {
                        results: vec![],
                        answer: None,
                        provider_metadata: None,
                        warnings: vec![],
                        response: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResult>),
        ),
        (
            "empty-SearchResultItem",
            "SearchResultItem",
            {
                let x: aimux_core::search_model::SearchResultItem =
                    aimux_core::search_model::SearchResultItem {
                        title: None,
                        url: None,
                        content: None,
                        raw_content: None,
                        score: None,
                        provider_metadata: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::search_model::SearchResultItem>),
        ),
        (
            "empty-SessionChainView",
            "SessionChainView",
            {
                let x: aimux_core::trace::store::SessionChainView =
                    aimux_core::trace::store::SessionChainView {
                        session_id: String::new(),
                        record_ids: vec![],
                        prefix_stability: 0.5,
                        breaks: vec![],
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::SessionChainView>),
        ),
        (
            "empty-Source",
            "Source",
            {
                let x: aimux_core::result::Source = aimux_core::result::Source::Url {
                    id: String::new(),
                    url: String::new(),
                    title: None,
                    provider_metadata: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::result::Source>),
        ),
        (
            "empty-SpeechCallOptions",
            "SpeechCallOptions",
            {
                let x: aimux_core::speech_model::SpeechCallOptions =
                    aimux_core::speech_model::SpeechCallOptions {
                        text: String::new(),
                        voice: None,
                        output_format: None,
                        instructions: None,
                        speed: None,
                        language: None,
                        provider_options: None,
                        max_retries: None,
                        timeout: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechCallOptions>),
        ),
        (
            "empty-SpeechRequest",
            "SpeechRequest",
            {
                let x: aimux_core::speech_model::SpeechRequest =
                    aimux_core::speech_model::SpeechRequest { body: None };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechRequest>),
        ),
        (
            "empty-SpeechResponse",
            "SpeechResponse",
            {
                let x: aimux_core::speech_model::SpeechResponse =
                    aimux_core::speech_model::SpeechResponse {
                        timestamp: None,
                        model_id: None,
                        headers: None,
                        body: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechResponse>),
        ),
        (
            "empty-SpeechResult",
            "SpeechResult",
            {
                let x: aimux_core::speech_model::SpeechResult =
                    aimux_core::speech_model::SpeechResult {
                        audio: aimux_core::speech_model::AudioData::Base64(String::new()),
                        warnings: vec![],
                        request: None,
                        response: aimux_core::speech_model::SpeechResponse {
                            timestamp: None,
                            model_id: None,
                            headers: None,
                            body: None,
                        },
                        provider_metadata: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::speech_model::SpeechResult>),
        ),
        (
            "empty-StreamPart",
            "StreamPart",
            {
                let x: aimux_core::stream_part::StreamPart =
                    aimux_core::stream_part::StreamPart::TextStart {
                        id: String::new(),
                        provider_metadata: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::stream_part::StreamPart>),
        ),
        (
            "empty-StreamTextResultAggregated",
            "StreamTextResultAggregated",
            {
                let x: aimux_core::result::StreamTextResultAggregated =
                    aimux_core::result::StreamTextResultAggregated {
                        content: vec![],
                        text: String::new(),
                        reasoning: vec![],
                        reasoning_text: String::new(),
                        tool_calls: vec![],
                        sources: vec![],
                        files: vec![],
                        finish_reason: aimux_core::types::FinishReason {
                            unified: aimux_core::types::FinishReasonUnified::Stop,
                            raw: None,
                        },
                        raw_finish_reason: None,
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
                        total_usage: aimux_core::types::Usage {
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
                        warnings: vec![],
                        provider_metadata: None,
                        request: aimux_core::shared::RequestInfo { body: None },
                        response: aimux_core::shared::ResponseInfo {
                            id: None,
                            timestamp: None,
                            model_id: None,
                            headers: None,
                            body: None,
                        },
                        response_messages: vec![],
                    };
                v(&x)
            },
            Some(back::<aimux_core::result::StreamTextResultAggregated>),
        ),
        (
            "empty-TimeoutConfiguration",
            "TimeoutConfiguration",
            {
                let x: aimux_core::options::TimeoutConfiguration =
                    aimux_core::options::TimeoutConfiguration {
                        total_ms: Default::default(),
                        step_ms: Default::default(),
                        first_chunk_ms: Default::default(),
                        chunk_ms: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::options::TimeoutConfiguration>),
        ),
        (
            "empty-Tool",
            "Tool",
            {
                let x: aimux_core::tool::Tool =
                    aimux_core::tool::Tool::Function(aimux_core::tool::FunctionTool {
                        name: String::new(),
                        description: None,
                        input_schema: serde_json::json!({}),
                        strict: None,
                        provider_options: None,
                        input_examples: None,
                    });
                v(&x)
            },
            Some(back::<aimux_core::tool::Tool>),
        ),
        (
            "empty-ToolCall",
            "ToolCall",
            {
                let x: aimux_core::tool::ToolCall = aimux_core::tool::ToolCall {
                    tool_call_id: String::new(),
                    tool_name: String::new(),
                    input: serde_json::json!({}),
                    provider_executed: None,
                    dynamic: None,
                    provider_metadata: None,
                    invalid: None,
                    error: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::tool::ToolCall>),
        ),
        (
            "empty-ToolChoice",
            "ToolChoice",
            {
                let x: aimux_core::tool::ToolChoice = Default::default();
                v(&x)
            },
            None,
        ),
        (
            "empty-TraceFilter",
            "TraceFilter",
            {
                let x: aimux_core::trace::store::TraceFilter =
                    aimux_core::trace::store::TraceFilter {
                        provider: None,
                        model: None,
                        session_id: None,
                        since_unix_ms: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::TraceFilter>),
        ),
        (
            "empty-TraceRecord",
            "TraceRecord",
            {
                let x: aimux_core::trace::record::TraceRecord =
                    aimux_core::trace::record::TraceRecord {
                        provider: String::new(),
                        model: String::new(),
                        request_id: None,
                        session_id: None,
                        call_id: String::new(),
                        sent_at_unix_ms: Default::default(),
                        lcp_token_upper: None,
                        ttft_ms: Default::default(),
                        fingerprint: aimux_core::trace::fingerprint::Fingerprint {
                            body_hash: String::new(),
                            len_bytes: 1,
                            block_size: 1,
                            block_hashes: vec![],
                            token_estimate: 1,
                        },
                        usage: aimux_core::trace::record::UsageSnapshot {
                            input_total: None,
                            input_no_cache: None,
                            cache_read: None,
                            cache_write: None,
                            output_total: None,
                            output_text: None,
                            output_reasoning: None,
                            raw: None,
                        },
                        response_cache_headers: None,
                        request_cache_hints: None,
                        verdict: None,
                        error: None,
                        monotonic_sent_ms: Default::default(),
                        scope_key: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::record::TraceRecord>),
        ),
        (
            "empty-TraceStats",
            "TraceStats",
            {
                let x: aimux_core::trace::store::TraceStats =
                    aimux_core::trace::store::TraceStats {
                        provider: String::new(),
                        model: String::new(),
                        requests: 1,
                        input_tokens_total: 1,
                        claimed_cache_read_total: 1,
                        claimed_cache_write_total: 1,
                        reported_hit_rate: None,
                        client_upper_bound_hit_rate: None,
                        verdict_counts: Default::default(),
                        ttft_p50_ms: None,
                        ttft_p95_ms: None,
                        errors: 1,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::store::TraceStats>),
        ),
        (
            "empty-TranscriptionCallOptions",
            "TranscriptionCallOptions",
            {
                let x: aimux_core::transcription_model::TranscriptionCallOptions =
                    aimux_core::transcription_model::TranscriptionCallOptions {
                        audio: aimux_core::transcription_model::AudioInput::Binary(vec![]),
                        media_type: String::new(),
                        provider_options: None,
                        max_retries: None,
                        timeout: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionCallOptions>),
        ),
        (
            "empty-TranscriptionRequest",
            "TranscriptionRequest",
            {
                let x: aimux_core::transcription_model::TranscriptionRequest =
                    aimux_core::transcription_model::TranscriptionRequest { body: None };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionRequest>),
        ),
        (
            "empty-TranscriptionResponse",
            "TranscriptionResponse",
            {
                let x: aimux_core::transcription_model::TranscriptionResponse =
                    aimux_core::transcription_model::TranscriptionResponse {
                        timestamp: None,
                        model_id: None,
                        headers: None,
                        body: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionResponse>),
        ),
        (
            "empty-TranscriptionResult",
            "TranscriptionResult",
            {
                let x: aimux_core::transcription_model::TranscriptionResult =
                    aimux_core::transcription_model::TranscriptionResult {
                        text: String::new(),
                        segments: vec![],
                        language: None,
                        duration_in_seconds: None,
                        warnings: vec![],
                        request: None,
                        response: aimux_core::transcription_model::TranscriptionResponse {
                            timestamp: None,
                            model_id: None,
                            headers: None,
                            body: None,
                        },
                        provider_metadata: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionResult>),
        ),
        (
            "empty-TranscriptionSegment",
            "TranscriptionSegment",
            {
                let x: aimux_core::transcription_model::TranscriptionSegment =
                    aimux_core::transcription_model::TranscriptionSegment {
                        text: String::new(),
                        start_second: 0.5,
                        end_second: 0.5,
                    };
                v(&x)
            },
            Some(back::<aimux_core::transcription_model::TranscriptionSegment>),
        ),
        (
            "empty-UploadFileCallOptions",
            "UploadFileCallOptions",
            {
                let x: aimux_core::files_model::UploadFileCallOptions =
                    aimux_core::files_model::UploadFileCallOptions {
                        data: aimux_core::files_model::UploadFileData::Data {
                            data: aimux_core::shared::FileBytes::Binary(vec![]),
                        },
                        media_type: String::new(),
                        filename: None,
                        provider_options: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::files_model::UploadFileCallOptions>),
        ),
        (
            "empty-UploadFileResult",
            "UploadFileResult",
            {
                let x: aimux_core::files_model::UploadFileResult =
                    aimux_core::files_model::UploadFileResult {
                        provider_reference: Default::default(),
                        media_type: None,
                        filename: None,
                        provider_metadata: None,
                        warnings: vec![],
                    };
                v(&x)
            },
            Some(back::<aimux_core::files_model::UploadFileResult>),
        ),
        (
            "empty-Usage",
            "Usage",
            {
                let x: aimux_core::types::Usage = aimux_core::types::Usage {
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
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Usage>),
        ),
        (
            "empty-UsageSnapshot",
            "UsageSnapshot",
            {
                let x: aimux_core::trace::record::UsageSnapshot =
                    aimux_core::trace::record::UsageSnapshot {
                        input_total: None,
                        input_no_cache: None,
                        cache_read: None,
                        cache_write: None,
                        output_total: None,
                        output_text: None,
                        output_reasoning: None,
                        raw: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::trace::record::UsageSnapshot>),
        ),
        (
            "empty-VideoCallOptions",
            "VideoCallOptions",
            {
                let x: aimux_core::video_model::VideoCallOptions =
                    aimux_core::video_model::VideoCallOptions {
                        prompt: None,
                        n: 1,
                        aspect_ratio: None,
                        resolution: None,
                        duration: None,
                        fps: None,
                        seed: Default::default(),
                        image: None,
                        frame_images: None,
                        input_references: None,
                        generate_audio: None,
                        provider_options: Default::default(),
                        max_retries: None,
                        poll: None,
                        timeout: None,
                        headers: None,
                        abort_signal: Default::default(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoCallOptions>),
        ),
        (
            "empty-VideoData",
            "VideoData",
            {
                let x: aimux_core::video_model::VideoData =
                    aimux_core::video_model::VideoData::Url {
                        url: String::new(),
                        media_type: String::new(),
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoData>),
        ),
        (
            "empty-VideoPollOptions",
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
            "empty-VideoResponse",
            "VideoResponse",
            {
                let x: aimux_core::video_model::VideoResponse =
                    aimux_core::video_model::VideoResponse {
                        timestamp: None,
                        model_id: None,
                        headers: None,
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoResponse>),
        ),
        (
            "empty-VideoResult",
            "VideoResult",
            {
                let x: aimux_core::video_model::VideoResult =
                    aimux_core::video_model::VideoResult {
                        videos: vec![],
                        warnings: vec![],
                        provider_metadata: None,
                        response: aimux_core::video_model::VideoResponse {
                            timestamp: None,
                            model_id: None,
                            headers: None,
                        },
                    };
                v(&x)
            },
            Some(back::<aimux_core::video_model::VideoResult>),
        ),
        (
            "empty-Warning",
            "Warning",
            {
                let x: aimux_core::types::Warning = aimux_core::types::Warning::Unsupported {
                    feature: String::new(),
                    details: None,
                };
                v(&x)
            },
            Some(back::<aimux_core::types::Warning>),
        ),
    ]
}
