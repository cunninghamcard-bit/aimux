#!/usr/bin/env python3
"""Run semantic ablations in a source copy; never mutate the working checkout.

Requires an installed Rust toolchain. --node also requires a built Node addon
and installed npm dependencies. Test failures are expected only for variants.
This is a correctness experiment, not a latency or memory benchmark.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--node", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    source = Path(tempfile.mkdtemp(prefix="source-", dir=output))
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root
    ).decode().split("\0")
    for name in files + ["Cargo.lock"]:
        if not name or not (root / name).is_file():
            continue
        target = source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / name, target)
    # Separate target directory prevents a variant from contaminating normal builds.
    env = dict(os.environ, CARGO_TARGET_DIR=str(output / "target"))
    results = []

    def run(name, command, cwd, failure_marker=None):
        result = subprocess.run(command, cwd=cwd, env=env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=600)
        (output / (name + ".log")).write_text(result.stdout)
        expected = (result.returncode == 0 if failure_marker is None
                    else result.returncode != 0 and failure_marker in result.stdout)
        results.append({"variant": name, "exit_code": result.returncode,
                        "expected_outcome": expected})
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(json.dumps(results[-1]), flush=True)
        if not expected:
            raise RuntimeError("unexpected outcome; inspect " + str(output / (name + ".log")))

    cargo = [args.cargo, "test", "-p", "aimux-operation"]
    run("baseline", cargo, source)
    variants = [
        ("no_repair_deadline", "aimux-core/src/generation_control.rs",
         "timeout::run(future, self.signal.as_ref(), self.timeout).await", "future.await",
         "direct_rust_repair_has_the_same_deadline"),
        ("no_reader_guard", "aimux-operation/src/lib.rs", "if claim.is_err() {", "if false {",
         "single_reader_claim_is_released_when_wait_is_dropped"),
        ("unbounded_output", "aimux-operation/src/lib.rs",
         "if state.output.len() < OUTPUT_CAPACITY {", "if true {",
         "bounded_output_stops_producer_and_cancel_releases_it"),
        ("overwrite_terminal", "aimux-operation/src/lib.rs",
         "if state.terminal.is_none() {", "if true {",
         "cancel_cannot_replace_an_existing_timeout_or_success"),
    ]
    for name, path, before, after, test in variants:
        target = source / path
        original = target.read_text()
        if before not in original:
            raise RuntimeError("mutation no longer applies: " + name)
        target.write_text(original.replace(before, after, 1))
        try:
            run(name, cargo + [test], source, "test result: FAILED")
        finally:
            target.write_text(original)

    if args.node:
        binding = source / "bindings/node"
        (binding / "node_modules").symlink_to(root / "bindings/node/node_modules", target_is_directory=True)
        for native in (root / "bindings/node").glob("*.node"):
            (binding / native.name).symlink_to(native)
        command = [str(binding / "node_modules/.bin/ava"), "__test__/operation.test.ts",
                   "--match", "*terminal observation*"]
        run("node_baseline", command, binding)
        target = binding / "src/operation.ts"
        original = target.read_text()
        before = "const terminal = op.finished().then(() => local.abort())"
        if before not in original:
            raise RuntimeError("terminal mutation no longer applies")
        target.write_text(original.replace(before, "const terminal = Promise.resolve()", 1))
        try:
            run("no_terminal_observer", command, binding, "1 test failed")
        finally:
            target.write_text(original)


if __name__ == "__main__":
    main()
