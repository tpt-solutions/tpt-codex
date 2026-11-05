// SPDX-License-Identifier: MIT OR Apache-2.0
//! The Flow view and pagination, measured with the bundled font.

use tpt_codex_markdown::from_markdown;
use tpt_codex_text::FontMeasure;
use tpt_codex_views::{FlowKind, FlowOptions, PageGeometry, layout_flow, paginate};

const FIXTURE: &str = include_str!("../../../fixtures/notes.md");

fn options() -> FlowOptions {
    FlowOptions {
        width: 300.0,
        block_spacing: 8.0,
        box_height: 40.0,
        row_height: 20.0,
        header_rows: 0,
    }
}

fn geometry() -> PageGeometry {
    PageGeometry {
        height: 160.0,
        margin_top: 10.0,
        margin_bottom: 10.0,
        min_lines: 2,
    }
}

#[test]
fn fixture_lays_out_and_paginates_with_the_bundled_font() {
    let doc = from_markdown(FIXTURE).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    let flow = layout_flow(&doc, &measure, options()).unwrap();
    let pages = paginate(&flow, geometry()).unwrap();

    assert!(!pages.is_empty());
    let lines_in = |items: &[tpt_codex_views::FlowItem]| {
        items
            .iter()
            .filter(|i| matches!(i.kind, FlowKind::Line { .. }))
            .count()
    };
    let flow_lines = lines_in(&flow.items);
    assert!(flow_lines > 0);
    // Pagination keeps every line exactly once.
    let paged_lines: usize = pages.iter().map(|p| lines_in(&p.items)).sum();
    assert_eq!(paged_lines, flow_lines);
}

#[test]
fn layout_with_the_bundled_font_is_deterministic() {
    let doc = from_markdown(FIXTURE).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    let a = layout_flow(&doc, &measure, options()).unwrap();
    let b = layout_flow(&doc, &measure, options()).unwrap();
    assert_eq!(a, b);
}
