//! Tool / function-calling types.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::error::AiMuxError;
use crate::shared::SharedProviderOptions;
use crate::types::ProviderMetadata;

/// A tool definition passed to the model in `CallOptions.tools`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FunctionTool {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema describing the tool's parameters.
    pub input_schema: Value,
    /// Whether the tool supports strict schema enforcement (OpenAI strict mode).
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// Provider-specific options, keyed by provider name
    /// (e.g. `{"anthropic": {"eagerInputStreaming": true}}`). Aligned with the
    /// V4 `providerOptions` field on function tools.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_options: Option<SharedProviderOptions>,
    /// Example inputs for the tool (V4 `inputExamples`), used by some providers
    /// to emit `input_examples` in the request body.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_examples: Option<Vec<FunctionToolInputExample>>,
}

/// An example input for a function tool.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FunctionToolInputExample {
    pub input: Map<String, Value>,
}

impl FunctionTool {
    /// Create a `FunctionTool` with the required fields and all optional fields
    /// set to `None`. Combined with struct-update syntax this avoids spelling
    /// out every `None` field in tests.
    pub fn new(name: impl Into<String>, input_schema: Value) -> Self {
        FunctionTool {
            name: name.into(),
            description: None,
            input_schema,
            strict: None,
            provider_options: None,
            input_examples: None,
        }
    }

    /// Builder-style setter for `description`.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builder-style setter for `strict`.
    #[must_use]
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = Some(strict);
        self
    }
}

/// A provider-defined tool (e.g. `anthropic.web_search_20250305`,
/// `google.googleSearch`). Mirrors the V4 `LanguageModelV4ProviderTool`.
///
/// Provider tools are specific to a certain provider. The input and output
/// schemas are defined by the provider, and some are executed on the provider's
/// servers (e.g. web search, code execution).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderTool {
    /// The ID of the tool, following the format `<provider-id>.<unique-tool-name>`.
    pub id: String,
    /// The name of the tool, unique within this model call.
    pub name: String,
    /// The arguments for configuring the tool. Must match the expected
    /// arguments defined by the provider for this tool.
    pub args: Map<String, Value>,
}

/// A tool that can be either a user-defined function tool or a
/// provider-defined tool. Mirrors the V4
/// `Array<LanguageModelV4FunctionTool | LanguageModelV4ProviderTool>`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Tool {
    /// A user-defined function tool.
    Function(FunctionTool),
    /// A provider-defined tool.
    Provider(ProviderTool),
}

impl From<FunctionTool> for Tool {
    fn from(ft: FunctionTool) -> Self {
        Tool::Function(ft)
    }
}

/// A provider's raw tool-argument text, kept verbatim (RFC-0042:80).
///
/// Serializes as the plain JSON string (`serde(transparent)`), so the wire and
/// the generated TypeScript (`string`) are unchanged. Core parses it into
/// [`ToolCall::input`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawArguments(String);

impl RawArguments {
    /// Wrap provider argument text as-is.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The argument text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Append streamed argument text (provider deltas).
    pub fn push_str(&mut self, delta: &str) {
        self.0.push_str(delta);
    }

    /// The argument text, by value.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for RawArguments {
    fn from(text: String) -> Self {
        Self(text)
    }
}

impl From<&str> for RawArguments {
    fn from(text: &str) -> Self {
        Self(text.to_owned())
    }
}

impl From<RawArguments> for String {
    fn from(arguments: RawArguments) -> Self {
        arguments.0
    }
}

impl std::ops::Deref for RawArguments {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RawArguments {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RawArguments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for RawArguments {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for RawArguments {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for RawArguments {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

/// Same as `String == Value`: equal to a JSON string with the same text.
impl PartialEq<Value> for RawArguments {
    fn eq(&self, other: &Value) -> bool {
        other.as_str() == Some(self.0.as_str())
    }
}

/// Provider-facing tool call before Core parses and validates its input.
///
/// The wire shape matches [`ToolCall`] field for field, except that `input`
/// is the provider's raw argument *text* rather than a parsed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RawToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    #[ts(as = "String")]
    pub input: RawArguments,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_executed: Option<bool>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<ProviderMetadata>,
}

// A7 (D2): static calls are tools declared up front; dynamic calls are tools
// defined at runtime (e.g. MCP). Only a dynamic call can be invalid, and an
// invalid call always carries its error (D26). The JSON shape is unchanged and
// follows the AI SDK canonical form (D25): a static call writes no `dynamic`
// (an input `"dynamic": false` is dropped), a dynamic call writes
// `"dynamic": true`, `invalid` is only written as `true` with its `error`, and
// `providerExecuted` is passed through as given. (Plain comments: the `///`
// line is the generated `ToolCall.ts` doc and must stay unchanged.)
/// A tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(try_from = "ToolCallWire", into = "ToolCallWire")]
#[ts(export, as = "ToolCallWire")]
pub enum ToolCall {
    /// A call to a tool declared in the request.
    Static(StaticToolCall),
    /// A call to a tool defined at runtime.
    Dynamic(DynamicToolCall),
}

/// A call to a statically declared tool. Always valid: a static call that
/// fails parsing or validation is reported as a dynamic invalid call.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticToolCall {
    /// Provider-assigned call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Parsed arguments.
    pub input: Value,
    /// Whether the tool call will be executed by the provider (passed through).
    pub provider_executed: Option<bool>,
    /// Additional provider-specific metadata associated with this call.
    pub provider_metadata: Option<ProviderMetadata>,
}

/// A call to a tool defined at runtime (e.g. MCP tools).
#[derive(Debug, Clone)]
pub struct DynamicToolCall {
    /// Provider-assigned call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Parsed arguments, or the original string when the call is invalid and
    /// the provider input was not valid JSON.
    pub input: Value,
    /// Whether the tool call will be executed by the provider (passed through).
    pub provider_executed: Option<bool>,
    /// Additional provider-specific metadata associated with this call.
    pub provider_metadata: Option<ProviderMetadata>,
    /// Whether lookup, parsing and validation succeeded.
    pub validity: ToolCallValidity,
}

/// Outcome of resolving a dynamic tool call.
#[derive(Debug, Clone)]
pub enum ToolCallValidity {
    /// The call can be executed.
    Valid,
    /// Lookup, JSON parsing, or schema validation still failed after the
    /// optional repair attempt.
    Invalid {
        /// Typed failure associated with the invalid call.
        error: AiMuxError,
    },
}

impl TryFrom<ToolCallWire> for ToolCall {
    type Error = String;

    fn try_from(wire: ToolCallWire) -> Result<Self, String> {
        let invalid = wire.invalid == Some(true);
        if wire.dynamic != Some(true) {
            if invalid {
                // D26: only a dynamic call can be invalid (#200:2290).
                return Err("tool call: `invalid: true` requires `dynamic: true`".into());
            }
            return Ok(Self::Static(StaticToolCall {
                tool_call_id: wire.tool_call_id,
                tool_name: wire.tool_name,
                input: wire.input,
                provider_executed: wire.provider_executed,
                provider_metadata: wire.provider_metadata,
            }));
        }
        let validity = match (invalid, wire.error) {
            (true, Some(error)) => ToolCallValidity::Invalid { error },
            // D26: an invalid call always carries its error.
            (true, None) => return Err("tool call: `invalid: true` requires `error`".into()),
            (false, _) => ToolCallValidity::Valid,
        };
        Ok(Self::Dynamic(DynamicToolCall {
            tool_call_id: wire.tool_call_id,
            tool_name: wire.tool_name,
            input: wire.input,
            provider_executed: wire.provider_executed,
            provider_metadata: wire.provider_metadata,
            validity,
        }))
    }
}

impl From<ToolCall> for ToolCallWire {
    fn from(call: ToolCall) -> Self {
        match call {
            ToolCall::Static(call) => Self {
                tool_call_id: call.tool_call_id,
                tool_name: call.tool_name,
                input: call.input,
                provider_executed: call.provider_executed,
                dynamic: None,
                provider_metadata: call.provider_metadata,
                invalid: None,
                error: None,
            },
            ToolCall::Dynamic(call) => {
                let (invalid, error) = match call.validity {
                    ToolCallValidity::Valid => (None, None),
                    ToolCallValidity::Invalid { error } => (Some(true), Some(error)),
                };
                Self {
                    tool_call_id: call.tool_call_id,
                    tool_name: call.tool_name,
                    input: call.input,
                    provider_executed: call.provider_executed,
                    dynamic: Some(true),
                    provider_metadata: call.provider_metadata,
                    invalid,
                    error,
                }
            }
        }
    }
}

impl ToolCall {
    /// Provider-assigned call id.
    #[must_use]
    pub fn tool_call_id(&self) -> &str {
        match self {
            Self::Static(call) => &call.tool_call_id,
            Self::Dynamic(call) => &call.tool_call_id,
        }
    }

    /// Tool name.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        match self {
            Self::Static(call) => &call.tool_name,
            Self::Dynamic(call) => &call.tool_name,
        }
    }

    /// Parsed arguments (the original string for an unparsable invalid call).
    #[must_use]
    pub fn input(&self) -> &Value {
        match self {
            Self::Static(call) => &call.input,
            Self::Dynamic(call) => &call.input,
        }
    }

    /// Mutable arguments.
    pub fn input_mut(&mut self) -> &mut Value {
        match self {
            Self::Static(call) => &mut call.input,
            Self::Dynamic(call) => &mut call.input,
        }
    }

    /// Whether the provider executes the call, as the provider reported it.
    #[must_use]
    pub fn provider_executed(&self) -> Option<bool> {
        match self {
            Self::Static(call) => call.provider_executed,
            Self::Dynamic(call) => call.provider_executed,
        }
    }

    /// Provider-specific metadata.
    #[must_use]
    pub fn provider_metadata(&self) -> Option<&ProviderMetadata> {
        match self {
            Self::Static(call) => call.provider_metadata.as_ref(),
            Self::Dynamic(call) => call.provider_metadata.as_ref(),
        }
    }

    /// Mutable provider-specific metadata.
    pub fn provider_metadata_mut(&mut self) -> &mut Option<ProviderMetadata> {
        match self {
            Self::Static(call) => &mut call.provider_metadata,
            Self::Dynamic(call) => &mut call.provider_metadata,
        }
    }

    /// Whether the tool is dynamic (defined at runtime).
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic(_))
    }

    /// Whether the call is invalid (always a dynamic call).
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.error().is_some()
    }

    /// The failure of an invalid call.
    #[must_use]
    pub fn error(&self) -> Option<&AiMuxError> {
        match self {
            Self::Dynamic(DynamicToolCall {
                validity: ToolCallValidity::Invalid { error },
                ..
            }) => Some(error),
            _ => None,
        }
    }
}

/// The JSON form of [`ToolCall`]: the pre-enum struct, unchanged, so the wire
/// and `ToolCall.ts` stay the same (A7). Private; conversions apply A7 D25/D26.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
struct ToolCallWire {
    /// Provider-assigned call id.
    tool_call_id: String,
    /// Tool name.
    tool_name: String,
    /// Parsed arguments, or the original string when `invalid` is true and the
    /// provider input was not valid JSON.
    input: Value,
    /// Whether the tool call will be executed by the provider.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_executed: Option<bool>,
    /// Whether the tool is dynamic (defined at runtime, e.g. MCP tools).
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dynamic: Option<bool>,
    /// Additional provider-specific metadata associated with this call.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_metadata: Option<ProviderMetadata>,
    /// Set when lookup, JSON parsing, or schema validation still failed after
    /// the optional repair attempt.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    invalid: Option<bool>,
    /// Typed failure associated with an invalid tool call.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<AiMuxError>,
}

/// A tool result produced by a provider-executed tool (e.g. xAI `file_search`,
/// web search), carried next to the [`RawToolCall`] it answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolResult {
    /// Must match the corresponding call's `tool_call_id`.
    pub tool_call_id: String,
    pub tool_name: String,
    /// The tool's output (usually a JSON-serializable value or plain text).
    pub result: Value,
    /// Whether the result is an error or error message.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// Whether the result is preliminary (replaces prior, e.g. image previews).
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preliminary: Option<bool>,
    /// Whether the tool is dynamic (defined at runtime, e.g. MCP tools).
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<bool>,
    /// Whether the tool was executed by the provider (AI SDK
    /// `generate-text/tool-result.ts:17,32`; #200:2292). Passed through as
    /// given: absent / false / true.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_executed: Option<bool>,
    /// Additional provider-specific metadata for the tool result.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<ProviderMetadata>,
}

/// How the model should choose tools.
///
/// Wire format aligns with Vercel AI SDK `toolChoice`:
/// `"auto" | "none" | "required" | { "type": "tool", "toolName": "..." }`.
/// This is a mixed tagged/untagged shape (unit variants as bare strings,
/// `Tool` variant as a tagged object), so serde is implemented by hand.
#[derive(Debug, Clone, Default, PartialEq, TS)]
#[ts(
    export,
    type = "\"auto\" | \"none\" | \"required\" | { type: \"tool\"; toolName: string }"
)]
pub enum ToolChoice {
    /// Model decides whether to call a tool (default).
    #[default]
    Auto,
    /// Model must not call any tools.
    None,
    /// Model must call at least one tool.
    Required,
    /// Model must call the specified tool.
    Tool { tool_name: String },
}

impl Serialize for ToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        match self {
            ToolChoice::Auto => serializer.serialize_str("auto"),
            ToolChoice::None => serializer.serialize_str("none"),
            ToolChoice::Required => serializer.serialize_str("required"),
            ToolChoice::Tool { tool_name } => {
                let mut s = serializer.serialize_struct("ToolChoice", 2)?;
                s.serialize_field("type", "tool")?;
                s.serialize_field("toolName", tool_name)?;
                s.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ToolChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Str(String),
            Tool {
                #[serde(rename = "type")]
                typ: String,
                #[serde(rename = "toolName")]
                tool_name: Option<String>,
            },
        }

        match Repr::deserialize(deserializer)? {
            Repr::Str(s) => match s.as_str() {
                "auto" => Ok(ToolChoice::Auto),
                "none" => Ok(ToolChoice::None),
                "required" => Ok(ToolChoice::Required),
                other => Err(D::Error::custom(format!(
                    "unknown toolChoice '{other}' (expected auto/none/required or {{type:\"tool\",toolName}})"
                ))),
            },
            Repr::Tool { typ, tool_name } => {
                if typ != "tool" {
                    return Err(D::Error::custom(format!(
                        "unknown toolChoice type '{typ}' (expected \"tool\")"
                    )));
                }
                let tool_name = tool_name.ok_or_else(|| {
                    D::Error::custom("toolChoice {type:\"tool\"} requires a toolName field")
                })?;
                Ok(ToolChoice::Tool { tool_name })
            }
        }
    }
}
