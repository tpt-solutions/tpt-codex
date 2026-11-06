// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 0 (todo.md): "CI is green, license gate demonstrably blocks an Apache-only dep,
//! `Block` tree round-trips through serialization."
//!
//! The CI and license-gate parts are checked by the workflow and by `scripts/`, not by this test.
//! The headless part is tested here: a `Block` tree is saved and loaded through the store, and the
//! loaded document is the same as the original.

mod common;

use common::MemStorage;
use tpt_codex_app::DocumentStore;
use tpt_codex_core::{
    Block, BlockId, CellInput, Document, LayoutMode, Rect, Sheet, SheetId, TextStyle,
};
use tpt_codex_markdown::from_markdown;

#[test]
fn block_tree_round_trips_through_serialization() {
    let doc = from_markdown(
        "# Title\n\nA paragraph with *emphasis* and a [link](https://example.com).\n\n\
         - first\n- second\n\n$$\nE = mc^2\n$$\n",
    )
    .unwrap();
    doc.validate().unwrap();

    let mut store = DocumentStore::new(MemStorage::default());
    store.save("notes", &doc).unwrap();
    let loaded = store.load("notes").unwrap();

    assert_eq!(loaded, doc, "the loaded tree is the saved tree");
    loaded.validate().unwrap();
}

#[test]
fn a_tree_with_grids_and_frames_round_trips_too() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "before the grid".into(),
            style: TextStyle::default(),
        },
    );
    let mut sheet = Sheet::default();
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 2.0 });
    sheet.cells.insert(
        "A2".into(),
        CellInput::Formula {
            source: "=A1*3".into(),
        },
    );
    doc.sheets.insert(SheetId(1), sheet);
    doc.insert(
        BlockId(2),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.frames.insert(
        BlockId(2),
        Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        },
    );

    let mut store = DocumentStore::new(MemStorage::default());
    store.save("grid", &doc).unwrap();
    let loaded = store.load("grid").unwrap();
    assert_eq!(loaded, doc);
    assert_eq!(
        loaded.sheets[&SheetId(1)].cells["A2"],
        CellInput::Formula {
            source: "=A1*3".into()
        }
    );
}
