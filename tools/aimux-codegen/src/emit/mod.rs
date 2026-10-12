//! Per-language emitters over the shared wire IR ([`crate::wire`]).
//!
//! Output goes under `target/codegen-out/<lang>/` only. Nothing here writes
//! to `bindings/` (the hand-written bindings stay untouched until a later
//! unit decides to replace them).

use std::path::{Component, Path, PathBuf};

use crate::Output;
use crate::wire::WireModel;

pub mod go;

/// One target language.
pub trait Emitter {
    /// Directory name under `target/codegen-out/` (e.g. `go`).
    fn lang(&self) -> &'static str;
    /// Files to write, paths relative to the language directory.
    fn emit(&self, model: &WireModel) -> Vec<Output>;
}

/// `<workspace>/target/codegen-out`.
#[must_use]
pub fn default_out_root(workspace: &Path) -> PathBuf {
    workspace.join("target").join("codegen-out")
}

/// Write `emitter`'s files under `<out_root>/<lang>/`.
///
/// # Errors
///
/// An output path that is absolute, climbs out with `..`, or would land in a
/// `bindings` directory, and I/O errors.
pub fn write(
    emitter: &dyn Emitter,
    model: &WireModel,
    out_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let dir = out_root.join(emitter.lang());
    guard(&dir)?;
    let mut written = Vec::new();
    for o in emitter.emit(model) {
        if o.path.is_absolute()
            || o.path
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(format!(
                "{}: output path escapes {}",
                o.path.display(),
                dir.display()
            ));
        }
        let path = dir.join(&o.path);
        guard(&path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, o.content).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// Refuse any path with a `bindings` component.
fn guard(path: &Path) -> Result<(), String> {
    if path
        .components()
        .any(|c| matches!(c, Component::Normal(n) if n == "bindings"))
    {
        return Err(format!(
            "{}: emitters never write under bindings/",
            path.display()
        ));
    }
    Ok(())
}
