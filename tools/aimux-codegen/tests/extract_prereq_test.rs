//! Unit 2.2 commit 1: extractor prerequisites. Each test mutates a scratch
//! copy of `aimux-core/src` (or reads the committed manifest) and asserts
//! the exact outcome, so removing the rule in `src/extract.rs` turns it red.

use std::path::{Path, PathBuf};

use aimux_codegen::manifest::{self, Inputs};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn inputs() -> Inputs {
    Inputs::from_workspace(&root().join("../.."))
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Scratch {
    dir: PathBuf,
    inputs: Inputs,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn scratch(label: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "aimux-prereq-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy_tree(&inputs().src_root, &dir.join("src"));
    let mut i = inputs();
    i.src_root = dir.join("src");
    Scratch { dir, inputs: i }
}

impl Scratch {
    fn edit(&self, file: &str, from: &str, to: &str) {
        let path = self.inputs.src_root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "`{from}` not in {file}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    }
    fn write(&self, file: &str, text: &str) {
        std::fs::write(self.inputs.src_root.join(file), text).unwrap();
    }
    fn errors(&self) -> Vec<String> {
        manifest::generate(&self.inputs).expect_err("generation should fail")
    }
    fn manifest(&self) -> serde_json::Value {
        serde_json::from_str(&manifest::generate(&self.inputs).unwrap()[0].content).unwrap()
    }
}

/// 1-based line of the first occurrence of `needle` in `file` (scratch).
fn line_of(s: &Scratch, file: &str, needle: &str) -> usize {
    let text = std::fs::read_to_string(s.inputs.src_root.join(file)).unwrap();
    text.lines().position(|l| l.contains(needle)).unwrap() + 1
}

fn committed() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root().join("generated/manifest.json")).unwrap())
        .unwrap()
}

fn find_type<'a>(m: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    m["types"]
        .as_array()
        .unwrap()
        .iter()
        .chain(m["auxiliaryTypes"].as_array().unwrap())
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} not in manifest"))
}

// (a) serde(skip) ---------------------------------------------------------

#[test]
fn skipped_fields_are_recorded() {
    let m = committed();
    let t = find_type(&m, "EmbeddingCallOptions");
    assert_eq!(t["shape"]["skipped"], serde_json::json!(["abort_signal"]));
    let fields = t["shape"]["fields"].as_array().unwrap();
    assert!(fields.iter().all(|f| f["rust"] != "abort_signal"));
}

#[test]
fn mutation_adding_serde_skip_moves_the_field_to_skipped() {
    let s = scratch("skip");
    s.edit(
        "tool.rs",
        "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub description",
        "#[serde(skip)]\n    pub description",
    );
    let m = s.manifest();
    let t = find_type(&m, "FunctionTool");
    assert_eq!(t["shape"]["skipped"], serde_json::json!(["description"]));
}

// (b) proxies -------------------------------------------------------------

#[test]
fn same_type_proxy_uses_the_proxy_shape() {
    let m = committed();
    for name in ["Size", "AspectRatio"] {
        assert_eq!(
            find_type(&m, name)["shape"],
            serde_json::json!({"kind": "newtype", "inner": {"prim": "String"}}),
            "{name}"
        );
    }
    assert_eq!(
        find_type(&m, "AiMuxError")["shape"],
        find_type(&m, "AiMuxErrorWire")["shape"]
    );
}

#[test]
fn mutation_conflicting_proxy_directions_fail_with_file_line() {
    let s = scratch("proxy");
    s.edit(
        "shared.rs",
        "#[serde(into = \"String\", try_from = \"String\")]\n#[ts(export, type = \"`${number}x${number}`\")]",
        "#[serde(into = \"String\", try_from = \"u64\")]\n#[ts(export, type = \"`${number}x${number}`\")]",
    );
    let line = line_of(&s, "shared.rs", "pub struct Size");
    assert_eq!(
        s.errors(),
        vec![format!(
            "aimux-core/src/shared.rs:{line}: Size proxy directions differ: from = {{\"prim\":\"u64\"}}, into = {{\"prim\":\"String\"}}"
        )]
    );
}

#[test]
fn mutation_one_way_proxy_fails() {
    let s = scratch("proxy_oneway");
    s.edit(
        "shared.rs",
        "#[serde(into = \"String\", try_from = \"String\")]\n#[ts(export, type = \"`${number}x${number}`\")]",
        "#[serde(try_from = \"String\")]\n#[ts(export, type = \"`${number}x${number}`\")]",
    );
    let errs = s.errors();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("Size proxy directions differ"), "{errs:?}");
}

// (c) generics ------------------------------------------------------------

#[test]
fn generic_parameters_stay_type_parameters() {
    let m = committed();
    let t = find_type(&m, "StreamPart");
    let text = t["shape"].to_string();
    assert!(text.contains("{\"generic\":\"C\"}"), "{text}");
    assert!(!text.contains("{\"name\":\"C\""), "{text}");
}

#[test]
fn mutation_concrete_naming_no_parameter_fails() {
    let s = scratch("generic_bad_key");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export, concrete(X = String))]\npub struct ExtraDto<T> {\n    pub a: T,\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:3: `ts(concrete(X = ..))` names no type parameter of ExtraDto"
        ]
    );
}

#[test]
fn mutation_unresolvable_concrete_type_fails() {
    let s = scratch("generic_bad_value");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export, concrete(T = Missing))]\npub struct ExtraDto<T> {\n    pub a: T,\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:3: unregistered type `Missing` (is not an exported type and has no registered codec)"
        ]
    );
}

// (d) cfg_attr ------------------------------------------------------------

#[test]
fn mutation_cfg_attr_serde_on_field_fails_with_file_line() {
    let s = scratch("cfg_attr_field");
    s.edit(
        "tool.rs",
        "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub description",
        "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n    #[cfg_attr(feature = \"x\", serde(rename = \"desc\"))]\n    pub description",
    );
    let line = line_of(&s, "tool.rs", "pub description");
    assert_eq!(
        s.errors(),
        vec![format!(
            "aimux-core/src/tool.rs:{line}: `cfg_attr` carrying serde/ts attributes is not supported: #[cfg_attr(feature=\"x\",serde(rename=\"desc\"))]"
        )]
    );
}

#[test]
fn mutation_cfg_attr_ts_on_container_fails_with_file_line() {
    let s = scratch("cfg_attr_container");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\n#[cfg_attr(test, ts(rename = \"X\"))]\npub struct ExtraDto {\n    pub a: String,\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:4: `cfg_attr` carrying serde/ts attributes is not supported: #[cfg_attr(test,ts(rename=\"X\"))]"
        ]
    );
}

#[test]
fn cfg_attr_without_serde_or_ts_is_fine() {
    let s = scratch("cfg_attr_ok");
    s.edit(
        "tool.rs",
        "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub description",
        "#[serde(default, skip_serializing_if = \"Option::is_none\")]\n    #[cfg_attr(test, allow(dead_code))]\n    pub description",
    );
    s.manifest();
}

// (e) FIELD_CODECS keyed by (owner, variant, field) -----------------------

#[test]
fn mutation_missing_registration_gives_exactly_one_error() {
    let s = scratch("missing_codec");
    // Removing DecisionBooleanCriteria.true_'s registration is modelled by renaming
    // the serde fn: the (owner, variant, field, serde) key no longer matches.
    s.edit(
        "decision_model.rs",
        "rename = \"true\",\n        default,\n        deserialize_with = \"present\",",
        "rename = \"true\",\n        default,\n        deserialize_with = \"present2\",",
    );
    let line = line_of(&s, "decision_model.rs", "pub true_:");
    assert_eq!(
        s.errors(),
        vec![format!(
            "aimux-core/src/decision_model.rs:{line}: DecisionBooleanCriteria.true_ uses serde `deserialize_with=present2` but has no `#[ts(type)]` and no FIELD_CODECS registration"
        )]
    );
}

#[test]
fn mutation_variant_field_does_not_match_a_struct_registration() {
    // Same owner name and field as the registered `Recording.schema`, but
    // inside a variant: the key includes the variant, so it must not match.
    let s = scratch("variant_codec");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub enum Recording {\n    A {\n        #[serde(deserialize_with = \"deserialize_current_schema\")]\n        schema: u32,\n    },\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:6: Recording::A.schema uses serde `deserialize_with=deserialize_current_schema` but has no `#[ts(type)]` and no FIELD_CODECS registration"
        ]
    );
}

// (f) flatten -------------------------------------------------------------

#[test]
fn mutation_flatten_of_a_map_fails_with_file_line() {
    let s = scratch("flatten_map");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct ExtraDto {\n    pub a: String,\n    #[serde(flatten)]\n    pub rest: std::collections::BTreeMap<String, String>,\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:6: ExtraDto.rest: `flatten` of {\"map\":{\"key\":{\"prim\":\"String\"},\"value\":{\"prim\":\"String\"}}} cannot be expanded (only a struct DTO or an internally tagged enum can)"
        ]
    );
}

#[test]
fn flatten_of_a_struct_is_accepted() {
    let s = scratch("flatten_struct");
    s.edit(
        "model_catalogue.rs",
        "#[serde(default)]\n    pub limits: ModelLimits,",
        "#[serde(default, flatten)]\n    pub limits: ModelLimits,",
    );
    s.manifest();
}

/// v4 finding: exactly the three TS-only tagged wrappers flatten (each over
/// an internally tagged enum or a struct); no map is flattened anywhere.
#[test]
fn only_the_tagged_wrappers_use_flatten() {
    let m = committed();
    let mut flat: Vec<&str> = m["types"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t.to_string().contains("\"flatten\":true"))
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    flat.sort_unstable();
    assert_eq!(flat, ["TaggedToolResult", "TypedSource", "TypedToolCall"]);
}

/// Mutation for extract.rs's `ts(concrete)` resolve-failure error: a
/// concrete value that does not parse as a Rust type.
#[test]
fn mutation_unparseable_concrete_type_fails() {
    let s = scratch("generic_unparseable_value");
    s.write(
        "extra_dto.rs",
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export, concrete(T = 1))]\npub struct ExtraDto<T> {\n    pub a: T,\n}\n",
    );
    assert_eq!(
        s.errors(),
        vec![
            "aimux-core/src/extra_dto.rs:3: unresolved generic parameter `T`: cannot parse `ts(concrete)` type `1`"
        ]
    );
}
