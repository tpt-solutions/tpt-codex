// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Canvas, Document, LayoutMode, Rect, TextStyle};
use tpt_codex_views::{CanvasError, layout_canvas, slide_ids};

/// Root holding slide 10 (two children, 11 and 12) and a flow block 20 with child 21.
fn deck() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(10), BlockId(20)],
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
        BlockId(20),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(21)],
        },
    );
    for id in [11, 12, 21] {
        d.insert(
            BlockId(id),
            Block::Text {
                content: "x".into(),
                style: TextStyle::default(),
            },
        );
    }
    d
}

fn rect(x: f64) -> Rect {
    Rect {
        x,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    }
}

#[test]
fn children_are_placed_in_z_order_at_their_frames() {
    let mut d = deck();
    d.canvases.insert(BlockId(10), Canvas::WIDESCREEN);
    d.frames.insert(BlockId(11), rect(5.0));
    let layout = layout_canvas(&d, BlockId(10)).unwrap();
    assert_eq!((layout.width, layout.height), (960.0, 540.0));
    let blocks: Vec<BlockId> = layout.items.iter().map(|i| i.block).collect();
    assert_eq!(blocks, vec![BlockId(11), BlockId(12)]);
    assert_eq!(layout.items[0].frame, Some(rect(5.0)));
    // A child with no frame is listed, but has no position.
    assert_eq!(layout.items[1].frame, None);
}

#[test]
fn a_slide_without_a_canvas_is_widescreen() {
    let layout = layout_canvas(&deck(), BlockId(10)).unwrap();
    assert!((layout.width / layout.height - 16.0 / 9.0).abs() < 1e-9);
}

#[test]
fn a_flow_block_is_not_a_slide() {
    assert_eq!(
        layout_canvas(&deck(), BlockId(20)).unwrap_err(),
        CanvasError::NotASlide(BlockId(20))
    );
    assert!(layout_canvas(&deck(), BlockId(999)).is_err());
}

#[test]
fn slides_are_the_absolute_containers_under_the_root() {
    assert_eq!(slide_ids(&deck()), vec![BlockId(10)]);
}
