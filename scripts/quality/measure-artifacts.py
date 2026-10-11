#!/usr/bin/env python3
"""Measure staged release artifacts (RFC-0038a §9.1)."""
import argparse, json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import budget_lib as bl

ap = argparse.ArgumentParser()
ap.add_argument("--manifest", required=True)
ap.add_argument("--input", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()
m = bl.validate_manifest(bl.load_json(a.manifest))
Path(a.out).write_text(json.dumps(bl.measure(m, a.input), indent=2) + "\n")
