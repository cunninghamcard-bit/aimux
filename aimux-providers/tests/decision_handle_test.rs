//! By-name decision provider handles (#226), the decision connection snapshot
//! and its secret redaction. No live API access.
use std::collections::HashMap;
use std::sync::Arc;

use aimux_core::decision_model::*;
use aimux_core::recording::{self, RingRecorder};
use aimux_providers::{ProviderOptions, provider_handle};
use serde_json::{Value, json};
use serial_test::serial;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn decision_providers_resolve_by_name() {
    for name in ["laya", "ollama", "llamacpp", "localai", "vllm", "sglang"] {
        let handle = provider_handle(name, None, None).unwrap();
        let model = handle.decision_model("served-alias").unwrap();
        assert_eq!(model.provider(), name);
        // The registry path (`create_provider`, used by the FFI and bindings)
        // knows every runtime too, `laya` included.
        let registered = aimux_providers::create_provider(name, Default::default()).unwrap();
        assert_eq!(
            registered
                .decision_model("served-alias")
                .unwrap()
                .provider(),
            name
        );
    }
    let openai = provider_handle("openai", Some("key".into()), None).unwrap();
    assert_eq!(
        openai.decision_model("gpt-6-luna").unwrap().provider(),
        "openai"
    );
}

#[test]
fn handle_options_reach_decisions() {
    let handle = provider_handle(
        "laya",
        None,
        Some(ProviderOptions {
            max_retries: Some(0),
            ..Default::default()
        }),
    )
    .unwrap();
    let model = handle.decision_model("served-alias").unwrap();
    assert_eq!(model.retry_config().max_retries, 0);
    let overridden = provider_handle(
        "vllm",
        None,
        Some(ProviderOptions {
            body_overrides: Some(json!({"temperature": 0})),
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(matches!(
        overridden.decision_model("served-alias"),
        Err(aimux_core::AiMuxError::UnsupportedFunctionality(_))
    ));
}

#[test]
fn config_snapshot_matches_model_identity() {
    for (name, key) in [
        ("openai", Some("sk-test")),
        ("vllm", Some("sk-test")),
        ("ollama", None),
    ] {
        let model = provider_handle(name, key.map(Into::into), None)
            .unwrap()
            .decision_model("served-alias")
            .unwrap();
        let snap = model.config_snapshot();
        assert_eq!(snap.provider, model.provider());
        assert_eq!(snap.provider_id, name);
        assert_eq!(snap.model_id, model.model_id());
        assert!(snap.base_url.is_some());
        assert_eq!(
            snap.api_key_source.as_deref(),
            Some(if key.is_some() { "explicit" } else { "none" })
        );
    }
}

struct Stop;
impl Drop for Stop {
    fn drop(&mut self) {
        recording::init_recording(None);
    }
}

/// A recorded decision file holds no raw `Authorization` / `x-api-key` value:
/// neither in the provider snapshot nor in the exchanges.
#[tokio::test]
#[serial]
async fn recorded_decision_file_contains_no_raw_credentials() {
    let server = MockServer::start().await;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/openai_decisions.json")).unwrap();
    Mock::given(method("POST"))
        .and(path("/v1/decisions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&fixture["response"]))
        .mount(&server)
        .await;
    let model = provider_handle(
        "openai",
        Some("sk-raw-openai-secret".into()),
        Some(ProviderOptions {
            base_url: Some(format!("{}/v1", server.uri())),
            headers: Some(HashMap::from([
                ("x-api-key".into(), "xk-raw-header-secret".into()),
                ("X-Client".into(), "decision-tests".into()),
            ])),
            ..Default::default()
        }),
    )
    .unwrap()
    .decision_model("gpt-6-luna")
    .unwrap();
    let recorder = Arc::new(RingRecorder::new());
    recording::init_recording(Some(recorder.clone()));
    let _stop = Stop;
    let options: DecisionCallOptions = serde_json::from_value(fixture["options"].clone()).unwrap();
    decide(model.as_ref(), options).await.unwrap();
    recording::init_recording(None);

    let mut file = Vec::new();
    recorder.export_jsonl(&mut file).unwrap();
    let path = std::env::temp_dir().join("aimux-decision-recording-redaction.jsonl");
    std::fs::write(&path, &file).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    for secret in ["sk-raw-openai-secret", "xk-raw-header-secret"] {
        assert!(
            !written.contains(secret),
            "{secret} leaked into {}",
            path.display()
        );
    }
    let record: Value = serde_json::from_str(written.lines().next().unwrap()).unwrap();
    let provider = &record["provider"];
    assert_eq!(provider["providerId"], "openai");
    assert_eq!(provider["apiKeySource"], "explicit");
    assert_eq!(
        provider["providerOptions"]["headers"]["x-api-key"],
        "[REDACTED]"
    );
    assert_eq!(
        provider["providerOptions"]["headers"]["X-Client"],
        "decision-tests"
    );
    println!(
        "checked {} ({} bytes): no raw Authorization / x-api-key values",
        path.display(),
        written.len()
    );
}
