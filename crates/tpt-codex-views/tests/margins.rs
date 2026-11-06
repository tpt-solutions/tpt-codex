// SPDX-License-Identifier: MIT OR Apache-2.0
//! Flow margins: adjacent bottom and top margins collapse to the larger one, headings get more
//! space above them, and the last block's bottom margin is kept as trailing space.

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{FlowKind, FlowOptions, MonospaceMeasure, layout_flow};

const LINE: f64 = 10.0;

fn measure() -> MonospaceMeasure {
    MonospaceMeasure {
        char_width: 6.0,
        line_height: LINE,
    }
}

fn options() -> FlowOptions {
    FlowOptions {
        width: 1000.0,
        block_spacing: 8.0,
        box_height: 40.0,
        row_height: 20.0,
    }
}

/// A root with one single-line paragraph per entry, each set in the given heading level (or body).
fn document(levels: &[Option<u8>]) -> Document {
    let children: Vec<BlockId> = (1..=levels.len() as u64).map(BlockId).collect();
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children,
        },
    );
    for (i, level) in levels.iter().enumerate() {
        doc.insert(
            BlockId(i as u64 + 1),
            Block::Text {
                content: format!("line {}", i + 1),
                style: TextStyle {
                    heading_level: *level,
                    ..TextStyle::default()
                },
            },
        );
    }
    doc
}

/// The top edge of each block's first line, in order.
fn tops(doc: &Document) -> Vec<f64> {
    let layout = layout_flow(doc, &measure(), options()).expect("layout");
    let mut seen = Vec::new();
    for item in &layout.items {
        if matches!(item.kind, FlowKind::Line { .. }) && !seen.iter().any(|(b, _)| *b == item.block)
        {
            seen.push((item.block, item.y));
        }
    }
    seen.into_iter().map(|(_, y)| y).collect()
}

#[test]
fn adjacent_paragraphs_are_separated_by_one_margin_not_two() {
    let doc = document(&[None, None]);
    let y = tops(&doc);
    // The first paragraph is one line; the gap is 8, not 16.
    assert_eq!(y, vec![0.0, LINE + 8.0]);
}

#[test]
fn trailing_space_is_the_last_bottom_margin() {
    let doc = document(&[None, None]);
    let layout = layout_flow(&doc, &measure(), options()).expect("layout");
    assert_eq!(layout.height, 2.0 * LINE + 8.0 + 8.0);
}

#[test]
fn a_heading_takes_the_larger_margin_above_it() {
    // Body then heading: the heading's top margin (2 x 8 = 16) wins over the body's bottom (8).
    let doc = document(&[None, Some(1)]);
    let y = tops(&doc);
    assert_eq!(y[1], LINE + 16.0);
}

#[test]
fn a_heading_after_a_heading_collapses_too() {
    let doc = document(&[Some(1), Some(2)]);
    let y = tops(&doc);
    // A level-1 line is 2 x 10 = 20 tall, then the bottom margin 8. The level-2 top margin 16 wins.
    assert_eq!(y, vec![0.0, 2.0 * LINE + 16.0]);
}

#[test]
fn nothing_is_gapped_before_the_first_block() {
    let doc = document(&[Some(1)]);
    assert_eq!(tops(&doc), vec![0.0]);
}

#[test]
fn a_lower_level_heading_has_less_space_above() {
    let doc = document(&[None, Some(3)]);
    let y = tops(&doc);
    // Level 3 uses 1.5 x 8 = 12 above it, larger than the 8 below the paragraph.
    assert_eq!(y[1], LINE + 12.0);
}
