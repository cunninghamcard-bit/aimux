//! `stream_text` forwards an empty text delta only when it carries provider
//! metadata (ai@7.0.127 stream-text.ts:2881-2891).

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;

use aimux_core::error::AiMuxError;
use aimux_core::generate::{GenerateTextOptions, stream_text};
use aimux_core::language_model::LanguageModel;
use aimux_core::options::CallOptions;
use aimux_core::result::{GenerateResult, StreamResult};
use aimux_core::stream_part::{StreamPart, TextStreamPart};

/// Replays fixed provider stream parts given as AI SDK JSON.
struct Scripted(serde_json::Value);

#[async_trait]
impl LanguageModel for Scripted {
    fn provider(&self) -> &str {
        "mock-provider"
    }
    fn model_id(&self) -> &str {
        "mock-model"
    }
    async fn do_generate(&self, _: &CallOptions) -> Result<GenerateResult, AiMuxError> {
        unreachable!()
    }
    async fn do_stream(&self, _: &CallOptions) -> Result<StreamResult, AiMuxError> {
        let parts: Vec<StreamPart> = serde_json::from_value(self.0.clone()).unwrap();
        Ok(StreamResult {
            stream: Box::pin(futures::stream::iter(parts.into_iter().map(Ok))),
            request: None,
            response: None,
        })
    }
}

async fn text_deltas(meta: Option<serde_json::Value>) -> Vec<(String, bool)> {
    let mut empty = json!({ "type": "text-delta", "id": "t", "delta": "" });
    if let Some(meta) = meta {
        empty["providerMetadata"] = meta;
    }
    let model = Scripted(json!([
        { "type": "stream-start", "warnings": [] },
        { "type": "text-start", "id": "t" },
        empty,
        { "type": "text-delta", "id": "t", "delta": "hi" },
        { "type": "text-end", "id": "t" },
        {
            "type": "finish",
            "finishReason": { "unified": "stop", "raw": "stop" },
            "usage": {
                "inputTokens": { "total": 1, "noCache": 1, "cacheRead": null, "cacheWrite": null },
                "outputTokens": { "total": 1, "text": 1, "reasoning": null }
            }
        }
    ]));
    let mut stream = stream_text(&model, "x", GenerateTextOptions::default())
        .await
        .unwrap()
        .stream;
    let mut out = Vec::new();
    while let Some(part) = stream.next().await {
        if let TextStreamPart::TextDelta {
            delta,
            provider_metadata,
            ..
        } = part.unwrap()
        {
            out.push((delta, provider_metadata.is_some()));
        }
    }
    out
}

#[tokio::test]
async fn empty_text_delta_without_provider_metadata_is_dropped() {
    assert_eq!(text_deltas(None).await, vec![("hi".to_owned(), false)]);
}

#[tokio::test]
async fn empty_text_delta_with_provider_metadata_is_kept() {
    let meta = json!({ "mock-provider": { "itemId": "msg_1" } });
    assert_eq!(
        text_deltas(Some(meta)).await,
        vec![(String::new(), true), ("hi".to_owned(), false)]
    );
}
