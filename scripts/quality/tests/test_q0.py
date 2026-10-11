"""Q0 negative and schema tests (RFC-0038a §3.2, §9.1; PHASES 1.9)."""

import io
import json
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import budget_lib as bl  # noqa: E402

REPO = HERE.parents[2]


def _manifest(cap, status="mapped", metric="raw"):
    a = {
        "package": "pkg", "kind": "bin", "target": "t", "feature_set": "default",
        "build_variant": "release", "metrics": {metric: {"product_cap_bytes": cap}},
        "status": status, "path": "f.bin",
    }
    if status == "mapped":
        a["workflow"] = {"file": ".github/workflows/release.yml", "job": "x"}
    else:
        a["reason"] = "not built by any workflow"
    return {"schema_version": 1, "artifacts": [a]}


def _budgets(B, C, H=None, metric="raw"):
    return {"schema_version": 1, "budgets": [{
        "artifact_id": "pkg/bin/t/default/release", "metric": metric,
        "baseline_bytes": B, "hard_cap_bytes": bl.hard_cap(B, C) if H is None else H,
        "product_cap_bytes": C, "evidence": "test",
    }]}


def _report(actual, metric="raw"):
    return {"schema_version": 1, "measurements": [
        {"artifact_id": "pkg/bin/t/default/release", "file": "f.bin", "metrics": {metric: actual}}
    ]}


def _run(B, C, actual):
    m = _manifest(C)
    b = bl.validate_budgets(_budgets(B, C), m)
    return bl.compare(m, b, _report(actual))


class HardCapFormula(unittest.TestCase):
    def test_ceil_is_integer_exact(self):
        self.assertEqual(bl.hard_cap(1000, 10**9), 1100)
        self.assertEqual(bl.hard_cap(1001, 10**9), 1102)  # 1101.1 -> 1102
        self.assertEqual(bl.hard_cap(1009, 10**9), 1110)  # 1109.9 -> 1110
        self.assertEqual(bl.hard_cap(0, 10), 0)
        # Large values where float 1.10 multiplication would round wrongly.
        B = 66_500_001
        self.assertEqual(bl.hard_cap(B, 10**12), -(-B * 11 // 10))

    def test_product_cap_wins_when_smaller(self):
        self.assertEqual(bl.hard_cap(1000, 1050), 1050)


class NegativeGate(unittest.TestCase):
    """PHASES 1.9 correction: actual == H passes, H + 1 is blocked."""

    def test_actual_equal_H_passes(self):
        B, C = 1000, 10**9
        H = bl.hard_cap(B, C)
        res = _run(B, C, H)
        self.assertEqual(res["failures"], [])
        self.assertEqual(res["passed"][0]["actual"], H)

    def test_actual_H_plus_one_blocked(self):
        B, C = 1000, 10**9
        H = bl.hard_cap(B, C)
        res = _run(B, C, H + 1)
        self.assertEqual(len(res["failures"]), 1)
        f = res["failures"][0]
        self.assertEqual(f["category"], bl.BUDGET_EXCEEDED)
        self.assertEqual((f["actual"], f["limit"], f["delta"]), (H + 1, H, 1))

    def test_rounded_ceiling_boundary(self):
        # B=1001: ceil(1101.1)=1102 passes; 1103 blocked.
        self.assertEqual(_run(1001, 10**9, 1102)["failures"], [])
        self.assertEqual(_run(1001, 10**9, 1103)["failures"][0]["category"], bl.BUDGET_EXCEEDED)

    def test_C_below_ceil_B_times_1_10_blocks(self):
        # C=1050 < ceil(1000*1.10)=1100 -> H=1050; 1051..1100 are blocked
        # even though they are within +10% of B.
        B, C = 1000, 1050
        self.assertEqual(bl.hard_cap(B, C), C)
        self.assertEqual(_run(B, C, 1050)["failures"], [])
        for actual in (1051, 1100):
            f = _run(B, C, actual)["failures"]
            self.assertEqual(f[0]["category"], bl.BUDGET_EXCEEDED, actual)
            self.assertEqual(f[0]["limit"], 1050)

    def test_stored_H_not_recomputed_from_candidate(self):
        # Comparator uses stored H; a tampered H is rejected at load.
        with self.assertRaises(bl.SchemaError):
            bl.validate_budgets(_budgets(1000, 10**9, H=1200), _manifest(10**9))

    def test_B_greater_than_C_cannot_register(self):
        with self.assertRaises(bl.SchemaError):
            bl.validate_budgets(_budgets(2000, 1000, H=1000), _manifest(1000))

    def test_missing_budget_is_unregistered_failure(self):
        m = _manifest(10)
        res = bl.compare(m, {"schema_version": 1, "budgets": []}, _report(1))
        self.assertEqual(res["failures"][0]["category"], bl.BUDGET_UNREGISTERED)

    def test_missing_measurement_is_artifact_missing(self):
        m = _manifest(10**9)
        b = _budgets(1000, 10**9)
        res = bl.compare(m, b, {"schema_version": 1, "measurements": []})
        self.assertEqual(res["failures"][0]["category"], bl.ARTIFACT_MISSING)

    def test_inventory_unregistered_never_fails(self):
        m = _manifest(10, status="unregistered")
        res = bl.compare(m, {"schema_version": 1, "budgets": []}, {"schema_version": 1, "measurements": []})
        self.assertEqual(res["failures"], [])
        self.assertEqual(res["unregistered"][0]["artifact_id"], "pkg/bin/t/default/release")


class StrictSchema(unittest.TestCase):
    def _load(self, text):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
            fh.write(text)
        return bl.load_json(fh.name)

    def test_rejects_float(self):
        with self.assertRaises(bl.SchemaError):
            self._load('{"a": 1.5}')

    def test_rejects_duplicate_key(self):
        with self.assertRaises(bl.SchemaError):
            self._load('{"a": 1, "a": 2}')

    def test_rejects_negative_and_bool(self):
        for v in (-1, True):
            m = _manifest(10)
            m["artifacts"][0]["metrics"]["raw"]["product_cap_bytes"] = v
            with self.assertRaises(bl.SchemaError):
                bl.validate_manifest(m)

    def test_rejects_unknown_key_and_dup_id_and_glob(self):
        m = _manifest(10)
        m["artifacts"][0]["extra"] = 1
        with self.assertRaises(bl.SchemaError):
            bl.validate_manifest(m)
        m = _manifest(10)
        m["artifacts"].append(dict(m["artifacts"][0]))
        with self.assertRaises(bl.SchemaError):
            bl.validate_manifest(m)
        m = _manifest(10)
        m["artifacts"][0]["target"] = "*"
        with self.assertRaises(bl.SchemaError):
            bl.validate_manifest(m)

    def test_unregistered_requires_reason(self):
        m = _manifest(10, status="unregistered")
        del m["artifacts"][0]["reason"]
        with self.assertRaises(bl.SchemaError):
            bl.validate_manifest(m)


class Measurer(unittest.TestCase):
    def test_raw_packed_unpacked(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            (d / "f.bin").write_bytes(b"x" * 123)
            with zipfile.ZipFile(d / "p.whl", "w") as z:
                z.writestr("a", b"1" * 10)
                z.writestr("b/c", b"2" * 5)
            m = _manifest(10**6)
            a2 = dict(m["artifacts"][0], kind="wheel", path="p.whl",
                      metrics={"packed": {"product_cap_bytes": 10**6}, "unpacked": {"product_cap_bytes": 10**6}})
            m["artifacts"].append(a2)
            rep = bl.measure(bl.validate_manifest(m), d)["measurements"]
            self.assertEqual(rep[0]["metrics"], {"raw": 123})
            self.assertEqual(rep[1]["metrics"]["unpacked"], 15)
            self.assertEqual(rep[1]["metrics"]["packed"], (d / "p.whl").stat().st_size)

    def test_rejects_traversal_and_duplicates(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "bad.tgz"
            with tarfile.open(p, "w:gz") as tf:
                ti = tarfile.TarInfo("../evil")
                ti.size = 1
                tf.addfile(ti, io.BytesIO(b"x"))
            with self.assertRaises(bl.SchemaError):
                bl._unpacked_bytes(p)
            p = Path(d) / "dup.tgz"
            with tarfile.open(p, "w:gz") as tf:
                for _ in range(2):
                    ti = tarfile.TarInfo("a")
                    ti.size = 1
                    tf.addfile(ti, io.BytesIO(b"x"))
            with self.assertRaises(bl.SchemaError):
                bl._unpacked_bytes(p)


class RepositoryInventory(unittest.TestCase):
    """The checked-in inventory maps onto real .github/workflows jobs."""

    def setUp(self):
        self.m = bl.validate_manifest(bl.load_json(REPO / "quality/artifacts.json"))
        bl.validate_budgets(bl.load_json(REPO / "quality/size-budgets.json"), self.m)

    def test_every_mapped_artifact_resolves_to_a_job(self):
        self.assertEqual(bl.check_inventory(self.m, REPO), [])

    def test_every_release_upload_or_publish_job_is_inventoried(self):
        jobs = bl.workflow_jobs((REPO / ".github/workflows/release.yml").read_text())
        producing = {j for j, (_, body) in jobs.items()
                     if "upload-artifact" in body or "publish" in body}
        producing.discard("github-release")  # aggregates the uploads above
        used = {a["workflow"]["job"] for a in self.m["artifacts"] if a["status"] == "mapped"}
        self.assertEqual(sorted(producing - used), [])

    def test_bad_job_name_is_reported(self):
        m = json.loads(json.dumps(self.m))
        m["artifacts"][0]["workflow"]["job"] = "no-such-job"
        self.assertTrue(bl.check_inventory(m, REPO))


if __name__ == "__main__":
    unittest.main()
