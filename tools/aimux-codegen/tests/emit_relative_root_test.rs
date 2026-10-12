//! Own test binary: this test changes the process-wide current directory,
//! so it must not share a process with tests that use relative paths.

use std::path::PathBuf;

use aimux_codegen::emit::{self, Emitter};
use aimux_codegen::wire::WireModel;

struct OnePath;

impl Emitter for OnePath {
    fn lang(&self) -> &'static str {
        "one"
    }
    fn emit(&self, _: &WireModel) -> Vec<aimux_codegen::Output> {
        vec![aimux_codegen::Output {
            path: PathBuf::from("ok.go"),
            content: String::new(),
        }]
    }
}

#[test]
fn relative_nonexistent_root_inside_bindings_is_rejected() {
    // Reviewer A: with cwd = <tmp>/bindings/go, a relative `newout` used to
    // stay relative through resolve(), never matched bindings, and was
    // written into bindings/go/newout.
    let dir = std::env::temp_dir().join(format!("aimux-emit-rel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let go = dir.join("bindings/go");
    std::fs::create_dir_all(&go).unwrap();
    let before = std::env::current_dir().unwrap();
    std::env::set_current_dir(&go).unwrap();
    let res = emit::write(
        &OnePath,
        &WireModel { types: vec![] },
        std::path::Path::new("newout"),
    );
    std::env::set_current_dir(before).unwrap();
    let wrote = go.join("newout").exists();
    let _ = std::fs::remove_dir_all(&dir);
    let err = res.expect_err("relative root under bindings/ must be refused");
    assert!(err.contains("never write under bindings/"), "{err}");
    assert!(!wrote, "files were written into bindings/go/newout");
}
