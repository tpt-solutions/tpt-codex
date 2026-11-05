// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Canvas, Document, EditError, History, LayoutMode, Op, Rect, TextStyle,
    ValidationError,
};

/// A slide (absolute container 0) holding text block 1, and a flow container 2 holding text 3.
fn deck() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(4), BlockId(2)],
        },
    );
    d.insert(
        BlockId(4),
        Block::Container {
            layout_mode: LayoutMode::Absolute,
            children: vec![BlockId(1)],
        },
    );
    d.insert(
        BlockId(2),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(3)],
        },
    );
    for id in [1, 3] {
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

fn frame(x: f64) -> Rect {
    Rect {
        x,
        y: 10.0,
        width: 100.0,
        height: 50.0,
    }
}

#[test]
fn a_child_of_an_absolute_container_can_be_placed_and_undone() {
    let mut d = deck();
    let mut h = History::new();
    h.commit(
        &mut d,
        vec![
            Op::SetCanvas {
                id: BlockId(4),
                canvas: Some(Canvas::WIDESCREEN),
            },
            Op::SetFrame {
                id: BlockId(1),
                frame: Some(frame(20.0)),
            },
        ],
    )
    .unwrap();
    assert_eq!(d.frames.get(&BlockId(1)), Some(&frame(20.0)));
    assert_eq!(d.canvases.get(&BlockId(4)), Some(&Canvas::WIDESCREEN));
    d.validate().unwrap();

    h.undo(&mut d).unwrap();
    assert!(d.frames.is_empty());
    h.undo(&mut d).unwrap();
    assert!(d.canvases.is_empty());
}

#[test]
fn a_frame_cannot_go_on_a_child_of_a_flow_container() {
    let mut d = deck();
    let err = tpt_codex_core::apply(
        &mut d,
        &Op::SetFrame {
            id: BlockId(3),
            frame: Some(frame(0.0)),
        },
    )
    .unwrap_err();
    assert_eq!(err, EditError::NotAbsolute(BlockId(3)));
    assert!(d.frames.is_empty());
}

#[test]
fn a_canvas_cannot_go_on_a_flow_container() {
    let mut d = deck();
    let err = tpt_codex_core::apply(
        &mut d,
        &Op::SetCanvas {
            id: BlockId(2),
            canvas: Some(Canvas::WIDESCREEN),
        },
    )
    .unwrap_err();
    assert_eq!(err, EditError::NotAbsolute(BlockId(2)));
}

#[test]
fn invalid_geometry_is_refused() {
    let mut d = deck();
    let negative = Rect {
        width: -1.0,
        ..frame(0.0)
    };
    assert_eq!(
        tpt_codex_core::apply(
            &mut d,
            &Op::SetFrame {
                id: BlockId(1),
                frame: Some(negative),
            },
        ),
        Err(EditError::InvalidGeometry(BlockId(1)))
    );
    let flat = Canvas {
        width: 0.0,
        height: 540.0,
    };
    assert_eq!(
        tpt_codex_core::apply(
            &mut d,
            &Op::SetCanvas {
                id: BlockId(4),
                canvas: Some(flat),
            },
        ),
        Err(EditError::InvalidGeometry(BlockId(4)))
    );
}

#[test]
fn a_positioned_block_cannot_be_removed_or_changed_to_flow() {
    let mut d = deck();
    let mut h = History::new();
    h.commit(
        &mut d,
        vec![
            Op::SetCanvas {
                id: BlockId(4),
                canvas: Some(Canvas::WIDESCREEN),
            },
            Op::SetFrame {
                id: BlockId(1),
                frame: Some(frame(0.0)),
            },
        ],
    )
    .unwrap();

    // The frame's block cannot be removed while it is placed.
    d.insert(
        BlockId(9),
        Block::Text {
            content: "y".into(),
            style: TextStyle::default(),
        },
    );
    let removal = Op::RemoveBlock { id: BlockId(1) };
    assert_eq!(
        tpt_codex_core::apply(&mut d, &removal),
        Err(EditError::IsPositioned(BlockId(1)))
    );

    // The slide cannot become flow while it has a canvas.
    let as_flow = Block::Container {
        layout_mode: LayoutMode::Flow,
        children: vec![BlockId(1)],
    };
    assert_eq!(
        tpt_codex_core::apply(
            &mut d,
            &Op::SetBlock {
                id: BlockId(4),
                block: as_flow,
            },
        ),
        Err(EditError::IsPositioned(BlockId(4)))
    );
}

#[test]
fn a_canvas_on_a_missing_or_flow_block_fails_validation() {
    let mut d = deck();
    d.canvases.insert(BlockId(2), Canvas::WIDESCREEN);
    assert_eq!(
        d.validate(),
        Err(ValidationError::CanvasNotAbsolute(BlockId(2)))
    );

    let mut d = deck();
    d.frames.insert(BlockId(77), frame(0.0));
    assert_eq!(
        d.validate(),
        Err(ValidationError::FrameOfMissingBlock(BlockId(77)))
    );

    let mut d = deck();
    d.frames.insert(
        BlockId(1),
        Rect {
            x: f64::NAN,
            ..frame(0.0)
        },
    );
    assert_eq!(d.validate(), Err(ValidationError::InvalidFrame(BlockId(1))));
}

#[test]
fn empty_position_maps_are_not_written_and_positions_round_trip() {
    let plain = serde_json::to_string(&deck()).unwrap();
    assert!(!plain.contains("frames"), "{plain}");
    assert!(!plain.contains("canvases"), "{plain}");

    let mut d = deck();
    d.canvases.insert(BlockId(4), Canvas::WIDESCREEN);
    d.frames.insert(BlockId(1), frame(5.0));
    let json = serde_json::to_string(&d).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
    back.validate().unwrap();
}

#[test]
fn widescreen_is_sixteen_by_nine() {
    let aspect = Canvas::WIDESCREEN.aspect();
    assert!((aspect - 16.0 / 9.0).abs() < 1e-9);
}
