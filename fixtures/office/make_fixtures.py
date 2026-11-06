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

import base64

import docx
import openpyxl
from pptx import Presentation
from pptx.util import Inches

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


# A 1x1 PNG, so the document has a real image part.
PNG_1X1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="
)


def make_docx() -> None:
    doc = docx.Document()
    doc.core_properties.title = "Field Notes"
    doc.core_properties.author = "A. Author"
    doc.add_heading("Introduction", level=1)
    doc.add_paragraph("Plain paragraph with two lines.")
    doc.paragraphs[-1].add_run().add_break()
    doc.paragraphs[-1].add_run("Second line.")
    mixed = doc.add_paragraph()
    mixed.add_run("bold ").bold = True
    mixed.add_run("then plain")
    doc.add_heading("Details", level=2)
    doc.add_paragraph("first bullet", style="List Bullet")
    doc.add_paragraph("second bullet", style="List Bullet")
    table = doc.add_table(rows=2, cols=2)
    table.cell(0, 0).text = "cell A1"
    table.cell(0, 1).text = "cell B1"
    table.cell(1, 0).text = "cell A2"
    table.cell(1, 1).text = "cell B2"
    doc.add_picture(str(HERE / "_pixel.png"))
    doc.add_paragraph("Final paragraph & <end>.")
    doc.save(HERE / "basic.docx")


def make_pptx() -> None:
    prs = Presentation()
    slide = prs.slides.add_slide(prs.slide_layouts[1])
    slide.shapes.title.text = "Agenda"
    body = slide.placeholders[1].text_frame
    body.text = "First point"
    body.add_paragraph().text = "Second point"
    slide.notes_slide.notes_text_frame.text = "speaker note text"
    slide2 = prs.slides.add_slide(prs.slide_layouts[5])
    slide2.shapes.title.text = "Figures"
    box = slide2.shapes.add_textbox(Inches(1), Inches(2), Inches(4), Inches(1))
    box.text_frame.text = "free text box & <ok>"
    prs.save(HERE / "basic.pptx")


def make_corpus_xlsx() -> None:
    """Three worksheets: formulas across sheets, a merged range, unicode text, a percent
    format and an empty sheet. Expected values are asserted in tests/corpus.rs."""
    wb = openpyxl.Workbook()
    costs = wb.active
    costs.title = "Costs"
    costs["A1"] = "Item"
    costs["B1"] = "Qty"
    costs["C1"] = "Cost"
    costs["A2"] = "café"
    costs["B2"] = 4
    costs["C2"] = "=B2*Rates!A1"
    costs["A3"] = "tax"
    costs["B3"] = 0.15
    costs["C3"] = "=C2*B3"
    costs["C3"].number_format = "0.0%"
    costs["D2"] = True
    costs.merge_cells("A5:C5")
    costs["A5"] = "merged title"
    rates = wb.create_sheet("Rates")
    rates["A1"] = 2.5
    rates["B2"] = "rate note"
    wb.create_sheet("Empty")
    wb.save(HERE / "corpus_workbook.xlsx")


def make_corpus_docx() -> None:
    """Styled runs, a three-column table, bullets and a numbered list. Expected values are
    asserted in tests/corpus.rs."""
    doc = docx.Document()
    doc.add_heading("Corpus report", level=1)
    styled = doc.add_paragraph()
    styled.add_run("bold ").bold = True
    styled.add_run("italic ").italic = True
    styled.add_run("bold italic").bold = True
    styled.runs[-1].italic = True
    styled.add_run(" plain tail")
    doc.add_heading("Table", level=3)
    table = doc.add_table(rows=2, cols=3)
    for r, row in enumerate([["name", "unit", "price"], ["widget", "each", "2.50"]]):
        for c, text in enumerate(row):
            table.cell(r, c).text = text
    doc.add_paragraph("bullet one", style="List Bullet")
    doc.add_paragraph("numbered one", style="List Number")
    doc.add_paragraph("unicode: café — ünïcode & <tags>")
    doc.save(HERE / "corpus_report.docx")


if __name__ == "__main__":
    (HERE / "_pixel.png").write_bytes(PNG_1X1)
    make_basic_xlsx()
    make_zips()
    make_docx()
    make_pptx()
    make_corpus_xlsx()
    make_corpus_docx()
    (HERE / "_pixel.png").unlink()
    print("wrote", sorted(p.name for p in HERE.iterdir() if p.suffix in (".xlsx", ".zip", ".docx", ".pptx")))
