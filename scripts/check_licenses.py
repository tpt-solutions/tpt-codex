#!/usr/bin/env python3
"""Strict licensing gate for tpt-codex.

Reads `cargo metadata` JSON and fails if:
  * any package's license expression cannot be satisfied by the allow-list
    (an Apache-2.0-only crate is therefore rejected: Apache-2.0 is not in the
    allow-list, and an "X OR Apache-2.0" expression is satisfied via X), or
  * any `-sys` crate appears in the dependency closure of a core/view crate.

Usage:
    cargo metadata --format-version 1 | python3 scripts/check_licenses.py
    python3 scripts/check_licenses.py --metadata path/to/metadata.json
"""

from __future__ import annotations

import argparse
import json
import sys

ALLOWED = {
    "MIT",
    "0BSD",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Unicode-DFS-2016",
    "Unicode-3.0",
    "Zlib",
}

# Crates whose dependency closure must not contain any `-sys` crate.
NO_SYS_CRATES = {
    "tpt-codex-core",
    "tpt-codex-views",
    "tpt-codex-markdown",
    "tpt-typeset",
}


def license_ok(expr: str) -> bool:
    """Return True if the SPDX-ish expression is satisfiable by ALLOWED.

    Supports AND (all clauses must pass) and OR (any alternative passes),
    with optional parentheses around sub-expressions.
    """
    # Older crates write "MIT/Apache-2.0" or "Apache-2.0 / MIT"; "/" means OR.
    expr = expr.replace("/", " OR ").strip()
    while expr.startswith("(") and expr.endswith(")") and _balanced(expr[1:-1]):
        expr = expr[1:-1].strip()
    and_parts = _split_top(expr, " AND ")
    if len(and_parts) > 1:
        return all(license_ok(p) for p in and_parts)
    or_parts = _split_top(expr, " OR ")
    if len(or_parts) > 1:
        return any(license_ok(p) for p in or_parts)
    # Leaf: a single license identifier (possibly with WITH exception).
    if " WITH " in expr:
        return False  # exceptions are an open question; reject until decided
    return expr in ALLOWED


def _balanced(s: str) -> bool:
    depth = 0
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth < 0:
                return False
    return depth == 0


def _split_top(expr: str, sep: str) -> list[str]:
    """Split on `sep` only at parenthesis depth 0."""
    parts, depth, start, i = [], 0, 0, 0
    while i < len(expr):
        ch = expr[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        elif depth == 0 and expr.startswith(sep, i):
            parts.append(expr[start:i].strip())
            i += len(sep)
            start = i
            continue
        i += 1
    parts.append(expr[start:].strip())
    return parts


def is_normal_edge(dep: dict) -> bool:
    """True unless the edge is dev-only. Missing dep_kinds counts as normal."""
    kinds = dep.get("dep_kinds")
    if not kinds:
        return True
    return any(k.get("kind") != "dev" for k in kinds)


def check(metadata: dict) -> list[str]:
    errors: list[str] = []
    packages = {p["id"]: p for p in metadata["packages"]}

    for pkg in metadata["packages"]:
        lic = pkg.get("license")
        if not lic:
            errors.append(f"{pkg['name']} {pkg['version']}: no license declared")
        elif not license_ok(lic):
            errors.append(
                f"{pkg['name']} {pkg['version']}: license '{lic}' is not allowed"
            )

    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    roots = [p["id"] for p in metadata["packages"] if p["name"] in NO_SYS_CRATES]
    for root in roots:
        seen, stack = set(), [root]
        while stack:
            node_id = stack.pop()
            if node_id in seen:
                continue
            seen.add(node_id)
            for dep in nodes.get(node_id, {}).get("deps", []):
                # Dev-only edges don't ship, so they don't count toward the C-FFI rule.
                if is_normal_edge(dep):
                    stack.append(dep["pkg"])
        for node_id in seen:
            name = packages[node_id]["name"]
            if name.endswith("-sys"):
                errors.append(
                    f"{name}: -sys crate is in the dependency closure of "
                    f"{packages[root]['name']}"
                )
    return sorted(set(errors))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--metadata", help="Path to cargo metadata JSON (default: stdin)")
    args = parser.parse_args(argv)
    if args.metadata:
        with open(args.metadata, encoding="utf-8") as fh:
            metadata = json.load(fh)
    else:
        metadata = json.load(sys.stdin)
    errors = check(metadata)
    for err in errors:
        print(f"license-gate: {err}", file=sys.stderr)
    if errors:
        print(f"license-gate: FAILED ({len(errors)} problem(s))", file=sys.stderr)
        return 1
    print("license-gate: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
