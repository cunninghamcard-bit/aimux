//! Regression tests for the OpenAI Responses provider **configuration**
//! plumbing (issue #166 B6 prerequisites).
//!
//! The Responses model must honour the same provider-level configuration as
//! the chat-completions model ([`OpenAIModel`]) and the AI SDK
//! `OpenAIResponsesLanguageModel`:
//!
//! 1. `OpenAIConfig.headers` / `OpenAIConfig.project` reach the wire
//!    (`build_auth_headers` parity with the chat path), with per-call
//!    `CallOptions.headers` winning.
//! 2. `OpenAIConfig.body_overrides` (provider level) and
//!    `CallOptions.body_overrides` (per call) are deep-merged into the built
//!    request body, per-call last, `null` deletes keys — exactly the
//!    chat-path `merge_body_overrides` contract (RFC-0017).
//! 3. The reasoning-stream `store` decision is derived from the effective
//!    request body, so provider options and body overrides cannot make the
//!    request and the reducer disagree.
//! 4. Provider options are read from the config-derived namespace
//!    (`"azure"` when the provider name contains `azure`, `"openai"`
//!    otherwise) with the AI SDK's `"openai"` fallback, and that same
//!    namespace is used for response provider-metadata.
//!
//! Streaming and non-streaming requests must build the identical body
//! (modulo `stream`), so every case is asserted on both paths.
//!
//! Each test mounts a `wiremock` server, drives `do_generate` / `do_stream`
//! and asserts on the *recorded HTTP request* (headers and body), not on
//! internals.

use std::collections::HashMap;

use futures::StreamExt;
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use aimux_core::content::ContentPart;
use aimux_core::language_model::LanguageModel;
use aimux_core::language_model_message::{LanguageModelPrompt, LanguageModelPromptMessage};
use aimux_core::message::Role;
use aimux_core::options::CallOptions;
use aimux_core::result::StreamResult;
use aimux_core::stream_part::StreamPart;

use aimux_providers::{OpenAIConfig, OpenAIProvider};

// ── fixtures ────────────────────────────────────────────────────────────────

fn test_prompt() -> LanguageModelPrompt {
    vec![LanguageModelPromptMessage {
        role: Role::User,
        content: vec![ContentPart::text("Hello")],
        ..Default::default()
    }]
}

fn provider_options(name: &str, value: Value) -> Option<HashMap<String, Value>> {
    let mut map = HashMap::new();
    map.insert(name.to_string(), value);
    Some(map)
}

fn config_headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// A minimal non-streaming Responses API body with a single text item.
fn text_response_body() -> Value {
    json!({
        "id": "resp_config",
        "object": "response",
        "created_at": 1741257730,
        "status": "completed",
        "error": null,
        "incomplete_details": null,
        "model": "gpt-4o-2024-07-18",
        "output": [
            {
                "id": "msg_config",
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [{ "type": "output_text", "text": "answer text", "annotations": [] }]
            }
        ],
        "usage": {
            "input_tokens": 10,
            "input_tokens_details": { "cached_tokens": 0 },
            "output_tokens": 20,
            "output_tokens_details": { "reasoning_tokens": 0 }
        }
    })
}

/// A minimal SSE stream: `response.created` -> `response.completed`.
fn minimal_sse_body() -> String {
    let events = [
        r#"{"type":"response.created","response":{"id":"resp_config","created_at":1741269019,"model":"gpt-4o-2024-07-18"}}"#,
        r#"{"type":"response.completed","response":{"id":"resp_config","created_at":1741269019,"model":"gpt-4o-2024-07-18","incomplete_details":null,"usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":0},"output_tokens":20,"output_tokens_details":{"reasoning_tokens":0}}}}"#,
    ];
    let mut body = String::new();
    for event in events {
        body.push_str("data: ");
        body.push_str(event);
        body.push_str("\n\n");
    }
    body.push_str("data: [DONE]\n\n");
    body
}

/// Mount a JSON response on `POST /responses`.
async fn mock_json_response(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(text_response_body()))
        .mount(server)
        .await;
}

/// Mount an SSE response on `POST /responses`.
async fn mock_sse_response(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(minimal_sse_body()),
        )
        .mount(server)
        .await;
}

/// Mount a JSON response on `POST /chat/completions` (chat-path parity checks).
async fn mock_chat_response(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl_config",
            "object": "chat.completion",
            "created": 1711115037,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "answer text" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 4, "completion_tokens": 3, "total_tokens": 7 }
        })))
        .mount(server)
        .await;
}

// ── recorded-request helpers ────────────────────────────────────────────────

/// The header value of the first recorded request, if present.
async fn request_header(server: &MockServer, name: &str) -> Option<String> {
    let requests = server
        .received_requests()
        .await
        .expect("no requests recorded");
    requests
        .first()
        .and_then(|r| r.headers.get(name))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// The JSON body of the first recorded request.
async fn request_body(server: &MockServer) -> Value {
    let requests = server
        .received_requests()
        .await
        .expect("no requests recorded");
    serde_json::from_slice(&requests[0].body).expect("request body should be JSON")
}

/// Drain a stream so the streaming request is fully recorded.
async fn drain(result: StreamResult) -> Vec<StreamPart> {
    let mut parts = Vec::new();
    let mut stream = result.stream;
    while let Some(part) = stream.next().await {
        parts.push(part.expect("stream part should be Ok"));
    }
    parts
}

// ════════════════════════════════════════════════════════════════════════════
// 1. Provider-level headers / project
// ════════════════════════════════════════════════════════════════════════════

mod headers {
    use super::*;

    /// `OpenAIConfig::with_headers` must reach the Responses request
    /// (`OpenAIConfig.headers` is documented as "merged into every request").
    #[tokio::test]
    async fn provider_headers_reach_generate_request() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_headers(config_headers(&[("x-provider-header", "provider-value")]));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        model
            .do_generate(&CallOptions::new(test_prompt()))
            .await
            .expect("do_generate should succeed");

        assert_eq!(
            request_header(&server, "x-provider-header").await,
            Some("provider-value".to_string())
        );
        assert_eq!(
            request_header(&server, "authorization").await,
            Some("Bearer test-key".to_string())
        );
    }

    /// `OpenAIConfig::with_project` must send `OpenAI-Project`.
    #[tokio::test]
    async fn project_header_reaches_generate_request() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_project("proj_123");
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        model
            .do_generate(&CallOptions::new(test_prompt()))
            .await
            .expect("do_generate should succeed");

        assert_eq!(
            request_header(&server, "openai-project").await,
            Some("proj_123".to_string())
        );
    }

    /// `OpenAIConfig::with_org_id` must keep sending `OpenAI-Organization`
    /// (behaviour that already worked — guarded against regression while the
    /// header builder is shared with the chat path).
    #[tokio::test]
    async fn organization_header_reaches_generate_request() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_org_id("org_123");
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        model
            .do_generate(&CallOptions::new(test_prompt()))
            .await
            .expect("do_generate should succeed");

        assert_eq!(
            request_header(&server, "openai-organization").await,
            Some("org_123".to_string())
        );
    }

    /// Per-call `CallOptions.headers` win over provider-level headers.
    #[tokio::test]
    async fn per_call_headers_override_provider_headers() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_headers(config_headers(&[
                ("x-scope", "provider"),
                ("x-provider-only", "kept"),
            ]));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            headers: Some(config_headers(&[("x-scope", "call")])),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        assert_eq!(
            request_header(&server, "x-scope").await,
            Some("call".to_string())
        );
        assert_eq!(
            request_header(&server, "x-provider-only").await,
            Some("kept".to_string())
        );
    }

    /// The streaming request carries the same provider headers as the
    /// non-streaming one.
    #[tokio::test]
    async fn provider_headers_reach_stream_request() {
        let server = MockServer::start().await;
        mock_sse_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_project("proj_123")
            .with_headers(config_headers(&[("x-provider-header", "provider-value")]));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let result = model
            .do_stream(&CallOptions::new(test_prompt()))
            .await
            .expect("do_stream should succeed");
        drain(result).await;

        assert_eq!(
            request_header(&server, "x-provider-header").await,
            Some("provider-value".to_string())
        );
        assert_eq!(
            request_header(&server, "openai-project").await,
            Some("proj_123".to_string())
        );
    }

    /// Header parity with the chat path: the same config + per-call headers
    /// must produce the same auth/organization/project/custom headers on
    /// `/chat/completions` and `/responses`.
    #[tokio::test]
    async fn headers_match_chat_path_for_the_same_config() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;
        mock_chat_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_org_id("org_123")
            .with_project("proj_123")
            .with_headers(config_headers(&[("x-provider-header", "provider-value")]));
        let provider = OpenAIProvider::new(config);

        let options = CallOptions {
            headers: Some(config_headers(&[("x-per-call", "call-value")])),
            ..CallOptions::new(test_prompt())
        };

        provider
            .model("gpt-4o")
            .do_generate(&options)
            .await
            .expect("chat do_generate should succeed");
        let chat_headers = {
            let requests = server.received_requests().await.expect("requests");
            requests[0].headers.clone()
        };

        provider
            .responses_model("gpt-4o")
            .do_generate(&options)
            .await
            .expect("responses do_generate should succeed");
        let responses_headers = {
            let requests = server.received_requests().await.expect("requests");
            requests[1].headers.clone()
        };

        for name in [
            "authorization",
            "openai-organization",
            "openai-project",
            "x-provider-header",
            "x-per-call",
        ] {
            assert_eq!(
                chat_headers.get(name).and_then(|v| v.to_str().ok()),
                responses_headers.get(name).and_then(|v| v.to_str().ok()),
                "header `{name}` must match between the chat and responses paths"
            );
        }
        assert_eq!(
            responses_headers
                .get("x-provider-header")
                .and_then(|v| v.to_str().ok()),
            Some("provider-value")
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 2. body_overrides (provider level + per call)
// ════════════════════════════════════════════════════════════════════════════

mod body_overrides {
    use super::*;

    /// Provider-level `OpenAIConfig.body_overrides` reach the Responses body.
    #[tokio::test]
    async fn provider_level_overrides_are_applied() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "enable_thinking": false }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        model
            .do_generate(&CallOptions::new(test_prompt()))
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["enable_thinking"], json!(false));
        assert_eq!(body["model"], json!("gpt-4o"));
    }

    /// Per-call `CallOptions.body_overrides` reach the Responses body.
    #[tokio::test]
    async fn per_call_overrides_are_applied() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            body_overrides: Some(json!({ "store": false, "safety_identifier": "user-1" })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["store"], json!(false));
        assert_eq!(body["safety_identifier"], json!("user-1"));
    }

    /// Per-call overrides win over provider-level ones; unrelated
    /// provider-level keys survive.
    #[tokio::test]
    async fn per_call_overrides_win_over_provider_level() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "temperature": 0.1, "user": "provider-user" }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            body_overrides: Some(json!({ "temperature": 0.9 })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["temperature"], json!(0.9));
        assert_eq!(body["user"], json!("provider-user"));
    }

    /// Provider-level and per-call overrides deep-merge (nested objects are
    /// merged key-by-key, not replaced wholesale).
    #[tokio::test]
    async fn provider_and_call_overrides_deep_merge() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "metadata": { "provider_key": "provider-value" } }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            body_overrides: Some(json!({ "metadata": { "call_key": "call-value" } })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["metadata"]["provider_key"], json!("provider-value"));
        assert_eq!(body["metadata"]["call_key"], json!("call-value"));
    }

    /// A `null` override deletes the key from the built body, at either level.
    #[tokio::test]
    async fn null_override_deletes_key() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "temperature": 0.4, "user": "provider-user" }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            temperature: Some(0.5),
            body_overrides: Some(json!({ "temperature": null, "user": null })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert!(
            body.get("temperature").is_none(),
            "null override must delete `temperature`: {body}"
        );
        assert!(
            body.get("user").is_none(),
            "null override must delete `user`: {body}"
        );
    }

    /// Overrides can replace a field the builder derived from a standard
    /// option (RFC-0017: applied last).
    #[tokio::test]
    async fn overrides_replace_builder_derived_fields() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            temperature: Some(0.9),
            body_overrides: Some(json!({ "temperature": 0.2 })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        assert_eq!(request_body(&server).await["temperature"], json!(0.2));
    }

    /// Without overrides the built body is unchanged.
    #[tokio::test]
    async fn no_overrides_leaves_body_unchanged() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        model
            .do_generate(&CallOptions::new(test_prompt()))
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["model"], json!("gpt-4o"));
        assert!(body.get("enable_thinking").is_none());
        assert!(body.get("user").is_none());
    }

    /// Provider-level and per-call overrides are applied **sequentially** to
    /// the built body, not composed into one patch: a provider `null` deletes
    /// the generated field, and the per-call patch then builds the object
    /// afresh (no resurrected `text.format`).
    #[tokio::test]
    async fn overrides_apply_sequentially_not_as_a_composed_patch() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "text": null }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            response_format: Some(aimux_core::options::ResponseFormat::Json {
                schema: Some(json!({ "type": "object" })),
                name: Some("response".to_string()),
                description: None,
            }),
            body_overrides: Some(json!({ "text": { "verbosity": "low" } })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(
            body["text"],
            json!({ "verbosity": "low" }),
            "provider `text: null` must delete the generated text.format before the \
             per-call patch rebuilds `text`: {body}"
        );
    }

    /// The converse shape: the provider patch replaces the generated object
    /// with a scalar and the per-call patch then installs an object — the
    /// result is the per-call object alone.
    #[tokio::test]
    async fn overrides_apply_sequentially_over_replaced_object() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "text": "scalar" }));
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            response_format: Some(aimux_core::options::ResponseFormat::Json {
                schema: Some(json!({ "type": "object" })),
                name: Some("response".to_string()),
                description: None,
            }),
            body_overrides: Some(json!({ "text": { "verbosity": "low" } })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        assert_eq!(
            request_body(&server).await["text"],
            json!({ "verbosity": "low" })
        );
    }

    /// Streaming and non-streaming builds agree (modulo `stream`).
    #[tokio::test]
    async fn streaming_and_non_streaming_bodies_are_equivalent() {
        let json_server = MockServer::start().await;
        mock_json_response(&json_server).await;
        let sse_server = MockServer::start().await;
        mock_sse_response(&sse_server).await;

        let config = |uri: String| {
            OpenAIConfig::new("test-key")
                .with_base_url(uri)
                .with_body_overrides(json!({ "metadata": { "provider_key": "provider-value" } }))
        };
        let options = || CallOptions {
            body_overrides: Some(json!({ "metadata": { "call_key": "call-value" } })),
            provider_options: provider_options("openai", json!({ "store": false })),
            ..CallOptions::new(test_prompt())
        };

        OpenAIProvider::new(config(json_server.uri()))
            .responses_model("gpt-4o")
            .do_generate(&options())
            .await
            .expect("do_generate should succeed");
        let mut generate_body = request_body(&json_server).await;

        let result = OpenAIProvider::new(config(sse_server.uri()))
            .responses_model("gpt-4o")
            .do_stream(&options())
            .await
            .expect("do_stream should succeed");
        drain(result).await;
        let mut stream_body = request_body(&sse_server).await;

        assert_eq!(generate_body.get("stream"), None);
        assert_eq!(stream_body["stream"], json!(true));
        generate_body.as_object_mut().unwrap().remove("stream");
        stream_body.as_object_mut().unwrap().remove("stream");
        assert_eq!(
            generate_body, stream_body,
            "streaming and non-streaming request bodies must be identical apart from `stream`"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 3. `store` cannot diverge between request and stream decision
// ════════════════════════════════════════════════════════════════════════════

mod store_flag {
    use super::*;

    /// An SSE stream that interleaves reasoning and text, so the position of
    /// `ReasoningEnd` reveals when the reducer concluded the reasoning summary:
    /// at `reasoning_summary_part.done` when the body stores the response,
    /// otherwise deferred to the reasoning `output_item.done`.
    fn reasoning_then_text_sse_body() -> String {
        let events = [
            r#"{"type":"response.created","response":{"id":"resp_store","created_at":1741269019,"model":"o3-mini-2025-01-31"}}"#,
            r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"rs_1","type":"reasoning"}}"#,
            r#"{"type":"response.reasoning_summary_part.added","item_id":"rs_1","summary_index":0}"#,
            r#"{"type":"response.reasoning_summary_text.delta","item_id":"rs_1","summary_index":0,"delta":"thinking"}"#,
            r#"{"type":"response.reasoning_summary_part.done","item_id":"rs_1","summary_index":0}"#,
            r#"{"type":"response.output_item.added","output_index":1,"item":{"id":"msg_1","type":"message","status":"in_progress","role":"assistant","content":[]}}"#,
            r#"{"type":"response.output_text.delta","item_id":"msg_1","output_index":1,"delta":"answer"}"#,
            r#"{"type":"response.output_item.done","output_index":0,"item":{"id":"rs_1","type":"reasoning","summary":[{"type":"summary_text","text":"thinking"}]}}"#,
            r#"{"type":"response.output_item.done","output_index":1,"item":{"id":"msg_1","type":"message","status":"completed","role":"assistant","content":[]}}"#,
            r#"{"type":"response.completed","response":{"id":"resp_store","created_at":1741269019,"model":"o3-mini-2025-01-31","incomplete_details":null,"usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":0},"output_tokens":20,"output_tokens_details":{"reasoning_tokens":20}}}}"#,
        ];
        let mut body = String::new();
        for event in events {
            body.push_str("data: ");
            body.push_str(event);
            body.push_str("\n\n");
        }
        body.push_str("data: [DONE]\n\n");
        body
    }

    async fn mock_reasoning_sse(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(reasoning_then_text_sse_body()),
            )
            .mount(server)
            .await;
    }

    fn index_of(parts: &[StreamPart], predicate: impl Fn(&StreamPart) -> bool) -> usize {
        parts
            .iter()
            .position(predicate)
            .expect("part should be present")
    }

    fn reasoning_end_before_text_start(parts: &[StreamPart]) -> bool {
        index_of(parts, |p| matches!(p, StreamPart::ReasoningEnd { .. }))
            < index_of(parts, |p| matches!(p, StreamPart::TextStart { .. }))
    }

    /// `provider_options.store = true` but a per-call `body_overrides.store =
    /// false`: the request sends `store: false` and the reducer must agree.
    #[tokio::test]
    async fn per_call_override_store_false_beats_provider_option_store_true() {
        let server = MockServer::start().await;
        mock_reasoning_sse(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("o3-mini");

        let options = CallOptions {
            provider_options: provider_options("openai", json!({ "store": true })),
            body_overrides: Some(json!({ "store": false })),
            ..CallOptions::new(test_prompt())
        };
        let result = model
            .do_stream(&options)
            .await
            .expect("do_stream should succeed");
        let parts = drain(result).await;

        assert_eq!(request_body(&server).await["store"], json!(false));
        assert!(
            !reasoning_end_before_text_start(&parts),
            "store=false must defer ReasoningEnd to output_item.done: {parts:?}"
        );
    }

    /// The converse: `provider_options.store = false` with a per-call
    /// `body_overrides.store = true` sends `store: true` and concludes the
    /// reasoning summary at `reasoning_summary_part.done`.
    #[tokio::test]
    async fn per_call_override_store_true_beats_provider_option_store_false() {
        let server = MockServer::start().await;
        mock_reasoning_sse(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("o3-mini");

        let options = CallOptions {
            provider_options: provider_options("openai", json!({ "store": false })),
            body_overrides: Some(json!({ "store": true })),
            ..CallOptions::new(test_prompt())
        };
        let result = model
            .do_stream(&options)
            .await
            .expect("do_stream should succeed");
        let parts = drain(result).await;

        assert_eq!(request_body(&server).await["store"], json!(true));
        assert!(
            reasoning_end_before_text_start(&parts),
            "store=true must conclude ReasoningEnd at reasoning_summary_part.done: {parts:?}"
        );
    }

    /// Provider-level `body_overrides.store` is honoured too (and reaches the
    /// same decision as the request).
    #[tokio::test]
    async fn provider_level_override_store_false_beats_provider_option_store_true() {
        let server = MockServer::start().await;
        mock_reasoning_sse(&server).await;

        let config = OpenAIConfig::new("test-key")
            .with_base_url(server.uri())
            .with_body_overrides(json!({ "store": false }));
        let model = OpenAIProvider::new(config).responses_model("o3-mini");

        let options = CallOptions {
            provider_options: provider_options("openai", json!({ "store": true })),
            ..CallOptions::new(test_prompt())
        };
        let result = model
            .do_stream(&options)
            .await
            .expect("do_stream should succeed");
        let parts = drain(result).await;

        assert_eq!(request_body(&server).await["store"], json!(false));
        assert!(
            !reasoning_end_before_text_start(&parts),
            "provider-level store=false must defer ReasoningEnd: {parts:?}"
        );
    }

    /// Without any `store` setting the body omits it and the reducer keeps the
    /// historical "not explicitly stored" behavior.
    #[tokio::test]
    async fn omitted_store_defers_reasoning_end() {
        let server = MockServer::start().await;
        mock_reasoning_sse(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("o3-mini");

        let result = model
            .do_stream(&CallOptions::new(test_prompt()))
            .await
            .expect("do_stream should succeed");
        let parts = drain(result).await;

        assert!(request_body(&server).await.get("store").is_none());
        assert!(!reasoning_end_before_text_start(&parts), "{parts:?}");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 4. Provider-options namespace key
// ════════════════════════════════════════════════════════════════════════════

mod provider_options_key {
    use super::*;

    /// Build a model whose provider name contains `azure` — the config-derived
    /// provider-options namespace (`providerOptionsName` in the AI SDK).
    fn azure_model(uri: String) -> impl LanguageModel {
        let config = OpenAIConfig::new("test-key")
            .with_base_url(uri)
            .with_provider("azure");
        OpenAIProvider::new(config).responses_model("gpt-4o")
    }

    /// The `azure` namespace is read when the provider is Azure.
    #[tokio::test]
    async fn azure_namespace_is_read_for_azure_provider() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let model = azure_model(server.uri());
        let options = CallOptions {
            provider_options: provider_options(
                "azure",
                json!({ "store": false, "maxToolCalls": 3, "user": "azure-user" }),
            ),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["store"], json!(false));
        assert_eq!(body["max_tool_calls"], json!(3));
        assert_eq!(body["user"], json!("azure-user"));
    }

    /// The `azure` namespace is read on the streaming path too.
    #[tokio::test]
    async fn azure_namespace_is_read_for_streaming() {
        let server = MockServer::start().await;
        mock_sse_response(&server).await;

        let model = azure_model(server.uri());
        let options = CallOptions {
            provider_options: provider_options("azure", json!({ "store": false })),
            ..CallOptions::new(test_prompt())
        };
        let result = model
            .do_stream(&options)
            .await
            .expect("do_stream should succeed");
        drain(result).await;

        assert_eq!(request_body(&server).await["store"], json!(false));
    }

    /// The AI SDK fallback: when the provider's own namespace is absent, the
    /// `openai` namespace is used.
    #[tokio::test]
    async fn openai_namespace_fallback_when_own_namespace_absent() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let model = azure_model(server.uri());
        let options = CallOptions {
            provider_options: provider_options("openai", json!({ "maxToolCalls": 7 })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        assert_eq!(request_body(&server).await["max_tool_calls"], json!(7));
    }

    /// The fallback is per namespace, not per key: when the provider's own
    /// namespace is present it is used exclusively (AI SDK
    /// `parseProviderOptions` semantics).
    #[tokio::test]
    async fn own_namespace_wins_over_openai_namespace() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let model = azure_model(server.uri());
        let mut map = HashMap::new();
        map.insert("azure".to_string(), json!({ "store": false }));
        map.insert(
            "openai".to_string(),
            json!({ "store": true, "maxToolCalls": 9 }),
        );

        let options = CallOptions {
            provider_options: Some(map),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        let body = request_body(&server).await;
        assert_eq!(body["store"], json!(false));
        assert!(
            body.get("max_tool_calls").is_none(),
            "the `openai` namespace must not be merged in when `azure` is present: {body}"
        );
    }

    /// The `openai` namespace keeps working for OpenAI-configured models.
    #[tokio::test]
    async fn openai_namespace_is_read_for_openai_provider() {
        let server = MockServer::start().await;
        mock_json_response(&server).await;

        let config = OpenAIConfig::new("test-key").with_base_url(server.uri());
        let model = OpenAIProvider::new(config).responses_model("gpt-4o");

        let options = CallOptions {
            provider_options: provider_options("openai", json!({ "store": false })),
            ..CallOptions::new(test_prompt())
        };
        model
            .do_generate(&options)
            .await
            .expect("do_generate should succeed");

        assert_eq!(request_body(&server).await["store"], json!(false));
    }

    /// Response provider-metadata uses the same namespace the request options
    /// were read from.
    #[tokio::test]
    async fn provider_metadata_uses_the_same_namespace() {
        let server = MockServer::start().await;
        mock_sse_response(&server).await;

        let model = azure_model(server.uri());
        let options = CallOptions {
            provider_options: provider_options("azure", json!({ "store": false })),
            ..CallOptions::new(test_prompt())
        };
        let result = model
            .do_stream(&options)
            .await
            .expect("do_stream should succeed");
        let parts = drain(result).await;

        let finish = parts
            .iter()
            .find(|p| matches!(p, StreamPart::Finish { .. }))
            .expect("should have a Finish part");
        match finish {
            StreamPart::Finish {
                provider_metadata, ..
            } => {
                let pm = provider_metadata.as_ref().expect("provider_metadata");
                assert_eq!(pm["azure"]["responseId"], json!("resp_config"));
                assert!(
                    pm.get("openai").is_none(),
                    "provider metadata must use the config-derived namespace: {pm}"
                );
            }
            _ => unreachable!(),
        }
    }
}
