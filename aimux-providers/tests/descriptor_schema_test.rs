//! Phase 1 unit 1.7: the package descriptor schema draft
//! (`descriptors/package-descriptor.schema.json`, #200 §6.5, RFC-0040 §2.1)
//! and three descriptors written from the Rust factories: openai, anthropic,
//! openai-compatible. Each validates against the schema; the schema rejects
//! malformed descriptors; and the descriptors agree with the factories they
//! describe (provider strings, default method, credential fields).

use std::path::PathBuf;

use aimux_core::language_model::LanguageModel;
use aimux_providers::anthropic::{AnthropicProviderSettings, create_anthropic};
use aimux_providers::openai::{OpenAIProviderSettings, create_openai};
use aimux_providers::openai_compatible::{
    OpenAICompatibleProviderSettings, create_openai_compatible,
};
use serde_json::{Value, json};

const PACKAGES: [&str; 3] = ["openai", "anthropic", "openai-compatible"];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../descriptors")
}

fn load(name: &str) -> Value {
    let path = dir().join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn validator() -> jsonschema::Validator {
    jsonschema::validator_for(&load("package-descriptor.schema.json")).expect("schema compiles")
}

fn errors(validator: &jsonschema::Validator, descriptor: &Value) -> Vec<String> {
    validator
        .iter_errors(descriptor)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect()
}

#[test]
fn every_descriptor_validates() {
    let validator = validator();
    for package in PACKAGES {
        let descriptor = load(&format!("{package}.json"));
        let errors = errors(&validator, &descriptor);
        assert!(errors.is_empty(), "{package}: {errors:#?}");
        assert_eq!(descriptor["package_id"], json!(package));
    }
}

type Edit = Box<dyn Fn(&mut Value)>;

/// The schema is not vacuous: each of these edits must be rejected.
#[test]
fn schema_rejects_malformed_descriptors() {
    let validator = validator();
    let base = load("openai.json");
    let cases: Vec<(&str, Edit)> = vec![
        (
            "missing required field",
            Box::new(|d| {
                d.as_object_mut().unwrap().remove("credential_fields");
            }),
        ),
        (
            "unknown top-level field",
            Box::new(|d| {
                d["body_overrides"] = json!({});
            }),
        ),
        (
            "settings_schema allows unknown settings",
            Box::new(|d| {
                d["settings_schema"]["additionalProperties"] = json!(true);
            }),
        ),
        (
            "bad package id",
            Box::new(|d| d["package_id"] = json!("OpenAI")),
        ),
        (
            "unknown method kind",
            Box::new(|d| {
                d["methods"]["list"][0]["kind"] = json!("chat-ish");
            }),
        ),
        (
            "unknown auth mode",
            Box::new(|d| d["auth"]["modes"] = json!(["magic"])),
        ),
        (
            "closure as data",
            Box::new(|d| {
                d["runtime_handle_fields"][0]["kind"] = json!("function");
            }),
        ),
        (
            "non-semver sdk version",
            Box::new(|d| {
                d["source_verification"]["sdk_version"] = json!("latest");
            }),
        ),
    ];
    for (label, edit) in cases {
        let mut descriptor = base.clone();
        edit(&mut descriptor);
        assert!(
            !validator.is_valid(&descriptor),
            "{label}: the schema accepted it"
        );
    }
}

/// Cross-field rules the schema cannot express: credential and runtime
/// fields are settings names; the default method is listed; examples follow
/// the template.
#[test]
fn descriptors_are_internally_consistent() {
    for package in PACKAGES {
        let d = load(&format!("{package}.json"));
        let settings = d["settings_schema"]["properties"].as_object().unwrap();
        for credential in d["credential_fields"].as_array().unwrap() {
            let field = credential["field"].as_str().unwrap();
            assert!(
                settings.contains_key(field),
                "{package}: credential {field} not a setting"
            );
        }
        for handle in d["runtime_handle_fields"].as_array().unwrap() {
            let field = handle["field"].as_str().unwrap();
            assert!(
                !settings.contains_key(field),
                "{package}: runtime handle {field} listed as data"
            );
        }
        let methods: Vec<&str> = d["methods"]["list"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        let default = d["methods"]["default_method"].as_str().unwrap();
        assert!(
            methods.contains(&default),
            "{package}: default method {default} not listed"
        );
        for example in d["provider_string_rules"]["examples"]
            .as_object()
            .unwrap()
            .keys()
        {
            assert!(
                methods.contains(&example.as_str()),
                "{package}: example for unknown method {example}"
            );
        }
    }
}

/// Each provider-string example is what the Rust factory actually reports.
#[test]
fn provider_string_examples_match_the_factories() {
    let examples = |package: &str| -> Vec<(String, String)> {
        load(&format!("{package}.json"))["provider_string_rules"]["examples"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect()
    };

    let openai = create_openai(OpenAIProviderSettings {
        api_key: Some("k".into()),
        ..Default::default()
    })
    .unwrap();
    for (method, expected) in examples("openai") {
        let actual = match method.as_str() {
            "chat" => openai.chat("m").provider().to_string(),
            "completion" => openai.completion("m").provider().to_string(),
            "responses" => openai.responses("m").provider().to_string(),
            other => panic!("no check for openai method {other}"),
        };
        assert_eq!(actual, expected, "openai {method}");
    }
    assert_eq!(
        openai.call("m").provider(),
        openai.responses("m").provider()
    );

    let anthropic = create_anthropic(AnthropicProviderSettings {
        api_key: Some("k".into()),
        ..Default::default()
    })
    .unwrap();
    for (method, expected) in examples("anthropic") {
        let actual = match method.as_str() {
            "messages" => anthropic.messages("m").provider().to_string(),
            "chat" => anthropic.chat("m").provider().to_string(),
            other => panic!("no check for anthropic method {other}"),
        };
        assert_eq!(actual, expected, "anthropic {method}");
    }

    let compat = create_openai_compatible(OpenAICompatibleProviderSettings {
        name: "local".into(),
        base_url: "http://localhost:8080/v1".into(),
        ..Default::default()
    })
    .unwrap();
    for (method, expected) in examples("openai-compatible") {
        let actual = match method.as_str() {
            "chat" => compat.chat("m").provider().to_string(),
            "completion" => compat.completion("m").provider().to_string(),
            other => panic!("no check for openai-compatible method {other}"),
        };
        assert_eq!(actual, expected, "openai-compatible {method}");
    }
    assert_eq!(compat.call("m").provider(), compat.chat("m").provider());
}
