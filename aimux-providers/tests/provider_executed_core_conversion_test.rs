//! A7 E.3: every provider that builds provider-executed tool results yields
//! `"providerExecuted": true` after core conversion, on the generate path
//! (`generate_text`, generate.rs step content) and the stream path
//! (`stream_text`, `into_text_stream_part`). Provider-layer output itself stays
//! unmarked (V4 has no providerExecuted on tool results).
//!
//! Inputs are the repo's recorded cassettes where one exists (anthropic,
//! gemini, openai responses), the gemini cassettes replayed against Vertex
//! (same wire), and the upstream-port mock bodies for xAI and Hugging Face.

#[path = "common/mock_fetch.rs"]
mod mock_fetch;

use aimux_core::generate::{GenerateTextOptions, generate_text, stream_text};
use aimux_core::language_model::LanguageModel;
use aimux_core::tool::{ProviderTool, Tool};
use aimux_provider_utils::Resolvable;
use aimux_providers::anthropic::{AnthropicProviderSettings, create_anthropic};
use aimux_providers::openai::{OpenAIProviderSettings, create_openai};
use aimux_providers::vertex::{VertexProviderSettings, create_google_vertex};
use aimux_providers::xai::{XAIProviderSettings, create_xai};
use aimux_providers::{
    GoogleProviderSettings, HuggingFaceProviderSettings, create_google, create_huggingface,
};
use futures::StreamExt;
use mock_fetch::{Canned, MockFetch};
use serde_json::{Value, json};

fn cassette(rel: &str) -> Canned {
    let path = format!("{}/tests/cassettes/{rel}", env!("CARGO_MANIFEST_DIR"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let body = v["response"]["body"].as_str().unwrap().to_owned();
    let trimmed = body.trim_start();
    let content_type = if trimmed.starts_with("event:") || trimmed.starts_with("data:") {
        "text/event-stream"
    } else {
        "application/json"
    };
    Canned {
        status: 200,
        headers: vec![("content-type".into(), content_type.into())],
        body: body.into_bytes(),
    }
}

fn sse(events: &[Value]) -> Canned {
    Canned {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        body: (events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>()
            + "data: [DONE]\n\n")
            .into_bytes(),
    }
}

fn options(tool_id: &str, name: &str) -> GenerateTextOptions {
    GenerateTextOptions {
        tools: Some(vec![Tool::Provider(ProviderTool {
            id: tool_id.into(),
            name: name.into(),
            args: serde_json::Map::new(),
        })]),
        ..Default::default()
    }
}

/// Every tool-result / tool-error in `parts` is marked; returns how many.
fn assert_all_marked(what: &str, parts: &Value) -> usize {
    let results: Vec<&Value> = parts
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["type"] == "tool-result" || p["type"] == "tool-error")
        .collect();
    assert!(!results.is_empty(), "{what}: no provider tool result");
    for r in &results {
        assert_eq!(r["providerExecuted"], json!(true), "{what}: {r}");
    }
    results.len()
}

async fn generate(what: &str, model: &dyn LanguageModel, opts: GenerateTextOptions) -> usize {
    let result = generate_text(model, "hi", opts).await.unwrap();
    assert_all_marked(what, &serde_json::to_value(&result.content).unwrap())
}

async fn stream(what: &str, model: &dyn LanguageModel, opts: GenerateTextOptions) -> usize {
    let result = stream_text(model, "hi", opts).await.unwrap();
    let parts: Vec<Value> = result
        .stream
        .map(|p| serde_json::to_value(p.unwrap()).unwrap())
        .collect()
        .await;
    assert_all_marked(what, &Value::Array(parts))
}

macro_rules! anthropic {
    ($canned:expr) => {
        create_anthropic(AnthropicProviderSettings {
            api_key: Some("k".into()),
            base_url: Some("https://example.test/v1".into()),
            fetch: Some(MockFetch::new(vec![$canned]).transport()),
            ..Default::default()
        })
        .unwrap()
        .messages("claude-sonnet-4-0")
    };
}

#[tokio::test]
async fn anthropic_generate_and_stream() {
    let ws = options("anthropic.web_search_20250305", "web_search");
    let model = anthropic!(cassette("anthropic/test_pause_turn_web_search_vcr_1.json"));
    generate("anthropic web_search", &model, ws).await;
    let ce = options("anthropic.code_execution_20250825", "code_execution");
    let model = anthropic!(cassette(
        "anthropic/test_anthropic_code_execution_tool_stream.json"
    ));
    stream("anthropic code_execution stream", &model, ce).await;
    let wf = options("anthropic.web_fetch_20250910", "web_fetch");
    let model = anthropic!(cassette(
        "anthropic/test_anthropic_web_fetch_tool_stream.json"
    ));
    stream("anthropic web_fetch stream", &model, wf).await;
}

fn google(canned: Canned) -> impl LanguageModel {
    create_google(GoogleProviderSettings {
        api_key: Some("k".into()),
        base_url: Some("https://example.test/v1beta".into()),
        fetch: Some(MockFetch::new(vec![canned]).transport()),
        ..Default::default()
    })
    .unwrap()
    .chat("gemini-3-flash-preview")
}

fn vertex(canned: Canned) -> impl LanguageModel {
    create_google_vertex(VertexProviderSettings {
        api_key: Some(Resolvable::Value("k".into())),
        base_url: Some("https://example.test".into()),
        fetch: Some(MockFetch::new(vec![canned]).transport()),
        ..Default::default()
    })
    .unwrap()
    .chat("gemini-2.5-flash")
}

#[tokio::test]
async fn google_generate_and_stream() {
    let ce = || options("google.code_execution", "code_execution");
    let model = google(cassette("gemini/test_code_execution.json"));
    generate("google code_execution", &model, ce()).await;
    let model = google(cassette("gemini/test_code_execution_stream.json"));
    stream("google code_execution stream", &model, ce()).await;
}

#[tokio::test]
async fn vertex_generate_and_stream() {
    let ce = || options("google.code_execution", "code_execution");
    let model = vertex(cassette("gemini/test_code_execution.json"));
    generate("vertex code_execution", &model, ce()).await;
    let model = vertex(cassette("gemini/test_code_execution_stream.json"));
    stream("vertex code_execution stream", &model, ce()).await;
}

fn openai_responses(canned: Canned) -> impl LanguageModel {
    create_openai(OpenAIProviderSettings {
        api_key: Some("k".into()),
        base_url: Some("https://example.test/v1".into()),
        fetch: Some(MockFetch::new(vec![canned]).transport()),
        ..Default::default()
    })
    .unwrap()
    .responses("gpt-5")
}

#[tokio::test]
async fn openai_responses_generate_and_stream() {
    let ws = || options("openai.web_search", "web_search");
    let model = openai_responses(cassette(
        "openai/test_openai_responses_model_web_search_tool_1.json",
    ));
    generate("openai web_search", &model, ws()).await;
    // Events from openai_upstream_e2e_test::responses_web_search_stream (the
    // recorded *_stream_1 cassette holds a non-streaming body).
    let events = [
        json!({"type":"response.created","response":{"id":"r","created_at":1741630255,"model":"test-model"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"type":"web_search_call","id":"ws_1","status":"in_progress"}}),
        json!({"type":"response.output_item.done","output_index":0,"item":{"type":"web_search_call","id":"ws_1","status":"completed"}}),
        json!({"type":"response.completed","response":{"id":"r","usage":{"input_tokens":1,"input_tokens_details":{"cached_tokens":0},"output_tokens":1,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":2}}}),
    ];
    let model = openai_responses(sse(&events));
    stream("openai web_search stream", &model, ws()).await;
}

fn xai(canned: Canned) -> impl LanguageModel {
    create_xai(XAIProviderSettings {
        api_key: Some("k".into()),
        base_url: Some("https://example.test/v1/".into()),
        fetch: Some(MockFetch::new(vec![canned]).transport()),
        ..Default::default()
    })
    .unwrap()
    .responses("grok-4-fast-non-reasoning")
}

#[tokio::test]
async fn xai_generate_and_stream() {
    // Body from xai_upstream_e2e_test::tool_results_preserve_actions_and_sources.
    let body = json!({
        "id": "resp_123", "object": "response", "status": "completed",
        "model": "grok-4-fast-non-reasoning",
        "output": [{"type": "web_search_call", "id": "ws_1", "name": "web_search", "call_id": "",
            "status": "completed", "action": {"type": "search", "query": "q", "sources": []}}],
        "usage": {"input_tokens": 10, "output_tokens": 5},
    });
    let model = xai(Canned::json(&body));
    generate(
        "xai web_search",
        &model,
        options("xai.web_search", "web_search"),
    )
    .await;
    let item = json!({"type": "file_search_call", "id": "fs_1", "status": "completed",
        "queries": ["q"], "results": [{"file_id": "f", "filename": "a.txt", "score": 1.0, "text": "t"}]});
    let events = [
        json!({"type": "response.created", "response": {"id": "resp_1", "object": "response", "model": "grok-4-fast-non-reasoning", "output": []}}),
        json!({"type": "response.output_item.added", "output_index": 0, "item": item}),
        json!({"type": "response.output_item.done", "output_index": 0, "item": item}),
        json!({"type": "response.completed", "response": {"id": "resp_1", "object": "response", "model": "grok-4-fast-non-reasoning", "status": "completed", "output": [item], "usage": {"input_tokens": 1, "output_tokens": 1}}}),
    ];
    let model = xai(sse(&events));
    let mut fs = options("xai.file_search", "file_search");
    if let Some(Tool::Provider(tool)) = fs.tools.as_mut().and_then(|t| t.first_mut()) {
        tool.args.insert("vectorStoreIds".into(), json!(["vs_1"]));
    }
    stream("xai file_search stream", &model, fs).await;
}

fn huggingface(canned: Canned) -> impl LanguageModel {
    create_huggingface(HuggingFaceProviderSettings {
        api_key: Some("k".to_string().into()),
        base_url: Some("https://example.test/v1".into()),
        fetch: Some(MockFetch::new(vec![canned]).transport()),
        ..Default::default()
    })
    .unwrap()
    .responses("deepseek-ai/DeepSeek-V3-0324")
}

#[tokio::test]
async fn huggingface_generate_and_stream() {
    // Bodies from huggingface_responses_test (MCP call; streamed call with output).
    let body = json!({
        "id": "resp_mcp", "model": "m", "object": "response", "status": "completed",
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2},
        "output": [{"id": "mcp_1", "type": "mcp_call", "server_label": "web_search",
            "name": "search", "arguments": "{}", "output": "Found 25"}]
    });
    let model = huggingface(Canned::json(&body));
    generate(
        "huggingface mcp_call",
        &model,
        GenerateTextOptions::default(),
    )
    .await;
    let item = r#"{"id":"fc_1","type":"function_call","call_id":"call_1","name":"calculator","arguments":"{}","output":"8"}"#;
    let body = [
        r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"calculator","arguments":""},"sequence_number":1}"#.to_owned(),
        format!(r#"{{"type":"response.output_item.done","output_index":0,"item":{item},"sequence_number":2}}"#),
        r#"{"type":"response.completed","response":{"id":"r","status":"completed","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}},"sequence_number":3}"#.to_owned(),
    ]
    .iter()
    .map(|e| format!("data: {e}\n\n"))
    .collect::<String>();
    let canned = Canned {
        status: 200,
        headers: vec![("content-type".into(), "text/event-stream".into())],
        body: body.into_bytes(),
    };
    let model = huggingface(canned);
    stream(
        "huggingface output stream",
        &model,
        GenerateTextOptions::default(),
    )
    .await;
}
