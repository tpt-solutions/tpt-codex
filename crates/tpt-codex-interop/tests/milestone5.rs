// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 5: open a real `.docx` and `.xlsx`, edit them, write them back out, and read the
//! results again.

use tpt_codex_core::{
    Block, BlockId, CellInput, Document, History, InlineRun, LayoutMode, Sheet, TextStyle,
    edit_text,
};
use tpt_codex_interop::{export_docx, export_xlsx, import_docx, import_xlsx, place_sheet};

const DOCX: &[u8] = include_bytes!("../../../fixtures/office/basic.docx");
const XLSX: &[u8] = include_bytes!("../../../fixtures/office/basic.xlsx");

fn texts(doc: &Document) -> Vec<String> {
    doc.blocks
        .values()
        .filter_map(|b| match b {
            Block::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_docx_opens_is_edited_and_written_back() {
    let import = import_docx(DOCX).unwrap();
    let mut doc = import.document;
    doc.validate().unwrap();

    // Add a sentence to the first paragraph, as the editor would.
    let mut history = History::new();
    let first = doc
        .blocks
        .iter()
        .find_map(|(id, b)| match b {
            Block::Text { content, .. } if !content.is_empty() => {
                Some((*id, content.chars().count()))
            }
            _ => None,
        })
        .unwrap();
    edit_text(
        &mut doc,
        &mut history,
        first.0,
        first.1,
        first.1,
        " Edited here.",
    )
    .unwrap();

    let written = export_docx(&doc).unwrap();
    let again = import_docx(&written.bytes).unwrap();
    again.document.validate().unwrap();
    assert!(
        texts(&again.document)
            .iter()
            .any(|t| t.ends_with("Edited here.")),
        "{:?}",
        texts(&again.document)
    );
    assert_eq!(texts(&again.document).len(), texts(&doc).len());
}

#[test]
fn an_xlsx_is_placed_in_a_document_edited_and_written_back() {
    let sheet: Sheet = import_xlsx(XLSX).unwrap().sheet;
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    let mut history = History::new();
    let placed = place_sheet(&mut doc, &mut history, sheet.clone(), BlockId(0), 0).unwrap();

    // Edit one cell through the sheet, then write the sheet out and read it back.
    let mut edited = doc.sheets[&placed.sheet].clone();
    edited
        .cells
        .insert("Z9".into(), CellInput::Number { value: 42.0 });
    let bytes = export_xlsx(&edited).unwrap();
    let back = import_xlsx(&bytes).unwrap().sheet;
    assert_eq!(
        back.cells.get("Z9"),
        Some(&CellInput::Number { value: 42.0 })
    );
    for (address, input) in &sheet.cells {
        assert_eq!(back.cells.get(address), Some(input), "cell {address}");
    }
    doc.validate().unwrap();
}

#[test]
fn emphasis_and_links_as_text_survive_a_docx_round_trip() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "plain bold italic link".into(),
            style: TextStyle {
                runs: vec![
                    InlineRun {
                        start: 6,
                        end: 10,
                        bold: true,
                        italic: false,
                        link: None,
                        math: None,
                        link_title: None,
                    },
                    InlineRun {
                        start: 11,
                        end: 17,
                        bold: false,
                        italic: true,
                        link: None,
                        math: None,
                        link_title: None,
                    },
                    InlineRun {
                        start: 18,
                        end: 22,
                        bold: false,
                        italic: false,
                        link: Some("https://example.test".into()),
                        math: None,
                        link_title: None,
                    },
                ],
                ..TextStyle::default()
            },
        },
    );
    let written = export_docx(&doc).unwrap();
    // The link is written as its text, and the report says so.
    assert!(written.report.items().iter().any(|i| i.feature == "links"));
    let back = import_docx(&written.bytes).unwrap().document;
    let Some(Block::Text { content, style }) = back.blocks.get(&BlockId(1)) else {
        panic!("no paragraph");
    };
    assert_eq!(content, "plain bold italic link");
    assert!(
        style
            .runs
            .iter()
            .any(|r| r.bold && r.start == 6 && r.end == 10),
        "{:?}",
        style.runs
    );
    assert!(
        style
            .runs
            .iter()
            .any(|r| r.italic && r.start == 11 && r.end == 17),
        "{:?}",
        style.runs
    );
}
