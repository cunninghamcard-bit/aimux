#!/usr/bin/env python3
"""Fail unless every job in .github/workflows/release.yml transitively
`needs` the check-toolchain job (which runs check_toolchain_pin.sh).

Stdlib only, so it runs on any runner without installs. It reads the
top-level `jobs:` mapping (job keys at 2-space indent) and each job's
`needs:` at 4-space indent, in scalar, flow-list or block-list form.
"""
import re
import sys
from pathlib import Path

GATE = "check-toolchain"
path = Path(sys.argv[1] if len(sys.argv) > 1 else
            Path(__file__).resolve().parent.parent / ".github/workflows/release.yml")

jobs: dict[str, list[str]] = {}
in_jobs = False
job = None
block_needs = False
for raw in path.read_text().splitlines():
    line = raw.split(" #", 1)[0].rstrip() if not raw.lstrip().startswith("#") else ""
    if not line.strip():
        continue
    if not line.startswith(" "):
        in_jobs = line == "jobs:"
        job = None
        continue
    if not in_jobs:
        continue
    if m := re.fullmatch(r"  ([A-Za-z0-9_-]+):", line):
        job = m.group(1)
        jobs[job] = []
        block_needs = False
        continue
    if job is None:
        continue
    if m := re.fullmatch(r"    needs:\s*(.*)", line):
        value = m.group(1).strip()
        block_needs = value == ""
        if value.startswith("["):
            jobs[job] += [v.strip().strip("'\"") for v in value.strip("[]").split(",") if v.strip()]
        elif value:
            jobs[job].append(value.strip("'\""))
        continue
    if block_needs and (m := re.fullmatch(r"\s{6,}-\s*(\S+)", line)):
        jobs[job].append(m.group(1).strip("'\""))
        continue
    if re.match(r"    \S", line):
        block_needs = False

if not jobs:
    sys.exit(f"{path}: no jobs found")
if GATE not in jobs:
    sys.exit(f"{path}: job '{GATE}' is missing")


def gated(name: str, seen: frozenset = frozenset()) -> bool:
    return any(n == GATE or (n in jobs and n not in seen and gated(n, seen | {name}))
               for n in jobs[name])


ungated = [j for j in jobs if j != GATE and not gated(j)]
if ungated:
    sys.exit(f"{path}: UNGATED jobs (do not transitively need '{GATE}'): {', '.join(ungated)}")
print(f"release gate OK: all {len(jobs) - 1} release.yml jobs transitively need {GATE}")
