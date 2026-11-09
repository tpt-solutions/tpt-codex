// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cell names: a name is a plain identifier, and it refers to an existing sheet and a cell address.

use tpt_codex_core::{
    Block, BlockId, Document, LayoutMode, NamedCell, Sheet, SheetId, is_cell_name,
};

fn doc_with_sheet() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    doc.insert_sheet(SheetId(1), Sheet::with_size(2, 2));
    doc
}

#[test]
fn a_name_is_an_identifier_and_not_an_address() {
    assert!(is_cell_name("Total"));
    assert!(is_cell_name("_rate_2"));
    assert!(!is_cell_name("2rate"));
    assert!(!is_cell_name("sheet1.B4"));
    assert!(!is_cell_name("B4"), "an address is not a name");
    assert!(is_cell_name("TOTAL1"), "five letters is not a cell address");
    assert!(
        !is_cell_name("AB12"),
        "a name that is an address is refused"
    );
    assert!(!is_cell_name(""));
}

#[test]
fn a_named_cell_must_be_in_an_existing_sheet() {
    let mut doc = doc_with_sheet();
    doc.cell_names.insert(
        "Total".into(),
        NamedCell {
            sheet: SheetId(1),
            address: "B2".into(),
        },
    );
    doc.validate().unwrap();

    doc.cell_names.insert(
        "Missing".into(),
        NamedCell {
            sheet: SheetId(9),
            address: "A1".into(),
        },
    );
    assert!(doc.validate().is_err());
}
