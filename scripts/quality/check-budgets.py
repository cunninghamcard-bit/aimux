#!/usr/bin/env python3
"""Compare a size report against registered budgets (RFC-0038a §3.2, §9.1).

Passes iff every registered metric has actual <= H. Inventory entries marked
'unregistered' are listed and never fail (PHASES 1.9; completion is 1.9b).
--report-only is the Q0 transitional mode (RFC-0038a §12): print, exit 0.
"""
import argparse, json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import budget_lib as bl

ap = argparse.ArgumentParser()
ap.add_argument("--manifest", default="quality/artifacts.json")
ap.add_argument("--budgets", default="quality/size-budgets.json")
ap.add_argument("--report", required=True)
ap.add_argument("--report-only", action="store_true")
a = ap.parse_args()
m = bl.validate_manifest(bl.load_json(a.manifest))
b = bl.validate_budgets(bl.load_json(a.budgets), m)
res = bl.compare(m, b, bl.load_json(a.report))
print(json.dumps(res, indent=2))
sys.exit(0 if a.report_only or not res["failures"] else 1)
