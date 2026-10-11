//! `language-model-v4-stream-part.ts` and the stream result envelope.

use serde::{Deserialize, Serialize};

use crate::{
    JsonValue, LanguageModelV4CustomContent, LanguageModelV4File, LanguageModelV4FinishReason,
    LanguageModelV4ResponseMetadata, LanguageModelV4Source, LanguageModelV4ToolApprovalRequest,
    LanguageModelV4ToolCall, LanguageModelV4ToolResult, LanguageModelV4Usage, SharedV4Headers,
    SharedV4ProviderMetadata, SharedV4Warning,
};

/// `LanguageModelV4StreamPart`. Text and reasoning deltas use `delta` (the
/// core layer calls it `text`; RFC-0042 §3.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4StreamPart {
    TextStart {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    TextDelta {
        id: String,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    TextEnd {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ReasoningStart {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ReasoningDelta {
        id: String,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ReasoningEnd {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ToolInputStart {
        id: String,
        tool_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_executed: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dynamic: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    ToolInputDelta {
        id: String,
        delta: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ToolInputEnd {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    ToolApprovalRequest(LanguageModelV4ToolApprovalRequest),
    ToolCall(LanguageModelV4ToolCall),
    ToolResult(LanguageModelV4ToolResult),
    Custom(LanguageModelV4CustomContent),
    File(LanguageModelV4File),
    ReasoningFile(LanguageModelV4File),
    Source(LanguageModelV4Source),
    StreamStart {
        warnings: Vec<SharedV4Warning>,
    },
    ResponseMetadata(LanguageModelV4ResponseMetadata),
    Finish {
        usage: LanguageModelV4Usage,
        finish_reason: LanguageModelV4FinishReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    Raw {
        raw_value: JsonValue,
    },
    Error {
        error: JsonValue,
    },
}

/// `LanguageModelV4StreamResult` without the stream itself.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageModelV4StreamResultInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<crate::LanguageModelV4RequestInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<LanguageModelV4StreamResponseInfo>,
}

/// `LanguageModelV4StreamResult['response']`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LanguageModelV4StreamResponseInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<SharedV4Headers>,
}
