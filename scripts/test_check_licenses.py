"""Fixture tests for scripts/check_licenses.py. Run: python3 -m unittest scripts/test_check_licenses.py"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import check_licenses as cl  # noqa: E402


def pkg(name, lic, pid=None):
    pid = pid or name
    return {"id": pid, "name": name, "version": "1.0.0", "license": lic}


def meta(packages, edges=None):
    edges = edges or {}
    nodes = [
        {"id": p["id"], "deps": [{"pkg": d} for d in edges.get(p["id"], [])]}
        for p in packages
    ]
    return {"packages": packages, "resolve": {"nodes": nodes}}


class LicenseExpressionTests(unittest.TestCase):
    def test_permissive_licenses_pass(self):
        for lic in ["MIT", "0BSD", "BSD-2-Clause", "BSD-3-Clause", "Unicode-DFS-2016", "Unicode-3.0", "Zlib"]:
            self.assertTrue(cl.license_ok(lic), lic)

    def test_apache_only_is_rejected(self):
        self.assertFalse(cl.license_ok("Apache-2.0"))

    def test_dual_license_passes_via_mit(self):
        self.assertTrue(cl.license_ok("MIT OR Apache-2.0"))
        self.assertTrue(cl.license_ok("Apache-2.0 OR MIT"))

    def test_and_requires_all_clauses(self):
        self.assertTrue(cl.license_ok("MIT AND Zlib"))
        self.assertFalse(cl.license_ok("MIT AND Apache-2.0"))

    def test_parenthesised_expressions(self):
        self.assertTrue(cl.license_ok("(MIT OR Apache-2.0) AND Unicode-DFS-2016"))
        self.assertTrue(cl.license_ok("(MIT OR Apache-2.0) AND Unicode-3.0"))
        self.assertFalse(cl.license_ok("(Apache-2.0) AND MIT"))

    def test_copyleft_is_rejected(self):
        for lic in ["GPL-3.0", "AGPL-3.0", "MPL-2.0", "GPL-2.0 OR Apache-2.0"]:
            self.assertFalse(cl.license_ok(lic), lic)

    def test_exception_is_rejected_until_decided(self):
        self.assertFalse(cl.license_ok("Apache-2.0 WITH LLVM-exception"))


class GateTests(unittest.TestCase):
    def test_clean_graph_passes(self):
        m = meta([pkg("tpt-codex-core", "MIT OR Apache-2.0", "core"), pkg("serde", "MIT OR Apache-2.0", "serde")],
                 edges={"core": ["serde"]})
        self.assertEqual(cl.check(m), [])

    def test_apache_only_fixture_fails(self):
        m = meta([pkg("tpt-codex-core", "MIT OR Apache-2.0", "core"), pkg("apache-only", "Apache-2.0", "ao")],
                 edges={"core": ["ao"]})
        errors = cl.check(m)
        self.assertEqual(len(errors), 1)
        self.assertIn("apache-only", errors[0])

    def test_missing_license_fails(self):
        m = meta([pkg("mystery", None)])
        self.assertTrue(any("no license" in e for e in cl.check(m)))

    def test_sys_crate_under_core_fails(self):
        m = meta([pkg("tpt-codex-core", "MIT", "core"), pkg("libz-sys", "MIT", "z")],
                 edges={"core": ["z"]})
        self.assertTrue(any("libz-sys" in e for e in cl.check(m)))

    def test_sys_crate_outside_core_closure_is_ignored(self):
        m = meta([pkg("tpt-codex-core", "MIT", "core"), pkg("tpt-codex-app", "MIT", "app"),
                  pkg("libz-sys", "MIT", "z")],
                 edges={"app": ["z"]})
        self.assertEqual(cl.check(m), [])


if __name__ == "__main__":
    unittest.main()
