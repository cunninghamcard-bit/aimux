"""Q0 size gate (RFC-0038a §3, §9.1): inventory, measurer and comparator.

Pure standard library; all byte values are integers. No floating point is
used when deriving H, so ceil(B * 1.10) is exact.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import tarfile
import zipfile
from pathlib import Path

SCHEMA_VERSION = 1
METRICS = ("raw", "packed", "unpacked")
STATUS_MAPPED = "mapped"
STATUS_UNREGISTERED = "unregistered"

# Failure categories (RFC-0038a §3.2, §9.4).
BUDGET_EXCEEDED = "BUDGET_EXCEEDED"
BUDGET_UNREGISTERED = "BUDGET_UNREGISTERED"
ARTIFACT_MISSING = "ARTIFACT_MISSING"
BUDGET_INVALID = "BUDGET_INVALID"


class SchemaError(ValueError):
    pass


# ── integer helpers ──────────────────────────────────────────────────────────


def _require_bytes(value, where: str) -> int:
    # bool is an int subclass; reject it along with floats and negatives.
    if isinstance(value, bool) or not isinstance(value, int):
        raise SchemaError(f"{where}: byte count must be an integer, got {value!r}")
    if value < 0:
        raise SchemaError(f"{where}: byte count must be non-negative, got {value}")
    return value


def hard_cap(baseline: int, product_cap: int) -> int:
    """H = min(C, ceil(B × 1.10)) — RFC-0038a §3.2, integer-exact."""
    _require_bytes(baseline, "B")
    _require_bytes(product_cap, "C")
    return min(product_cap, (baseline * 11 + 9) // 10)


def passes(actual: int, cap: int) -> bool:
    """Normal CI compares only actual <= H (RFC-0038a §3.2)."""
    return _require_bytes(actual, "actual") <= _require_bytes(cap, "H")


# ── strict JSON loading ──────────────────────────────────────────────────────


def _no_dup_keys(pairs):
    out = {}
    for k, v in pairs:
        if k in out:
            raise SchemaError(f"duplicate JSON key {k!r}")
        out[k] = v
    return out


def _reject_float(text):
    raise SchemaError(f"floating point number {text} is not allowed")


def load_json(path) -> dict:
    with open(path, encoding="utf-8") as fh:
        return json.load(
            fh, object_pairs_hook=_no_dup_keys, parse_float=_reject_float
        )


def _check_keys(obj, required, optional, where):
    if not isinstance(obj, dict):
        raise SchemaError(f"{where}: expected an object")
    missing = [k for k in required if k not in obj]
    if missing:
        raise SchemaError(f"{where}: missing field(s) {missing}")
    unknown = sorted(set(obj) - set(required) - set(optional))
    if unknown:
        raise SchemaError(f"{where}: unknown field(s) {unknown}")


def _check_schema_version(doc, where):
    if doc.get("schema_version") != SCHEMA_VERSION:
        raise SchemaError(f"{where}: schema_version must be {SCHEMA_VERSION}")


ID_PARTS = ("package", "kind", "target", "feature_set", "build_variant")


def artifact_id(entry: dict) -> str:
    return "/".join(entry[p] for p in ID_PARTS)


def validate_manifest(doc: dict) -> dict:
    _check_keys(doc, ["schema_version", "artifacts"], [], "manifest")
    _check_schema_version(doc, "manifest")
    seen = set()
    for i, a in enumerate(doc["artifacts"]):
        where = f"artifacts[{i}]"
        _check_keys(
            a,
            [*ID_PARTS, "metrics", "status"],
            ["path", "workflow", "reason", "note"],
            where,
        )
        for p in ID_PARTS:
            if not isinstance(a[p], str) or not a[p] or "/" in a[p]:
                raise SchemaError(f"{where}.{p}: must be a non-empty string without '/'")
            if "*" in a[p]:
                raise SchemaError(f"{where}.{p}: unconstrained glob is not allowed")
        aid = artifact_id(a)
        if aid in seen:
            raise SchemaError(f"{where}: duplicate artifact id {aid}")
        seen.add(aid)
        m = a["metrics"]
        if not isinstance(m, dict) or not m:
            raise SchemaError(f"{where}.metrics: must be a non-empty object")
        for metric, spec in m.items():
            if metric not in METRICS:
                raise SchemaError(f"{where}.metrics: unknown metric {metric!r}")
            _check_keys(spec, ["product_cap_bytes"], [], f"{where}.metrics.{metric}")
            _require_bytes(spec["product_cap_bytes"], f"{aid}:{metric}:C")
        if a["status"] == STATUS_MAPPED:
            wf = a.get("workflow")
            _check_keys(wf, ["file", "job"], ["matrix"], f"{where}.workflow")
            if "reason" in a:
                raise SchemaError(f"{where}: mapped artifact must not carry 'reason'")
        elif a["status"] == STATUS_UNREGISTERED:
            if "workflow" in a:
                raise SchemaError(f"{where}: unregistered artifact must not carry 'workflow'")
            if not a.get("reason"):
                raise SchemaError(f"{where}: unregistered artifact needs a 'reason'")
        else:
            raise SchemaError(f"{where}.status: must be mapped or unregistered")
        if "path" in a and "*" in a["path"]:
            raise SchemaError(f"{where}.path: unconstrained glob is not allowed")
    return doc


def validate_budgets(doc: dict, manifest: dict) -> dict:
    _check_keys(doc, ["schema_version", "budgets"], [], "budgets")
    _check_schema_version(doc, "budgets")
    caps = {
        (artifact_id(a), metric): spec["product_cap_bytes"]
        for a in manifest["artifacts"]
        for metric, spec in a["metrics"].items()
    }
    seen = set()
    for i, b in enumerate(doc["budgets"]):
        where = f"budgets[{i}]"
        _check_keys(
            b,
            ["artifact_id", "metric", "baseline_bytes", "hard_cap_bytes", "product_cap_bytes", "evidence"],
            ["approved_at", "source_sha"],
            where,
        )
        key = (b["artifact_id"], b["metric"])
        if key in seen:
            raise SchemaError(f"{where}: duplicate budget {key}")
        seen.add(key)
        if key not in caps:
            raise SchemaError(f"{where}: {key} is not in the artifact manifest")
        B = _require_bytes(b["baseline_bytes"], f"{where}.B")
        H = _require_bytes(b["hard_cap_bytes"], f"{where}.H")
        C = _require_bytes(b["product_cap_bytes"], f"{where}.C")
        if C != caps[key]:
            raise SchemaError(f"{where}: C={C} disagrees with manifest C={caps[key]}")
        if B > C:
            raise SchemaError(f"{where}: B={B} > C={C}; cannot register as green")
        if H != hard_cap(B, C):
            raise SchemaError(
                f"{where}: stored H={H} != min(C, ceil(B*1.10))={hard_cap(B, C)}"
            )
    return doc


# ── workflow inventory mapping ───────────────────────────────────────────────

_JOB_RE = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")


def workflow_jobs(text: str) -> dict:
    """Map top-level job id -> (1-based line, job block text).

    Minimal parser for the repository's own workflow layout (two-space job
    keys under `jobs:`); avoids a PyYAML dependency.
    """
    lines = text.splitlines()
    try:
        start = next(i for i, l in enumerate(lines) if l.rstrip() == "jobs:")
    except StopIteration:
        return {}
    jobs, cur, buf = {}, None, []
    for i in range(start + 1, len(lines)):
        line = lines[i]
        m = _JOB_RE.match(line)
        if m:
            if cur:
                jobs[cur[0]] = (cur[1], "\n".join(buf))
            cur, buf = (m.group(1), i + 1), []
        elif line and not line.startswith(" ") and not line.startswith("#"):
            break
        else:
            buf.append(line)
    if cur:
        jobs[cur[0]] = (cur[1], "\n".join(buf))
    return jobs


def check_inventory(manifest: dict, repo_root) -> list:
    """Return a list of problems with workflow mappings (empty = OK).

    Unregistered artifacts are reported, never failed (PHASES 1.9 correction).
    """
    root = Path(repo_root)
    cache, problems = {}, []
    for a in manifest["artifacts"]:
        if a["status"] != STATUS_MAPPED:
            continue
        wf = a["workflow"]
        if wf["file"] not in cache:
            p = root / wf["file"]
            cache[wf["file"]] = workflow_jobs(p.read_text()) if p.exists() else None
        jobs = cache[wf["file"]]
        aid = artifact_id(a)
        if jobs is None:
            problems.append(f"{aid}: workflow file {wf['file']} not found")
            continue
        if wf["job"] not in jobs:
            problems.append(f"{aid}: job {wf['job']!r} not in {wf['file']}")
            continue
        body = jobs[wf["job"]][1]
        for k, v in (wf.get("matrix") or {}).items():
            if not re.search(rf"\b{re.escape(k)}:\s*\[?.*\b{re.escape(v)}\b", body):
                problems.append(f"{aid}: matrix {k}={v} not found in job {wf['job']!r}")
    return problems


def mapping_rows(manifest: dict, repo_root):
    """(artifact_id, status, file:line or reason) for every artifact."""
    root = Path(repo_root)
    cache, rows = {}, []
    for a in manifest["artifacts"]:
        aid = artifact_id(a)
        if a["status"] == STATUS_UNREGISTERED:
            rows.append((aid, STATUS_UNREGISTERED, a["reason"]))
            continue
        wf = a["workflow"]
        if wf["file"] not in cache:
            cache[wf["file"]] = workflow_jobs((root / wf["file"]).read_text())
        line = cache[wf["file"]][wf["job"]][0]
        mx = ",".join(f"{k}={v}" for k, v in (wf.get("matrix") or {}).items())
        rows.append((aid, STATUS_MAPPED, f"{wf['file']}:{line} {wf['job']}" + (f" [{mx}]" if mx else "")))
    return rows


# ── measurer ─────────────────────────────────────────────────────────────────


def _unpacked_bytes(path: Path) -> int:
    name = path.name
    total, seen = 0, set()

    def member(mname, size, is_file, is_dir):
        nonlocal total
        norm = os.path.normpath(mname)
        if mname.startswith("/") or norm == ".." or norm.startswith("../"):
            raise SchemaError(f"{path}: path traversal member {mname!r}")
        if is_dir:
            return
        if not is_file:
            raise SchemaError(f"{path}: unsupported member type {mname!r}")
        if norm in seen:
            raise SchemaError(f"{path}: duplicate member path {mname!r}")
        seen.add(norm)
        total += size

    if name.endswith((".tgz", ".tar.gz", ".tar")):
        with tarfile.open(path) as tf:
            for m in tf.getmembers():
                member(m.name, m.size, m.isfile(), m.isdir())
    elif name.endswith((".whl", ".jar", ".zip")):
        with zipfile.ZipFile(path) as zf:
            for info in zf.infolist():
                is_link = (info.external_attr >> 16) & 0o170000 == 0o120000
                member(info.filename, info.file_size, not info.is_dir() and not is_link, info.is_dir())
    elif path.is_dir():
        for p in sorted(path.rglob("*")):
            rel = p.relative_to(path).as_posix()
            if p.is_symlink():
                raise SchemaError(f"{path}: symlink member {rel!r}")
            member(rel, p.stat().st_size if p.is_file() else 0, p.is_file(), p.is_dir())
    else:
        raise SchemaError(f"{path}: cannot compute unpacked bytes for this format")
    return total


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def measure(manifest: dict, staging) -> dict:
    """Measure every mapped artifact with a `path` present under staging."""
    staging = Path(staging)
    out = []
    for a in manifest["artifacts"]:
        if a["status"] != STATUS_MAPPED or "path" not in a:
            continue
        p = staging / a["path"]
        if not p.exists():
            continue
        rec = {"artifact_id": artifact_id(a), "file": a["path"], "metrics": {}}
        if p.is_file():
            rec["sha256"] = _sha256(p)
        for metric in a["metrics"]:
            if metric in ("raw", "packed"):
                if p.is_dir():
                    raise SchemaError(f"{p}: {metric} needs a file")
                rec["metrics"][metric] = p.stat().st_size
            else:
                rec["metrics"][metric] = _unpacked_bytes(p)
        out.append(rec)
    return {"schema_version": SCHEMA_VERSION, "measurements": out}


# ── comparator ───────────────────────────────────────────────────────────────


def compare(manifest: dict, budgets: dict, report: dict) -> dict:
    """Return {'failures': [...], 'passed': [...], 'unregistered': [...]}.

    - mapped artifact, budget present, measured: pass iff actual <= H
    - mapped artifact, budget present, not measured: ARTIFACT_MISSING
    - mapped artifact, no budget: BUDGET_UNREGISTERED
    - inventory status 'unregistered': listed, never a failure (1.9b)
    """
    measured = {}
    for r in report["measurements"]:
        for metric, v in r["metrics"].items():
            measured[(r["artifact_id"], metric)] = _require_bytes(v, r["artifact_id"])
    bud = {(b["artifact_id"], b["metric"]): b for b in budgets["budgets"]}
    failures, passed, unregistered = [], [], []
    for a in manifest["artifacts"]:
        aid = artifact_id(a)
        if a["status"] == STATUS_UNREGISTERED:
            unregistered.append({"artifact_id": aid, "reason": a["reason"]})
            continue
        for metric in a["metrics"]:
            key = (aid, metric)
            b = bud.get(key)
            if b is None:
                failures.append({"artifact_id": aid, "metric": metric, "category": BUDGET_UNREGISTERED})
                continue
            H = b["hard_cap_bytes"]
            if H != hard_cap(b["baseline_bytes"], b["product_cap_bytes"]):
                failures.append({"artifact_id": aid, "metric": metric, "category": BUDGET_INVALID})
                continue
            if key not in measured:
                failures.append({"artifact_id": aid, "metric": metric, "category": ARTIFACT_MISSING, "limit": H})
                continue
            actual = measured[key]
            rec = {"artifact_id": aid, "metric": metric, "actual": actual, "limit": H, "delta": actual - H}
            if passes(actual, H):
                passed.append(rec)
            else:
                failures.append({**rec, "category": BUDGET_EXCEEDED})
    return {"failures": failures, "passed": passed, "unregistered": unregistered}
