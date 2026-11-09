// SPDX-License-Identifier: MIT OR Apache-2.0
//! Block quotes and sub-lists in the flow: a quote's text is indented from the margin, and a
//! sub-list sits one list indent further in than the item above it, with its own numbers.

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

/// A root flow container over `children`. Other blocks are added with `insert`.
fn root(children: Vec<BlockId>) -> Document {
    Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children,
        },
    )
}

/// The `(text, marker, indent)` of each text line in the layout, in order.
fn lines(doc: &Document) -> Vec<(String, Option<String>, f64)> {
    let layout = layout_flow(doc, &MEASURE, options(400.0)).unwrap();
    layout
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            FlowKind::Line {
                text,
                marker,
                indent,
                ..
            } => Some((text.clone(), marker.clone(), *indent)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_quote_indents_its_text_from_the_margin() {
    let mut doc = root(vec![BlockId(1), BlockId(2)]);
    doc.insert(BlockId(1), text("plain"));
    doc.insert(
        BlockId(2),
        Block::Quote {
            children: vec![BlockId(3)],
        },
    );
    doc.insert(BlockId(3), text("quoted"));
    let got = lines(&doc);
    assert_eq!(got[0], ("plain".to_string(), None, 0.0));
    assert_eq!(got[1].0, "quoted");
    assert!(got[1].2 > 0.0, "quoted text is indented: {:?}", got[1]);
}

#[test]
fn a_sub_list_sits_one_list_indent_further_in() {
    // - a
    //   - b
    let mut doc = root(vec![BlockId(1)]);
    doc.insert(
        BlockId(1),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2), BlockId(4)],
        },
    );
    doc.insert(BlockId(2), text("a"));
    doc.insert(
        BlockId(4),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(3)],
        },
    );
    doc.insert(BlockId(3), text("b"));
    let got = lines(&doc);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, "a");
    assert_eq!(got[0].2, LIST_INDENT);
    assert_eq!(got[1].0, "b");
    assert_eq!(got[1].2, 2.0 * LIST_INDENT);
    assert_eq!(got[1].1.as_deref(), Some("\u{2022}"));
}

#[test]
fn numbers_count_the_list_own_items_not_its_sub_lists() {
    // 1. a / - x / 2. c
    let mut doc = root(vec![BlockId(1)]);
    doc.insert(
        BlockId(1),
        Block::List {
            ordered: true,
            start: 1,
            items: vec![BlockId(2), BlockId(4), BlockId(3)],
        },
    );
    doc.insert(BlockId(2), text("a"));
    doc.insert(BlockId(3), text("c"));
    doc.insert(
        BlockId(4),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(5)],
        },
    );
    doc.insert(BlockId(5), text("x"));
    let markers: Vec<Option<String>> = lines(&doc).into_iter().map(|l| l.1).collect();
    assert_eq!(
        markers,
        vec![
            Some("1.".to_string()),
            Some("\u{2022}".to_string()),
            Some("2.".to_string()),
        ]
    );
}

#[test]
fn a_text_table_is_laid_out_as_one_line_per_row() {
    use tpt_codex_core::Block;
    use tpt_codex_core::{BlockId, Document, LayoutMode};
    use tpt_codex_views::{FlowKind, FlowOptions, MonospaceMeasure, layout_flow};
    let rows = vec![
        vec!["a".to_string(), "b".to_string()],
        vec!["c".to_string(), "d".to_string()],
    ];
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(BlockId(1), Block::Table { rows });
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 10.0,
    };
    let options = FlowOptions {
        width: 400.0,
        block_spacing: 0.0,
        box_height: 10.0,
        row_height: 10.0,
    };
    let layout = layout_flow(&doc, &measure, options).unwrap();
    let lines: Vec<&str> = layout
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            FlowKind::Line { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(lines, ["a | b", "c | d"]);
    assert!(
        layout
            .items
            .iter()
            .all(|i| !matches!(i.kind, FlowKind::Box))
    );
}
