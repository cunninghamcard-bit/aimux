//! #226's by-name provider handle for native OpenAI and the official local
//! decision runtimes, kept with #226's resolution rules (explicit argument,
//! then the runtime's `*_BASE_URL` / `*_API_KEY` environment variables, then
//! the default local endpoint) on top of the package factories.
//!
//! Other names resolve through [`create_provider`](crate::create_provider).

use std::collections::HashMap;
use std::sync::Arc;

use aimux_core::decision_model::{
    DecisionCallOptions, DecisionCapabilities, DecisionModel, DecisionResult,
};
use aimux_core::provider::ProviderDiscovery;
use aimux_core::retry::RetryConfig;
use aimux_core::{
    AiMuxError, EmbeddingModel, EvaluationModel, Files, ImageModel, LanguageModel, Provider,
    RerankingModel, SearchModel, Skills, SpeechModel, TranscriptionModel, VideoModel,
};
use aimux_provider_utils::HeaderMapOpt;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::openai::OpenAIProviderSettings;
use crate::openai_compatible::{OpenAICompatibleProviderSettings, create_openai_compatible};

/// Per-handle options of [`provider_handle`] (#226).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderOptions {
    /// Override the default or environment base URL.
    pub base_url: Option<String>,
    /// Extra headers merged into every request.
    pub headers: Option<HashMap<String, String>>,
    /// OpenAI organization ID (`OpenAI-Organization` header).
    pub organization: Option<String>,
    /// OpenAI project ID (`OpenAI-Project` header).
    pub project: Option<String>,
    /// Decision retry-count default; a per-call `max_retries` still wins.
    pub max_retries: Option<u32>,
    /// Generation request-body overrides. Native decisions refuse a handle
    /// that carries them (#226).
    pub body_overrides: Option<Value>,
}

/// Official local decision runtimes: name, default base URL, base URL env.
const LOCAL_RUNTIMES: &[(&str, &str, &str)] = &[
    ("ollama", "http://127.0.0.1:11434/v1", "OLLAMA_BASE_URL"),
    ("llamacpp", "http://127.0.0.1:8080/v1", "LLAMACPP_BASE_URL"),
    ("localai", "http://127.0.0.1:8080/v1", "LOCALAI_BASE_URL"),
    ("vllm", "http://127.0.0.1:8000/v1", "VLLM_BASE_URL"),
    ("sglang", "http://127.0.0.1:30000/v1", "SGLANG_BASE_URL"),
    ("laya", "http://127.0.0.1:8000/v1", "LAYA_BASE_URL"),
];

/// #226's key for a local runtime started without one.
const PLACEHOLDER_API_KEY: &str = "not-needed";

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Build a provider handle by name (#226): `"openai"`, the local runtimes
/// (`ollama`, `llamacpp`, `localai`, `vllm`, `sglang`, `laya`) or any other
/// built-in package / registry preset.
///
/// - `openai`: `api_key = None` reads `OPENAI_API_KEY` now.
/// - local runtimes: `api_key = None` reads `{NAME}_API_KEY` if set, otherwise
///   sends the placeholder key `not-needed`; the base URL is `options.base_url`, else `{NAME}_BASE_URL`,
///   else the runtime's default local endpoint.
///
/// # Errors
///
/// `LoadApiKey` for a missing OpenAI key, `NoSuchProvider` for an unknown
/// name, and the factory's settings errors.
pub fn provider_handle(
    name: &str,
    api_key: Option<String>,
    options: Option<ProviderOptions>,
) -> Result<Box<dyn Provider>, AiMuxError> {
    let options = options.unwrap_or_default();
    let mut headers: HeaderMapOpt = options
        .headers
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, value)| (name, Some(value)))
        .collect();
    let mut api_key_source = None;
    let inner: Arc<dyn Provider> = if name == "openai" {
        let api_key =
            aimux_provider_utils::load_api_key(api_key.as_deref(), "OPENAI_API_KEY", "OpenAI")?;
        Arc::new(crate::create_openai(OpenAIProviderSettings {
            base_url: options.base_url.clone(),
            api_key: Some(api_key),
            organization: options.organization.clone(),
            project: options.project.clone(),
            headers: Some(headers),
            ..Default::default()
        })?)
    } else {
        if let Some(organization) = &options.organization {
            headers.insert("OpenAI-Organization".into(), Some(organization.clone()));
        }
        if let Some(project) = &options.project {
            headers.insert("OpenAI-Project".into(), Some(project.clone()));
        }
        if let Some((_, default_url, url_env)) =
            LOCAL_RUNTIMES.iter().find(|(runtime, ..)| *runtime == name)
        {
            let base_url = options
                .base_url
                .clone()
                .or_else(|| non_empty_env(url_env))
                .unwrap_or_else(|| (*default_url).into());
            // #226: explicit key, else `{NAME}_API_KEY`, else the placeholder
            // `not-needed` (sent as `Authorization: Bearer not-needed`),
            // recorded with source `none`.
            let key_env = format!("{}_API_KEY", name.to_uppercase());
            let api_key = match (api_key, non_empty_env(&key_env)) {
                (Some(key), _) => key,
                (None, Some(key)) => {
                    api_key_source = Some(format!("env:{key_env}"));
                    key
                }
                (None, None) => {
                    api_key_source = Some("none".into());
                    PLACEHOLDER_API_KEY.into()
                }
            };
            Arc::new(create_openai_compatible(
                OpenAICompatibleProviderSettings {
                    name: name.into(),
                    base_url,
                    api_key: Some(api_key),
                    headers: Some(headers),
                    ..Default::default()
                },
            )?)
        } else {
            crate::create_provider(
                name,
                crate::preset::PresetSettings {
                    api_key: api_key.map(Into::into),
                    base_url: options.base_url.clone(),
                    headers: Some(headers),
                    ..Default::default()
                },
            )?
        }
    };
    Ok(Box::new(ProviderHandle {
        inner,
        max_retries: options.max_retries,
        has_body_overrides: options.body_overrides.is_some(),
        api_key_source,
    }))
}

/// A provider with #226's handle-level decision settings.
struct ProviderHandle {
    inner: Arc<dyn Provider>,
    max_retries: Option<u32>,
    has_body_overrides: bool,
    /// Credential source when the handle resolved the key itself (the
    /// placeholder or an environment variable); `None` keeps the model's own.
    api_key_source: Option<String>,
}

impl ProviderHandle {
    fn generation_overrides_unsupported(&self) -> Result<(), AiMuxError> {
        if self.has_body_overrides {
            return Err(AiMuxError::UnsupportedFunctionality(
                "body_overrides are not supported by the package model factories".into(),
            ));
        }
        Ok(())
    }
}

impl Provider for ProviderHandle {
    fn decision_model(&self, id: &str) -> Result<Box<dyn DecisionModel>, AiMuxError> {
        if self.has_body_overrides {
            return Err(AiMuxError::UnsupportedFunctionality(
                "native decisions require an official adapter without generation body_overrides"
                    .into(),
            ));
        }
        let model = self.inner.decision_model(id)?;
        if self.max_retries.is_none() && self.api_key_source.is_none() {
            return Ok(model);
        }
        let retry_config = RetryConfig {
            max_retries: self.max_retries.unwrap_or(model.retry_config().max_retries),
            ..model.retry_config()
        };
        Ok(Box::new(HandleDecisionModel {
            inner: model,
            retry_config,
            api_key_source: self.api_key_source.clone(),
        }))
    }
    fn language_model(&self, id: &str) -> Result<Arc<dyn LanguageModel>, AiMuxError> {
        self.generation_overrides_unsupported()?;
        self.inner.language_model(id)
    }
    fn embedding_model(&self, id: &str) -> Result<Arc<dyn EmbeddingModel>, AiMuxError> {
        self.generation_overrides_unsupported()?;
        self.inner.embedding_model(id)
    }
    fn image_model(&self, id: &str) -> Result<Arc<dyn ImageModel>, AiMuxError> {
        self.generation_overrides_unsupported()?;
        self.inner.image_model(id)
    }
    fn transcription_model(
        &self,
        id: &str,
    ) -> Option<Result<Arc<dyn TranscriptionModel>, AiMuxError>> {
        self.inner.transcription_model(id)
    }
    fn speech_model(&self, id: &str) -> Option<Result<Arc<dyn SpeechModel>, AiMuxError>> {
        self.inner.speech_model(id)
    }
    fn reranking_model(&self, id: &str) -> Option<Result<Arc<dyn RerankingModel>, AiMuxError>> {
        self.inner.reranking_model(id)
    }
    fn evaluation_model(&self, id: &str) -> Option<Result<Arc<dyn EvaluationModel>, AiMuxError>> {
        self.inner.evaluation_model(id)
    }
    fn files(&self) -> Option<Arc<dyn Files>> {
        self.inner.files()
    }
    fn skills(&self) -> Option<Arc<dyn Skills>> {
        self.inner.skills()
    }
    fn video_model(&self, id: &str) -> Option<Result<Arc<dyn VideoModel>, AiMuxError>> {
        self.inner.video_model(id)
    }
    fn search_model(&self, id: &str) -> Option<Result<Arc<dyn SearchModel>, AiMuxError>> {
        self.inner.search_model(id)
    }
    fn discovery(&self) -> Option<&dyn ProviderDiscovery> {
        self.inner.discovery()
    }
}

/// A decision model with handle-level retry settings and credential source.
struct HandleDecisionModel {
    inner: Box<dyn DecisionModel>,
    retry_config: RetryConfig,
    api_key_source: Option<String>,
}

#[async_trait]
impl DecisionModel for HandleDecisionModel {
    fn provider(&self) -> &str {
        self.inner.provider()
    }
    fn model_id(&self) -> &str {
        self.inner.model_id()
    }
    fn capabilities(&self) -> DecisionCapabilities {
        self.inner.capabilities()
    }
    fn retry_config(&self) -> RetryConfig {
        self.retry_config
    }
    fn config_snapshot(&self) -> aimux_core::recording::ProviderRecord {
        let mut snapshot = self.inner.config_snapshot();
        if let Some(source) = &self.api_key_source {
            snapshot.api_key_source = Some(source.clone());
        }
        if let Some(serde_json::Value::Object(options)) = &mut snapshot.provider_options {
            options.insert("max_retries".into(), self.retry_config.max_retries.into());
        }
        snapshot
    }
    async fn do_decide(&self, options: &DecisionCallOptions) -> Result<DecisionResult, AiMuxError> {
        self.inner.do_decide(options).await
    }
}
