//! OpenAI language model — implements `LanguageModel` trait.
//!
//! The HTTP request/response handling lives in the crate-internal functions
//! `execute_generate` and `execute_stream`, which take a prepared request (URL,
//! headers, transport) and a model id. They call the `aimux-provider-utils` API helpers —
//! **no `reqwest` types cross this boundary**. This lets other providers that
//! speak the OpenAI chat-completions wire format (notably Azure OpenAI) reuse
//! the conversion + streaming logic while supplying their own URL and auth.

use aimux_core::tool::RawToolCall;
use std::collections::HashMap;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};

use aimux_core::error::AiMuxError;
use aimux_core::language_model::LanguageModel;
use aimux_core::options::CallOptions;
use aimux_core::result::{GenerateContent, GenerateResult, ReasoningOutput, Source, StreamResult};
use aimux_core::shared::{Warning, provider_namespace};
use aimux_core::stream_part::StreamPart;
use aimux_core::types::{FinishReason, FinishReasonUnified, ResponseMetadata, Usage};

use aimux_provider_utils::{
    HttpRequest, StreamingToolCallDelta, StreamingToolCallTracker, TransformStreamController,
    Transformer, TypeValidation, generate_id, pipe_through,
};

use super::config::OpenAIModelConfig;
use super::convert::RequestBodyResult;
use super::decode::{convert_usage, finish_reason, parse_finish_reason};
use super::types::{ChatCompletionResponse, StreamChunk, UsageResponse};

/// An OpenAI chat-completions language model.
///
/// Does **not** hold an HTTP client — the `aimux-provider-utils` API helpers use the
/// process-wide shared `Client` internally (RFC-0009 §4.1).
pub struct OpenAIModel {
    model_id: String,
    config: OpenAIModelConfig,
}

impl OpenAIModel {
    pub(crate) fn from_config(model_id: String, config: OpenAIModelConfig) -> Self {
        Self { model_id, config }
    }
}

// ── Usage conversion ─────────────────────────────────────────────────────────

#[async_trait]
impl LanguageModel for OpenAIModel {
    /// Provider identity for recording/routing: `"{name}.chat"` (`"openai.chat"`
    /// by default).
    fn provider(&self) -> &str {
        &self.config.provider
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn supported_urls(&self) -> aimux_core::language_model::SupportedUrls {
        self.config.supported_urls.clone()
    }

    async fn do_generate(&self, options: &CallOptions) -> Result<GenerateResult, AiMuxError> {
        let encoded = self.encode_request(options, false).await?;
        let http = self
            .config
            .http_request(encoded.url.clone(), encoded.headers.clone(), options);
        execute_generate(http, encoded).await
    }

    async fn do_stream(&self, options: &CallOptions) -> Result<StreamResult, AiMuxError> {
        let encoded = self.encode_request(options, true).await?;
        let http = self
            .config
            .http_request(encoded.url.clone(), encoded.headers.clone(), options);
        execute_stream(http, encoded, options).await
    }
}

impl OpenAIModel {
    /// The request this model would send for `options`, as plain data and
    /// without touching the transport (W.1). Resolves the configured URL and
    /// headers (credential included), then calls the pure [`encode_chat`].
    ///
    /// # Errors
    ///
    /// Returns header/credential resolution errors and request-build
    /// conversion errors.
    #[doc(hidden)]
    pub async fn encode_request(
        &self,
        options: &CallOptions,
        stream: bool,
    ) -> Result<EncodedChatRequest, AiMuxError> {
        let headers = self
            .config
            .request_headers(options.headers.as_ref())
            .await?;
        encode_chat(
            self.config.url("/chat/completions")?,
            headers,
            &self.model_id,
            options,
            stream,
            self.config.chat_options,
        )
    }
}

/// An encoded OpenAI chat-completions request: plain data only (no
/// transport, no closures). Content-type, user-agent and the transport are
/// added by the `aimux-provider-utils` helper when it is sent.
#[derive(Debug, Clone)]
pub struct EncodedChatRequest {
    /// Full chat-completions URL.
    pub url: String,
    /// Resolved request headers (credential included).
    pub headers: Vec<(String, String)>,
    /// The JSON request body.
    pub body: Value,
    /// Call warnings produced while building the body.
    pub warnings: Vec<Warning>,
}

/// Encode an OpenAI chat-completions request (W.1): a pure function from the
/// resolved URL/headers, model id and call options to plain data. The only
/// place the chat body is built.
///
/// # Errors
///
/// Returns request-build conversion errors.
pub(crate) fn encode_chat(
    url: String,
    headers: Vec<(String, String)>,
    model_id: &str,
    options: &CallOptions,
    stream: bool,
    parse_chat_options: super::options::ChatOptionsParser,
) -> Result<EncodedChatRequest, AiMuxError> {
    let RequestBodyResult { body, warnings } =
        super::convert::build_request_body_with_chat_options(
            model_id,
            options,
            stream,
            parse_chat_options,
        )?;
    Ok(EncodedChatRequest {
        url,
        headers,
        body,
        warnings,
    })
}

// ── Shared OpenAI chat-completions execution ─────────────────────────────────
//
// These free functions contain the actual HTTP + response-parsing logic. They
// are shared with providers that speak the OpenAI chat wire format (Azure
// OpenAI), which supply their own request: URL, auth headers and transport.

/// Execute a non-streaming OpenAI chat-completion request.
///
/// `http` carries the full chat-completions URL, the auth and request headers
/// and the transport; `encoded` is the request from [`encode_chat`].
///
/// # Errors
///
/// Returns request-build conversion errors, `ApiCall` for HTTP/transport
/// failures, `JsonParse` for a malformed body, and `InvalidResponseData` when
/// `choices` is empty.
pub(crate) async fn execute_generate(
    http: HttpRequest,
    encoded: EncodedChatRequest,
) -> Result<GenerateResult, AiMuxError> {
    let EncodedChatRequest { body, warnings, .. } = encoded;

    let resp = aimux_provider_utils::post_json_to_api(
        http,
        body.clone(),
        aimux_provider_utils::create_json_response_handler::<ChatCompletionResponse>(),
        super::openai_failed_response_handler(),
    )
    .await?;

    let response_headers = resp.response_headers;

    // Parse the raw body once: the `Value` keeps the provider's original
    // fields (incl. vendor-specific usage fields) for `Usage.raw` (M10).
    let response_value = resp.raw_value.unwrap_or(Value::Null);
    let data = resp.value;

    let choice = data.choices.into_iter().next().ok_or_else(|| {
        AiMuxError::InvalidResponseData("Response did not contain any choices.".to_string())
    })?;

    // Build content array.
    let mut content = Vec::new();
    if let Some(text) = choice
        .message
        .content
        .filter(|text| !text.is_empty())
        .or_else(|| choice.message.audio.and_then(|audio| audio.transcript))
        && !text.is_empty()
    {
        content.push(GenerateContent::Text {
            text,
            provider_metadata: None,
        });
    }
    // Reasoning: prefer reasoning_content over reasoning.
    let reasoning_text = choice
        .message
        .reasoning_content
        .clone()
        .or_else(|| choice.message.reasoning.clone());
    if let Some(reasoning) = reasoning_text
        && !reasoning.is_empty()
    {
        content.push(GenerateContent::Reasoning(ReasoningOutput {
            text: reasoning,
            provider_metadata: None,
        }));
    }
    if let Some(tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            let input = tc.function.arguments;
            content.push(GenerateContent::ToolCall(RawToolCall {
                tool_call_id: tc.id,
                tool_name: tc.function.name,
                input: input.into(),
                provider_executed: None,
                dynamic: None,
                provider_metadata: None,
            }));
        }
    }
    // Parse annotations (URL citations) → Source content items.
    if let Some(annotations) = choice.message.annotations {
        for (i, ann) in annotations.iter().enumerate() {
            if ann.get("type").and_then(|v| v.as_str()) == Some("url_citation")
                && let Some(uc) = ann.get("url_citation")
            {
                content.push(GenerateContent::Source(Source::Url {
                    id: format!("annotation-{i}"),
                    url: uc
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    title: uc
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(std::string::ToString::to_string),
                    provider_metadata: None,
                }));
            }
        }
    }

    let finish_reason = finish_reason(choice.finish_reason.as_deref());

    let usage = data.usage.as_ref().map_or_else(Usage::default, |usage| {
        convert_usage(usage, response_value.get("usage"))
    });
    let raw_usage = data.usage.unwrap_or_default();

    // Build provider metadata: logprobs + prediction tokens.
    let mut pm_openai = serde_json::json!({});
    if let Some(ref lp) = choice.logprobs
        && let Some(content_lp) = lp.get("content")
    {
        pm_openai["logprobs"] = content_lp.clone();
    }
    // Accepted/rejected prediction tokens from completion_tokens_details.
    if let Some(ref details) = raw_usage.completion_tokens_details {
        if let Some(apt) = details.accepted_prediction_tokens {
            pm_openai["acceptedPredictionTokens"] = json!(apt);
        }
        if let Some(rpt) = details.rejected_prediction_tokens {
            pm_openai["rejectedPredictionTokens"] = json!(rpt);
        }
    }
    let provider_metadata =
        Some(provider_namespace("openai", pm_openai).expect("provider metadata must be an object"));

    Ok(GenerateResult {
        content,
        finish_reason,
        usage,
        warnings,
        provider_metadata,
        response: Some(aimux_core::shared::ResponseInfo {
            id: data.id,
            timestamp: data
                .created
                .filter(|created| *created != 0)
                .and_then(|secs| chrono::DateTime::from_timestamp(secs as i64, 0))
                .map(|dt| dt.to_rfc3339()),
            model_id: data.model,
            headers: Some(response_headers),
            body: Some(response_value),
        }),
        request: Some(aimux_core::shared::RequestInfo { body: Some(body) }),
    })
}

/// Execute a streaming OpenAI chat-completion request.
///
/// `http` carries the full chat-completions URL, the auth and request headers
/// and the transport; `encoded` is the request from [`encode_chat`].
///
/// # Errors
///
/// Returns request-build conversion errors and `ApiCall` when establishing the
/// stream fails; transport errors surface as `Err` items in the stream.
pub(crate) async fn execute_stream(
    http: HttpRequest,
    encoded: EncodedChatRequest,
    options: &CallOptions,
) -> Result<StreamResult, AiMuxError> {
    // M9 (RFC-0016): keep the warnings computed while building the body —
    // they are emitted in `StreamStart` below instead of being dropped.
    let EncodedChatRequest { body, warnings, .. } = encoded;
    let endpoint = http.url.clone();

    let resp = aimux_provider_utils::post_json_to_api(
        http,
        body.clone(),
        aimux_provider_utils::create_event_source_response_handler::<Value>(),
        super::openai_failed_response_handler(),
    )
    .await?;

    let response_headers = resp.response_headers;

    let mut sse_stream = resp.value;

    // Aimux checks only the first SSE event before returning the stream. The
    // baseline OpenAI provider scans until semantic output; this narrower peek
    // still keeps an immediately reported provider error inside Core's
    // operation-retry boundary (RFC-0031 §8.3). A normal event is chained back
    // below and is never consumed.
    let first_event = match sse_stream.next().await {
        Some(Err(error @ AiMuxError::ApiCall(_))) => return Err(error),
        first_event => first_event,
    };
    if let Some(Ok(ref event)) = first_event
        && let Some(err_obj) = event.get("error")
    {
        return Err(super::openai_stream_error(
            err_obj,
            &endpoint,
            body.clone(),
            response_headers.clone(),
        ));
    }

    let transformer = OpenAIChatStream {
        warnings,
        // M2 (RFC-0016): whether raw chunks should be emitted.
        include_raw_chunks: options.include_raw_chunks == Some(true),
        url: endpoint,
        request_body: body.clone(),
        response_headers: response_headers.clone(),
        text_started: false,
        reasoning_started: false,
        final_usage: Usage::default(),
        final_usage_raw: None,
        final_finish_reason: None,
        response_metadata_emitted: false,
        final_logprobs: None,
        // Same options as `@ai-sdk/openai`'s chat model.
        tool_calls: StreamingToolCallTracker::new()
            .with_generate_id(generate_id)
            .with_type_validation(TypeValidation::IfPresent),
        tool_parts: Vec::new(),
        stream_errored: false,
    };
    // Process the first event (already peeked) then the rest.
    let events = futures::stream::iter(first_event).chain(sse_stream);
    let stream = pipe_through(events, transformer);

    Ok(StreamResult {
        stream: Box::pin(stream),
        request: Some(aimux_core::shared::RequestInfo { body: Some(body) }),
        response: Some(aimux_core::shared::StreamResponseInfo {
            headers: Some(response_headers),
        }),
    })
}

const TEXT_ID: &str = "0";
const REASONING_ID: &str = "reasoning-0";

/// The `TransformStream` of the Chat Completions stream ([`execute_stream`]):
/// turns the parsed SSE events into stream parts.
struct OpenAIChatStream {
    warnings: Vec<Warning>,
    include_raw_chunks: bool,
    /// Request context for in-stream `error` payloads.
    url: String,
    request_body: Value,
    response_headers: HashMap<String, String>,

    text_started: bool,
    reasoning_started: bool,
    final_usage: Usage,
    final_usage_raw: Option<UsageResponse>,
    final_finish_reason: Option<FinishReason>,
    response_metadata_emitted: bool,
    final_logprobs: Option<Value>,
    /// Streamed tool calls, correlated by wire id, index and function name
    /// (the AI SDK's StreamingToolCallTracker) and finalized on flush.
    tool_calls: StreamingToolCallTracker,
    tool_parts: Vec<StreamPart>,
    stream_errored: bool,
}

impl OpenAIChatStream {
    fn end_reasoning(&mut self, controller: &mut TransformStreamController<StreamPart>) {
        if self.reasoning_started {
            controller.enqueue(StreamPart::ReasoningEnd {
                id: REASONING_ID.to_string(),
                provider_metadata: None,
            });
            self.reasoning_started = false;
        }
    }
}

impl Transformer for OpenAIChatStream {
    type Input = Result<Value, AiMuxError>;
    type Output = StreamPart;

    fn start(&mut self, controller: &mut TransformStreamController<StreamPart>) {
        controller.enqueue(StreamPart::StreamStart {
            warnings: std::mem::take(&mut self.warnings),
        });
    }

    fn transform(
        &mut self,
        event: Result<Value, AiMuxError>,
        controller: &mut TransformStreamController<StreamPart>,
    ) {
        let parsed = match event {
            Ok(parsed) => parsed,
            Err(error) => {
                let recoverable = error.is_recoverable_stream_error();
                self.stream_errored = true;
                controller.enqueue(StreamPart::Error { error });
                if !recoverable {
                    controller.terminate();
                }
                return;
            }
        };

        // M2 (RFC-0016): emit the raw provider chunk for debugging before it
        // is consumed below.
        if self.include_raw_chunks {
            controller.enqueue(StreamPart::Raw {
                raw_value: parsed.clone(),
            });
        }

        // Check for mid-stream error.
        if let Some(err_obj) = parsed.get("error") {
            controller.enqueue(StreamPart::Error {
                error: super::openai_stream_error(
                    err_obj,
                    &self.url,
                    self.request_body.clone(),
                    self.response_headers.clone(),
                ),
            });
            self.stream_errored = true;
            return;
        }

        // The chunk's top-level `usage`, taken from the raw JSON before the
        // chunk is consumed below. `chunk_usage_raw` keeps the provider's
        // original object for `Usage.raw` (M10).
        let chunk_usage_raw: Option<Value> = parsed.get("usage").cloned();
        let chunk_usage: Option<UsageResponse> = chunk_usage_raw
            .as_ref()
            .and_then(|u| serde_json::from_value(u.clone()).ok());

        let chunk: StreamChunk = match serde_json::from_value(parsed) {
            Ok(c) => c,
            Err(e) => {
                self.stream_errored = true;
                controller.enqueue(StreamPart::Error { error: e.into() });
                return;
            }
        };

        // Emit ResponseMetadata from the first valid chunk.
        if !self.response_metadata_emitted
            && (chunk.id.as_ref().is_some_and(|id| !id.is_empty())
                || chunk.model.as_ref().is_some_and(|model| !model.is_empty())
                || chunk.created.is_some_and(|created| created != 0))
        {
            self.response_metadata_emitted = true;
            controller.enqueue(StreamPart::ResponseMetadata(ResponseMetadata {
                id: chunk.id.clone(),
                timestamp: chunk
                    .created
                    .filter(|created| *created != 0)
                    .and_then(|secs| chrono::DateTime::from_timestamp(secs as i64, 0))
                    .map(|dt| dt.to_rfc3339()),
                model_id: chunk.model.clone(),
            }));
        }

        // Update usage from the chunk that carries it.
        if let Some(usage) = &chunk_usage {
            self.final_usage = convert_usage(usage, chunk_usage_raw.as_ref());
            self.final_usage_raw = Some(usage.clone());
        }

        for choice in chunk.choices {
            // Capture logprobs from the finish_reason chunk.
            if let Some(lp) = &choice.logprobs
                && let Some(content) = lp.get("content")
            {
                self.final_logprobs = Some(content.clone());
            }

            // Reasoning delta: prefer reasoning_content over reasoning.
            let reasoning_delta = choice
                .delta
                .reasoning_content
                .clone()
                .or_else(|| choice.delta.reasoning.clone());
            if let Some(reasoning) = reasoning_delta
                && !reasoning.is_empty()
            {
                if !self.reasoning_started {
                    self.reasoning_started = true;
                    controller.enqueue(StreamPart::ReasoningStart {
                        id: REASONING_ID.to_string(),
                        provider_metadata: None,
                    });
                }
                controller.enqueue(StreamPart::ReasoningDelta {
                    id: REASONING_ID.to_string(),
                    delta: reasoning,
                    provider_metadata: None,
                });
            }

            // Text delta; an active reasoning block ends before text starts.
            if let Some(content) = choice.delta.content {
                self.end_reasoning(controller);
                if !self.text_started {
                    self.text_started = true;
                    controller.enqueue(StreamPart::TextStart {
                        id: TEXT_ID.to_string(),
                        provider_metadata: None,
                    });
                }
                controller.enqueue(StreamPart::TextDelta {
                    id: TEXT_ID.to_string(),
                    delta: content,
                    provider_metadata: None,
                });
            }

            // Tool-call deltas; an active reasoning block ends first.
            if let Some(tool_call_deltas) = choice.delta.tool_calls {
                self.end_reasoning(controller);
                for dtc in &tool_call_deltas {
                    let function = dtc.function.as_ref();
                    let delta = StreamingToolCallDelta {
                        index: Some(dtc.index),
                        id: dtc.id.as_deref(),
                        r#type: dtc.r#type.as_deref(),
                        name: function.and_then(|f| f.name.as_deref()),
                        arguments: function.and_then(|f| f.arguments.as_deref()),
                        provider_metadata: None,
                    };
                    // A malformed delta (new call without a function name, or
                    // a non-`function` type) is invalid response data, as in
                    // the AI SDK; the stream ends.
                    if let Err(error) = self.tool_calls.process(delta, &mut self.tool_parts) {
                        controller.error(error.into());
                        return;
                    }
                    for part in self.tool_parts.drain(..) {
                        controller.enqueue(part);
                    }
                }
            }

            // Annotations / citations (URL citations → Source).
            if let Some(annotations) = choice.delta.annotations {
                for (i, ann) in annotations.iter().enumerate() {
                    if ann.get("type").and_then(|v| v.as_str()) == Some("url_citation")
                        && let Some(uc) = ann.get("url_citation")
                    {
                        controller.enqueue(StreamPart::Source(Source::Url {
                            id: format!("annotation-{i}"),
                            url: uc
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            title: uc
                                .get("title")
                                .and_then(|v| v.as_str())
                                .map(std::string::ToString::to_string),
                            provider_metadata: None,
                        }));
                    }
                }
            }

            // Finish reason; closes any open reasoning segment.
            if let Some(reason) = choice.finish_reason {
                self.end_reasoning(controller);
                self.stream_errored = false;
                self.final_finish_reason = Some(parse_finish_reason(&reason));
            }
        }
    }

    fn flush(mut self, controller: &mut TransformStreamController<StreamPart>) {
        // Close any remaining open reasoning and text segments.
        self.end_reasoning(controller);
        if self.text_started {
            controller.enqueue(StreamPart::TextEnd {
                id: TEXT_ID.to_string(),
                provider_metadata: None,
            });
        }

        // A parsable argument buffer can still be a prefix of a longer input:
        // like the AI SDK's tracker, finalize only when the stream flushes.
        self.tool_calls.finish(&mut self.tool_parts);
        for part in self.tool_parts.drain(..) {
            controller.enqueue(part);
        }

        // Provider metadata for the Finish part: logprobs + prediction tokens.
        let mut pm_openai = serde_json::json!({});
        if let Some(ref logprobs) = self.final_logprobs {
            pm_openai["logprobs"] = json!(logprobs);
        }
        if let Some(ref raw_usage) = self.final_usage_raw
            && let Some(ref details) = raw_usage.completion_tokens_details
        {
            if let Some(apt) = details.accepted_prediction_tokens {
                pm_openai["acceptedPredictionTokens"] = json!(apt);
            }
            if let Some(rpt) = details.rejected_prediction_tokens {
                pm_openai["rejectedPredictionTokens"] = json!(rpt);
            }
        }
        let provider_metadata =
            provider_namespace("openai", pm_openai).expect("provider metadata must be an object");

        controller.enqueue(StreamPart::Finish {
            finish_reason: if self.stream_errored {
                FinishReason {
                    unified: FinishReasonUnified::Error,
                    raw: None,
                }
            } else {
                self.final_finish_reason
                    .unwrap_or_else(|| finish_reason(None))
            },
            usage: self.final_usage,
            provider_metadata: Some(provider_metadata),
        });
    }
}

// ── Model listing (RFC-0027) ─────────────────────────────────────────────────

/// OpenAI-compatible `/models` response shape.
#[derive(serde::Deserialize)]
struct ModelsListResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}
#[derive(serde::Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
    #[serde(default)]
    created: Option<u64>,
}

/// One `GET {base_url}/models` exchange (OpenAI-compatible): no retry, no
/// recording. Discovery is not a Core operation and the AI SDK has no
/// equivalent, so a failure is reported to the caller as it happened.
///
/// # Errors
///
/// Returns the header-resolution error (a missing key is `LoadApiKey`),
/// `ApiCall` for HTTP/transport failures and `JsonParse` when the body does
/// not deserialize into the models list.
pub(crate) async fn list_models_once(
    config: &OpenAIModelConfig,
) -> Result<Vec<aimux_core::model_catalogue::RuntimeModel>, AiMuxError> {
    let headers = config.request_headers(None).await?;
    let resp = aimux_provider_utils::get_from_api(
        config.with_transport(HttpRequest {
            url: config.url("/models")?,
            headers,
            ..Default::default()
        }),
        aimux_provider_utils::create_json_response_handler(),
        super::openai_failed_response_handler(),
    )
    .await?;
    let parsed: ModelsListResponse = resp.value;

    Ok(parsed
        .data
        .into_iter()
        .map(|m| aimux_core::model_catalogue::RuntimeModel {
            id: m.id,
            owned_by: m.owned_by,
            created: m.created,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// `encode_chat` is pure: its URL and headers are passed through as given
    /// and the body is built from the model id, options and stream flag.
    #[test]
    fn encode_chat_is_plain_data_from_its_inputs() {
        let options = CallOptions {
            prompt: serde_json::from_value(json!([
                { "role": "user", "content": [{ "type": "text", "text": "hi" }] }
            ]))
            .unwrap(),
            max_output_tokens: Some(5),
            ..CallOptions::default()
        };
        let headers = vec![("authorization".to_string(), "Bearer k".to_string())];
        let url = "https://example.test/v1/chat/completions".to_string();

        let encoded = encode_chat(
            url.clone(),
            headers.clone(),
            "gpt-4o",
            &options,
            false,
            super::super::options::parse_chat_options,
        )
        .unwrap();
        assert_eq!(encoded.url, url);
        assert_eq!(encoded.headers, headers);
        assert!(encoded.warnings.is_empty(), "{:?}", encoded.warnings);
        assert_eq!(
            encoded.body,
            json!({
                "model": "gpt-4o",
                "max_tokens": 5,
                "messages": [{ "role": "user", "content": "hi" }],
            })
        );

        let streamed = encode_chat(
            url,
            headers,
            "gpt-4o",
            &options,
            true,
            super::super::options::parse_chat_options,
        )
        .unwrap();
        assert_eq!(streamed.body["stream"], json!(true));
        assert_eq!(
            streamed.body["stream_options"],
            json!({ "include_usage": true })
        );
    }
}
