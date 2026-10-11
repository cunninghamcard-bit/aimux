//! #226's by-name provider handle for native OpenAI and the official local
//! decision runtimes on top of the package factories. A key is only the
//! explicit argument or a package's own default env (`OPENAI_API_KEY`, read
//! at request time); there is no generic `<NAME>_API_KEY` lookup (A6 item 4).
//! A local runtime's base URL is the argument, then `*_BASE_URL`, then the
//! default local endpoint.
//!
//! Other names resolve through [`create_provider`](crate::create_provider).

use std::collections::HashMap;
use std::sync::Arc;

use aimux_core::decision_model::{
    DecisionCallOptions, DecisionCapabilities, DecisionModel, DecisionResult,
};
use aimux_core::provider::ProviderDiscovery;
use aimux_core::{
    AiMuxError, EmbeddingModel, EvaluationModel, Files, ImageModel, LanguageModel, Provider,
    RerankingModel, SearchModel, Skills, SpeechModel, TranscriptionModel, VideoModel,
};
use aimux_provider_utils::HeaderMapOpt;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::openai::OpenAIProviderSettings;
use crate::openai_compatible::{OpenAICompatibleProviderSettings, create_openai_compatible};

/// Per-handle options of [`provider_handle`] (#226).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderOptions {
    /// Override the default or environment base URL.
    pub base_url: Option<String>,
    /// Extra headers merged into every request.
    pub headers: Option<HashMap<String, String>>,
    /// OpenAI organization ID (`OpenAI-Organization` header).
    pub organization: Option<String>,
    /// OpenAI project ID (`OpenAI-Project` header).
    pub project: Option<String>,
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
/// - `openai`: `api_key = None` reads the package default `OPENAI_API_KEY` at
///   request time (#200:538); `Some("")` is used as given and never falls
///   back (RFC-0040:67). With `options.base_url` set and no key, no env is
///   read and no credential is sent.
/// - local runtimes: `api_key = None` sends the placeholder key `not-needed`;
///   no `{NAME}_API_KEY` is read. The base URL is `options.base_url`, else
///   `{NAME}_BASE_URL`, else the runtime's default local endpoint.
///
/// # Errors
///
/// `NoSuchProvider` for an unknown name, and the factory's settings errors.
/// A missing OpenAI key fails the request with `LoadApiKey`.
pub fn provider_handle(
    name: &str,
    api_key: Option<String>,
    options: Option<ProviderOptions>,
) -> Result<Box<dyn Provider>, AiMuxError> {
    provider_handle_with(name, api_key, options, Handles::default())
}

/// What [`provider_handle`] uses from its surroundings: the env source of a
/// package-default key and the transport. Only this crate's tests set them.
#[derive(Default)]
struct Handles {
    env: crate::shared::EnvSource,
    fetch: Option<aimux_provider_utils::FetchFunction>,
}

fn provider_handle_with(
    name: &str,
    api_key: Option<String>,
    options: Option<ProviderOptions>,
    handles: Handles,
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
        let auth_none = api_key.is_none() && options.base_url.is_some();
        let provider = crate::create_openai(OpenAIProviderSettings {
            base_url: options.base_url.clone(),
            api_key,
            organization: options.organization.clone(),
            project: options.project.clone(),
            headers: Some(headers),
            fetch: handles.fetch,
            ..Default::default()
        })?;
        #[cfg(test)]
        let provider = provider.with_env_source(handles.env);
        #[cfg(not(test))]
        let _ = handles.env;
        Arc::new(if auth_none {
            provider.without_credential()
        } else {
            provider
        })
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
            // Explicit key, else the placeholder `not-needed` (sent as
            // `Authorization: Bearer not-needed`), recorded with source
            // `none`. No `{NAME}_API_KEY` is read.
            let api_key = api_key.unwrap_or_else(|| {
                api_key_source = Some("none".into());
                PLACEHOLDER_API_KEY.into()
            });
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
        api_key_source,
    }))
}

/// A provider with #226's handle-level decision settings.
struct ProviderHandle {
    inner: Arc<dyn Provider>,
    /// Credential source when the handle resolved the key itself (the
    /// placeholder or an environment variable); `None` keeps the model's own.
    api_key_source: Option<String>,
}

impl Provider for ProviderHandle {
    fn decision_model(&self, id: &str) -> Result<Box<dyn DecisionModel>, AiMuxError> {
        let model = self.inner.decision_model(id)?;
        if self.api_key_source.is_none() {
            return Ok(model);
        }
        Ok(Box::new(HandleDecisionModel {
            inner: model,
            api_key_source: self.api_key_source.clone(),
        }))
    }
    fn language_model(&self, id: &str) -> Result<Arc<dyn LanguageModel>, AiMuxError> {
        self.inner.language_model(id)
    }
    fn embedding_model(&self, id: &str) -> Result<Arc<dyn EmbeddingModel>, AiMuxError> {
        self.inner.embedding_model(id)
    }
    fn image_model(&self, id: &str) -> Result<Arc<dyn ImageModel>, AiMuxError> {
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

/// A decision model with the handle's credential source.
struct HandleDecisionModel {
    inner: Box<dyn DecisionModel>,
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
    fn config_snapshot(&self) -> aimux_core::recording::ProviderRecord {
        let mut snapshot = self.inner.config_snapshot();
        if let Some(source) = &self.api_key_source {
            snapshot.api_key_source = Some(source.clone());
        }
        snapshot
    }
    async fn do_decide(&self, options: &DecisionCallOptions) -> Result<DecisionResult, AiMuxError> {
        self.inner.do_decide(options).await
    }
}

#[cfg(test)]
mod env_tests {
    //! A6 item 4 / D23. The env a key could come from is an injected plain
    //! map ([`EnvSource::Vars`]) that logs every read, never the process
    //! environment and never `std::env::set_var`.
    use std::sync::{Arc, Mutex};

    use aimux_core::decision_model::decide;
    use aimux_core::recording::{self, RingRecorder};
    use aimux_core::replay::MockDecisionReplayModel;
    use aimux_provider_utils::{
        Fetch, FetchError, FetchFunction, FetchRequest, FetchResponse, ReqwestFetch,
    };
    use serde_json::Value;
    use serial_test::serial;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::shared::{EnvSource, TestEnv};

    fn env(vars: &[(&str, &str)]) -> Arc<Mutex<TestEnv>> {
        Arc::new(Mutex::new(TestEnv {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            reads: Vec::new(),
        }))
    }

    fn reads(env: &Arc<Mutex<TestEnv>>) -> Vec<String> {
        env.lock().unwrap().reads.clone()
    }

    /// Sends every request to `target`'s origin, keeping path and headers, so
    /// the OpenAI handle keeps its default base URL.
    struct Redirect(String);

    #[async_trait]
    impl Fetch for Redirect {
        async fn fetch(&self, mut request: FetchRequest) -> Result<FetchResponse, FetchError> {
            let target = url::Url::parse(&self.0).unwrap();
            request.url.set_scheme(target.scheme()).unwrap();
            request.url.set_host(target.host_str()).unwrap();
            request.url.set_port(target.port()).unwrap();
            ReqwestFetch.fetch(request).await
        }
    }

    fn fixture(name: &str) -> Value {
        let text = match name {
            "openai" => include_str!("../tests/fixtures/openai_decisions.json"),
            _ => include_str!("../tests/fixtures/runtime_decisions.json"),
        };
        serde_json::from_str(text).unwrap()
    }

    fn options(name: &str) -> DecisionCallOptions {
        let mut options: DecisionCallOptions =
            serde_json::from_value(fixture(name)["options"].clone()).unwrap();
        options.max_retries = Some(0);
        options
    }

    async fn mock(route: &str, name: &str) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_json(&fixture(name)["response"]))
            .mount(&server)
            .await;
        server
    }

    async fn authorization(server: &MockServer) -> Vec<Option<String>> {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| {
                request
                    .headers
                    .get("authorization")
                    .map(|v| v.to_str().unwrap().to_string())
            })
            .collect()
    }

    fn openai(
        api_key: Option<&str>,
        options: Option<ProviderOptions>,
        env: &Arc<Mutex<TestEnv>>,
        server: &MockServer,
    ) -> Box<dyn Provider> {
        let fetch: FetchFunction = Arc::new(Redirect(server.uri()));
        provider_handle_with(
            "openai",
            api_key.map(Into::into),
            options,
            Handles {
                env: EnvSource::Vars(env.clone()),
                fetch: Some(fetch),
            },
        )
        .unwrap()
    }

    /// (1) The generic `<NAME>_API_KEY` is gone: a local runtime without a key
    /// never looks one up, so `OLLAMA_API_KEY` / `VLLM_API_KEY` never reach
    /// the wire.
    #[tokio::test]
    #[serial]
    async fn generic_provider_api_key_env_is_not_read() {
        for name in ["ollama", "vllm"] {
            let server = mock("/v1/systemone", "runtime").await;
            let env = env(&[("OLLAMA_API_KEY", "x-ollama"), ("VLLM_API_KEY", "x-vllm")]);
            let handle = provider_handle_with(
                name,
                None,
                Some(ProviderOptions {
                    base_url: Some(format!("{}/v1", server.uri())),
                    ..Default::default()
                }),
                Handles {
                    env: EnvSource::Vars(env.clone()),
                    fetch: None,
                },
            )
            .unwrap();
            decide(
                handle.decision_model("served").unwrap().as_ref(),
                options("runtime"),
            )
            .await
            .unwrap();
            let sent = authorization(&server).await;
            assert_eq!(sent.len(), 1);
            let sent = sent[0].clone().unwrap_or_default();
            assert!(
                !sent.contains("x-ollama") && !sent.contains("x-vllm"),
                "{sent}"
            );
            assert!(reads(&env).is_empty(), "{name}: {:?}", reads(&env));
        }
    }

    /// (2) The OpenAI package default env is read at request time: the key is
    /// put into the env only after the handle and the model exist.
    #[tokio::test]
    #[serial]
    async fn openai_default_env_is_read_at_request_time() {
        let server = mock("/v1/decisions", "openai").await;
        let env = env(&[]);
        let handle = openai(None, None, &env, &server);
        let model = handle.decision_model("gpt-6-luna").unwrap();
        assert!(reads(&env).is_empty(), "read at construction");
        env.lock()
            .unwrap()
            .vars
            .insert("OPENAI_API_KEY".into(), "sk-set-after-construction".into());
        decide(model.as_ref(), options("openai")).await.unwrap();
        assert_eq!(
            authorization(&server).await,
            vec![Some("Bearer sk-set-after-construction".into())]
        );
        assert_eq!(reads(&env), vec!["OPENAI_API_KEY".to_string()]);
    }

    /// Unset default env fails the request (not the construction) with
    /// `LoadApiKey`, and nothing is sent.
    #[tokio::test]
    #[serial]
    async fn openai_missing_default_env_fails_the_request() {
        let server = mock("/v1/decisions", "openai").await;
        let env = env(&[]);
        let handle = openai(None, None, &env, &server);
        let model = handle.decision_model("gpt-6-luna").unwrap();
        let error = decide(model.as_ref(), options("openai")).await.unwrap_err();
        assert!(matches!(error, AiMuxError::LoadApiKey { .. }), "{error:?}");
        assert!(authorization(&server).await.is_empty());
    }

    /// (3) `Some("")` is used as given and never falls back to the env.
    #[tokio::test]
    #[serial]
    async fn explicit_empty_key_does_not_fall_back_to_env() {
        let server = mock("/v1/decisions", "openai").await;
        let env = env(&[("OPENAI_API_KEY", "sk-from-env")]);
        let handle = openai(Some(""), None, &env, &server);
        decide(
            handle.decision_model("gpt-6-luna").unwrap().as_ref(),
            options("openai"),
        )
        .await
        .unwrap();
        let sent = authorization(&server).await;
        assert_eq!(sent.len(), 1);
        assert!(
            !sent[0].clone().unwrap_or_default().contains("sk-from-env"),
            "{sent:?}"
        );
        assert!(reads(&env).is_empty(), "{:?}", reads(&env));
    }

    /// (4a) A changed base URL without a key reads no env and sends no
    /// credential.
    #[tokio::test]
    #[serial]
    async fn changed_base_url_does_not_read_env() {
        let server = mock("/v1/decisions", "openai").await;
        let env = env(&[("OPENAI_API_KEY", "sk-from-env")]);
        let handle = openai(
            None,
            Some(ProviderOptions {
                base_url: Some(format!("{}/v1", server.uri())),
                ..Default::default()
            }),
            &env,
            &server,
        );
        decide(
            handle.decision_model("gpt-6-luna").unwrap().as_ref(),
            options("openai"),
        )
        .await
        .unwrap();
        assert_eq!(authorization(&server).await, vec![None]);
        assert!(reads(&env).is_empty(), "{:?}", reads(&env));
    }

    /// (4b) `auth = none` (a keyless local runtime) reads no env.
    #[tokio::test]
    #[serial]
    async fn auth_none_does_not_read_env() {
        let server = mock("/v1/systemone", "runtime").await;
        let env = env(&[("OPENAI_API_KEY", "sk-from-env"), ("LAYA_API_KEY", "x")]);
        let handle = provider_handle_with(
            "laya",
            None,
            Some(ProviderOptions {
                base_url: Some(format!("{}/v1", server.uri())),
                ..Default::default()
            }),
            Handles {
                env: EnvSource::Vars(env.clone()),
                fetch: None,
            },
        )
        .unwrap();
        decide(
            handle.decision_model("served").unwrap().as_ref(),
            options("runtime"),
        )
        .await
        .unwrap();
        assert!(reads(&env).is_empty(), "{:?}", reads(&env));
    }

    /// (4c) Offline replay of a recorded decision reads no env and makes no
    /// request, even though the live handle reads its key from the env.
    #[tokio::test]
    #[serial]
    async fn offline_replay_does_not_read_env() {
        let server = mock("/v1/decisions", "openai").await;
        let env = env(&[("OPENAI_API_KEY", "sk-from-env")]);
        let handle = openai(None, None, &env, &server);
        let recorder = Arc::new(RingRecorder::new());
        recording::init_recording(Some(recorder.clone()));
        let live = decide(
            handle.decision_model("gpt-6-luna").unwrap().as_ref(),
            options("openai"),
        )
        .await;
        recording::init_recording(None);
        live.unwrap();
        let records = recorder.completed();
        assert_eq!(records.len(), 1);
        env.lock().unwrap().reads.clear();
        let replay = MockDecisionReplayModel::new("openai", "gpt-6-luna", records).unwrap();
        decide(&replay, options("openai")).await.unwrap();
        assert!(reads(&env).is_empty(), "{:?}", reads(&env));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}
