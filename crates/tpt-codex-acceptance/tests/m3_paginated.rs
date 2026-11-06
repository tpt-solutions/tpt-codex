// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 3 (todo.md): "One-click export of the Phase 1 hybrid document to a byte-exact,
//! paginated PDF."
//!
//! The PDF is blocked on `tpt-canvas-pdf`, so its test is ignored. The headless parts are tested
//! here: the hybrid document (text, a live grid and a media block) paginates across pages, repeats
//! the grid's header rows on each continuation page, and lays out the same way every time. The
//! golden file in `tests/golden.rs` pins the bytes of the paginated view.

use tpt_codex_core::{
    Block, BlockId, CellInput, Document, LayoutMode, MediaSource, Sheet, SheetId, TextStyle,
};
use tpt_codex_views::{FlowKind, FlowOptions, PageGeometry, layout_flow, paginate};

mod common;
use common::MEASURE;

/// A hybrid document: a title, a long paragraph, a 40-row grid with two header rows, and a
/// picture.
fn hybrid_document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "Hybrid report".into(),
            style: TextStyle {
                heading_level: Some(1),
                ..TextStyle::default()
            },
        },
    );
    doc.insert(
        BlockId(2),
        Block::Text {
            content: "A paragraph long enough to wrap across several lines at this width, so the \
                      layout has to break it and keep the lines together."
                .repeat(3),
            style: TextStyle::default(),
        },
    );
    let mut sheet = Sheet {
        header_rows: 2,
        ..Sheet::default()
    };
    for row in 1..=40u32 {
        sheet
            .cells
            .insert(format!("A{row}"), CellInput::Number { value: row as f64 });
    }
    doc.sheets.insert(SheetId(1), sheet);
    doc.insert(
        BlockId(3),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.insert(
        BlockId(4),
        Block::Media {
            source: MediaSource::Uri {
                uri: "figure.png".into(),
            },
            alt_text: Some("a figure".into()),
        },
    );
    doc
}

fn paginate_hybrid(doc: &Document) -> Vec<tpt_codex_views::Page> {
    let flow = layout_flow(
        doc,
        &MEASURE,
        FlowOptions {
            width: 300.0,
            block_spacing: 6.0,
            box_height: 80.0,
            row_height: 14.0,
        },
    )
    .unwrap();
    paginate(
        &flow,
        PageGeometry {
            height: 260.0,
            margin_top: 20.0,
            margin_bottom: 20.0,
            min_lines: 2,
        },
    )
    .unwrap()
}

#[test]
fn the_hybrid_document_paginates_with_grid_headers_repeated() {
    let doc = hybrid_document();
    let pages = paginate_hybrid(&doc);
    assert!(
        pages.len() > 2,
        "the document spans several pages: {}",
        pages.len()
    );

    // Every page is numbered from 1, in order.
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(page.number, index + 1);
    }

    // The grid's two header rows appear at the top of each page that holds grid rows.
    let header_rows_on = |page: &tpt_codex_views::Page| {
        page.items
            .iter()
            .filter(|item| matches!(item.kind, FlowKind::GridRow { header: true, .. }))
            .count()
    };
    let pages_with_grid: Vec<usize> = pages
        .iter()
        .enumerate()
        .filter(|(_, page)| {
            page.items
                .iter()
                .any(|item| matches!(item.kind, FlowKind::GridRow { .. }))
        })
        .map(|(i, _)| i)
        .collect();
    assert!(
        pages_with_grid.len() >= 2,
        "the grid continues onto a second page"
    );
    for index in pages_with_grid.iter().skip(1) {
        assert_eq!(
            header_rows_on(&pages[*index]),
            2,
            "page {} repeats the header rows",
            index + 1
        );
    }
}

#[test]
fn the_same_document_lays_out_to_the_same_pages_every_time() {
    let doc = hybrid_document();
    let first = format!("{:?}", paginate_hybrid(&doc));
    let second = format!("{:?}", paginate_hybrid(&doc.clone()));
    assert_eq!(first, second, "layout and pagination are deterministic");
}

#[test]
#[ignore = "PDF export is blocked: tpt-canvas-pdf writes glyph runs as placeholder comments"]
fn one_click_export_gives_a_byte_exact_paginated_pdf() {
    unreachable!("enable when tpt-canvas-pdf draws glyph runs");
}
