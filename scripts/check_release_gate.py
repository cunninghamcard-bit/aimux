#!/usr/bin/env python3
"""Fail unless every job in .github/workflows/release.yml transitively
`needs` the check-toolchain job (which runs check_toolchain_pin.sh), and
nothing lets a job run past or swallow a failed gate:

- no job `if:` uses always(), cancelled() / !cancelled() or failure()
  (each runs a job even though a job it needs failed);
- no `continue-on-error` on any job or on any check-toolchain step.

Needs PyYAML; exits nonzero if it can't be imported.
"""
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError as error:
    sys.exit(f"check_release_gate: cannot import PyYAML ({error}); install python3-yaml")

GATE = "check-toolchain"
BYPASS = re.compile(r"\b(always|cancelled|failure)\s*\(\s*\)")
path = Path(sys.argv[1] if len(sys.argv) > 1 else
            Path(__file__).resolve().parent.parent / ".github/workflows/release.yml")

doc = yaml.safe_load(path.read_text()) or {}
jobs = doc.get("jobs") or {}
if not isinstance(jobs, dict) or not jobs:
    sys.exit(f"{path}: no jobs parsed")
if GATE not in jobs:
    sys.exit(f"{path}: job '{GATE}' is missing")


def needs(name):
    value = jobs[name].get("needs") or []
    return [value] if isinstance(value, str) else list(value)


def gated(name, seen=frozenset()):
    return any(n == GATE or (n in jobs and n not in seen and gated(n, seen | {name}))
               for n in needs(name))


def continues(value):
    # `continue-on-error: true`, or an expression that may evaluate true.
    return value is not None and value is not False and str(value).strip().lower() != "false"


errors = []
ungated = [j for j in jobs if j != GATE and not gated(j)]
if ungated:
    errors.append(f"UNGATED jobs (do not transitively need '{GATE}'): {', '.join(ungated)}")
for name, job in jobs.items():
    condition = str(job.get("if", ""))
    if BYPASS.search(condition):
        errors.append(f"job '{name}' if: {condition!r} bypasses a failed '{GATE}'")
    if continues(job.get("continue-on-error")):
        errors.append(f"job '{name}' has continue-on-error: {job.get('continue-on-error')!r}")
for index, step in enumerate(jobs[GATE].get("steps") or []):
    if continues(step.get("continue-on-error")):
        errors.append(f"job '{GATE}' step {index} has continue-on-error: {step.get('continue-on-error')!r}")

if errors:
    sys.exit("\n".join(f"{path}: {e}" for e in errors))
print(f"release gate OK: all {len(jobs) - 1} release.yml jobs transitively need {GATE}")
