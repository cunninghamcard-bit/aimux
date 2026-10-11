//! The `@ai-sdk/*` package versions whose user-agent each provider sends
//! (`ai-sdk-{package}/{version}`). Each must equal its pin in
//! `fixtures/aisdk/VERSIONS.json`; the test below fails on drift. They are
//! constants rather than read at build time because the published crate does
//! not ship `fixtures/`.

pub(crate) const ANTHROPIC: &str = "4.0.71";
pub(crate) const GOOGLE: &str = "4.0.87";
pub(crate) const OPENAI: &str = "4.0.83";
pub(crate) const OPENAI_COMPATIBLE: &str = "3.0.62";
pub(crate) const AMAZON_BEDROCK: &str = "5.0.100";
pub(crate) const AZURE: &str = "4.0.84";
pub(crate) const COHERE: &str = "4.0.52";
pub(crate) const DEEPSEEK: &str = "3.0.56";
pub(crate) const GOOGLE_VERTEX: &str = "5.0.98";
pub(crate) const GROQ: &str = "4.0.52";
pub(crate) const MISTRAL: &str = "4.0.54";
pub(crate) const XAI: &str = "5.0.12";

#[cfg(test)]
mod tests {
    #[test]
    fn versions_equal_the_aisdk_pins() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/aisdk/VERSIONS.json"
        );
        let versions: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for (package, version) in [
            ("@ai-sdk/anthropic", super::ANTHROPIC),
            ("@ai-sdk/google", super::GOOGLE),
            ("@ai-sdk/openai", super::OPENAI),
            ("@ai-sdk/openai-compatible", super::OPENAI_COMPATIBLE),
            ("@ai-sdk/amazon-bedrock", super::AMAZON_BEDROCK),
            ("@ai-sdk/azure", super::AZURE),
            ("@ai-sdk/cohere", super::COHERE),
            ("@ai-sdk/deepseek", super::DEEPSEEK),
            ("@ai-sdk/google-vertex", super::GOOGLE_VERTEX),
            ("@ai-sdk/groq", super::GROQ),
            ("@ai-sdk/mistral", super::MISTRAL),
            ("@ai-sdk/xai", super::XAI),
        ] {
            let entry = &versions[package];
            let pinned = entry["version"].as_str().or_else(|| entry.as_str());
            assert_eq!(pinned, Some(version), "{package}");
        }
    }
}
