//! `@ai-sdk/provider` `src/errors`: the provider error classes as one Rust
//! error type.
//!
//! Each variant mirrors one upstream class. `message` is the upstream
//! `message`; `cause` is reachable through [`std::error::Error::source`].
//! The constructors fill in upstream's default messages. Mapping to aimux
//! error codes and golden fixtures is deferred to 3.3 (D21).

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;

use crate::{JsonValue, SharedV4ProviderReference};

/// A boxed underlying error (`cause`).
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// `NoSuchModelError.modelType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    LanguageModel,
    EmbeddingModel,
    ImageModel,
    TranscriptionModel,
    SpeechModel,
    RerankingModel,
    VideoModel,
    EvaluationModel,
}

impl ModelType {
    /// Upstream spelling (`languageModel`, ...).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LanguageModel => "languageModel",
            Self::EmbeddingModel => "embeddingModel",
            Self::ImageModel => "imageModel",
            Self::TranscriptionModel => "transcriptionModel",
            Self::SpeechModel => "speechModel",
            Self::RerankingModel => "rerankingModel",
            Self::VideoModel => "videoModel",
            Self::EvaluationModel => "evaluationModel",
        }
    }
}

impl fmt::Display for ModelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `TypeValidationContext`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeValidationContext {
    pub field: Option<String>,
    pub entity_name: Option<String>,
    pub entity_id: Option<String>,
}

/// The `@ai-sdk/provider@4.0.21` error classes.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProviderError {
    /// `AISDKError` used directly (`ai-sdk-error.ts`).
    AiSdk {
        name: String,
        message: String,
        cause: Option<BoxError>,
    },
    /// `APICallError` (`api-call-error.ts`).
    ApiCall {
        message: String,
        url: String,
        request_body_values: JsonValue,
        status_code: Option<u16>,
        response_headers: Option<BTreeMap<String, String>>,
        response_body: Option<String>,
        is_retryable: bool,
        data: Option<JsonValue>,
        cause: Option<BoxError>,
    },
    /// `EmptyResponseBodyError` (`empty-response-body-error.ts`).
    EmptyResponseBody { message: String },
    /// `EvaluationUnsupportedQuestionTypeError`.
    EvaluationUnsupportedQuestionType {
        message: String,
        question_id: String,
        question_type: String,
        provider: String,
        model_id: String,
    },
    /// `InvalidArgumentError` (`invalid-argument-error.ts`).
    InvalidArgument {
        message: String,
        argument: String,
        cause: Option<BoxError>,
    },
    /// `InvalidPromptError` (`invalid-prompt-error.ts`).
    InvalidPrompt {
        message: String,
        prompt: JsonValue,
        cause: Option<BoxError>,
    },
    /// `InvalidResponseDataError` (`invalid-response-data-error.ts`).
    InvalidResponseData { message: String, data: JsonValue },
    /// `JSONParseError` (`json-parse-error.ts`).
    JsonParse {
        message: String,
        text: String,
        cause: Option<BoxError>,
    },
    /// `LoadAPIKeyError` (`load-api-key-error.ts`).
    LoadApiKey { message: String },
    /// `LoadSettingError` (`load-setting-error.ts`).
    LoadSetting { message: String },
    /// `NoContentGeneratedError` (`no-content-generated-error.ts`).
    NoContentGenerated { message: String },
    /// `NoSuchModelError` (`no-such-model-error.ts`). `error_name` defaults to
    /// `AI_NoSuchModelError`; subclasses pass their own.
    NoSuchModel {
        message: String,
        error_name: String,
        model_id: String,
        model_type: ModelType,
    },
    /// `NoSuchProviderReferenceError`.
    NoSuchProviderReference {
        message: String,
        provider: String,
        reference: SharedV4ProviderReference,
    },
    /// `TooManyEmbeddingValuesForCallError`.
    TooManyEmbeddingValuesForCall {
        message: String,
        provider: String,
        model_id: String,
        max_embeddings_per_call: usize,
        values: Vec<JsonValue>,
    },
    /// `TypeValidationError` (`type-validation-error.ts`).
    TypeValidation {
        message: String,
        value: JsonValue,
        context: Option<TypeValidationContext>,
        cause: Option<BoxError>,
    },
    /// `UnsupportedFunctionalityError`.
    UnsupportedFunctionality {
        message: String,
        functionality: String,
    },
}

/// `getErrorMessage(cause)`.
fn error_message(cause: Option<&BoxError>) -> String {
    cause.map_or_else(|| "unknown error".to_owned(), ToString::to_string)
}

impl ProviderError {
    /// `APICallError` with upstream's default `isRetryable`: status 408, 409,
    /// 429 or >= 500.
    #[must_use]
    pub fn api_call(
        message: impl Into<String>,
        url: impl Into<String>,
        request_body_values: JsonValue,
        status_code: Option<u16>,
    ) -> Self {
        Self::ApiCall {
            message: message.into(),
            url: url.into(),
            request_body_values,
            status_code,
            response_headers: None,
            response_body: None,
            is_retryable: status_code.is_some_and(|s| matches!(s, 408 | 409 | 429) || s >= 500),
            data: None,
            cause: None,
        }
    }

    /// `EmptyResponseBodyError` with the default message.
    #[must_use]
    pub fn empty_response_body() -> Self {
        Self::EmptyResponseBody {
            message: "Empty response body".into(),
        }
    }

    /// `EvaluationUnsupportedQuestionTypeError` with the default message.
    #[must_use]
    pub fn evaluation_unsupported_question_type(
        question_id: impl Into<String>,
        question_type: impl Into<String>,
        provider: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        let (question_id, question_type, provider, model_id) = (
            question_id.into(),
            question_type.into(),
            provider.into(),
            model_id.into(),
        );
        Self::EvaluationUnsupportedQuestionType {
            message: format!(
                "Question \"{question_id}\" has type \"{question_type}\", which is not supported by provider \"{provider}\" and model \"{model_id}\"."
            ),
            question_id,
            question_type,
            provider,
            model_id,
        }
    }

    /// `InvalidResponseDataError` with the default message.
    #[must_use]
    pub fn invalid_response_data(data: JsonValue) -> Self {
        Self::InvalidResponseData {
            message: format!("Invalid response data: {data}."),
            data,
        }
    }

    /// `JSONParseError`; the message embeds the text and the cause.
    #[must_use]
    pub fn json_parse(text: impl Into<String>, cause: Option<BoxError>) -> Self {
        let text = text.into();
        Self::JsonParse {
            message: format!(
                "JSON parsing failed: Text: {text}.\nError message: {}",
                error_message(cause.as_ref())
            ),
            text,
            cause,
        }
    }

    /// `NoContentGeneratedError` with the default message.
    #[must_use]
    pub fn no_content_generated() -> Self {
        Self::NoContentGenerated {
            message: "No content generated.".into(),
        }
    }

    /// `NoSuchModelError` with the default name and message.
    #[must_use]
    pub fn no_such_model(model_id: impl Into<String>, model_type: ModelType) -> Self {
        let model_id = model_id.into();
        Self::NoSuchModel {
            message: format!("No such {model_type}: {model_id}"),
            error_name: "AI_NoSuchModelError".into(),
            model_id,
            model_type,
        }
    }

    /// `NoSuchProviderReferenceError` with the default message.
    #[must_use]
    pub fn no_such_provider_reference(
        provider: impl Into<String>,
        reference: SharedV4ProviderReference,
    ) -> Self {
        let provider = provider.into();
        let available: Vec<&str> = reference.keys().map(String::as_str).collect();
        Self::NoSuchProviderReference {
            message: format!(
                "No provider reference found for provider '{provider}'. Available providers: {}",
                available.join(", ")
            ),
            provider,
            reference,
        }
    }

    /// `TooManyEmbeddingValuesForCallError`; the message counts `values`.
    #[must_use]
    pub fn too_many_embedding_values_for_call(
        provider: impl Into<String>,
        model_id: impl Into<String>,
        max_embeddings_per_call: usize,
        values: Vec<JsonValue>,
    ) -> Self {
        let (provider, model_id) = (provider.into(), model_id.into());
        Self::TooManyEmbeddingValuesForCall {
            message: format!(
                "Too many values for a single embedding call. The {provider} model \"{model_id}\" can only embed up to {max_embeddings_per_call} values per call, but {} values were provided.",
                values.len()
            ),
            provider,
            model_id,
            max_embeddings_per_call,
            values,
        }
    }

    /// `TypeValidationError`; the message embeds the context, value and cause.
    #[must_use]
    pub fn type_validation(
        value: JsonValue,
        context: Option<TypeValidationContext>,
        cause: Option<BoxError>,
    ) -> Self {
        let mut prefix = String::from("Type validation failed");
        if let Some(ctx) = &context {
            if let Some(field) = &ctx.field {
                prefix.push_str(&format!(" for {field}"));
            }
            if ctx.entity_name.is_some() || ctx.entity_id.is_some() {
                let mut parts = Vec::new();
                if let Some(name) = &ctx.entity_name {
                    parts.push(name.clone());
                }
                if let Some(id) = &ctx.entity_id {
                    parts.push(format!("id: \"{id}\""));
                }
                prefix.push_str(&format!(" ({})", parts.join(", ")));
            }
        }
        Self::TypeValidation {
            message: format!(
                "{prefix}: Value: {value}.\nError message: {}",
                error_message(cause.as_ref())
            ),
            value,
            context,
            cause,
        }
    }

    /// `UnsupportedFunctionalityError` with the default message.
    #[must_use]
    pub fn unsupported_functionality(functionality: impl Into<String>) -> Self {
        let functionality = functionality.into();
        Self::UnsupportedFunctionality {
            message: format!("'{functionality}' functionality not supported."),
            functionality,
        }
    }

    /// The upstream `name` (`AI_APICallError`, ...).
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::AiSdk { name, .. } => name,
            Self::ApiCall { .. } => "AI_APICallError",
            Self::EmptyResponseBody { .. } => "AI_EmptyResponseBodyError",
            Self::EvaluationUnsupportedQuestionType { .. } => {
                "AI_EvaluationUnsupportedQuestionTypeError"
            }
            Self::InvalidArgument { .. } => "AI_InvalidArgumentError",
            Self::InvalidPrompt { .. } => "AI_InvalidPromptError",
            Self::InvalidResponseData { .. } => "AI_InvalidResponseDataError",
            Self::JsonParse { .. } => "AI_JSONParseError",
            Self::LoadApiKey { .. } => "AI_LoadAPIKeyError",
            Self::LoadSetting { .. } => "AI_LoadSettingError",
            Self::NoContentGenerated { .. } => "AI_NoContentGeneratedError",
            Self::NoSuchModel { error_name, .. } => error_name,
            Self::NoSuchProviderReference { .. } => "AI_NoSuchProviderReferenceError",
            Self::TooManyEmbeddingValuesForCall { .. } => "AI_TooManyEmbeddingValuesForCallError",
            Self::TypeValidation { .. } => "AI_TypeValidationError",
            Self::UnsupportedFunctionality { .. } => "AI_UnsupportedFunctionalityError",
        }
    }

    /// The upstream `message`.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::AiSdk { message, .. }
            | Self::ApiCall { message, .. }
            | Self::EmptyResponseBody { message }
            | Self::EvaluationUnsupportedQuestionType { message, .. }
            | Self::InvalidArgument { message, .. }
            | Self::InvalidPrompt { message, .. }
            | Self::InvalidResponseData { message, .. }
            | Self::JsonParse { message, .. }
            | Self::LoadApiKey { message }
            | Self::LoadSetting { message }
            | Self::NoContentGenerated { message }
            | Self::NoSuchModel { message, .. }
            | Self::NoSuchProviderReference { message, .. }
            | Self::TooManyEmbeddingValuesForCall { message, .. }
            | Self::TypeValidation { message, .. }
            | Self::UnsupportedFunctionality { message, .. } => message,
        }
    }

    /// Upstream `APICallError.isRetryable`; no other class is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ApiCall {
                is_retryable: true,
                ..
            }
        )
    }
}

impl fmt::Display for ProviderError {
    /// `message`, as upstream `Error.message`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl StdError for ProviderError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::AiSdk { cause, .. }
            | Self::ApiCall { cause, .. }
            | Self::InvalidArgument { cause, .. }
            | Self::InvalidPrompt { cause, .. }
            | Self::JsonParse { cause, .. }
            | Self::TypeValidation { cause, .. } => {
                cause.as_deref().map(|c| c as &(dyn StdError + 'static))
            }
            _ => None,
        }
    }
}
