// SPDX-License-Identifier: MIT OR Apache-2.0
//! Office-format tests. The fixtures in `fixtures/office/` are written by openpyxl and Python's
//! `zipfile` (see `make_fixtures.py`), so these tests read files the in-house writer did not make.

use tpt_codex_core::{CellInput, Sheet};
use tpt_codex_interop::{Archive, ArchiveWriter, Fidelity, XlsxError, export_xlsx, import_xlsx};

const BASIC: &[u8] = include_bytes!("../../../fixtures/office/basic.xlsx");
const ZIP: &[u8] = include_bytes!("../../../fixtures/office/stored_and_deflated.zip");
const BZIP2_ZIP: &[u8] = include_bytes!("../../../fixtures/office/bzip2.zip");

fn num(v: f64) -> CellInput {
    CellInput::Number { value: v }
}

fn text(v: &str) -> CellInput {
    CellInput::Text { value: v.into() }
}

fn formula(src: &str) -> CellInput {
    CellInput::Formula { source: src.into() }
}

// ---- the archive reader, on files from Python's zipfile ----

#[test]
fn reads_stored_and_deflated_entries_from_python() {
    let archive = Archive::open(ZIP).unwrap();
    assert_eq!(
        archive.names().collect::<Vec<_>>(),
        ["hello.txt", "dir/deflated.txt"]
    );
    assert_eq!(
        archive.read("hello.txt").unwrap(),
        b"hello, stored world\n".repeat(4)
    );
    assert_eq!(
        archive.read("dir/deflated.txt").unwrap(),
        b"abc".repeat(500)
    );
}

#[test]
fn missing_entries_are_reported_by_name() {
    let archive = Archive::open(ZIP).unwrap();
    assert!(matches!(
        archive.read("nope.txt"),
        Err(tpt_codex_interop::ZipError::NotFound(n)) if n == "nope.txt"
    ));
}

#[test]
fn unsupported_compression_is_an_error_not_a_guess() {
    let err = Archive::open(BZIP2_ZIP).unwrap_err();
    assert!(err.to_string().contains("compression method 12"), "{err}");
}

#[test]
fn a_flipped_byte_fails_the_checksum() {
    let mut bytes = ZIP.to_vec();
    // The first entry's data starts after its 30-byte local header and 9-byte name.
    bytes[30 + 9] ^= 0x01;
    let archive = Archive::open(&bytes).unwrap();
    assert!(matches!(
        archive.read("hello.txt"),
        Err(tpt_codex_interop::ZipError::BadChecksum { .. })
    ));
}

#[test]
fn truncated_and_non_zip_inputs_are_errors() {
    assert!(Archive::open(&ZIP[..ZIP.len() - 30]).is_err());
    assert!(Archive::open(b"not a zip file at all").is_err());
    assert!(Archive::open(&[]).is_err());
}

#[test]
fn writer_rejects_unsafe_and_duplicate_names() {
    let mut w = ArchiveWriter::new();
    for bad in ["", "/abs.xml", "../escape.xml", "a/../b", "naïve.xml"] {
        assert!(w.add(bad, vec![]).is_err(), "{bad}");
    }
    w.add("ok.xml", vec![1]).unwrap();
    assert!(w.add("ok.xml", vec![2]).is_err());
}

#[test]
fn writer_output_reads_back_and_is_deterministic() {
    let build = || {
        let mut w = ArchiveWriter::new();
        w.add("a.txt", b"first".repeat(100)).unwrap();
        w.add("dir/b.txt", Vec::new()).unwrap();
        w.finish().unwrap()
    };
    let first = build();
    assert_eq!(first, build(), "same entries must give the same bytes");
    let archive = Archive::open(&first).unwrap();
    assert_eq!(archive.read("a.txt").unwrap(), b"first".repeat(100));
    assert_eq!(archive.read("dir/b.txt").unwrap(), b"");
}

// ---- xlsx import, on a workbook written by openpyxl ----

#[test]
fn imports_the_first_sheet_of_an_openpyxl_workbook() {
    let import = import_xlsx(BASIC).unwrap();
    let cells = &import.sheet.cells;
    assert_eq!(cells["A1"], num(21.0));
    assert_eq!(cells["B1"], formula("=A1*2"));
    assert_eq!(cells["C1"], text("hello & <world>"));
    assert_eq!(cells["D1"], CellInput::Boolean { value: true });
    assert_eq!(cells["A2"], num(0.5));
    assert_eq!(cells["B2"], formula("=A2+A1"));
    assert_eq!(cells["A3"], text("text with   spaces"));
    assert_eq!(cells["A10"], num(7.0));
    assert_eq!(cells["A4"], text("merged"));
}

#[test]
fn the_report_says_what_was_left_out() {
    let import = import_xlsx(BASIC).unwrap();
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "worksheet"
                && i.location == "sheet \"Other\""
                && i.outcome == Fidelity::Dropped)
    );
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "merged cells" && i.outcome == Fidelity::Approximated)
    );
    assert!(!import.report.is_lossless());
}

#[test]
fn not_an_xlsx_is_an_error() {
    assert!(import_xlsx(ZIP).is_err(), "a zip without workbook parts");
    assert!(import_xlsx(b"junk").is_err());
}

// ---- xlsx import, on a hand-built workbook: cases openpyxl does not produce ----

fn package(sheet_xml: &str, shared: Option<&str>) -> Vec<u8> {
    let mut w = ArchiveWriter::new();
    w.add(
        "xl/workbook.xml",
        br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets><definedNames><definedName name="x">S!$A$1</definedName></definedNames></workbook>"#
            .to_vec(),
    )
    .unwrap();
    w.add(
        "xl/_rels/workbook.xml.rels",
        br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="x" Target="worksheets/sheet1.xml"/></Relationships>"#
            .to_vec(),
    )
    .unwrap();
    w.add("xl/worksheets/sheet1.xml", sheet_xml.as_bytes().to_vec())
        .unwrap();
    if let Some(s) = shared {
        w.add("xl/sharedStrings.xml", s.as_bytes().to_vec())
            .unwrap();
    }
    w.finish().unwrap()
}

#[test]
fn shared_strings_styled_blanks_errors_and_shared_formulas() {
    let shared = r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>shared text</t></si><si><r><t>rich </t></r><r><t>text</t></r></si></sst>"#;
    let sheet = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
        <row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" s="3"/><c r="C1" t="e"><v>#DIV/0!</v></c><c r="D1"><f t="shared" si="0"/><v>5</v></c></row>
        <row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2" t="inlineStr"><is><t>inline</t></is></c><c r="C2"><f>1+2</f><v>3</v></c></row>
        </sheetData><mergeCells count="1"><mergeCell ref="A3:B3"/></mergeCells></worksheet>"#;
    let import = import_xlsx(&package(sheet, Some(shared))).unwrap();
    let cells = &import.sheet.cells;
    assert_eq!(cells["A1"], text("shared text"));
    assert_eq!(cells["A2"], text("rich text"));
    assert_eq!(cells["B2"], text("inline"));
    assert_eq!(cells["C2"], formula("=1+2"));
    assert_eq!(cells["D1"], num(5.0), "a shared formula keeps its value");
    assert!(
        !cells.contains_key("B1"),
        "styled blank cells are not content"
    );
    assert!(!cells.contains_key("C1"), "error values are dropped");

    let items = import.report.items();
    assert!(
        items
            .iter()
            .any(|i| i.location == "sheet \"S\"!C1" && i.outcome == Fidelity::Dropped)
    );
    assert!(items.iter().any(|i| i.location == "sheet \"S\"!D1"
        && i.outcome == Fidelity::Approximated
        && i.note.contains("shared")));
    assert!(
        items
            .iter()
            .any(|i| i.feature == "named ranges" && i.outcome == Fidelity::Dropped)
    );
    // Styled blank cells must not show up as losses.
    assert!(!items.iter().any(|i| i.location == "sheet \"S\"!B1"));
}

#[test]
fn malformed_sheet_xml_is_an_error() {
    let sheet = r#"<worksheet><sheetData><row r="1"><c r="A1"></sheetData></worksheet>"#;
    assert!(import_xlsx(&package(sheet, None)).is_err());
}

// ---- xlsx export, and the round trip through our own importer ----

fn sample_sheet() -> Sheet {
    let mut s = Sheet::default();
    s.cells.insert("A1".into(), num(21.0));
    s.cells.insert("A2".into(), num(0.1));
    s.cells.insert("B1".into(), formula("=A1 * 2"));
    s.cells.insert("B2".into(), formula("=A2 + A1"));
    s.cells.insert("C1".into(), text("a & b < c \"quoted\""));
    s.cells.insert("C2".into(), text("  padded  "));
    s.cells
        .insert("D1".into(), CellInput::Boolean { value: false });
    s.cells.insert("A10".into(), num(-3.25));
    s
}

#[test]
fn export_then_import_gives_the_same_cells() {
    let sheet = sample_sheet();
    let bytes = export_xlsx(&sheet).unwrap();
    let back = import_xlsx(&bytes).unwrap();
    assert_eq!(back.sheet, sheet);
    assert!(back.report.is_lossless(), "{}", back.report);
}

#[test]
fn exported_formulas_carry_their_computed_values() {
    let bytes = export_xlsx(&sample_sheet()).unwrap();
    let archive = Archive::open(&bytes).unwrap();
    let xml = String::from_utf8(archive.read("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    assert!(xml.contains("<f>A1 * 2</f><v>42</v>"), "{xml}");
    assert!(xml.contains(r#"<c r="C1" t="inlineStr"><is><t xml:space="preserve">a &amp; b &lt; c &quot;quoted&quot;</t></is></c>"#), "{xml}");
}

#[test]
fn export_is_deterministic_and_rows_come_out_in_order() {
    let a = export_xlsx(&sample_sheet()).unwrap();
    assert_eq!(a, export_xlsx(&sample_sheet()).unwrap());
    let archive = Archive::open(&a).unwrap();
    let xml = String::from_utf8(archive.read("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    // Row 10 must come after row 2, even though "A10" sorts before "A2" as text.
    let row2 = xml.find(r#"<row r="2">"#).unwrap();
    let row10 = xml.find(r#"<row r="10">"#).unwrap();
    assert!(row2 < row10);
}

#[test]
fn export_refuses_what_cannot_be_written() {
    let mut s = Sheet::default();
    s.cells.insert("A1".into(), text("bad \u{1} control"));
    assert!(export_xlsx(&s).is_err());

    let mut s = Sheet::default();
    s.cells.insert("A1".into(), num(f64::INFINITY));
    assert!(export_xlsx(&s).is_err());

    let mut s = Sheet::default();
    s.cells.insert("A1".into(), formula("="));
    assert!(export_xlsx(&s).is_err());
}

#[test]
fn an_empty_sheet_exports_and_imports() {
    let bytes = export_xlsx(&Sheet::default()).unwrap();
    assert!(import_xlsx(&bytes).unwrap().sheet.cells.is_empty());
}

#[test]
fn error_type_is_displayable() {
    let e: XlsxError = XlsxError("x".into());
    assert_eq!(e.to_string(), "xlsx: x");
}
