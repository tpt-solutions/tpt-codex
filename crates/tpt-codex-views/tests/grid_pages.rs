// SPDX-License-Identifier: MIT OR Apache-2.0
//! Grids in the flow are one item per row, and pagination breaks between rows, repeating the
//! header rows on each continuation page.

use tpt_codex_core::{Block, BlockId, CellInput, Document, LayoutMode, Sheet, SheetId, TextStyle};
use tpt_codex_views::{
    FlowKind, FlowOptions, MonospaceMeasure, Page, PageGeometry, layout_flow, paginate,
};

const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 6.0,
    line_height: 14.0,
};

/// A document with one grid of `rows` rows, sheet 1.
fn grid_doc(rows: u32) -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    d.insert(
        BlockId(1),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    let mut sheet = Sheet {
        rows,
        ..Sheet::default()
    };
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 1.0 });
    d.insert_sheet(SheetId(1), sheet);
    d
}

fn options(header_rows: u32) -> FlowOptions {
    FlowOptions {
        width: 300.0,
        block_spacing: 0.0,
        box_height: 40.0,
        row_height: 20.0,
        header_rows,
    }
}

fn geometry() -> PageGeometry {
    PageGeometry {
        height: 110.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 1,
    }
}

/// The row numbers shown on a page, in order.
fn rows_on(page: &Page) -> Vec<u32> {
    page.items
        .iter()
        .filter_map(|it| match it.kind {
            FlowKind::GridRow { row, .. } => Some(row),
            _ => None,
        })
        .collect()
}

#[test]
fn every_grid_row_is_a_flow_item() {
    let doc = grid_doc(7);
    let flow = layout_flow(&doc, &MEASURE, options(1)).unwrap();
    let rows: Vec<u32> = flow
        .items
        .iter()
        .filter_map(|it| match it.kind {
            FlowKind::GridRow { row, .. } => Some(row),
            _ => None,
        })
        .collect();
    assert_eq!(rows, (1..=7).collect::<Vec<u32>>());
    assert_eq!(flow.height, 7.0 * 20.0);
}

#[test]
fn a_grid_breaks_between_rows_and_repeats_its_header() {
    let doc = grid_doc(10);
    let flow = layout_flow(&doc, &MEASURE, options(1)).unwrap();
    // Each page fits five rows: 110 / 20 = 5.
    let pages = paginate(&flow, geometry()).unwrap();
    assert!(pages.len() > 1, "{pages:?}");

    // Every page after the first starts with the header row.
    for page in pages.iter().skip(1) {
        assert_eq!(rows_on(page)[0], 1, "page {}", page.number);
    }

    // The body rows 2..=10 each appear once, in order, across the pages.
    let body: Vec<u32> = pages
        .iter()
        .flat_map(|p| rows_on(p).into_iter().filter(|&r| r != 1))
        .collect();
    assert_eq!(body, (2..=10).collect::<Vec<u32>>());
}

#[test]
fn a_grid_without_header_rows_repeats_nothing() {
    let doc = grid_doc(10);
    let flow = layout_flow(&doc, &MEASURE, options(0)).unwrap();
    let pages = paginate(&flow, geometry()).unwrap();
    let all: Vec<u32> = pages.iter().flat_map(rows_on).collect();
    assert_eq!(all, (1..=10).collect::<Vec<u32>>());
}

#[test]
fn text_after_a_grid_still_paginates() {
    let mut doc = grid_doc(3);
    doc.insert(
        BlockId(2),
        Block::Text {
            content: "after".into(),
            style: TextStyle::default(),
        },
    );
    if let Some(Block::Container { children, .. }) = doc.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(2));
    }
    let flow = layout_flow(&doc, &MEASURE, options(1)).unwrap();
    let pages = paginate(&flow, geometry()).unwrap();
    let lines: usize = pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter(|it| matches!(it.kind, FlowKind::Line { .. }))
        .count();
    assert_eq!(lines, 1);
}

#[test]
fn a_grid_with_a_missing_sheet_is_a_placeholder_box() {
    let mut doc = grid_doc(3);
    doc.sheets.clear();
    let flow = layout_flow(&doc, &MEASURE, options(1)).unwrap();
    assert_eq!(flow.items.len(), 1);
    assert!(matches!(flow.items[0].kind, FlowKind::Box));
}
