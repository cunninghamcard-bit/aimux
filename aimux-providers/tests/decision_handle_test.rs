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

/// A6 item 2: retry is call-level only, so the handle has no `max_retries`.
#[test]
fn handle_has_no_max_retries_option() {
    let error = serde_json::from_value::<ProviderOptions>(json!({"max_retries": 0}))
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field `max_retries`"), "{error}");
}

/// `body_overrides` no longer exists (A6 item 3): it is an unknown field like
/// any other, not a specially rejected one.
#[test]
fn body_overrides_is_an_unknown_field() {
    let error = serde_json::from_value::<ProviderOptions>(json!({
        "body_overrides": {"temperature": 0}
    }))
    .unwrap_err()
    .to_string();
    assert!(error.contains("unknown field `body_overrides`"), "{error}");
    let other = serde_json::from_value::<ProviderOptions>(json!({"not_a_field": 1}))
        .unwrap_err()
        .to_string();
    assert!(other.contains("unknown field `not_a_field`"), "{other}");
}

#[test]
fn recorded_identity_matches_model_identity() {
    for (name, key) in [
        ("openai", Some("sk-test")),
        ("vllm", Some("sk-test")),
        ("ollama", None),
    ] {
        let model = provider_handle(name, key.map(Into::into), None)
            .unwrap()
            .decision_model("served-alias")
            .unwrap();
        // What `decide` records: provider name + model name only (A6 item 1).
        let snap =
            aimux_core::recording::ProviderRecord::from_model(model.provider(), model.model_id());
        assert_eq!(snap.provider, model.provider());
        assert_eq!(snap.provider_id, name);
        assert_eq!(snap.model_id, model.model_id());
        let json = serde_json::to_value(&snap).unwrap();
        assert!(json.get("apiKeySource").is_none(), "{json}");
        assert!(json.get("baseUrl").is_none(), "{json}");
        let _ = key;
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
    assert_eq!(provider["modelId"], "gpt-6-luna");
    // Identity only (A6 item 1): no credential source, base URL or headers.
    assert!(provider.get("apiKeySource").is_none(), "{provider}");
    assert!(provider.get("baseUrl").is_none(), "{provider}");
    assert!(provider.get("providerOptions").is_none(), "{provider}");
    println!(
        "checked {} ({} bytes): no raw Authorization / x-api-key values",
        path.display(),
        written.len()
    );
}

/// #226 (D15): a keyless local runtime sends the placeholder credential
/// `Authorization: Bearer not-needed`.
#[tokio::test]
#[serial]
async fn keyless_local_runtime_sends_placeholder_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "stop"})))
        .mount(&server)
        .await;
    let model = provider_handle(
        "ollama",
        None,
        Some(ProviderOptions {
            base_url: Some(format!("{}/v1", server.uri())),
            ..Default::default()
        }),
    )
    .unwrap()
    .decision_model("served-alias")
    .unwrap();
    let mut options: DecisionCallOptions =
        serde_json::from_value(
            serde_json::from_str::<Value>(include_str!("fixtures/runtime_decisions.json")).unwrap()
                ["options"]
                .clone(),
        )
        .unwrap();
    options.max_retries = Some(0);
    let recorder = Arc::new(RingRecorder::new());
    recording::init_recording(Some(recorder.clone()));
    let _stop = Stop;
    assert!(decide(model.as_ref(), options).await.is_err());
    recording::init_recording(None);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].headers["authorization"], "Bearer not-needed");
    let recorded = serde_json::to_string(&recorder.completed()).unwrap();
    assert!(
        !recorded.contains("not-needed"),
        "placeholder key leaked into the recording: {recorded}"
    );
}

/// Explicit `Authorization` and `x-api-key` request headers reach the wire but
/// their values never appear in the recording.
#[tokio::test]
#[serial]
async fn recorded_explicit_auth_headers_are_redacted() {
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
        Some("sk-arg-raw-secret".into()),
        Some(ProviderOptions {
            base_url: Some(format!("{}/v1", server.uri())),
            headers: Some(HashMap::from([
                ("Authorization".into(), "Bearer hdr-raw-auth-secret".into()),
                ("x-api-key".into(), "hdr-raw-xkey-secret".into()),
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

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].headers["x-api-key"], "hdr-raw-xkey-secret");
    let mut file = Vec::new();
    recorder.export_jsonl(&mut file).unwrap();
    let written = String::from_utf8(file).unwrap();
    for secret in [
        "hdr-raw-auth-secret",
        "hdr-raw-xkey-secret",
        "sk-arg-raw-secret",
    ] {
        assert!(!written.contains(secret), "{secret} leaked: {written}");
    }
    let record: Value = serde_json::from_str(written.lines().next().unwrap()).unwrap();
    // The provider record is identity only (A6 item 1): the connection
    // headers are no longer copied into it at all.
    assert!(
        record["provider"].get("providerOptions").is_none(),
        "{record}"
    );
}
