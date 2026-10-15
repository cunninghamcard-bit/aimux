//! Phase 2 unit 2.1 acceptance: the manifest covers every `#[ts(export)]`
//! type, its coverage report equals the 1.5 closure DTO list
//! (`docs/plan/closure-dtos.txt`), and each mutation below turns red.

use std::path::{Path, PathBuf};
use std::process::Command;

use aimux_codegen::manifest::{self, Inputs, exported_section};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace() -> PathBuf {
    root().join("../..")
}

fn inputs() -> Inputs {
    Inputs::from_workspace(&workspace())
}

fn tmp(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "aimux-manifest-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// Inputs whose `aimux-core/src` is a scratch copy.
fn scratch_src(label: &str) -> (PathBuf, Inputs) {
    let dir = tmp(label);
    let src = dir.join("src");
    copy_tree(&inputs().src_root, &src);
    let mut i = inputs();
    i.src_root = src;
    (dir, i)
}

fn errors(i: &Inputs) -> Vec<String> {
    manifest::generate(i).expect_err("generation should fail")
}

#[test]
fn committed_manifest_is_up_to_date() {
    manifest::check(&inputs(), &root().join("generated")).unwrap();
}

#[test]
fn coverage_report_equals_the_closure_dto_list() {
    let list = std::fs::read_to_string(&inputs().list).unwrap();
    let expected = exported_section(&list);
    let header = list.lines().find(|l| l.starts_with("[exported]")).unwrap();
    assert_eq!(header, format!("[exported] {}", expected.len()));
    let report = std::fs::read_to_string(root().join("generated/coverage.txt")).unwrap();
    let got: Vec<String> = report
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    assert_eq!(got, expected);
    for category in [
        "user",
        "provider",
        "core",
        "error",
        "recording",
        "multimodal",
        "tool",
    ] {
        assert!(
            got.iter().any(|l| l.split(' ').nth(1) == Some(category)),
            "no {category} type in the manifest"
        );
    }
}

/// The `ai` pin as v4 `fixtures/aisdk/VERSIONS.json` writes it
/// (`{ "version", "integrity" }` per package).
const AI_PIN: &str = "\"ai\": {\n    \"version\": \"7.0.127\"";

#[test]
fn manifest_records_versions_json_verbatim() {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("generated/manifest.json")).unwrap(),
    )
    .unwrap();
    let versions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&inputs().versions).unwrap()).unwrap();
    assert_eq!(manifest["sdk"], versions);
    // Regenerating from an edited VERSIONS.json carries the edit through:
    // the manifest copies the file, it does not keep its own table.
    let dir = tmp("versions_verbatim");
    let mut edited = versions.clone();
    edited["ai"]["integrity"] = serde_json::json!("sha512-edited");
    edited["@ai-sdk/extra"] = serde_json::json!({ "version": "0.0.1", "integrity": "sha512-x" });
    let path = dir.join("VERSIONS.json");
    std::fs::write(&path, serde_json::to_string_pretty(&edited).unwrap()).unwrap();
    let mut i = inputs();
    i.versions = path;
    let fresh: serde_json::Value =
        serde_json::from_str(&manifest::generate(&i).unwrap()[0].content).unwrap();
    assert_eq!(fresh["sdk"], edited);
    let _ = std::fs::remove_dir_all(dir);
}

/// Mutation 1: a type loses `#[ts(export)]` (it leaves the manifest) →
/// red, naming exactly that list line.
#[test]
fn removed_type_is_named() {
    let (dir, i) = scratch_src("removed");
    let file = i.src_root.join("recording.rs");
    let text = std::fs::read_to_string(&file).unwrap();
    let item = text
        .find("pub struct HttpExchange")
        .expect("HttpExchange in recording.rs");
    let attr = text[..item].rfind("#[ts(export)]").unwrap();
    let mutated = format!("{}{}", &text[..attr], &text[attr + "#[ts(export)]".len()..]);
    std::fs::write(&file, mutated).unwrap();
    let errs = errors(&i);
    assert_eq!(
        errs,
        vec!["coverage: missing from manifest: HttpExchange recording aimux-core/src/recording.rs"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Mutation 2: a fake line in the list → red, naming it.
#[test]
fn fake_list_line_is_named() {
    let dir = tmp("fake");
    let list = std::fs::read_to_string(&inputs().list).unwrap();
    let mutated = list.replacen(
        "\n[exported] ",
        "\n[exported] \nFakeDto core aimux-core/src/fake.rs\n# count: ",
        1,
    );
    let mut i = inputs();
    i.list = dir.join("closure-dtos.txt");
    std::fs::write(&i.list, mutated).unwrap();
    assert_eq!(
        errors(&i),
        vec!["coverage: missing from manifest: FakeDto core aimux-core/src/fake.rs"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Mutation 3: an extra exported type not in the list → red, naming it.
#[test]
fn extra_manifest_type_is_named() {
    let (dir, i) = scratch_src("extra");
    std::fs::write(
        i.src_root.join("extra_dto.rs"),
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct ExtraDto {\n    pub a: String,\n}\n",
    )
    .unwrap();
    assert_eq!(
        errors(&i),
        vec!["coverage: not in closure-dtos.txt: ExtraDto core aimux-core/src/extra_dto.rs"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Mutation 4: VERSIONS.json changes → committed manifest is stale (lib and
/// CLI), and the new version lands in a regenerated manifest.
#[test]
fn versions_change_makes_manifest_stale() {
    let dir = tmp("versions");
    let text = std::fs::read_to_string(&inputs().versions).unwrap();
    assert!(text.contains(AI_PIN));
    let versions = dir.join("VERSIONS.json");
    std::fs::write(
        &versions,
        text.replace(AI_PIN, "\"ai\": {\n    \"version\": \"7.0.999\""),
    )
    .unwrap();
    let mut i = inputs();
    i.versions = versions.clone();
    let problems = manifest::check(&i, &root().join("generated")).unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].ends_with("manifest.json is out of date"),
        "{problems:?}"
    );
    let fresh = manifest::generate(&i).unwrap();
    let fresh: serde_json::Value = serde_json::from_str(&fresh[0].content).unwrap();
    assert_eq!(fresh["sdk"]["ai"]["version"], "7.0.999");

    let out = Command::new(env!("CARGO_BIN_EXE_aimux-codegen"))
        .args(["--check", "--versions"])
        .arg(&versions)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("manifest.json is out of date"));
    let _ = std::fs::remove_dir_all(dir);
}

/// Mutation 5: an exported type with a field type that is neither a DTO,
/// alias, primitive/container nor registered codec → generation error with
/// file:line; nothing is written.
#[test]
fn unregistered_field_type_fails_generation() {
    let (dir, i) = scratch_src("unregistered");
    std::fs::write(
        i.src_root.join("extra_dto.rs"),
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct ExtraDto {\n    pub at: std::time::Instant,\n}\n",
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter().any(|e| e
            == "aimux-core/src/extra_dto.rs:3: unregistered type `Instant` (is not an exported type and has no registered codec)"),
        "{errs:?}"
    );

    let out_dir = dir.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_aimux-codegen"))
        .args(["--src"])
        .arg(&i.src_root)
        .arg("--out")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unregistered type `Instant`"));
    assert_eq!(
        std::fs::read_dir(&out_dir).unwrap().count(),
        0,
        "failed generation wrote files"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Hand-written serde without a declared codec is rejected too.
#[test]
fn hand_written_serde_without_codec_fails() {
    let (dir, i) = scratch_src("handwritten");
    std::fs::write(
        i.src_root.join("extra_dto.rs"),
        "#[derive(TS)]\n#[ts(export)]\npub enum ExtraDto { A }\n",
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter()
            .any(|e| e.contains("extra_dto.rs:3: ExtraDto has hand-written serde")),
        "{errs:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Every struct field wire name and every tagged variant value in the
/// manifest appears in the current ts-rs export for that type, so the
/// extracted serde rules (rename / rename_all / tag) match what serde emits.
#[test]
fn wire_names_match_the_current_ts_exports() {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("generated/manifest.json")).unwrap(),
    )
    .unwrap();
    let mut checked = 0;
    let mut problems = Vec::new();
    for t in manifest["types"].as_array().unwrap() {
        if t.get("tsOverride").is_some() {
            continue;
        }
        let name = t.get("tsName").unwrap_or(&t["name"]).as_str().unwrap();
        let path = workspace().join(format!("bindings/node/src/types/{name}.ts"));
        let ts = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let shape = &t["shape"];
        let mut names = Vec::new();
        match shape["kind"].as_str().unwrap() {
            "struct" => {
                for f in shape["fields"].as_array().unwrap() {
                    if f.get("flatten").is_none() {
                        names.push(f["wire"].as_str().unwrap().to_string());
                    }
                }
            }
            "enum" => {
                for v in shape["variants"].as_array().unwrap() {
                    if let Some(w) = v.get("wire") {
                        names.push(format!("\"{}\"", w.as_str().unwrap()));
                    }
                }
            }
            _ => {}
        }
        for n in names {
            checked += 1;
            let found = if n.starts_with('"') {
                ts.contains(&n)
            } else {
                ts.contains(&format!("{n}:"))
                    || ts.contains(&format!("{n}?:"))
                    || ts.contains(&format!("\"{n}\":"))
                    || ts.contains(&format!("\"{n}\"?:"))
            };
            if !found {
                problems.push(format!("{name}: {n}"));
            }
        }
    }
    assert!(checked > 800, "only {checked} names checked");
    assert!(problems.is_empty(), "{problems:?}");
}

/// A CRLF checkout (Windows `core.autocrlf=true`) of the same sources and
/// VERSIONS.json gives the same input hash and a byte-identical manifest.
#[test]
fn crlf_checkout_gives_identical_manifest() {
    fn to_crlf(dir: &Path) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                to_crlf(&p);
            } else {
                let text = std::fs::read_to_string(&p).unwrap();
                std::fs::write(&p, text.replace("\r\n", "\n").replace('\n', "\r\n")).unwrap();
            }
        }
    }
    let (dir, mut i) = scratch_src("crlf");
    to_crlf(&i.src_root);
    let versions = dir.join("VERSIONS.json");
    let text = std::fs::read_to_string(&inputs().versions).unwrap();
    std::fs::write(&versions, text.replace('\n', "\r\n")).unwrap();
    i.versions = versions;
    assert!(
        std::fs::read_to_string(i.src_root.join("lib.rs"))
            .unwrap()
            .contains("\r\n")
    );

    let lf_out = manifest::generate(&inputs()).unwrap();
    let crlf_out = manifest::generate(&i).unwrap();
    let hash = |o: &[aimux_codegen::Output]| {
        let v: serde_json::Value = serde_json::from_str(&o[0].content).unwrap();
        v["inputHash"].as_str().unwrap().to_string()
    };
    assert_eq!(hash(&crlf_out), hash(&lf_out));
    assert_eq!(crlf_out, lf_out);
    manifest::check(&i, &root().join("generated")).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

/// Manifest of `i` as JSON, without `inputHash` (which changes on any edit).
fn manifest_json(i: &Inputs) -> serde_json::Value {
    let out = manifest::generate(i).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&out[0].content).unwrap();
    v.as_object_mut().unwrap().remove("inputHash");
    v
}

fn find_type<'a>(m: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    m["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap()
}

fn find_field<'a>(t: &'a serde_json::Value, rust: &str) -> &'a serde_json::Value {
    t["shape"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rust"] == rust)
        .unwrap()
}

/// Each serde rule the extractor models changes the specific manifest field
/// it drives (manifests compared without `inputHash`), and the committed
/// manifest goes stale.
#[test]
fn serde_attribute_mutations_change_the_modelled_field() {
    type Probe = fn(&serde_json::Value) -> serde_json::Value;
    let cases: &[(
        &str,
        &str,
        &str,
        &str,
        Probe,
        serde_json::Value,
        serde_json::Value,
    )] = &[
        (
            "skip_serializing_if",
            "tool.rs",
            "#[serde(default, skip_serializing_if = \"Option::is_none\")]",
            "#[serde(default)]",
            |m| find_field(find_type(m, "FunctionTool"), "description")["omitWhenEmpty"].clone(),
            serde_json::json!(true),
            serde_json::Value::Null,
        ),
        (
            "flatten",
            "tool.rs",
            "#[serde(default, skip_serializing_if = \"Option::is_none\")]",
            "#[serde(flatten, default, skip_serializing_if = \"Option::is_none\")]",
            |m| find_field(find_type(m, "FunctionTool"), "description")["flatten"].clone(),
            serde_json::Value::Null,
            serde_json::json!(true),
        ),
        (
            "untagged",
            "decision_model.rs",
            "#[serde(untagged)]",
            "",
            |m| find_type(m, "DecisionInput")["shape"]["representation"]["style"].clone(),
            serde_json::json!("untagged"),
            serde_json::json!("external"),
        ),
        (
            "rename_all",
            "tool.rs",
            "#[serde(rename_all = \"camelCase\")]",
            "#[serde(rename_all = \"snake_case\")]",
            |m| find_field(find_type(m, "FunctionTool"), "input_schema")["wire"].clone(),
            serde_json::json!("inputSchema"),
            serde_json::json!("input_schema"),
        ),
        (
            "field rename",
            "decision_model.rs",
            "rename = \"true\",",
            "rename = \"yes\",",
            |m| find_field(find_type(m, "DecisionBooleanCriteria"), "true_")["wire"].clone(),
            serde_json::json!("true"),
            serde_json::json!("yes"),
        ),
    ];
    let base = manifest_json(&inputs());
    for (label, file, from, to, probe, before, after) in cases {
        let (dir, i) = scratch_src(&label.replace(' ', "_"));
        let path = i.src_root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{label}: `{from}` not in {file}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
        let mutated = manifest_json(&i);
        assert_eq!(&probe(&base), before, "{label}: baseline");
        assert_eq!(&probe(&mutated), after, "{label}: mutated");
        let problems = manifest::check(&i, &root().join("generated"))
            .expect_err(&format!("{label}: mutation not detected"));
        assert!(
            problems
                .iter()
                .any(|p| p.ends_with("manifest.json is out of date")),
            "{label}: {problems:?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Control: a comment-only edit changes `inputHash` but nothing else.
#[test]
fn comment_only_edit_leaves_manifest_identical_except_input_hash() {
    let (dir, i) = scratch_src("comment");
    let path = i.src_root.join("tool.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("// control comment\n{text}")).unwrap();
    let hash = |i: &Inputs| {
        let v: serde_json::Value =
            serde_json::from_str(&manifest::generate(i).unwrap()[0].content).unwrap();
        v["inputHash"].clone()
    };
    assert_ne!(hash(&i), hash(&inputs()));
    assert_eq!(
        serde_json::to_string_pretty(&manifest_json(&i)).unwrap(),
        serde_json::to_string_pretty(&manifest_json(&inputs())).unwrap()
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The acceptance list is computed from source by the script (not edited by
/// hand): regenerating it reproduces the committed file, including the
/// 193rd type `ToolChoice`, whose `#[ts(\n export, ..)]` spans lines
/// (aimux-core/src/tool.rs:210-213).
#[test]
fn closure_dto_list_is_reproducible_from_source() {
    let out = Command::new("python3")
        .arg(workspace().join("scripts/gen_closure_inventory.py"))
        .arg("--check")
        .output()
        .expect("python3 is required to verify docs/plan/closure-dtos.txt");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let list = std::fs::read_to_string(&inputs().list).unwrap();
    let exported = exported_section(&list);
    let model = aimux_codegen::extract::extract(&inputs().src_root, "aimux-core/src").unwrap();
    assert_eq!(exported.len(), model.types.len());
    assert!(list.contains(&format!("\n[exported] {}\n", model.types.len())));
    let tool = std::fs::read_to_string(workspace().join("aimux-core/src/tool.rs")).unwrap();
    assert!(
        tool.contains("#[ts(\n    export,"),
        "multi-line export fixture moved"
    );
    assert!(
        exported_section(&list).contains(&"ToolChoice tool aimux-core/src/tool.rs".to_string())
    );
}

/// `DecisionBooleanCriteria.true/false` (v4 decision_model.rs:88-100) keep three wire
/// states: absent key, explicit null, value. The manifest says so with
/// `triState` and the `presence` codec instead of `Option<Option<T>>`.
#[test]
fn boolean_criteria_manifest_entry_is_tri_state() {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("generated/manifest.json")).unwrap(),
    )
    .unwrap();
    let t = manifest["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "DecisionBooleanCriteria")
        .unwrap();
    let fields: Vec<_> = t["shape"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["wire"] == "true" || f["wire"] == "false")
        .collect();
    assert_eq!(fields.len(), 2);
    for (f, wire) in fields.iter().zip(["true", "false"]) {
        assert_eq!(f["wire"], wire);
        assert_eq!(
            f["type"],
            serde_json::json!({ "triState": { "ref": { "name": "DecisionInput", "args": [] } } })
        );
        assert!(!f["type"].to_string().contains("\"option\""), "{f}");
        assert_eq!(f["codec"]["name"], "presence");
        assert_eq!(f["omitWhenEmpty"], true);
        assert_eq!(f["default"], true);
    }
    let recording = manifest["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "Recording")
        .unwrap();
    let schema = recording["shape"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["wire"] == "schema")
        .unwrap();
    assert_eq!(schema["codec"]["name"], "knownRecordingSchema");
    assert_eq!(schema["type"], serde_json::json!({ "prim": "u32" }));
}

/// A serde `with`-style field with neither `#[ts(type)]` nor a FIELD_CODECS
/// registration fails generation at the field's file:line.
#[test]
fn unregistered_serde_with_field_fails_generation() {
    let (dir, i) = scratch_src("serde_with");
    std::fs::write(
        i.src_root.join("extra_dto.rs"),
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct ExtraDto {\n    #[serde(deserialize_with = \"custom\")]\n    pub a: u32,\n}\n",
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter().any(|e| e
            == "aimux-core/src/extra_dto.rs:5: ExtraDto.a uses serde `deserialize_with=custom` but has no `#[ts(type)]` and no FIELD_CODECS registration"),
        "{errs:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// v4 TypeScript-only tagged wrappers (`TypedToolCall`, tool.rs:198) are
/// modelled from their `#[ts]` field attributes: a literal `type` tag plus
/// the flattened DTO. Dropping `#[ts(flatten)]` makes the wrapper
/// unmodellable again and generation fails at the type's file:line.
#[test]
fn ts_only_tagged_wrapper_is_modelled_and_guarded() {
    let m = manifest_json(&inputs());
    let t = find_type(&m, "TypedToolCall");
    let tag = find_field(t, "tag");
    assert_eq!(tag["wire"], "type");
    assert!(tag["type"].to_string().contains("tool-call"), "{tag}");
    let call = find_field(t, "call");
    assert_eq!(call["flatten"], true);
    assert_eq!(call["type"]["ref"]["name"], "ToolCall");

    let (dir, i) = scratch_src("ts_only");
    let path = i.src_root.join("tool.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = "    #[ts(flatten)]\n    call: ToolCall,";
    assert!(text.contains(from));
    std::fs::write(&path, text.replacen(from, "    call: ToolCall,", 1)).unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter().any(|e| e.starts_with("aimux-core/src/tool.rs:")
            && e.contains("TypedToolCall has hand-written serde")),
        "{errs:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// `#[serde(transparent)]` (v4 `NamespacedKind`, types.rs:354) is a newtype
/// on the wire; on a struct with two fields it is rejected.
#[test]
fn serde_transparent_is_a_newtype_and_needs_one_field() {
    let m = manifest_json(&inputs());
    let kind = find_type(&m, "NamespacedKind");
    assert_eq!(
        kind["tsOverride"],
        serde_json::json!({ "ts": "`${string}.${string}`" })
    );
    assert_eq!(kind["shape"]["kind"], "newtype");
    assert_eq!(
        kind["shape"]["inner"],
        serde_json::json!({ "prim": "String" })
    );

    // A named one-field struct with `transparent` is a newtype of that
    // field's type, not a one-field object.
    let only = tmp("transparent_only");
    std::fs::write(
        only.join("wrap.rs"),
        "#[derive(Serialize, Deserialize, TS)]\n#[serde(transparent)]\n#[ts(export)]\npub struct Wrap {\n    pub a: u32,\n}\n",
    )
    .unwrap();
    let model = aimux_codegen::extract::extract(&only, "x").unwrap();
    let wrap = serde_json::to_value(&model.types[0]).unwrap();
    assert_eq!(wrap["shape"]["kind"], "newtype");
    assert_eq!(wrap["shape"]["inner"], serde_json::json!({ "prim": "u32" }));
    let _ = std::fs::remove_dir_all(only);

    let (dir, i) = scratch_src("transparent");
    std::fs::write(
        i.src_root.join("extra_dto.rs"),
        "#[derive(Serialize, Deserialize, TS)]\n#[serde(transparent)]\n#[ts(export)]\npub struct Wrap {\n    pub a: u32,\n}\n#[derive(Serialize, Deserialize, TS)]\n#[serde(transparent)]\n#[ts(export)]\npub struct Two {\n    pub a: u32,\n    pub b: u32,\n}\n",
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter()
            .any(|e| e.contains("extra_dto.rs:10: Two `transparent` needs exactly one field")),
        "{errs:?}"
    );
    assert!(!errs.iter().any(|e| e.contains("Wrap ")), "{errs:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// Omission and `null` are two facts. `ApiCallError.data`
/// (aimux-core/src/error.rs:76, `Option<Value>` + `crate::wire::present`)
/// and `ApiCallError.response_body` (error.rs:64, plain `Option<String>`)
/// are both omitted when `None`, but only `data` keeps `null` apart from an
/// absent key. The manifest says so, the real serde impl agrees, and
/// removing the `present` codec flips the manifest fact.
#[test]
fn omitted_and_null_are_distinct_manifest_facts() {
    use aimux_core::error::ApiCallError;
    let m = manifest_json(&inputs());
    let t = find_type(&m, "ApiCallError");
    let data = find_field(t, "data");
    let body = find_field(t, "response_body");
    for f in [data, body] {
        assert_eq!(f["omittedWhen"], "Option::is_none", "{f}");
    }
    assert_eq!(data["null"]["input"], "distinct");
    assert_eq!(data["null"]["output"], "asValue");
    assert_eq!(body["null"]["input"], "asAbsent");
    assert_eq!(body["null"]["output"], "never");

    let base = serde_json::json!({ "url": "u", "requestBodyValues": {}, "message": "m" });
    let decode = |extra: serde_json::Value| {
        let mut v = base.clone();
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<ApiCallError>(v).unwrap()
    };
    let absent = decode(serde_json::json!({}));
    let null = decode(serde_json::json!({ "data": null, "responseBody": null }));
    assert!(absent.data.is_none());
    assert_eq!(null.data, Some(serde_json::Value::Null));
    assert!(null.response_body.is_none());
    let written = serde_json::to_value(&null).unwrap();
    assert_eq!(written["data"], serde_json::Value::Null);
    assert!(written.as_object().unwrap().contains_key("data"));
    assert!(!written.as_object().unwrap().contains_key("responseBody"));
    assert!(
        !serde_json::to_value(&absent)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("data")
    );

    let (dir, i) = scratch_src("null_facts");
    let path = i.src_root.join("error.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = "        deserialize_with = \"crate::wire::present\",\n        skip_serializing_if = \"Option::is_none\"\n    )]\n    pub data:";
    assert!(text.contains(from));
    std::fs::write(
        &path,
        text.replacen(
            from,
            "        skip_serializing_if = \"Option::is_none\"\n    )]\n    pub data:",
            1,
        ),
    )
    .unwrap();
    let mutated = manifest_json(&i);
    let data = find_field(find_type(&mutated, "ApiCallError"), "data");
    assert_eq!(data["null"]["input"], "asAbsent");
    assert_eq!(data["omittedWhen"], "Option::is_none");
    let _ = std::fs::remove_dir_all(dir);
}

/// The manifest's TS-only wrapper (`type` literal + flattened inner type)
/// against what `aimux_core::wire::Tagged` (wire.rs:148-164) really writes.
fn assert_wrapper_matches<T: serde::Serialize + aimux_core::wire::TypeTag>(
    m: &serde_json::Value,
    wrapper: &str,
    value: &T,
) {
    let t = find_type(m, wrapper);
    let fields = t["shape"]["fields"].as_array().unwrap();
    let tag = fields.iter().find(|f| f["wire"] == "type").unwrap();
    let literal = tag["type"]["codec"]["wire"]["literal"].as_str().unwrap();
    let inner = fields.iter().find(|f| f["flatten"] == true).unwrap();
    let inner_name = inner["type"]["ref"]["name"].as_str().unwrap();
    assert_eq!(
        std::any::type_name::<T>().rsplit("::").next(),
        Some(inner_name)
    );

    let tagged = serde_json::to_value(aimux_core::wire::Tagged(value)).unwrap();
    assert_eq!(tagged["type"], literal, "{wrapper}");
    let mut rest = tagged.as_object().unwrap().clone();
    rest.remove("type");
    assert_eq!(
        serde_json::Value::Object(rest),
        serde_json::to_value(value).unwrap()
    );
}

#[test]
fn ts_only_wrappers_match_wire_tagged_output() {
    use aimux_core::result::{Source, ToolApprovalResponseOutput, TypedToolResult};
    use aimux_core::tool::ToolCall;
    let m = manifest_json(&inputs());
    let call = ToolCall {
        tool_call_id: "c1".into(),
        tool_name: "weather".into(),
        input: serde_json::json!({ "city": "SH" }),
        provider_executed: Some(true),
        dynamic: None,
        provider_metadata: None,
        invalid: None,
        error: None,
    };
    assert_wrapper_matches(&m, "TypedToolCall", &call);
    let source = Source::Url {
        id: "s1".into(),
        url: "https://example.com".into(),
        title: Some("t".into()),
        provider_metadata: None,
    };
    assert_wrapper_matches(&m, "TypedSource", &source);
    let result = TypedToolResult {
        tool_call_id: "c1".into(),
        tool_name: "weather".into(),
        input: Some(serde_json::json!({ "city": "SH" })),
        output: serde_json::json!({ "temp": 20 }),
        provider_executed: None,
        provider_metadata: None,
        dynamic: None,
        preliminary: None,
    };
    assert_wrapper_matches(&m, "TaggedToolResult", &result);

    // Nested: ToolApprovalResponseOutput.toolCall is declared as
    // TypedToolCall and serialized through `crate::wire::typed`.
    let field = find_field(find_type(&m, "ToolApprovalResponseOutput"), "tool_call");
    assert_eq!(
        field["type"]["codec"]["wire"]["type"]["ref"]["name"], "TypedToolCall",
        "{field}"
    );
    let reply = ToolApprovalResponseOutput {
        approval_id: "a1".into(),
        tool_call: call.clone(),
        approved: true,
        reason: None,
        provider_executed: None,
    };
    let written = serde_json::to_value(&reply).unwrap();
    assert_eq!(
        written["toolCall"],
        serde_json::to_value(aimux_core::wire::Tagged(&call)).unwrap()
    );
}

/// Rust type text keeps the spaces Rust needs between word tokens.
#[test]
fn rust_type_text_keeps_word_spacing() {
    let text = |s: &str| aimux_codegen::extract::rust_text(&syn::parse_str(s).unwrap());
    assert_eq!(text("Vec<&'a Source>"), "Vec<&'a Source>");
    assert_eq!(text("&mut  Vec < String >"), "&mut Vec<String>");
    assert_eq!(text("Box<dyn Fn(u8) -> u8>"), "Box<dyn Fn(u8)->u8>");
    let m = manifest_json(&inputs());
    let sources = find_field(find_type(&m, "GenerateTextResultSnapshot"), "sources");
    assert!(sources.to_string().contains("Vec<&'a Source>"), "{sources}");
}

/// `Option<Option<T>>` + `skip_serializing_if = "Option::is_none"`
/// (`DecisionBooleanCriteria.true_`, decision_model.rs:88-100): `Some(None)`
/// is written as `null`, `None` is left out. The manifest says `asValue`.
#[test]
fn option_option_with_skip_writes_some_none_as_null() {
    use aimux_core::decision_model::DecisionBooleanCriteria;
    let m = manifest_json(&inputs());
    let t = find_type(&m, "DecisionBooleanCriteria");
    for rust in ["true_", "false_"] {
        let f = find_field(t, rust);
        assert_eq!(f["null"]["output"], "asValue", "{f}");
        assert_eq!(f["omittedWhen"], "Option::is_none", "{f}");
    }
    let written = serde_json::to_value(DecisionBooleanCriteria {
        true_: Some(None),
        false_: None,
    })
    .unwrap();
    assert_eq!(written, serde_json::json!({ "true": null }));
    let read: DecisionBooleanCriteria =
        serde_json::from_value(serde_json::json!({ "true": null })).unwrap();
    assert_eq!(read.true_, Some(None));
    assert_eq!(read.false_, None);
}

/// `Option<T>` without `skip_serializing_if` (`ChatCompletionChoice
/// .finish_reason`, openai_output.rs:73): `None` is written as `null`. The
/// manifest says `whenNone`. The neighbour `logprobs` (`Option<Value>` +
/// `skip_serializing_if = "Option::is_none"`) leaves `None` out
/// (`omittedWhen`) and writes a held JSON `null` (`asValue`).
#[test]
fn option_without_skip_writes_none_as_null() {
    use aimux_core::openai_output::{ChatCompletionChoice, ChatCompletionMessage};
    let m = manifest_json(&inputs());
    let t = find_type(&m, "ChatCompletionChoice");
    let finish = find_field(t, "finish_reason");
    assert_eq!(finish["null"]["output"], "whenNone");
    assert!(finish["omittedWhen"].is_null());
    let logprobs = find_field(t, "logprobs");
    assert_eq!(logprobs["omittedWhen"], "Option::is_none");
    assert_eq!(logprobs["null"]["output"], "asValue");
    let choice = ChatCompletionChoice {
        index: 0,
        message: ChatCompletionMessage {
            role: "assistant".into(),
            content: None,
            reasoning_content: None,
            tool_calls: None,
            annotations: None,
        },
        finish_reason: None,
        logprobs: None,
    };
    let written = serde_json::to_value(&choice).unwrap();
    let obj = written.as_object().unwrap();
    assert_eq!(obj.get("finish_reason"), Some(&serde_json::Value::Null));
    assert!(!obj.contains_key("logprobs"));
    let held = serde_json::to_value(ChatCompletionChoice {
        logprobs: Some(serde_json::Value::Null),
        ..choice
    })
    .unwrap();
    assert_eq!(held.get("logprobs"), Some(&serde_json::Value::Null));
}

/// `nonNullJson` (`ToolResult.result`, tool.rs:219): the manifest says
/// `rejected`, and the real decoder rejects `null` while taking `{}`.
#[test]
fn non_null_json_codec_rejects_null() {
    use aimux_core::tool::ToolResult;
    let m = manifest_json(&inputs());
    let f = find_field(find_type(&m, "ToolResult"), "result");
    assert_eq!(f["codec"]["name"], "nonNullJson");
    assert_eq!(f["null"]["input"], "rejected");
    let with = |result: serde_json::Value| {
        serde_json::from_value::<ToolResult>(
            serde_json::json!({ "toolCallId": "c", "toolName": "t", "result": result }),
        )
    };
    assert!(with(serde_json::Value::Null).is_err());
    assert!(with(serde_json::json!({})).is_ok());
}

fn extract_one(src: &str) -> Result<serde_json::Value, Vec<String>> {
    let dir = tmp("extract_one");
    std::fs::write(dir.join("sample.rs"), src).unwrap();
    let r = aimux_codegen::extract::extract(&dir, "x")
        .map(|m| serde_json::to_value(&m.types[0]).unwrap());
    let _ = std::fs::remove_dir_all(dir);
    r
}

/// A `deserialize_with` field (declared with `#[ts(type)]`) leaves null
/// handling to its codec: the manifest says `codec`, not a guess.
#[test]
fn deserialize_with_field_null_is_codec() {
    let t = extract_one(
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct S {\n    #[serde(deserialize_with = \"lenient\")]\n    #[ts(type = \"number\")]\n    pub a: Option<u32>,\n}\n",
    )
    .unwrap();
    let a = &t["shape"]["fields"][0];
    assert_eq!(a["null"]["input"], "codec", "{a}");
    assert_eq!(a["null"]["output"], "whenNone", "{a}");
}

/// `skip_serializing_if` predicates are matched by last path segment
/// (`is_none`, `is_empty`, ..); any other predicate, or an `Option` field
/// skipped by something other than `is_none`, fails generation with
/// file:line instead of defaulting to `whenNone`.
#[test]
fn skip_predicates_are_registered_by_last_segment() {
    let ok = extract_one(
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct S {\n    #[serde(skip_serializing_if = \"crate::util::is_empty\")]\n    pub a: Vec<u32>,\n    #[serde(skip_serializing_if = \"std::option::Option::is_none\")]\n    pub b: Option<u32>,\n}\n",
    )
    .unwrap();
    assert_eq!(
        ok["shape"]["fields"][0]["omittedWhen"],
        "crate::util::is_empty"
    );
    assert_eq!(ok["shape"]["fields"][1]["null"]["output"], "never");

    let errs = extract_one(
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct S {\n    #[serde(skip_serializing_if = \"is_zero\")]\n    pub a: u32,\n}\n",
    )
    .unwrap_err();
    assert!(
        errs.iter().any(
            |e| e == "x/sample.rs:5: S.a: unregistered skip_serializing_if predicate `is_zero`"
        ),
        "{errs:?}"
    );
    let errs = extract_one(
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct S {\n    #[serde(skip_serializing_if = \"Vec::is_empty\")]\n    pub a: Option<Vec<u32>>,\n}\n",
    )
    .unwrap_err();
    assert!(
        errs.iter()
            .any(|e| e.starts_with("x/sample.rs:5: S.a: Option field skipped by `Vec::is_empty`")),
        "{errs:?}"
    );
}

/// The `presence` codec needs a `Some` payload that can hold `null`.
/// Narrowing `ApiCallError.data` (error.rs:76) to `Option<String>` keeps
/// the registration but fails generation at the field's line.
#[test]
fn presence_codec_needs_a_nullable_payload() {
    let m = manifest_json(&inputs());
    let mut distinct = 0;
    let mut walk = vec![];
    for t in m["types"].as_array().unwrap() {
        walk.push(t["shape"].clone());
    }
    while let Some(shape) = walk.pop() {
        for f in shape["fields"].as_array().into_iter().flatten() {
            if f["codec"]["name"] == "presence" {
                assert_eq!(f["null"]["input"], "distinct", "{f}");
                distinct += 1;
            }
        }
        for v in shape["variants"].as_array().into_iter().flatten() {
            walk.push(v["shape"].clone());
        }
    }
    assert_eq!(
        distinct,
        aimux_codegen::extract::FIELD_CODECS
            .iter()
            .filter(|c| c.name == "presence")
            .count()
    );

    let (dir, i) = scratch_src("presence_payload");
    let path = i.src_root.join("error.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = "    pub data: Option<serde_json::Value>,";
    assert!(text.contains(from));
    std::fs::write(
        &path,
        text.replacen(from, "    pub data: Option<String>,", 1),
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("aimux-core/src/error.rs:")
                && e.contains(
                    "ApiCallError.data: presence codec needs Option<Option<T>> or Option<Value>"
                )),
        "{errs:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A `serialize_with` field (declared with `#[ts(type)]`) leaves null
/// *output* to its codec; input stays the type's own rule.
#[test]
fn serialize_with_field_null_output_is_codec() {
    let t = extract_one(
        "#[derive(Serialize, Deserialize, TS)]\n#[ts(export)]\npub struct S {\n    #[serde(serialize_with = \"as_text\")]\n    #[ts(type = \"string\")]\n    pub a: Option<u32>,\n}\n",
    )
    .unwrap();
    let a = &t["shape"]["fields"][0];
    assert_eq!(a["null"]["output"], "codec", "{a}");
    assert_eq!(a["null"]["input"], "asAbsent", "{a}");
    assert_eq!(
        a["type"]["codec"]["wire"],
        serde_json::json!({ "ts": "string" })
    );
}

/// `#[ts(as = "<Rust type>")]` is recorded as a resolved type reference,
/// checked like any field type: `ToolApprovalRequestOutput.tool_call`
/// (result.rs:254) and a map override (decision_model.rs:173) resolve, and
/// pointing the override at an unknown type fails generation.
#[test]
fn ts_as_override_is_a_checked_type_reference() {
    let m = manifest_json(&inputs());
    let call = find_field(find_type(&m, "ToolApprovalRequestOutput"), "tool_call");
    assert_eq!(call["type"]["codec"]["rust"], "crate::tool::ToolCall");
    assert_eq!(
        call["type"]["codec"]["wire"],
        serde_json::json!({ "type": { "ref": { "name": "TypedToolCall", "args": [] } } })
    );
    let found = m["types"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["shape"]["fields"].as_array().cloned().unwrap_or_default())
        .any(|f| {
            f["type"]["codec"]["wire"]["type"]["map"]["value"]["ref"]["name"] == "DecisionQuestion"
        });
    assert!(
        found,
        "HashMap<String, DecisionQuestion> override not structured"
    );

    let (dir, i) = scratch_src("ts_as_unknown");
    let path = i.src_root.join("result.rs");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = "#[ts(as = \"crate::tool::TypedToolCall\")]";
    assert!(text.contains(from));
    std::fs::write(
        &path,
        text.replacen(from, "#[ts(as = \"crate::tool::NoSuchCall\")]", 1),
    )
    .unwrap();
    let errs = errors(&i);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("aimux-core/src/result.rs:") && e.contains("NoSuchCall")),
        "{errs:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}
