// SPDX-License-Identifier: MIT OR Apache-2.0
//! Block quotes and sub-lists, the schema version 3 shapes.

use tpt_codex_core::{
    Block, BlockId, Document, History, LayoutMode, Op, TextStyle, ValidationError,
};

fn text(content: &str) -> Block {
    Block::Text {
        content: content.into(),
        style: TextStyle::default(),
    }
}

/// A document whose root holds the blocks `root`.
fn doc_with(root: Vec<BlockId>) -> Document {
    Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: root,
        },
    )
}

#[test]
fn a_sub_list_is_a_valid_list_item() {
    let mut doc = doc_with(vec![BlockId(4)]);
    doc.insert(BlockId(1), text("outer"));
    doc.insert(BlockId(2), text("inner"));
    doc.insert(
        BlockId(3),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2)],
        },
    );
    doc.insert(
        BlockId(4),
        Block::List {
            ordered: true,
            start: 1,
            items: vec![BlockId(1), BlockId(3)],
        },
    );
    doc.validate().unwrap();
}

#[test]
fn a_quote_is_refused_as_a_list_item() {
    let mut doc = doc_with(vec![BlockId(4)]);
    doc.insert(BlockId(1), text("item"));
    doc.insert(BlockId(2), Block::Quote { children: vec![] });
    doc.insert(
        BlockId(4),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1), BlockId(2)],
        },
    );
    assert_eq!(
        doc.validate(),
        Err(ValidationError::ListItemNotText(BlockId(4)))
    );
}

#[test]
fn a_quote_holds_blocks_and_its_children_must_exist() {
    let mut doc = doc_with(vec![BlockId(2)]);
    doc.insert(BlockId(1), text("quoted"));
    doc.insert(
        BlockId(2),
        Block::Quote {
            children: vec![BlockId(1), BlockId(9)],
        },
    );
    assert_eq!(
        doc.validate(),
        Err(ValidationError::DanglingChild {
            parent: BlockId(2),
            child: BlockId(9),
        })
    );
}

#[test]
fn a_sub_list_can_be_added_to_a_list_as_one_undoable_step() {
    let mut doc = doc_with(vec![BlockId(3)]);
    doc.insert(BlockId(1), text("first"));
    doc.insert(BlockId(2), text("nested"));
    doc.insert(
        BlockId(3),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1)],
        },
    );
    let mut history = History::new();
    history
        .commit(
            &mut doc,
            vec![
                Op::AddBlock {
                    id: BlockId(4),
                    block: Block::List {
                        ordered: true,
                        start: 1,
                        items: vec![BlockId(2)],
                    },
                },
                Op::InsertChild {
                    parent: BlockId(3),
                    index: 1,
                    child: BlockId(4),
                },
            ],
        )
        .unwrap();
    doc.validate().unwrap();
    assert!(history.undo(&mut doc).unwrap());
    assert_eq!(
        doc.get(BlockId(3)),
        Some(&Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(1)],
        })
    );
}
