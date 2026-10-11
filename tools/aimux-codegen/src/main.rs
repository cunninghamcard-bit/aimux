//! `aimux-codegen [--check]`: regenerate (or verify) `generated/` from `spec/`.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut spec = root.join("spec");
    let mut out = root.join("generated");
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--spec" => spec = args.next().map_or(spec, PathBuf::from),
            "--out" => out = args.next().map_or(out, PathBuf::from),
            other => {
                eprintln!(
                    "unknown argument {other}; usage: aimux-codegen [--check] [--spec DIR] [--out DIR]"
                );
                return ExitCode::from(2);
            }
        }
    }
    if check {
        match aimux_codegen::check(&spec, &out) {
            Ok(()) => {
                println!("{}: up to date", out.display());
                ExitCode::SUCCESS
            }
            Err(problems) => {
                for p in problems {
                    eprintln!("{p}");
                }
                eprintln!("run `cargo run -p aimux-codegen` to regenerate");
                ExitCode::FAILURE
            }
        }
    } else {
        match aimux_codegen::write(&spec, &out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        }
    }
}
