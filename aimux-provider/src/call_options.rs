//! `language-model-v4-call-options.ts`, tool definitions and tool choice.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{JsonObject, JsonValue, LanguageModelV4Prompt, SharedV4ProviderOptions};

/// `LanguageModelV4CallOptions` without `abortSignal` (behaviour, not data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4CallOptions {
    pub prompt: LanguageModelV4Prompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<LanguageModelV4ResponseFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<LanguageModelV4Tool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<LanguageModelV4ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_raw_chunks: Option<bool>,
    /// A `None` value removes the header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, Option<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<LanguageModelV4Reasoning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// `responseFormat`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4ResponseFormat {
    Text,
    Json {
        /// JSON Schema 7.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<JsonValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

/// `reasoning`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageModelV4Reasoning {
    ProviderDefault,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

/// `LanguageModelV4FunctionTool | LanguageModelV4ProviderTool`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LanguageModelV4Tool {
    Function(LanguageModelV4FunctionTool),
    Provider(LanguageModelV4ProviderTool),
}

/// `LanguageModelV4FunctionTool`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4FunctionTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema 7.
    pub input_schema: JsonValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_examples: Option<Vec<LanguageModelV4InputExample>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedV4ProviderOptions>,
}

/// An `inputExamples` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageModelV4InputExample {
    pub input: JsonObject,
}

/// `LanguageModelV4ProviderTool`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageModelV4ProviderTool {
    /// `{provider}.{tool}`.
    pub id: String,
    pub name: String,
    pub args: JsonObject,
}

/// `LanguageModelV4ToolChoice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum LanguageModelV4ToolChoice {
    Auto,
    None,
    Required,
    Tool { tool_name: String },
}
