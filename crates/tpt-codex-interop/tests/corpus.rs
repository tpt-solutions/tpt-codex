// SPDX-License-Identifier: MIT OR Apache-2.0
//! Corpus tests: files written by openpyxl and python-docx (see `fixtures/office/make_fixtures.py`),
//! not by this crate. Each expected value below was read from the source file.

use tpt_codex_core::{Block, CellInput, Document};
use tpt_codex_interop::{Fidelity, import_docx, import_xlsx, import_xlsx_workbook};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../fixtures/office/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// The root container's children, in order, as blocks.
fn flow(doc: &Document) -> Vec<&Block> {
    match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => {
            children.iter().map(|id| doc.get(*id).unwrap()).collect()
        }
        other => panic!("root is not a container: {other:?}"),
    }
}

#[test]
fn workbook_keeps_every_worksheet_in_order() {
    let book = import_xlsx_workbook(&fixture("corpus_workbook.xlsx")).unwrap();
    let names: Vec<&str> = book.sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Costs", "Rates", "Empty"]);
    assert!(
        book.sheets[2].sheet.cells.is_empty(),
        "the empty sheet has no cells"
    );
}

#[test]
fn workbook_cells_keep_their_types_and_formulas() {
    let book = import_xlsx_workbook(&fixture("corpus_workbook.xlsx")).unwrap();
    let costs = &book.sheets[0].sheet.cells;
    assert_eq!(
        costs["A2"],
        CellInput::Text {
            value: "café".into()
        },
        "unicode text"
    );
    assert_eq!(costs["B2"], CellInput::Number { value: 4.0 });
    assert_eq!(costs["B3"], CellInput::Number { value: 0.15 });
    assert_eq!(costs["D2"], CellInput::Boolean { value: true });
    assert_eq!(
        costs["C3"],
        CellInput::Formula {
            source: "=C2*B3".into()
        },
        "same-sheet formula"
    );
    assert_eq!(
        costs["C2"],
        CellInput::Formula {
            source: "=B2*Rates!A1".into()
        },
        "cross-sheet formula is kept as written"
    );
    let rates = &book.sheets[1].sheet.cells;
    assert_eq!(rates["A1"], CellInput::Number { value: 2.5 });
}

#[test]
fn workbook_report_names_the_merged_range_and_the_cross_sheet_formula() {
    let book = import_xlsx_workbook(&fixture("corpus_workbook.xlsx")).unwrap();
    let items = book.report.items();
    assert!(
        items.iter().any(|i| i.location == "sheet \"Costs\""
            && i.feature == "merged cells"
            && i.outcome == Fidelity::Approximated),
        "merged range: {items:#?}"
    );
    assert!(
        items.iter().any(|i| i.location == "sheet \"Costs\"!C2"
            && i.feature == "formula"
            && i.outcome == Fidelity::Approximated
            && i.note.contains("another worksheet")),
        "cross-sheet formula: {items:#?}"
    );
    assert!(
        !items.iter().any(|i| i.location.contains("C3")),
        "a same-sheet formula is not reported"
    );
}

#[test]
fn first_sheet_import_reports_the_other_worksheets_as_dropped() {
    let import = import_xlsx(&fixture("corpus_workbook.xlsx")).unwrap();
    assert_eq!(
        import.sheet.cells.len(),
        11,
        "only Costs is imported: its eleven non-empty cells"
    );
    for name in ["Rates", "Empty"] {
        assert!(
            import.report.items().iter().any(
                |i| i.location == format!("sheet \"{name}\"") && i.outcome == Fidelity::Dropped
            ),
            "worksheet {name} is reported as dropped"
        );
    }
}

#[test]
fn docx_styled_runs_keep_their_bold_and_italic_spans() {
    let import = import_docx(&fixture("corpus_report.docx")).unwrap();
    let blocks = flow(&import.document);
    let styled = blocks
        .iter()
        .find_map(|b| match b {
            Block::Text { content, style } if content.starts_with("bold italic") => Some(style),
            _ => None,
        })
        .expect("styled paragraph");
    assert_eq!(styled.runs.len(), 3, "three styled runs: {styled:#?}");
    let spans: Vec<(usize, usize, bool, bool)> = styled
        .runs
        .iter()
        .map(|r| (r.start, r.end, r.bold, r.italic))
        .collect();
    // Spaces at the edge of a run are not part of it, so each span starts and ends on a letter.
    let text: Vec<char> = match blocks.iter().find_map(|b| match b {
        Block::Text { content, .. } if content.starts_with("bold italic") => Some(content),
        _ => None,
    }) {
        Some(content) => content.chars().collect(),
        None => unreachable!(),
    };
    for (start, end, _, _) in &spans {
        assert!(!text[*start].is_whitespace() && !text[*end - 1].is_whitespace());
    }
    assert_eq!(
        spans,
        [
            (0, 4, true, false),
            (5, 11, false, true),
            (12, 23, true, true)
        ]
    );
}

#[test]
fn docx_headings_and_text_come_in_document_order() {
    let import = import_docx(&fixture("corpus_report.docx")).unwrap();
    let texts: Vec<(Option<u8>, String)> = flow(&import.document)
        .iter()
        .filter_map(|b| match b {
            Block::Text { content, style } => Some((style.heading_level, content.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(texts[0], (Some(1), "Corpus report".to_string()));
    assert_eq!(texts[1].1, "bold italic bold italic plain tail");
    assert_eq!(texts[2], (Some(3), "Table".to_string()));
    // The two list paragraphs are items of two lists, so the text blocks left are the last one.
    let rest: Vec<&str> = texts[3..].iter().map(|(_, t)| t.as_str()).collect();
    assert_eq!(rest, ["unicode: café — ünïcode & <tags>"]);
    let lists: Vec<(bool, Vec<String>)> = flow(&import.document)
        .iter()
        .filter_map(|b| match b {
            Block::List { ordered, items, .. } => Some((
                *ordered,
                items
                    .iter()
                    .filter_map(|id| match import.document.get(*id) {
                        Some(Block::Text { content, .. }) => Some(content.clone()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        lists,
        [
            (false, vec!["bullet one".to_string()]),
            (true, vec!["numbered one".to_string()]),
        ],
        "the bulleted and numbered paragraphs are lists, bulleted or numbered from the numbering"
    );
    // The table's cells are in its grid: a number stays a number.
    let sheet = import.document.sheets.values().next().expect("a sheet");
    assert_eq!(
        sheet.cells.get("A1"),
        Some(&CellInput::Text {
            value: "name".into()
        })
    );
    assert_eq!(
        sheet.cells.get("C2"),
        Some(&CellInput::Number { value: 2.5 })
    );
}

#[test]
fn docx_report_has_no_table_item_when_nothing_is_merged() {
    let import = import_docx(&fixture("corpus_report.docx")).unwrap();
    let items = import.report.items();
    assert!(items.iter().all(|i| i.feature != "tables"), "{items:#?}");
    // Lists come through as lists, with no nesting to report.
    assert!(items.iter().all(|i| i.feature != "lists"), "{items:#?}");
}
