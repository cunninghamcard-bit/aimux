//! The `@ai-sdk/*` package versions whose user-agent each provider sends
//! (`ai-sdk-{package}/{version}`). Each must equal its pin in
//! `fixtures/aisdk/VERSIONS.json`; the test below fails on drift. They are
//! constants rather than read at build time because the published crate does
//! not ship `fixtures/`.

pub(crate) const ANTHROPIC: &str = "4.0.71";
pub(crate) const GOOGLE: &str = "4.0.87";
pub(crate) const OPENAI: &str = "4.0.83";
pub(crate) const OPENAI_COMPATIBLE: &str = "3.0.62";

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
        ] {
            assert_eq!(versions[package].as_str(), Some(version), "{package}");
        }
    }
}
