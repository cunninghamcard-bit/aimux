#!/usr/bin/env python3
"""Validate quality/artifacts.json and its mapping to .github/workflows jobs.

Mapped artifacts must name an existing job (and matrix value); artifacts
marked 'unregistered' are printed but do not fail (PHASES 1.9 / 1.9b).
"""
import argparse, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import budget_lib as bl

ap = argparse.ArgumentParser()
ap.add_argument("--manifest", default="quality/artifacts.json")
ap.add_argument("--budgets", default="quality/size-budgets.json")
ap.add_argument("--root", default=".")
a = ap.parse_args()
m = bl.validate_manifest(bl.load_json(a.manifest))
bl.validate_budgets(bl.load_json(a.budgets), m)
for aid, status, where in bl.mapping_rows(m, a.root) if not bl.check_inventory(m, a.root) else []:
    print(f"{status:12} {aid}  {where}")
problems = bl.check_inventory(m, a.root)
for p in problems:
    print("ERROR", p, file=sys.stderr)
sys.exit(1 if problems else 0)
