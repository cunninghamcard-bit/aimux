//! A7 E.3: core marks provider tool results `providerExecuted: true` where it
//! converts provider output (generate.rs step content, stream_part.rs text
//! stream), as AI SDK does (convert-language-model-content.ts:113,
//! stream-language-model-call.ts:772). The provider layer stays unmarked.

use aimux_core::error::AiMuxError;
use aimux_core::generate::{GenerateTextOptions, generate_text};
use aimux_core::language_model::LanguageModel;
use aimux_core::options::CallOptions;
use aimux_core::result::{GenerateContent, GenerateResult, StreamResult};
use aimux_core::stream_part::{StreamPart, TextStreamPart};
use aimux_core::tool::{ToolCall, ToolResult, ToolResultSource};
use aimux_core::types::{FinishReason, FinishReasonUnified, Usage};
use serde_json::json;

fn v4_result() -> ToolResult {
    ToolResult {
        tool_call_id: "ws_1".into(),
        tool_name: "web_search".into(),
        result: json!({"hits": 3}),
        is_error: None,
        preliminary: None,
        source: ToolResultSource::provider(None),
        provider_metadata: None,
    }
}

struct Model;

#[async_trait::async_trait]
impl LanguageModel for Model {
    fn provider(&self) -> &str {
        "mock"
    }
    fn model_id(&self) -> &str {
        "mock"
    }
    async fn do_generate(&self, _: &CallOptions) -> Result<GenerateResult, AiMuxError> {
        Ok(GenerateResult {
            content: vec![GenerateContent::ToolResult(v4_result())],
            finish_reason: FinishReason {
                unified: FinishReasonUnified::Stop,
                raw: None,
            },
            usage: Usage::default(),
            warnings: vec![],
            provider_metadata: None,
            request: None,
            response: None,
        })
    }
    async fn do_stream(&self, _: &CallOptions) -> Result<StreamResult, AiMuxError> {
        unimplemented!()
    }
}

#[tokio::test]
async fn generate_marks_provider_tool_result() {
    let result = generate_text(&Model, "hi", GenerateTextOptions::default())
        .await
        .unwrap();
    let parts = serde_json::to_value(&result.content).unwrap();
    let tool_result = parts
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["type"] == "tool-result")
        .expect("tool-result in step content");
    assert_eq!(tool_result["providerExecuted"], json!(true));
    assert!(
        tool_result.get("dynamic").is_none(),
        "dynamic passed through"
    );
}

#[test]
fn text_stream_marks_provider_tool_result() {
    let part: StreamPart<ToolCall, _, _> = StreamPart::ToolResult(v4_result());
    let provider_json = serde_json::to_value(&part).unwrap();
    assert!(
        provider_json.get("providerExecuted").is_none(),
        "V4 layer unmarked"
    );
    let Some(TextStreamPart::ToolResult(result)) = part.into_text_stream_part() else {
        panic!("tool-result");
    };
    assert_eq!(result.provider_executed(), Some(true));
    assert_eq!(result.dynamic(), None);
}
