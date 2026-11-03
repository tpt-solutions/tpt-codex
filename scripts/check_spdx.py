#!/usr/bin/env python3
"""Fails if any tracked Rust source file lacks the SPDX header.

Required first line of every .rs file:  // SPDX-License-Identifier: MIT OR Apache-2.0
Usage: python3 scripts/check_spdx.py [root]
"""

import pathlib
import sys

HEADER = "// SPDX-License-Identifier: MIT OR Apache-2.0"
SKIP_DIRS = {"target", ".git"}


def missing(root: pathlib.Path) -> list[pathlib.Path]:
    bad = []
    for path in sorted(root.rglob("*.rs")):
        if SKIP_DIRS & set(path.parts):
            continue
        lines = path.read_text(encoding="utf-8").splitlines()
        if not lines or lines[0].strip() != HEADER:
            bad.append(path)
    return bad


def main(argv: list[str]) -> int:
    root = pathlib.Path(argv[1] if len(argv) > 1 else ".")
    bad = missing(root)
    for path in bad:
        print(f"spdx: missing header: {path}", file=sys.stderr)
    if bad:
        return 1
    print("spdx: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
