// SPDX-License-Identifier: MIT OR Apache-2.0
//! Block quotes and nested lists in Markdown: reading, writing, and the shapes refused.

use tpt_codex_core::{Block, BlockId, Document, LayoutMode};
use tpt_codex_markdown::{from_markdown, to_markdown};

/// The blocks of the root, in order.
fn root_blocks(doc: &Document) -> Vec<&Block> {
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!("root is not a container");
    };
    children
        .iter()
        .map(|id| doc.get(*id).expect("child exists"))
        .collect()
}

fn text_of(doc: &Document, id: BlockId) -> String {
    match doc.get(id) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn a_quote_reads_its_lines_as_blocks() {
    let doc = from_markdown("> first\n>\n> second\n").unwrap();
    let blocks = root_blocks(&doc);
    assert_eq!(blocks.len(), 1);
    let Block::Quote { children } = blocks[0] else {
        panic!("expected a quote, got {:?}", blocks[0]);
    };
    assert_eq!(children.len(), 2);
    assert_eq!(text_of(&doc, children[0]), "first");
    assert_eq!(text_of(&doc, children[1]), "second");
}

#[test]
fn a_quote_round_trips_with_lists_and_a_nested_quote() {
    let src = "> intro\n>\n> - one\n>   - nested\n> - two\n>\n> > deeper\n";
    let doc = from_markdown(src).unwrap();
    assert_eq!(to_markdown(&doc).unwrap(), src);
}

#[test]
fn a_gt_line_inside_a_code_fence_is_code_not_a_quote() {
    let src = "```\n> not a quote\n```\n";
    let doc = from_markdown(src).unwrap();
    assert!(matches!(
        root_blocks(&doc)[0],
        Block::Code { code, .. } if code == "> not a quote"
    ));
}

#[test]
fn a_paragraph_after_a_quote_is_a_paragraph() {
    let doc = from_markdown("> quoted\n\nafter\n").unwrap();
    let blocks = root_blocks(&doc);
    assert!(matches!(blocks[0], Block::Quote { .. }));
    assert!(matches!(blocks[1], Block::Text { content, .. } if content == "after"));
}

#[test]
fn a_sub_list_is_indented_under_its_item() {
    let src = "- a\n  - b\n    - c\n- d\n";
    let doc = from_markdown(src).unwrap();
    let blocks = root_blocks(&doc);
    let Block::List { items, .. } = blocks[0] else {
        panic!("expected a list");
    };
    // The top-level items are a, the sub-list under it, and d.
    assert_eq!(items.len(), 3);
    assert_eq!(text_of(&doc, items[0]), "a");
    let Some(Block::List {
        items: sub_items, ..
    }) = doc.get(items[1])
    else {
        panic!("the item after a is not a sub-list");
    };
    // b, then the sub-list under b that holds c.
    assert_eq!(sub_items.len(), 2);
    assert_eq!(text_of(&doc, sub_items[0]), "b");
    assert_eq!(text_of(&doc, items[2]), "d");
    assert_eq!(to_markdown(&doc).unwrap(), src);
}

#[test]
fn sub_lists_keep_their_kind_and_numbering() {
    let src = "1. first\n  - nested bullet\n2. second\n";
    let doc = from_markdown(src).unwrap();
    assert_eq!(to_markdown(&doc).unwrap(), src);
}

#[test]
fn a_sub_list_must_follow_an_item_when_written() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(3)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "x".into(),
            style: Default::default(),
        },
    );
    doc.insert(
        BlockId(2),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(3),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2)],
        },
    );
    doc.validate().unwrap();
    let err = to_markdown(&doc).unwrap_err();
    assert!(err.reason.contains("sub-list"), "{}", err.reason);
}

#[test]
fn two_adjacent_sub_lists_of_one_kind_are_refused_when_written() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(5)],
        },
    );
    let text = |content: &str| Block::Text {
        content: content.into(),
        style: Default::default(),
    };
    doc.insert(BlockId(1), text("a"));
    doc.insert(BlockId(2), text("b"));
    doc.insert(BlockId(3), text("c"));
    doc.insert(
        BlockId(4),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2)],
        },
    );
    doc.insert(
        BlockId(6),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(3)],
        },
    );
    doc.insert(
        BlockId(5),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1), BlockId(4), BlockId(6)],
        },
    );
    doc.validate().unwrap();
    assert!(to_markdown(&doc).is_err());
}

#[test]
fn an_empty_quote_is_refused_when_written() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(BlockId(1), Block::Quote { children: vec![] });
    assert!(to_markdown(&doc).is_err());
}
