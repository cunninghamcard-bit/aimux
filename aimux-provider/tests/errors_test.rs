//! `ProviderError`: construction, Display text (upstream messages),
//! `is_retryable`, and the `source()` chain.

use std::error::Error;
use std::io;

use aimux_provider::{ModelType, ProviderError, TypeValidationContext};
use serde_json::json;

fn io_cause(text: &str) -> aimux_provider::BoxError {
    Box::new(io::Error::other(text.to_owned()))
}

#[test]
fn api_call_retryable_follows_upstream_status_rule() {
    for (status, retryable) in [
        (None, false),
        (Some(400), false),
        (Some(401), false),
        (Some(404), false),
        (Some(408), true),
        (Some(409), true),
        (Some(429), true),
        (Some(499), false),
        (Some(500), true),
        (Some(503), true),
    ] {
        let error = ProviderError::api_call("Bad", "https://x/v1", json!({"a": 1}), status);
        assert_eq!(error.is_retryable(), retryable, "status {status:?}");
        assert_eq!(error.name(), "AI_APICallError");
        assert_eq!(error.to_string(), "Bad");
    }
    // An explicit isRetryable overrides the default, as upstream.
    let ProviderError::ApiCall { .. } = ProviderError::api_call("m", "u", json!(null), Some(500))
    else {
        panic!()
    };
    let explicit = ProviderError::ApiCall {
        message: "m".into(),
        url: "u".into(),
        request_body_values: json!(null),
        status_code: Some(500),
        response_headers: None,
        response_body: Some("oops".into()),
        is_retryable: false,
        data: None,
        cause: Some(io_cause("reset")),
    };
    assert!(!explicit.is_retryable());
    assert_eq!(explicit.source().unwrap().to_string(), "reset");
}

#[test]
fn only_api_call_is_retryable() {
    for error in [
        ProviderError::empty_response_body(),
        ProviderError::invalid_response_data(json!({"x": 1})),
        ProviderError::no_content_generated(),
        ProviderError::LoadApiKey {
            message: "k".into(),
        },
        ProviderError::unsupported_functionality("seed"),
    ] {
        assert!(!error.is_retryable(), "{}", error.name());
    }
}

#[test]
fn default_messages_match_upstream() {
    let cases: Vec<(ProviderError, &str, &str)> = vec![
        (
            ProviderError::empty_response_body(),
            "AI_EmptyResponseBodyError",
            "Empty response body",
        ),
        (
            ProviderError::evaluation_unsupported_question_type("q1", "rating", "openai", "gpt"),
            "AI_EvaluationUnsupportedQuestionTypeError",
            "Question \"q1\" has type \"rating\", which is not supported by provider \"openai\" and model \"gpt\".",
        ),
        (
            ProviderError::invalid_response_data(json!({"a": [1]})),
            "AI_InvalidResponseDataError",
            "Invalid response data: {\"a\":[1]}.",
        ),
        (
            ProviderError::no_content_generated(),
            "AI_NoContentGeneratedError",
            "No content generated.",
        ),
        (
            ProviderError::no_such_model("m1", ModelType::EmbeddingModel),
            "AI_NoSuchModelError",
            "No such embeddingModel: m1",
        ),
        (
            ProviderError::no_such_provider_reference(
                "google",
                [
                    ("openai".to_string(), "f1".to_string()),
                    ("anthropic".to_string(), "f2".to_string()),
                ]
                .into(),
            ),
            "AI_NoSuchProviderReferenceError",
            "No provider reference found for provider 'google'. Available providers: anthropic, openai",
        ),
        (
            ProviderError::too_many_embedding_values_for_call(
                "openai",
                "e3",
                2,
                vec![json!("a"), json!("b"), json!("c")],
            ),
            "AI_TooManyEmbeddingValuesForCallError",
            "Too many values for a single embedding call. The openai model \"e3\" can only embed up to 2 values per call, but 3 values were provided.",
        ),
        (
            ProviderError::unsupported_functionality("topK"),
            "AI_UnsupportedFunctionalityError",
            "'topK' functionality not supported.",
        ),
        (
            ProviderError::LoadApiKey {
                message: "OpenAI API key is missing.".into(),
            },
            "AI_LoadAPIKeyError",
            "OpenAI API key is missing.",
        ),
        (
            ProviderError::LoadSetting {
                message: "region missing".into(),
            },
            "AI_LoadSettingError",
            "region missing",
        ),
        (
            ProviderError::InvalidArgument {
                message: "bad n".into(),
                argument: "n".into(),
                cause: None,
            },
            "AI_InvalidArgumentError",
            "bad n",
        ),
        (
            ProviderError::InvalidPrompt {
                message: "empty".into(),
                prompt: json!([]),
                cause: None,
            },
            "AI_InvalidPromptError",
            "empty",
        ),
        (
            ProviderError::AiSdk {
                name: "AI_Custom".into(),
                message: "custom".into(),
                cause: None,
            },
            "AI_Custom",
            "custom",
        ),
    ];
    for (error, name, text) in cases {
        assert_eq!(error.name(), name);
        assert_eq!(error.to_string(), text);
        assert!(error.source().is_none(), "{name}");
    }
}

#[test]
fn json_parse_message_and_source() {
    let error = ProviderError::json_parse("{\"a\":", Some(io_cause("EOF while parsing")));
    assert_eq!(error.name(), "AI_JSONParseError");
    assert_eq!(
        error.to_string(),
        "JSON parsing failed: Text: {\"a\":.\nError message: EOF while parsing"
    );
    assert_eq!(error.source().unwrap().to_string(), "EOF while parsing");
    let no_cause = ProviderError::json_parse("x", None);
    assert!(
        no_cause
            .to_string()
            .ends_with("Error message: unknown error")
    );
}

#[test]
fn type_validation_message_context_and_source_chain() {
    let inner = ProviderError::unsupported_functionality("x");
    let error = ProviderError::type_validation(
        json!({"id": 7}),
        Some(TypeValidationContext {
            field: Some("message.parts[3].data".into()),
            entity_name: Some("Message".into()),
            entity_id: Some("m-1".into()),
        }),
        Some(Box::new(inner)),
    );
    assert_eq!(
        error.to_string(),
        "Type validation failed for message.parts[3].data (Message, id: \"m-1\"): Value: {\"id\":7}.\nError message: 'x' functionality not supported."
    );
    // Two-level chain: TypeValidation -> UnsupportedFunctionality -> none.
    let first = error.source().unwrap();
    assert_eq!(first.to_string(), "'x' functionality not supported.");
    assert!(first.source().is_none());

    let bare = ProviderError::type_validation(json!(1), None, None);
    assert_eq!(
        bare.to_string(),
        "Type validation failed: Value: 1.\nError message: unknown error"
    );
}

#[test]
fn no_such_model_subclass_name_is_kept() {
    let ProviderError::NoSuchModel { model_type, .. } =
        ProviderError::no_such_model("x", ModelType::LanguageModel)
    else {
        panic!()
    };
    assert_eq!(model_type.as_str(), "languageModel");
    let sub = ProviderError::NoSuchModel {
        message: "No such provider: p".into(),
        error_name: "AI_NoSuchProviderError".into(),
        model_id: "x".into(),
        model_type: ModelType::LanguageModel,
    };
    assert_eq!(sub.name(), "AI_NoSuchProviderError");
}

#[test]
fn errors_are_send_sync_and_boxable() {
    fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<ProviderError>();
    let boxed: Box<dyn Error + Send + Sync> = Box::new(ProviderError::no_content_generated());
    assert_eq!(boxed.to_string(), "No content generated.");
}
