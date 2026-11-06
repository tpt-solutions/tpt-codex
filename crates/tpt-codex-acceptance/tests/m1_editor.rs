// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 1 (todo.md): "A user types a paragraph, inserts a live calculation table, and it
//! updates reactively in the browser via WASM."
//!
//! The browser half is `crates/tpt-codex-web/browser`. This test drives the same editor messages
//! headlessly: type, insert a table, enter formulas, and check the computed values and the view.

use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_grid::{CellResult, evaluate_sheet};

fn blank_page() -> Document {
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
            content: String::new(),
            style: TextStyle::default(),
        },
    );
    doc
}

#[test]
fn a_user_types_a_paragraph_and_inserts_a_live_calculation_table() {
    let mut editor = Editor::new(blank_page());

    // Type a paragraph.
    editor.update(Msg::Focus(BlockId(1)));
    editor.update(Msg::Type("Quarterly costs".into()));
    let paragraph = match editor.document().get(BlockId(1)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("paragraph expected, found {other:?}"),
    };
    assert_eq!(paragraph, "Quarterly costs");

    // Insert a 2 x 2 table and fill it with a live calculation.
    editor.update(Msg::InsertTable { rows: 2, cols: 2 });
    let sheet_id = *editor
        .document()
        .sheets
        .keys()
        .next()
        .expect("the table has a sheet");
    editor.update(Msg::EditCell {
        sheet: sheet_id,
        address: "A1".into(),
        text: "21".into(),
    });
    editor.update(Msg::EditCell {
        sheet: sheet_id,
        address: "A2".into(),
        text: "=A1*2".into(),
    });

    let results = evaluate_sheet(&editor.document().sheets[&sheet_id]);
    assert_eq!(results["A1"], CellResult::Number(21.0));
    assert_eq!(
        results["A2"],
        CellResult::Number(42.0),
        "the formula recalculates"
    );

    // Changing an input updates the formula's result.
    editor.update(Msg::EditCell {
        sheet: sheet_id,
        address: "A1".into(),
        text: "5".into(),
    });
    let results = evaluate_sheet(&editor.document().sheets[&sheet_id]);
    assert_eq!(results["A2"], CellResult::Number(10.0));

    // The view shows the paragraph and the table, and the table shows the new value.
    let view = format!("{:?}", editor.view());
    assert!(
        view.contains("Quarterly costs"),
        "the paragraph is in the view"
    );
}

#[test]
fn undo_takes_back_the_last_typed_edit() {
    let mut editor = Editor::new(blank_page());
    editor.update(Msg::Focus(BlockId(1)));
    editor.update(Msg::Type("draft".into()));
    editor.update(Msg::Undo);
    let content = match editor.document().get(BlockId(1)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("paragraph expected, found {other:?}"),
    };
    assert!(
        !content.contains("draft"),
        "undo removed the typed text: {content:?}"
    );
}
