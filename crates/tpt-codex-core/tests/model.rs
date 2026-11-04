// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Document, LayoutMode, MediaSource, Metadata, Query, Rect, SheetId, TextStyle,
    ValidationError,
};

fn text(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

fn container(children: Vec<BlockId>) -> Block {
    Block::Container {
        layout_mode: LayoutMode::Flow,
        children,
    }
}

/// A representative tree that exercises every block kind.
fn sample() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        container(vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)]),
    );
    doc.metadata = Metadata {
        title: Some("A Modern Approach".into()),
        author: Some("Phillip @ TPT Solutions".into()),
        date: Some("2026-11-04".into()),
        class: Some("article".into()),
    };
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "Introduction".into(),
            style: TextStyle {
                bold: true,
                italic: false,
                heading_level: Some(1),
            },
        },
    );
    doc.insert(
        BlockId(2),
        Block::Grid {
            sheet_id: SheetId(7),
            view_bounds: Some(Rect {
                x: 0.0,
                y: 12.5,
                width: 480.0,
                height: 96.0,
            }),
        },
    );
    doc.insert(
        BlockId(3),
        Block::Media {
            source: MediaSource::Svg {
                markup: "<svg/>".into(),
            },
            alt_text: Some("A diagram".into()),
        },
    );
    doc.insert(
        BlockId(4),
        Block::Query(Query {
            source: "SELECT 1".into(),
        }),
    );
    doc
}

#[test]
fn sample_document_validates() {
    assert_eq!(sample().validate(), Ok(()));
}

#[test]
fn json_round_trip_preserves_document() {
    let doc = sample();
    let json = serde_json::to_string(&doc).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, back);
    assert_eq!(back.validate(), Ok(()));
}

#[test]
fn serialization_is_deterministic() {
    let a = serde_json::to_string(&sample()).unwrap();
    let b = serde_json::to_string(&sample()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn block_tags_are_snake_case() {
    let json = serde_json::to_string(&text("hi")).unwrap();
    assert!(json.contains(r#""type":"text""#), "{json}");
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let mut doc = sample();
    doc.schema_version = 99;
    assert_eq!(
        doc.validate(),
        Err(ValidationError::UnsupportedSchemaVersion {
            found: 99,
            supported: 1
        })
    );
}

#[test]
fn missing_root_is_rejected() {
    let mut doc = sample();
    doc.root = BlockId(42);
    assert_eq!(
        doc.validate(),
        Err(ValidationError::MissingRoot(BlockId(42)))
    );
}

#[test]
fn dangling_child_is_rejected() {
    let mut doc = sample();
    doc.insert(BlockId(0), container(vec![BlockId(1), BlockId(99)]));
    assert_eq!(
        doc.validate(),
        Err(ValidationError::DanglingChild {
            parent: BlockId(0),
            child: BlockId(99)
        })
    );
}

#[test]
fn self_cycle_is_rejected() {
    let mut doc = sample();
    doc.insert(BlockId(1), container(vec![BlockId(1)]));
    assert_eq!(doc.validate(), Err(ValidationError::Cycle(BlockId(1))));
}

#[test]
fn indirect_cycle_is_rejected() {
    let mut doc = sample();
    doc.insert(BlockId(1), container(vec![BlockId(2)]));
    doc.insert(BlockId(2), container(vec![BlockId(3)]));
    doc.insert(BlockId(3), container(vec![BlockId(1)]));
    assert!(matches!(doc.validate(), Err(ValidationError::Cycle(_))));
}

#[test]
fn shared_children_form_a_valid_dag() {
    // Two containers sharing one child is a DAG, not a cycle.
    let mut doc = sample();
    doc.insert(BlockId(1), container(vec![BlockId(4)]));
    doc.insert(BlockId(0), container(vec![BlockId(1), BlockId(4)]));
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn cycle_outside_root_reachability_is_still_rejected() {
    let mut doc = sample();
    doc.insert(BlockId(10), container(vec![BlockId(11)]));
    doc.insert(BlockId(11), container(vec![BlockId(10)]));
    assert!(matches!(doc.validate(), Err(ValidationError::Cycle(_))));
}

#[test]
fn error_messages_are_readable() {
    let msg = ValidationError::DanglingChild {
        parent: BlockId(1),
        child: BlockId(2),
    }
    .to_string();
    assert_eq!(msg, "block 1 references missing child block 2");
}

#[test]
fn list_items_are_checked_like_container_children() {
    use tpt_codex_core::{Block, BlockId, Document, LayoutMode, ValidationError};

    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2)],
        },
    );
    assert_eq!(
        doc.validate(),
        Err(ValidationError::DanglingChild {
            parent: BlockId(1),
            child: BlockId(2),
        })
    );

    doc.insert(
        BlockId(2),
        Block::Text {
            content: "item".into(),
            style: Default::default(),
        },
    );
    assert_eq!(doc.validate(), Ok(()));

    // An item that points back at its list is a cycle.
    doc.insert(
        BlockId(2),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1)],
        },
    );
    assert!(matches!(doc.validate(), Err(ValidationError::Cycle(_))));
}
