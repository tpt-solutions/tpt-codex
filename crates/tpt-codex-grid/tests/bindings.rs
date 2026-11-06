// SPDX-License-Identifier: MIT OR Apache-2.0
//! Live values in prose: `[[sheet1.B4]]` shows what the grid computes, and follows edits.

use tpt_codex_core::{Block, BlockId, CellInput, Document, SheetId};
use tpt_codex_grid::{format_number, grid_bindings, resolve_document};
use tpt_codex_markdown::from_markdown;

const BUDGET: &str = "# Budget

::: grid { id=\"1\" }
| Item | Cost |
| --- | --- |
| Design | 1200 |
| Build | 3400 |
| Total | =SUM(B2:B3) |
:::

The total is [[sheet1.B4]] and the design cost is [[sheet1.B2]].
";

fn text_of(doc: &Document, id: BlockId) -> String {
    match doc.get(id) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("not text: {other:?}"),
    }
}

/// The paragraph that holds the bindings: the last child of the root.
fn last_paragraph(doc: &Document) -> BlockId {
    match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => *children.last().unwrap(),
        _ => panic!("no root container"),
    }
}

#[test]
fn formulas_are_evaluated_and_shown_in_the_prose() {
    let doc = from_markdown(BUDGET).unwrap();
    let resolved = resolve_document(&doc).expect("the paragraph has bindings");
    assert_eq!(
        text_of(&resolved, last_paragraph(&resolved)),
        "The total is 4600 and the design cost is 1200."
    );
}

#[test]
fn the_text_follows_an_edit_to_the_grid() {
    let mut doc = from_markdown(BUDGET).unwrap();
    doc.sheets
        .get_mut(&SheetId(1))
        .unwrap()
        .cells
        .insert("B3".into(), CellInput::Number { value: 3500.5 });
    let resolved = resolve_document(&doc).unwrap();
    assert_eq!(
        text_of(&resolved, last_paragraph(&resolved)),
        "The total is 4700.5 and the design cost is 1200."
    );
}

#[test]
fn a_document_without_bindings_is_not_copied() {
    let doc = from_markdown("# Plain\n\nNo values here.\n").unwrap();
    assert!(resolve_document(&doc).is_none());
}

#[test]
fn empty_cells_and_errors_are_not_bound() {
    let mut doc = from_markdown(BUDGET).unwrap();
    doc.sheets.get_mut(&SheetId(1)).unwrap().cells.insert(
        "C1".into(),
        CellInput::Formula {
            source: "=1/0".into(),
        },
    );
    let bindings = grid_bindings(&doc);
    assert_eq!(bindings.get("sheet1.B4"), Some("4600"));
    assert_eq!(
        bindings.get("sheet1.C1"),
        None,
        "a formula error is not a value"
    );
    assert_eq!(
        bindings.get("sheet1.Z99"),
        None,
        "an empty cell is not a value"
    );
}

#[test]
fn numbers_read_as_prose() {
    assert_eq!(format_number(4600.0), "4600");
    assert_eq!(format_number(0.1 + 0.2), "0.3");
    assert_eq!(format_number(-2.50), "-2.5");
    assert_eq!(format_number(1.0 / 3.0), "0.3333333333");
    assert_eq!(format_number(-0.0), "0");
    assert_eq!(format_number(1e15), "1000000000000000");
    assert_eq!(format_number(f64::INFINITY), "inf");
}
