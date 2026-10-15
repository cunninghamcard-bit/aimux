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
    let b = block(&src, "DecisionBooleanCriteria");
    assert!(
        b.contains("True TriState[DecisionInput] `json:\"true,omitzero\"`"),
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
    let di = block(&src, "DecisionInput");
    assert!(di.contains("Variant string"), "{di}");
    // Declaration order (decision_model.rs): Text, Object, Array.
    let f = src.find("func (x *DecisionInput) UnmarshalJSON").unwrap();
    let body = &src[f..];
    let (t, o, a) = (
        body.find("\"Text\"").unwrap(),
        body.find("\"Object\"").unwrap(),
        body.find("\"Array\"").unwrap(),
    );
    assert!(t < o && o < a, "{body}");
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
    // Mutation (symlink bypass): an innocently named link whose target is a
    // `bindings` directory. The guard resolves the link and refuses before
    // anything is written there.
    let real = dir.join("bindings");
    std::fs::create_dir_all(&real).unwrap();
    let link = dir.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let err = emit::write(&Go, &model(), &link).unwrap_err();
    assert!(err.contains("never write under bindings/"), "{err}");
    assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0);
    // A link one level up, with the rest of the path not yet existing.
    let err = emit::write(&Go, &model(), &link.join("new/deeper")).unwrap_err();
    assert!(err.contains("never write under bindings/"), "{err}");
    assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0);
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

/// An emitter that returns one fixed output path.
struct OnePath(&'static str);

impl Emitter for OnePath {
    fn lang(&self) -> &'static str {
        "one"
    }
    fn emit(&self, _: &WireModel) -> Vec<aimux_codegen::Output> {
        vec![aimux_codegen::Output {
            path: PathBuf::from(self.0),
            content: String::new(),
        }]
    }
}

#[test]
fn output_paths_with_parent_dir_or_absolute_are_rejected() {
    let dir = std::env::temp_dir().join(format!("aimux-emit-esc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for p in ["../x.go", "a/../../x.go", "/tmp/x.go"] {
        let err = emit::write(&OnePath(p), &model(), &dir).unwrap_err();
        assert!(err.contains("output path escapes"), "{p}: {err}");
    }
    assert_eq!(
        emit::write(&OnePath("ok.go"), &model(), &dir).unwrap(),
        vec![dir.join("one/ok.go")]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dangling_symlink_output_file_is_rejected() {
    // out/one/ok.go -> <dir>/bindings/go/new.go, which does not exist yet:
    // canonicalize fails on a dangling link, so only a symlink_metadata walk
    // catches it.
    let dir = std::env::temp_dir().join(format!("aimux-emit-dangle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let target = dir.join("bindings/go/new.go");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(out.join("one")).unwrap();
    std::os::unix::fs::symlink(&target, out.join("one/ok.go")).unwrap();
    let err = emit::write(&OnePath("ok.go"), &model(), &out).unwrap_err();
    assert!(err.contains("is a symlink"), "{err}");
    assert!(!target.exists(), "the write went through the dangling link");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn output_root_reached_through_a_symlinked_parent_still_works() {
    // Reviewer A: /tmp or /workspace may themselves be symlinks; only links
    // below the output root are refused.
    let dir = std::env::temp_dir().join(format!("aimux-emit-parent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("real")).unwrap();
    std::os::unix::fs::symlink(dir.join("real"), dir.join("alias")).unwrap();
    let paths = emit::write(&OnePath("ok.go"), &model(), &dir.join("alias/out")).unwrap();
    assert_eq!(paths.len(), 1);
    assert!(dir.join("real/out/one/ok.go").is_file());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn output_root_symlinked_into_bindings_is_rejected() {
    // A symlinked root is allowed, but its resolved target is still checked:
    // a root that resolves to a directory under bindings/ is refused.
    let dir = std::env::temp_dir().join(format!("aimux-emit-rootlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let target = dir.join("bindings/go");
    std::fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&target, dir.join("emit-out")).unwrap();
    for root in [dir.join("emit-out"), dir.join("emit-out/sub")] {
        let err = emit::write(&OnePath("ok.go"), &model(), &root).unwrap_err();
        assert!(err.contains("never write under bindings/"), "{err}");
    }
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn literal_parent_dir_into_bindings_is_rejected_before_resolving() {
    // `target/../bindings/...` is refused on its literal components. Here
    // <dir>/bindings is a symlink to <dir>/elsewhere, so the resolved path
    // has no `bindings` component and only the literal check catches it.
    let dir = std::env::temp_dir().join(format!("aimux-emit-literal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("bindings")).unwrap();
    for root in [
        dir.join("target/../bindings"),
        dir.join("target/../bindings/go"),
    ] {
        let err = emit::write(&OnePath("ok.go"), &model(), &root).unwrap_err();
        assert!(err.contains("never write under bindings/"), "{err}");
    }
    assert_eq!(std::fs::read_dir(dir.join("elsewhere")).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn default_out_root_under_worktree_target_is_writable() {
    // The worktree's target/ may be a symlink (mbx: target ->
    // ~/.cache/mbx/targets/...). It is in the root's own path, so it is
    // resolved, not refused.
    let root = emit::default_out_root(&workspace());
    let paths = emit::write(&OnePath("ok.go"), &model(), &root).unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].is_file());
    let _ = std::fs::remove_dir_all(paths[0].parent().unwrap());
}

#[test]
fn dangling_symlink_output_root_is_rejected() {
    // The output root itself is a dangling link: to a missing path, and to a
    // missing path under an existing bindings/go.
    let dir = std::env::temp_dir().join(format!("aimux-emit-rootdangle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bindings/go")).unwrap();
    std::os::unix::fs::symlink(dir.join("missing"), dir.join("emit-out")).unwrap();
    std::os::unix::fs::symlink(dir.join("bindings/go/x"), dir.join("emit-out2")).unwrap();
    for root in [
        dir.join("emit-out"),
        dir.join("emit-out2"),
        dir.join("emit-out2/sub"),
    ] {
        let err = emit::write(&OnePath("ok.go"), &model(), &root).unwrap_err();
        assert!(err.contains("dangling symlink"), "{err}");
    }
    assert!(!dir.join("missing").exists());
    assert_eq!(
        std::fs::read_dir(dir.join("bindings/go")).unwrap().count(),
        0
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn symlinked_intermediate_dir_below_root_is_rejected() {
    // out/one/a is a pre-made symlink; the output file is a/b/c.go, so
    // create_dir_all would create b inside the link's target.
    let dir = std::env::temp_dir().join(format!("aimux-emit-mid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("out/one")).unwrap();
    std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("out/one/a")).unwrap();
    let err = emit::write(&OnePath("a/b/c.go"), &model(), &dir.join("out")).unwrap_err();
    assert!(err.contains("is a symlink below the output root"), "{err}");
    assert_eq!(std::fs::read_dir(dir.join("elsewhere")).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}
