//! Shared transport and inline image handling for native decision adapters.
use std::collections::HashMap;

use aimux_core::AiMuxError;
use aimux_core::decision_model::{DecisionCallOptions, DecisionImage};
use aimux_core::shared::FileBytes;
use aimux_provider_utils::HttpRequest;
use base64::Engine;
use serde_json::Value;

use crate::openai::config::OpenAIModelConfig;
use crate::openai_compatible::config::CompatModelConfig;

/// Where a native decision adapter sends its requests: the connection data of
/// the provider it was taken from (RFC-0036). The endpoint and credential come
/// from the caller's provider configuration; nothing here reads the
/// environment or is written into recordings.
#[derive(Clone)]
pub(crate) enum DecisionEndpoint {
    /// Native OpenAI (`OpenAIProvider`).
    OpenAI(OpenAIModelConfig),
    /// An OpenAI-compatible runtime (presets and `create_openai_compatible`).
    Compatible(CompatModelConfig),
}

impl DecisionEndpoint {
    /// The provider base URL without a trailing slash.
    ///
    /// # Errors
    ///
    /// Returns the provider's setting error (an unresolved preset template).
    pub(crate) fn base_url(&self) -> Result<String, AiMuxError> {
        let base = match self {
            Self::OpenAI(config) => config.url("")?,
            Self::Compatible(config) => config.base_url.resolve()?,
        };
        Ok(base.trim_end_matches('/').to_string())
    }

    fn headers(&self) -> &crate::shared::ProviderHeaders {
        match self {
            Self::OpenAI(config) => &config.headers,
            Self::Compatible(config) => &config.headers,
        }
    }

    /// #226's decision connection snapshot: identity, base URL, credential
    /// *source* and the connection headers. The credential value is
    /// never read; sensitive headers are redacted again when recorded.
    pub(crate) fn snapshot(
        &self,
        provider: &str,
        model_id: &str,
        profile: Value,
    ) -> aimux_core::recording::ProviderRecord {
        use crate::shared::Credential;
        let headers = self.headers();
        let mut record = aimux_core::recording::ProviderRecord::from_model(provider, model_id);
        record.base_url = self.base_url().ok();
        record.api_key_source = Some(match &headers.credential {
            Credential::None => "none".into(),
            Credential::Explicit(_) => "explicit".into(),
            Credential::Env { var, .. } => format!("env:{var}"),
        });
        let mut wire_headers = serde_json::Map::new();
        for (name, value) in &headers.fixed {
            wire_headers.insert(name.clone(), Value::String(value.clone()));
        }
        for (name, value) in headers.user.iter().flatten() {
            if let Some(value) = value {
                wire_headers.insert(name.clone(), Value::String(value.clone()));
            }
        }
        record.provider_options = Some(aimux_core::recording::redact_provider_options(
            serde_json::json!({ "headers": wire_headers }),
        ));
        record.profile = Some(profile);
        record
    }

    async fn request_headers(
        &self,
        call_headers: Option<&HashMap<String, String>>,
    ) -> Result<Vec<(String, String)>, AiMuxError> {
        match self {
            Self::OpenAI(config) => config.request_headers(call_headers).await,
            Self::Compatible(config) => config.request_headers(call_headers).await,
        }
    }

    fn http_request(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
        options: &DecisionCallOptions,
    ) -> Result<HttpRequest, AiMuxError> {
        Ok(match self {
            Self::OpenAI(config) => config.http_request(url.to_string(), headers, options),
            Self::Compatible(config) => config.with_transport(
                HttpRequest::new(url, headers, options),
                config.base_url.resolve()?,
            ),
        })
    }
}

pub(crate) struct InlineImage {
    pub data_url: String,
    pub base64: String,
    pub decoded_len: usize,
}

pub(crate) fn inline_image(image: &DecisionImage) -> Result<InlineImage, AiMuxError> {
    if !image.media_type.starts_with("image/") || image.media_type.contains([';', ',']) {
        return Err(AiMuxError::InvalidArgument(
            "decision image requires an image MIME type".into(),
        ));
    }
    let engine = base64::engine::general_purpose::STANDARD;
    let (base64, decoded_len) = match &image.data {
        FileBytes::Binary(bytes) => (engine.encode(bytes), bytes.len()),
        FileBytes::Base64(data) => {
            let bytes = engine
                .decode(data)
                .map_err(|e| AiMuxError::InvalidArgument(format!("decision image base64: {e}")))?;
            (data.clone(), bytes.len())
        }
    };
    if decoded_len == 0 {
        return Err(AiMuxError::InvalidArgument(
            "decision image is empty".into(),
        ));
    }
    Ok(InlineImage {
        data_url: format!("data:{};base64,{base64}", image.media_type),
        base64,
        decoded_len,
    })
}

pub(crate) async fn post<T>(
    endpoint: &DecisionEndpoint,
    options: &DecisionCallOptions,
    url: &str,
    body: Value,
    convert: impl FnOnce(Value, HashMap<String, String>) -> Result<T, AiMuxError>,
) -> Result<T, AiMuxError> {
    let headers = endpoint.request_headers(options.headers.as_ref()).await?;
    let response = aimux_provider_utils::post_json_to_api(
        endpoint.http_request(url, headers, options)?,
        body.clone(),
        aimux_provider_utils::create_json_response_handler::<Value>(),
        aimux_provider_utils::create_json_error_response_handler(|data| {
            let error = data
                .get("error")
                .or_else(|| data.get("detail"))
                .or_else(|| {
                    data.get("errors")
                        .and_then(Value::as_array)
                        .and_then(|v| v.first())
                })
                .unwrap_or(data);
            aimux_provider_utils::ProviderErrorParts {
                message: error
                    .as_str()
                    .or_else(|| error.get("message").and_then(Value::as_str))
                    .unwrap_or_default()
                    .into(),
                provider_code: error
                    .get("code")
                    .or_else(|| error.get("type"))
                    .and_then(|code| match code {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    }),
            }
        }),
    )
    .await?;
    let raw = response.value;
    let headers = response.response_headers;
    convert(raw.clone(), headers.clone()).map_err(|error| {
        aimux_provider_utils::invalid_response_api_call(
            error.to_string(),
            200,
            url,
            body,
            raw,
            headers,
        )
    })
}

pub(crate) fn invalid(message: impl Into<String>) -> AiMuxError {
    AiMuxError::InvalidResponseData(message.into())
}

pub(crate) fn options(
    request: &DecisionCallOptions,
    provider: &str,
) -> Result<serde_json::Map<String, Value>, AiMuxError> {
    match request
        .provider_options
        .as_ref()
        .and_then(|options| options.get(provider))
    {
        None => Ok(serde_json::Map::new()),
        Some(options) => Ok(options.clone()),
    }
}
