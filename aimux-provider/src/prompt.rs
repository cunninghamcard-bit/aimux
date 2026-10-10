//! `language-model-v4-prompt.ts`: the V4 input layer.

use serde::{Deserialize, Serialize};

use crate::{JsonValue, SharedV4FileData, SharedV4InlineFileData, SharedV4ProviderOptions};

/// `LanguageModelV4Prompt`.
pub type LanguageModelV4Prompt = Vec<LanguageModelV4Message>;

/// `LanguageModelV4Message`, tagged by `role`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "role",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4Message {
    System {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    User {
        content: Vec<LanguageModelV4UserPart>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    Assistant {
        content: Vec<LanguageModelV4AssistantPart>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    Tool {
        content: Vec<LanguageModelV4ToolPart>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
}

/// `LanguageModelV4TextPart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4TextPart {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4FilePart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4FilePart {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub data: SharedV4FileData,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4ReasoningFilePart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ReasoningFilePart {
    pub data: SharedV4InlineFileData,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4CustomPart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4CustomPart {
    /// `{provider}.{kind}`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4ToolCallPart`. `input` is parsed JSON (input layer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolCallPart {
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: JsonValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_executed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4ToolResultPart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolResultPart {
    pub tool_call_id: String,
    pub tool_name: String,
    pub output: LanguageModelV4ToolResultOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `LanguageModelV4ToolApprovalResponsePart`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ToolApprovalResponsePart {
    pub approval_id: String,
    pub approved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// User message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4UserPart {
    Text(LanguageModelV4TextPart),
    File(LanguageModelV4FilePart),
}

/// Assistant message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4AssistantPart {
    Text(LanguageModelV4TextPart),
    File(LanguageModelV4FilePart),
    Custom(LanguageModelV4CustomPart),
    Reasoning(LanguageModelV4TextPart),
    ReasoningFile(LanguageModelV4ReasoningFilePart),
    ToolCall(LanguageModelV4ToolCallPart),
    ToolResult(LanguageModelV4ToolResultPart),
}

/// Tool message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4ToolPart {
    ToolResult(LanguageModelV4ToolResultPart),
    ToolApprovalResponse(LanguageModelV4ToolApprovalResponsePart),
}

/// `LanguageModelV4ToolResultOutput`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4ToolResultOutput {
    Text {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    Json {
        value: JsonValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    ExecutionDenied {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    ErrorText {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    ErrorJson {
        value: JsonValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    Content {
        value: Vec<LanguageModelV4ToolResultContentPart>,
    },
}

/// An element of the `content` tool-result output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4ToolResultContentPart {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    File {
        data: SharedV4FileData,
        media_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
    Custom {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_options: Option<SharedV4ProviderOptions>,
    },
}
