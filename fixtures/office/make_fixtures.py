# SPDX-License-Identifier: MIT OR Apache-2.0
#!/usr/bin/env python3
"""Writes the Office fixtures from independent writers (openpyxl, zipfile).

The Rust tests read these files. They are generated here, not by tpt-codex, so the reader is
checked against files it did not write.

Run from the repository root:
    python fixtures/office/make_fixtures.py
Requires: openpyxl (3.x).
"""

import pathlib
import zipfile

import openpyxl

HERE = pathlib.Path(__file__).resolve().parent


def make_basic_xlsx() -> None:
    wb = openpyxl.Workbook()
    ws = wb.active
    ws.title = "Data"
    ws["A1"] = 21
    ws["B1"] = "=A1*2"
    ws["C1"] = "hello & <world>"
    ws["D1"] = True
    ws["A2"] = 0.5
    ws["B2"] = "=A2+A1"
    ws["A3"] = "text with   spaces"
    ws["A10"] = 7
    ws.merge_cells("A4:B4")
    ws["A4"] = "merged"
    other = wb.create_sheet("Other")
    other["A1"] = "second sheet"
    wb.save(HERE / "basic.xlsx")


def make_zips() -> None:
    entries = {
        "hello.txt": b"hello, stored world\n" * 4,
        "dir/deflated.txt": b"abc" * 500,
    }
    with zipfile.ZipFile(HERE / "stored_and_deflated.zip", "w") as z:
        z.writestr(zipfile.ZipInfo("hello.txt"), entries["hello.txt"], compress_type=zipfile.ZIP_STORED)
        z.writestr(zipfile.ZipInfo("dir/deflated.txt"), entries["dir/deflated.txt"], compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(HERE / "bzip2.zip", "w") as z:
        z.writestr("data.bin", b"x" * 100, compress_type=zipfile.ZIP_BZIP2)


if __name__ == "__main__":
    make_basic_xlsx()
    make_zips()
    print("wrote", sorted(p.name for p in HERE.iterdir() if p.suffix in (".xlsx", ".zip")))
