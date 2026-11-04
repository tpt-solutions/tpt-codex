// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, CellInput, Document, EditError, History, LayoutMode, Op, Sheet, SheetId,
    ValidationError,
};

fn doc_with_grid() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Grid {
            sheet_id: SheetId(7),
            view_bounds: None,
        },
    );
    doc.insert_sheet(SheetId(7), Sheet::default());
    doc
}

fn set(address: &str, input: Option<CellInput>) -> Op {
    Op::SetCell {
        sheet: SheetId(7),
        address: address.into(),
        input,
    }
}

fn number(value: f64) -> Option<CellInput> {
    Some(CellInput::Number { value })
}

#[test]
fn set_and_clear_cells_with_undo_and_redo() {
    let mut doc = doc_with_grid();
    let mut h = History::new();
    h.commit(&mut doc, vec![set("A1", number(21.0))]).unwrap();
    h.commit(
        &mut doc,
        vec![set(
            "B1",
            Some(CellInput::Formula {
                source: "=A1 * 2".into(),
            }),
        )],
    )
    .unwrap();
    let cells = &doc.sheets[&SheetId(7)].cells;
    assert_eq!(cells.len(), 2);
    assert_eq!(cells["A1"], CellInput::Number { value: 21.0 });

    // Overwrite A1, then clear it, then undo both.
    h.commit(&mut doc, vec![set("A1", number(5.0))]).unwrap();
    h.commit(&mut doc, vec![set("A1", None)]).unwrap();
    assert!(!doc.sheets[&SheetId(7)].cells.contains_key("A1"));
    h.undo(&mut doc).unwrap();
    assert_eq!(
        doc.sheets[&SheetId(7)].cells["A1"],
        CellInput::Number { value: 5.0 }
    );
    h.undo(&mut doc).unwrap();
    h.undo(&mut doc).unwrap();
    assert_eq!(
        doc.sheets[&SheetId(7)].cells["A1"],
        CellInput::Number { value: 21.0 }
    );
    // Redo replays the most recently undone step first: the B1 formula, then the A1 overwrite.
    h.redo(&mut doc).unwrap();
    assert!(doc.sheets[&SheetId(7)].cells.contains_key("B1"));
    assert_eq!(
        doc.sheets[&SheetId(7)].cells["A1"],
        CellInput::Number { value: 21.0 }
    );
    h.redo(&mut doc).unwrap();
    assert_eq!(
        doc.sheets[&SheetId(7)].cells["A1"],
        CellInput::Number { value: 5.0 }
    );
    doc.validate().unwrap();
}

#[test]
fn cell_edits_do_not_change_block_records() {
    let mut doc = doc_with_grid();
    let mut h = History::new();
    h.commit(&mut doc, vec![set("A1", number(1.0))]).unwrap();
    assert!(h.last_changed().is_empty());
}

#[test]
fn bad_edits_are_rejected_and_change_nothing() {
    let mut doc = doc_with_grid();
    let mut h = History::new();
    let before = doc.clone();

    assert_eq!(
        h.commit(&mut doc, vec![set("a1", number(1.0))]),
        Err(EditError::InvalidAddress("a1".into()))
    );
    assert_eq!(
        h.commit(
            &mut doc,
            vec![set(
                "A2",
                Some(CellInput::Formula {
                    source: "A1".into()
                })
            )]
        ),
        Err(EditError::InvalidFormula("A2".into()))
    );
    let missing = Op::SetCell {
        sheet: SheetId(99),
        address: "A1".into(),
        input: number(1.0),
    };
    assert_eq!(
        h.commit(&mut doc, vec![missing]),
        Err(EditError::NoSuchSheet(SheetId(99)))
    );
    assert_eq!(doc, before);
    assert_eq!(h.undo_len(), 0);
}

#[test]
fn validation_catches_missing_sheets_and_bad_cells() {
    let mut doc = doc_with_grid();
    doc.sheets.clear();
    assert_eq!(
        doc.validate(),
        Err(ValidationError::MissingSheet {
            block: BlockId(1),
            sheet: SheetId(7)
        })
    );

    let mut doc = doc_with_grid();
    doc.sheets
        .get_mut(&SheetId(7))
        .unwrap()
        .cells
        .insert("bad".into(), CellInput::Number { value: 1.0 });
    assert_eq!(
        doc.validate(),
        Err(ValidationError::InvalidAddress {
            sheet: SheetId(7),
            address: "bad".into()
        })
    );

    let mut doc = doc_with_grid();
    doc.sheets.get_mut(&SheetId(7)).unwrap().cells.insert(
        "A1".into(),
        CellInput::Formula {
            source: "1+1".into(),
        },
    );
    assert_eq!(
        doc.validate(),
        Err(ValidationError::InvalidFormula {
            sheet: SheetId(7),
            address: "A1".into()
        })
    );
}

#[test]
fn sheets_round_trip_through_json_and_empty_sheets_are_omitted() {
    let mut doc = doc_with_grid();
    let mut h = History::new();
    h.commit(&mut doc, vec![set("A1", number(2.5))]).unwrap();
    let json = serde_json::to_string(&doc).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(back, doc);

    // A document with no sheets writes no `sheets` key, so existing files are unchanged.
    let plain = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    assert!(!serde_json::to_string(&plain).unwrap().contains("sheets"));
}
