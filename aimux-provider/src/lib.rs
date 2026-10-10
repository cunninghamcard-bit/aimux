//! AI SDK `@ai-sdk/provider` V4 language-model specification types
//! (`language-model/v4`, `shared/v4`), pinned to `@ai-sdk/provider@4.0.21`.
//!
//! Design draft for the "V4 input" and "V4 output" layers of RFC-0042 §3.1
//! (#200:58). Nothing in the workspace depends on this crate yet; it is not
//! wired into any provider or core path.
//!
//! Wire shape is AI SDK JSON: camelCase fields, kebab-case `type` tags,
//! optional fields omitted when absent, bytes as base64 strings.
//!
//! Layer rules (RFC-0042 §3.1):
//! - V4 input: [`LanguageModelV4Prompt`] / [`LanguageModelV4CallOptions`]; a
//!   prompt tool call's `input` is parsed JSON. No core retry, session or
//!   recording settings.
//! - V4 output: [`LanguageModelV4Content`] / [`LanguageModelV4StreamPart`]; a
//!   tool call's `input` is the provider's argument *text* (`String`). No core
//!   `invalid` / `error` / tool metadata.
//!
//! Errors: [`ProviderError`] mirrors `src/errors` (std `Error`, no wire
//! format; D21).
//!
//! Not modelled: the `LanguageModelV4` interface itself (`doGenerate`,
//! `doStream`, `supportedUrls`) and `abortSignal`, which are behaviour, not
//! data.

mod call_options;
mod content;
mod errors;
mod prompt;
mod shared;
mod stream;

pub use call_options::*;
pub use content::*;
pub use errors::*;
pub use prompt::*;
pub use shared::*;
pub use stream::*;

/// `JSONValue`.
pub type JsonValue = serde_json::Value;
/// `JSONObject`.
pub type JsonObject = serde_json::Map<String, serde_json::Value>;
