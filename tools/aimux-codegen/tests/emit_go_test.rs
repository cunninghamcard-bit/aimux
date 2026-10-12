//! Unit 2.2 commit 3: the Go emitter's mapping, output location, and that
//! the output compiles with the real `go` toolchain (missing `go` = fail).

use std::path::PathBuf;
use std::process::Command;

use aimux_codegen::emit::{self, Emitter, go::Go};
use aimux_codegen::manifest::Inputs;
use aimux_codegen::wire::{self, WireModel};

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model() -> WireModel {
    wire::from_inputs(&Inputs::from_workspace(&workspace())).unwrap()
}

fn types_go() -> String {
    Go.emit(&model())
        .into_iter()
        .find(|o| o.path.as_os_str() == "types.go")
        .unwrap()
        .content
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The block of `type <name> struct { .. }`.
fn block(src: &str, name: &str) -> String {
    let start = src
        .find(&format!("\ntype {name} "))
        .unwrap_or_else(|| panic!("{name} not emitted"));
    let rest = &src[start + 1..];
    let end = rest.find("\n}\n").map_or(rest.len(), |e| e + 3);
    rest[..end].to_string()
}

#[test]
fn omitempty_only_with_skip_serializing_if() {
    let src = types_go();
    let ft = block(&src, "FunctionTool");
    assert!(
        ft.contains("Description *string `json:\"description,omitempty\"`"),
        "{ft}"
    );
    let msg = block(&src, "ChatCompletionMessage");
    // Option without skip_serializing_if: pointer, marshals null, no omitempty.
    assert!(msg.contains("Content *string `json:\"content\"`"), "{msg}");
    let n_omit = src.matches(",omitempty").count();
    let m = model();
    let skip_if: usize = m
        .types
        .iter()
        .map(|t| match &t.body {
            wire::WireBody::Record { fields } => fields
                .iter()
                .filter(|f| f.presence.skip_if_empty && !f.presence.tri_state)
                .count(),
            wire::WireBody::Union { variants, .. } => variants
                .iter()
                .map(|v| match &v.body {
                    wire::VariantBody::Fields(fs) => fs
                        .iter()
                        .filter(|f| f.presence.skip_if_empty && !f.presence.tri_state)
                        .count(),
                    _ => 0,
                })
                .sum(),
            _ => 0,
        })
        .sum();
    assert_eq!(n_omit, skip_if);
}

#[test]
fn tri_state_uses_the_helper() {
    let src = types_go();
    let b = block(&src, "BooleanCriteria");
    assert!(
        b.contains("True TriState[EvaluationInput] `json:\"true,omitzero\"`"),
        "{b}"
    );
}

#[test]
fn string_enum_is_a_string_type_with_constants() {
    let src = types_go();
    assert!(src.contains("type FinishReasonUnified string"));
    assert!(src.contains("FinishReasonUnifiedToolCalls FinishReasonUnified = \"tool-calls\""));
    assert!(src.contains("func (x *FinishReasonUnified) UnmarshalJSON"));
}

#[test]
fn unions_get_wrappers() {
    let src = types_go();
    let sp = block(&src, "StreamPart");
    assert!(
        sp.contains("Tag string") && sp.contains("Raw json.RawMessage"),
        "{sp}"
    );
    assert!(src.contains("func (x *StreamPart) UnmarshalJSON(b []byte) error {\n\tx.Raw = clone(b)\n\ttag, ok := tagOf(b, \"type\")"));
    let io = block(&src, "ImageOutputs");
    assert!(io.contains("Variant string"), "{io}");
    // Declaration order: Base64 is tried before Binary.
    let f = src.find("func (x *ImageOutputs) UnmarshalJSON").unwrap();
    let body = &src[f..];
    assert!(body.find("\"Base64\"").unwrap() < body.find("\"Binary\"").unwrap());
}

#[test]
fn write_targets_codegen_out_and_never_bindings() {
    let dir = std::env::temp_dir().join(format!("aimux-emit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.join("target/codegen-out");
    let paths = emit::write(&Go, &model(), &out).unwrap();
    assert_eq!(paths.len(), 3);
    assert!(paths.iter().all(|p| p.starts_with(out.join("go"))));
    let err = emit::write(&Go, &model(), &dir.join("bindings")).unwrap_err();
    assert!(err.contains("never write under bindings/"), "{err}");
    assert!(!dir.join("bindings").exists());
    assert_eq!(
        emit::default_out_root(&workspace()),
        workspace().join("target/codegen-out")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn generated_go_builds_with_real_go() {
    let dir = std::env::temp_dir().join(format!("aimux-emit-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    emit::write(&Go, &model(), &dir).unwrap();
    let out = Command::new("go")
        .args(["vet", "./..."])
        .current_dir(dir.join("go"))
        .output()
        .expect("`go` is required: the Go emitter output must be compiled");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Files `gofmt -l` reports in `dir` (real gofmt; missing gofmt = fail).
fn gofmt_dirty(dir: &std::path::Path) -> Vec<String> {
    let out = Command::new("gofmt")
        .arg("-l")
        .arg(".")
        .current_dir(dir)
        .output()
        .expect("`gofmt` is required: the Go emitter output must be gofmt-clean");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn generated_go_is_gofmt_clean() {
    let dir = std::env::temp_dir().join(format!("aimux-emit-gofmt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    emit::write(&Go, &model(), &dir).unwrap();
    let go = dir.join("go");
    assert_eq!(gofmt_dirty(&go), Vec::<String>::new());
    // Mutation: misalign one struct field; the check must report types.go.
    let path = go.join("types.go");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = "\tToolCallId       string";
    assert!(text.contains(from), "ToolCall field layout changed");
    std::fs::write(&path, text.replacen(from, "\tToolCallId string", 1)).unwrap();
    assert_eq!(gofmt_dirty(&go), vec!["types.go".to_string()]);
    let _ = std::fs::remove_dir_all(dir);
}
