// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, History, LayoutMode, TextStyle};
use tpt_codex_interop::{PlaceError, import_xlsx, place_sheet};

const BASIC: &[u8] = include_bytes!("../../../fixtures/office/basic.xlsx");

fn doc() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    d.insert(
        BlockId(1),
        Block::Text {
            content: "before".into(),
            style: TextStyle::default(),
        },
    );
    d
}

#[test]
fn an_imported_sheet_is_placed_as_a_grid_and_undoes_in_one_step() {
    let import = import_xlsx(BASIC).unwrap();
    let mut d = doc();
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(1));
    }
    let mut h = History::new();
    let placed = place_sheet(&mut d, &mut h, import.sheet.clone(), BlockId(0), 1).unwrap();

    assert!(matches!(
        d.get(placed.block),
        Some(Block::Grid { sheet_id, .. }) if *sheet_id == placed.sheet
    ));
    assert_eq!(d.sheets[&placed.sheet], import.sheet);
    match d.get(BlockId(0)) {
        Some(Block::Container { children, .. }) => {
            assert_eq!(children, &vec![BlockId(1), placed.block]);
        }
        other => panic!("{other:?}"),
    }
    d.validate().unwrap();

    h.undo(&mut d).unwrap();
    assert!(d.sheets.is_empty());
    assert!(!d.blocks.contains_key(&placed.block));
}

#[test]
fn placing_outside_a_container_or_past_its_end_is_refused() {
    let sheet = import_xlsx(BASIC).unwrap().sheet;
    let mut d = doc();
    let mut h = History::new();
    assert!(matches!(
        place_sheet(&mut d, &mut h, sheet.clone(), BlockId(1), 0),
        Err(PlaceError::NotAContainer(BlockId(1)))
    ));
    assert!(matches!(
        place_sheet(&mut d, &mut h, sheet, BlockId(0), 5),
        Err(PlaceError::IndexOutOfBounds { .. })
    ));
    assert!(d.sheets.is_empty());
}
