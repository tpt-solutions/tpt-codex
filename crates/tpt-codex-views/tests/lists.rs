// SPDX-License-Identifier: MIT OR Apache-2.0
//! Lists in the flow: each item is text lines with a marker on its first line, indented from the
//! column edge, and never a placeholder box.
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{FlowKind, FlowOptions, LIST_INDENT, MonospaceMeasure, layout_flow};

const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 1.0,
    line_height: 10.0,
};

fn options(width: f64) -> FlowOptions {
    FlowOptions {
        width,
        block_spacing: 0.0,
        box_height: 50.0,
        row_height: 20.0,
    }
}

fn text(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

/// A root flow container whose only child is a list (id 1) over text items (ids 2, 3, ...).
fn list_doc(ordered: bool, start: u32, items: &[&str]) -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    let ids: Vec<BlockId> = (0..items.len() as u64).map(|i| BlockId(2 + i)).collect();
    doc.insert(
        BlockId(1),
        Block::List {
            ordered,
            start,
            items: ids.clone(),
        },
    );
    for (id, s) in ids.into_iter().zip(items) {
        doc.insert(id, text(s));
    }
    doc
}

#[test]
fn bulleted_items_get_a_bullet_on_their_first_line_and_an_indent() {
    let doc = list_doc(false, 1, &["first", "second"]);
    let layout = layout_flow(&doc, &MEASURE, options(200.0)).unwrap();

    assert!(
        layout
            .items
            .iter()
            .all(|i| !matches!(i.kind, FlowKind::Box)),
        "a list is laid out as text, not a placeholder box"
    );
    let lines: Vec<(&str, Option<&str>, f64)> = layout
        .items
        .iter()
        .map(|i| match &i.kind {
            FlowKind::Line {
                text,
                marker,
                indent,
                ..
            } => (text.as_str(), marker.as_deref(), *indent),
            other => panic!("unexpected item {other:?}"),
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            ("first", Some("\u{2022}"), LIST_INDENT),
            ("second", Some("\u{2022}"), LIST_INDENT),
        ]
    );
}

#[test]
fn numbered_items_count_from_start() {
    let doc = list_doc(true, 3, &["a", "b", "c"]);
    let layout = layout_flow(&doc, &MEASURE, options(200.0)).unwrap();
    let markers: Vec<Option<&str>> = layout
        .items
        .iter()
        .map(|i| match &i.kind {
            FlowKind::Line { marker, .. } => marker.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(markers, vec![Some("3."), Some("4."), Some("5.")]);
}

#[test]
fn wrapped_item_marks_only_its_first_line() {
    // Width 30 less the indent leaves 12 points: "one two three" wraps to several lines.
    let doc = list_doc(false, 1, &["one two three four"]);
    let layout = layout_flow(&doc, &MEASURE, options(LIST_INDENT + 12.0)).unwrap();
    let markers: Vec<Option<&str>> = layout
        .items
        .iter()
        .map(|i| match &i.kind {
            FlowKind::Line { marker, .. } => marker.as_deref(),
            _ => None,
        })
        .collect();
    assert!(markers.len() > 1, "the item should wrap: {markers:?}");
    assert_eq!(markers[0], Some("\u{2022}"));
    assert!(markers[1..].iter().all(Option::is_none));
}

#[test]
fn text_outside_a_list_has_no_marker_or_indent() {
    let doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    let mut doc = doc;
    doc.insert(BlockId(1), text("plain"));
    let layout = layout_flow(&doc, &MEASURE, options(200.0)).unwrap();
    assert_eq!(layout.items.len(), 1);
    match &layout.items[0].kind {
        FlowKind::Line { marker, indent, .. } => {
            assert_eq!(marker, &None);
            assert_eq!(*indent, 0.0);
        }
        other => panic!("unexpected item {other:?}"),
    }
}
