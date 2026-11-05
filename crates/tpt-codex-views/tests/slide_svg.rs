// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Rect, TextStyle};
use tpt_codex_views::{CanvasError, MonospaceMeasure, render_slide_svg};

const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 6.0,
    line_height: 14.0,
};

fn slide_with(text: &str) -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(10)],
        },
    );
    d.insert(
        BlockId(10),
        Block::Container {
            layout_mode: LayoutMode::Absolute,
            children: vec![BlockId(11), BlockId(12)],
        },
    );
    d.insert(
        BlockId(11),
        Block::Text {
            content: text.into(),
            style: TextStyle::default(),
        },
    );
    d.insert(BlockId(12), Block::ThematicBreak);
    d.frames.insert(
        BlockId(11),
        Rect {
            x: 10.0,
            y: 20.0,
            width: 60.0,
            height: 100.0,
        },
    );
    d.frames.insert(
        BlockId(12),
        Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
    );
    d
}

#[test]
fn text_is_wrapped_and_escaped_inside_the_slide() {
    let svg = render_slide_svg(&slide_with("a < b & c"), BlockId(10), &MEASURE).unwrap();
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.contains("viewBox=\"0 0 960 540\""), "{svg}");
    assert!(svg.contains("a &lt; b &amp; c"), "{svg}");
    assert!(!svg.contains("a < b"), "{svg}");
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn long_text_takes_several_lines_within_the_frame_width() {
    let svg = render_slide_svg(
        &slide_with("one two three four five six seven eight nine ten"),
        BlockId(10),
        &MEASURE,
    )
    .unwrap();
    assert!(svg.matches("<text ").count() > 1, "{svg}");
}

#[test]
fn other_blocks_are_drawn_as_labelled_boxes_and_unplaced_ones_are_not_drawn() {
    let mut d = slide_with("x");
    d.frames.remove(&BlockId(12));
    let svg = render_slide_svg(&d, BlockId(10), &MEASURE).unwrap();
    assert!(!svg.contains("stroke-dasharray"), "{svg}");

    let svg = render_slide_svg(&slide_with("x"), BlockId(10), &MEASURE).unwrap();
    assert!(svg.contains("<title>block 12</title>"), "{svg}");
}

#[test]
fn a_flow_block_is_not_a_slide() {
    assert_eq!(
        render_slide_svg(&slide_with("x"), BlockId(0), &MEASURE).unwrap_err(),
        CanvasError::NotASlide(BlockId(0))
    );
}
