//! #200 §2.1 constraint 1: `aimux-provider` depends on no other aimux crate.

#[test]
fn manifest_has_no_aimux_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let mut section = "";
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        if section.contains("dependencies") {
            assert!(
                !line.starts_with("aimux"),
                "{section} has an aimux dependency: {line}"
            );
        }
    }
}
