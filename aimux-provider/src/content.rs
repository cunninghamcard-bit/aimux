//! `language-model-v4-content.ts` and the generate result: the V4 output
//! layer.

use serde::{Deserialize, Serialize};

use crate::{
    JsonObject, JsonValue, SharedV4Headers, SharedV4InlineFileData, SharedV4ProviderMetadata,
    SharedV4Warning,
};

/// `LanguageModelV4Text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4Text {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4Reasoning`.
pub type LanguageModelV4ReasoningContent = LanguageModelV4Text;

/// `LanguageModelV4CustomContent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4CustomContent {
    /// `{provider}.{kind}`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4File` / `LanguageModelV4ReasoningFile`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4File {
    pub media_type: String,
    pub data: SharedV4InlineFileData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4ToolApprovalRequest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolApprovalRequest {
    pub approval_id: String,
    pub tool_call_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4Source`, tagged by `sourceType`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "sourceType",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4Source {
    Url {
        id: String,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
    Document {
        id: String,
        media_type: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_metadata: Option<SharedV4ProviderMetadata>,
    },
}

/// `LanguageModelV4ToolCall`. `input` is the provider's argument text,
/// unparsed (RFC-0042 §3.1/§3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_executed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4ToolResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    /// `NonNullable<JSONValue>`.
    pub result: JsonValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preliminary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
}

/// `LanguageModelV4Content`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4Content {
    Text(LanguageModelV4Text),
    Reasoning(LanguageModelV4ReasoningContent),
    Custom(LanguageModelV4CustomContent),
    ReasoningFile(LanguageModelV4File),
    File(LanguageModelV4File),
    ToolApprovalRequest(LanguageModelV4ToolApprovalRequest),
    Source(LanguageModelV4Source),
    ToolCall(LanguageModelV4ToolCall),
    ToolResult(LanguageModelV4ToolResult),
}

/// `LanguageModelV4FinishReason`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageModelV4FinishReason {
    pub unified: LanguageModelV4UnifiedFinishReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

/// `LanguageModelV4FinishReason['unified']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageModelV4UnifiedFinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
    Error,
    Other,
}

/// `LanguageModelV4Usage`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4Usage {
    pub input_tokens: LanguageModelV4InputTokens,
    pub output_tokens: LanguageModelV4OutputTokens,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<JsonObject>,
}

/// `LanguageModelV4Usage['inputTokens']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4InputTokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_cache: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u64>,
}

/// `LanguageModelV4Usage['outputTokens']`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4OutputTokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<u64>,
}

/// `LanguageModelV4ResponseMetadata`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ResponseMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// ISO 8601 (`Date` on the TS side).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

/// `LanguageModelV4GenerateResult['request']`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LanguageModelV4RequestInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<JsonValue>,
}

/// `LanguageModelV4GenerateResult['response']`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ResponseInfo {
    #[serde(flatten)]
    pub metadata: LanguageModelV4ResponseMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<SharedV4Headers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<JsonValue>,
}

/// `LanguageModelV4GenerateResult`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4GenerateResult {
    pub content: Vec<LanguageModelV4Content>,
    pub finish_reason: LanguageModelV4FinishReason,
    pub usage: LanguageModelV4Usage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<SharedV4ProviderMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<LanguageModelV4RequestInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<LanguageModelV4ResponseInfo>,
    pub warnings: Vec<SharedV4Warning>,
}
