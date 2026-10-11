//! `shared/v4`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::JsonObject;

/// `SharedV4ProviderOptions`: provider name to options object.
pub type SharedV4ProviderOptions = BTreeMap<String, JsonObject>;
/// `SharedV4ProviderMetadata`: provider name to metadata object.
pub type SharedV4ProviderMetadata = BTreeMap<String, JsonObject>;
/// `SharedV4Headers`.
pub type SharedV4Headers = BTreeMap<String, String>;
/// `SharedV4ProviderReference`: provider name to provider file id.
pub type SharedV4ProviderReference = BTreeMap<String, String>;

/// `SharedV4AudioFormat`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedV4AudioFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
}

/// `SharedV4FileDataData | SharedV4FileDataUrl`: the inline or linked file
/// data allowed in model output and reasoning files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum SharedV4InlineFileData {
    /// Base64 text (`Uint8Array` crosses the wire as base64).
    Data { data: String },
    Url {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        original_url: Option<String>,
    },
}

/// `SharedV4FileData`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum SharedV4FileData {
    /// Base64 text (`Uint8Array` crosses the wire as base64).
    Data {
        data: String,
    },
    Url {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        original_url: Option<String>,
    },
    Reference {
        reference: SharedV4ProviderReference,
    },
    Text {
        text: String,
    },
}

/// `SharedV4Warning`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum SharedV4Warning {
    Unsupported {
        feature: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    Compatibility {
        feature: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    Deprecated {
        setting: String,
        message: String,
    },
    Other {
        message: String,
    },
}
