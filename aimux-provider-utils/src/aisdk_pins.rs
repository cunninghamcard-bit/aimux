//! The `@ai-sdk/provider-utils` version whose user-agent suffix
//! (`ai-sdk-provider-utils/{version}`) every request carries. It must equal
//! its pin in `fixtures/aisdk/VERSIONS.json`; the test below fails on drift.
//! The provider package versions live in `aimux-providers`'
//! `shared/aisdk_pins.rs`.

pub(crate) const PROVIDER_UTILS: &str = "5.0.53";

#[cfg(test)]
mod tests {
    #[test]
    fn provider_utils_version_is_pinned() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/aisdk/VERSIONS.json"
        );
        let versions: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        // `{version, integrity}` (unit 1.11) or a bare version string.
        let entry = &versions["@ai-sdk/provider-utils"];
        let pinned = entry["version"].as_str().or_else(|| entry.as_str());
        assert_eq!(pinned, Some(super::PROVIDER_UTILS));
    }
}
