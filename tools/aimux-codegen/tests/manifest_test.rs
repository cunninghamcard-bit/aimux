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

#[test]
fn manifest_records_versions_json_verbatim() {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("generated/manifest.json")).unwrap(),
    )
    .unwrap();
    let versions: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&inputs().versions).unwrap()).unwrap();
    assert_eq!(manifest["sdk"], versions);
    assert_eq!(versions.as_object().unwrap().len(), 7);
    assert_eq!(manifest["generator"]["name"], "aimux-codegen");
    assert!(
        manifest["inputHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
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
    assert!(text.contains("\"ai\": \"7.0.127\""));
    let versions = dir.join("VERSIONS.json");
    std::fs::write(
        &versions,
        text.replace("\"ai\": \"7.0.127\"", "\"ai\": \"7.0.999\""),
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
    assert!(fresh[0].content.contains("\"ai\": \"7.0.999\""));

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

/// Each serde rule the extractor models changes the manifest: one mutation
/// per attribute in a scratch copy makes the committed manifest stale.
#[test]
fn serde_attribute_mutations_make_manifest_stale() {
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "skip_serializing_if",
            "tool.rs",
            "#[serde(default, skip_serializing_if = \"Option::is_none\")]",
            "#[serde(default)]",
        ),
        (
            "flatten",
            "tool.rs",
            "#[serde(default, skip_serializing_if = \"Option::is_none\")]",
            "#[serde(flatten, default, skip_serializing_if = \"Option::is_none\")]",
        ),
        (
            "untagged",
            "transcription_model.rs",
            "#[serde(untagged)]",
            "",
        ),
        (
            "rename_all",
            "tool.rs",
            "#[serde(rename_all = \"camelCase\")]",
            "#[serde(rename_all = \"snake_case\")]",
        ),
        (
            "field rename",
            "decision_model.rs",
            "#[serde(rename = \"true\",",
            "#[serde(rename = \"yes\",",
        ),
    ];
    for (label, file, from, to) in cases {
        let (dir, i) = scratch_src(&label.replace(' ', "_"));
        let path = i.src_root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{label}: `{from}` not in {file}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
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
    assert!(list.contains("\n[exported] 193\n"));
    let tool = std::fs::read_to_string(workspace().join("aimux-core/src/tool.rs")).unwrap();
    assert!(
        tool.contains("#[ts(\n    export,"),
        "multi-line export fixture moved"
    );
    assert!(
        exported_section(&list).contains(&"ToolChoice tool aimux-core/src/tool.rs".to_string())
    );
}

/// `BooleanCriteria.true/false` (evaluation_model.rs:62-73) keep three wire
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
        .find(|t| t["name"] == "BooleanCriteria")
        .unwrap();
    let fields = t["shape"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2);
    for (f, wire) in fields.iter().zip(["true", "false"]) {
        assert_eq!(f["wire"], wire);
        assert_eq!(
            f["type"],
            serde_json::json!({ "triState": { "ref": { "name": "EvaluationInput", "args": [] } } })
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
    assert_eq!(schema["codec"]["name"], "currentRecordingSchema");
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
