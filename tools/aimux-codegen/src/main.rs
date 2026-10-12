//! `aimux-codegen [--check] [--spec DIR] [--out DIR] [--src DIR] [--versions FILE] [--list FILE]`:
//! regenerate (or verify) `generated/` — the 1.8 PoC outputs from `spec/`
//! and the 2.1 `manifest.json` + `coverage.txt` from `aimux-core/src`.

use std::path::PathBuf;
use std::process::ExitCode;

use aimux_codegen::manifest::{self, Inputs};

const USAGE: &str = "usage: aimux-codegen [--check] [--spec DIR] [--out DIR] [--src DIR] [--versions FILE] [--list FILE]";

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut spec = root.join("spec");
    let mut out = root.join("generated");
    let mut inputs = Inputs::from_workspace(&root.join("../.."));
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut path = |current: PathBuf| args.next().map_or(current, PathBuf::from);
        match arg.as_str() {
            "--check" => check = true,
            "--spec" => spec = path(spec),
            "--out" => out = path(out),
            "--src" => inputs.src_root = path(inputs.src_root),
            "--versions" => inputs.versions = path(inputs.versions),
            "--list" => inputs.list = path(inputs.list),
            other => {
                eprintln!("unknown argument {other}; {USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let mut problems = Vec::new();
    if check {
        if let Err(p) = aimux_codegen::check(&spec, &out) {
            problems.extend(p);
        }
        if let Err(p) = manifest::check(&inputs, &out) {
            problems.extend(p);
        }
    } else {
        // Manifest first: a coverage or extraction failure writes nothing.
        match manifest::generate(&inputs) {
            Err(p) => problems.extend(p),
            Ok(_) => {
                if let Err(e) = aimux_codegen::write(&spec, &out) {
                    problems.push(e);
                }
                if let Err(p) = manifest::write(&inputs, &out) {
                    problems.extend(p);
                }
            }
        }
    }
    if problems.is_empty() {
        if check {
            println!("{}: up to date", out.display());
        }
        return ExitCode::SUCCESS;
    }
    for p in problems {
        eprintln!("{p}");
    }
    if check {
        eprintln!("run `cargo run -p aimux-codegen` to regenerate");
    }
    ExitCode::FAILURE
}
