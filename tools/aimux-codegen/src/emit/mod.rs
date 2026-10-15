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
    /// Model constructs this language refuses to emit; [`write`] runs it
    /// first and writes nothing on an error.
    ///
    /// # Errors
    ///
    /// One message per refused construct.
    fn check(&self, _model: &WireModel) -> Result<(), Vec<String>> {
        Ok(())
    }
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
/// `bindings` directory (the root is resolved first), a symlink anywhere
/// below the output root, and I/O errors.
pub fn write(
    emitter: &dyn Emitter,
    model: &WireModel,
    out_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    // Relative roots are made absolute against the current directory first,
    // so every check below sees where the files would really go.
    let abs;
    let out_root = if out_root.is_absolute() {
        out_root
    } else {
        abs = std::env::current_dir()
            .map_err(|e| format!("current directory: {e}"))?
            .join(out_root);
        abs.as_path()
    };
    // Literal check first, before anything is resolved: `target/../bindings`
    // or a `bindings` component that is itself a symlink elsewhere is
    // refused as written.
    reject_named(out_root)?;
    // The root is canonicalized once: symlinks in its own path (/tmp,
    // /workspace, a symlinked checkout) are resolved, not rejected. A
    // dangling link in it is refused. The bindings checks run on the
    // resolved root.
    let root = resolve(out_root)?;
    emitter
        .check(model)
        .map_err(|e| format!("{} emitter: {}", emitter.lang(), e.join("; ")))?;
    guard(&root)?;
    let lang = Path::new(emitter.lang());
    guard_below(&root, lang)?;
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
                root.join(lang).display()
            ));
        }
        let rel = lang.join(&o.path);
        guard_below(&root, &rel)?;
        let path = root.join(&rel);
        guard(&path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, o.content).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// Refuse a symlink (dangling or not) at any component of `rel` below the
/// canonical `root`, checked with `symlink_metadata` (never followed). A
/// component that does not exist ends the walk: nothing below it exists.
fn guard_below(root: &Path, rel: &Path) -> Result<(), String> {
    let mut cur = root.to_path_buf();
    for c in rel.components() {
        cur.push(c);
        match std::fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{}: is a symlink below the output root; emitters never write through symlinks (bindings/ guard)",
                    cur.display()
                ));
            }
            Ok(_) => {}
            Err(_) => return Ok(()),
        }
    }
    Ok(())
}

/// Canonical form of `path`: its nearest existing ancestor is canonicalized
/// (resolving symlinks) and the not-yet-existing rest is appended.
///
/// # Errors
///
/// A component that exists (per `symlink_metadata`) but cannot be
/// canonicalized: a dangling symlink, whose target is unknown.
fn resolve(path: &Path) -> Result<PathBuf, String> {
    let mut base = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(c) = base.canonicalize() {
            return Ok(rest.iter().rev().fold(c, |p, n| p.join(n)));
        }
        if std::fs::symlink_metadata(base).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{}: dangling symlink in the output path ({} does not resolve)",
                path.display(),
                base.display()
            ));
        }
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                base = parent;
            }
            _ => return Ok(path.to_path_buf()),
        }
    }
}

fn named_bindings(p: &Path) -> bool {
    p.components()
        .any(|c| matches!(c, Component::Normal(n) if n == "bindings"))
}

/// Refuse a path whose literal (unresolved) components include `bindings`.
fn reject_named(path: &Path) -> Result<(), String> {
    if named_bindings(path) {
        return Err(format!(
            "{}: emitters never write under bindings/ (literal path)",
            path.display()
        ));
    }
    Ok(())
}

/// Refuse any path that is, or resolves to, a location with a `bindings`
/// component or inside this workspace's `bindings/`.
fn guard(path: &Path) -> Result<(), String> {
    let bindings = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings")
        .canonicalize()
        .ok();
    let real = resolve(path)?;
    if named_bindings(path)
        || named_bindings(&real)
        || bindings.is_some_and(|b| real.starts_with(b))
    {
        return Err(format!(
            "{}: emitters never write under bindings/ (resolves to {})",
            path.display(),
            real.display()
        ));
    }
    Ok(())
}
