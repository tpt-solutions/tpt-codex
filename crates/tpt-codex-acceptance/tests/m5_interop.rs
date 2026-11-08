// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 5 (todo.md): "Open a real `.docx`/`.xlsx` and get a usable editable document."
//!
//! The files are the corpus written by openpyxl and python-docx (`fixtures/office`). The test
//! opens them, places every worksheet as a live grid, edits a cell, and writes the results back
//! out to check they read again.

mod common;

use common::office_fixture;
use tpt_codex_core::{Block, CellInput, Document, History, Op};
use tpt_codex_grid::{CellResult, evaluate_sheet};
use tpt_codex_interop::{
    export_docx, export_xlsx, import_docx, import_xlsx, import_xlsx_workbook, place_sheet,
};

/// The document a user starts from: the docx text, then one grid per worksheet.
fn open_both() -> (Document, History) {
    let report = import_docx(&office_fixture("corpus_report.docx")).unwrap();
    let mut doc = report.document;
    let mut history = History::new();
    let book = import_xlsx_workbook(&office_fixture("corpus_workbook.xlsx")).unwrap();
    let root = doc.root;
    let index = match doc.get(root) {
        Some(Block::Container { children, .. }) => children.len(),
        other => panic!("root is not a container: {other:?}"),
    };
    for (offset, worksheet) in book.sheets.into_iter().enumerate() {
        place_sheet(
            &mut doc,
            &mut history,
            worksheet.sheet,
            root,
            index + offset,
        )
        .unwrap();
    }
    (doc, history)
}

#[test]
fn a_real_docx_and_xlsx_open_as_an_editable_document() {
    let (mut doc, mut history) = open_both();
    doc.validate().unwrap();

    // The docx text is there, its table is a grid, and the workbook's three sheets are three grids.
    let grids = doc
        .blocks
        .values()
        .filter(|b| matches!(b, Block::Grid { .. }))
        .count();
    assert_eq!(grids, 4, "the docx table, then one grid per worksheet");
    assert!(
        doc.blocks
            .values()
            .any(|b| matches!(b, Block::Text { content, .. } if content == "Corpus report")),
        "the docx heading is in the document"
    );

    // The Costs sheet computes: C3 is C2*B3, and C2 refers to another sheet, so it is kept as
    // written rather than computed.
    let costs = doc
        .sheets
        .values()
        .find(|s| s.cells.contains_key("C3"))
        .expect("the Costs sheet");
    assert_eq!(
        costs.cells["C2"],
        CellInput::Formula {
            source: "=B2*Rates!A1".into()
        }
    );

    // Edit a cell through the history, as the editor does: it is one undoable step.
    let sheet_id = *doc
        .sheets
        .iter()
        .find(|(_, s)| s.cells.contains_key("C3"))
        .unwrap()
        .0;
    history
        .commit(
            &mut doc,
            vec![Op::SetCell {
                sheet: sheet_id,
                address: "B2".into(),
                input: Some(CellInput::Number { value: 10.0 }),
            }],
        )
        .unwrap();
    let results = evaluate_sheet(&doc.sheets[&sheet_id]);
    assert_eq!(results["B2"], CellResult::Number(10.0));
    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(
        doc.sheets[&sheet_id].cells["B2"],
        CellInput::Number { value: 4.0 },
        "undo restores the imported value"
    );
}

#[test]
fn edited_content_writes_back_out_and_reads_again() {
    let (doc, _) = open_both();

    // The docx writes back, and the text reads again in the same order.
    let docx = export_docx(&doc).unwrap();
    let again = import_docx(&docx.bytes).unwrap();
    let text = |d: &Document| -> Vec<String> {
        d.blocks
            .values()
            .filter_map(|b| match b {
                Block::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect()
    };
    for line in ["Corpus report", "unicode: café — ünïcode & <tags>"] {
        assert!(
            text(&again.document).iter().any(|t| t == line),
            "{line:?} survives the docx round trip"
        );
    }
    // The table's cells are in its grid, and come back the same.
    let cell = CellInput::Text {
        value: "widget".into(),
    };
    assert!(
        again
            .document
            .sheets
            .values()
            .any(|s| s.cells.values().any(|c| *c == cell)),
        "the table cell 'widget' survives the docx round trip"
    );

    // The first worksheet writes back as an xlsx and reads again with the same cells.
    let first = import_xlsx(&office_fixture("corpus_workbook.xlsx")).unwrap();
    let bytes = export_xlsx(&first.sheet).unwrap();
    let back = import_xlsx(&bytes).unwrap();
    assert_eq!(
        back.sheet.cells["A2"],
        CellInput::Text {
            value: "café".into()
        }
    );
    assert_eq!(back.sheet.cells["B2"], CellInput::Number { value: 4.0 });
    assert_eq!(back.sheet.cells["D2"], CellInput::Boolean { value: true });
}
