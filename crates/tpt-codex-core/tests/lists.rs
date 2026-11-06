// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::lists::{add_item, remove_item, set_kind};
use tpt_codex_core::{Block, BlockId, Document, EditError, History, LayoutMode, Op, TextStyle};

fn text(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

/// Root 0 -> [1]; 1 is a bulleted list of [2, 3]; 4 is a paragraph outside the list.
fn base() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(4)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(2), BlockId(3)],
        },
    );
    doc.insert(BlockId(2), text("one"));
    doc.insert(BlockId(3), text("two"));
    doc.insert(BlockId(4), text("after"));
    doc
}

fn items(doc: &Document, id: u64) -> Vec<u64> {
    match doc.get(BlockId(id)).unwrap() {
        Block::List { items, .. } => items.iter().map(|i| i.0).collect(),
        _ => panic!("not a list"),
    }
}

fn content(doc: &Document, id: u64) -> String {
    match doc.get(BlockId(id)).unwrap() {
        Block::Text { content, .. } => content.clone(),
        _ => panic!("not text"),
    }
}

#[test]
fn add_item_appends_and_undo_removes_it() {
    let mut doc = base();
    let mut hist = History::new();
    let (id, ops) = add_item(&doc, BlockId(1), 2, "three").unwrap();
    hist.commit(&mut doc, ops).unwrap();
    assert_eq!(items(&doc, 1), vec![2, 3, id.0]);
    assert_eq!(content(&doc, id.0), "three");
    assert!(
        hist.last_changed().contains(&BlockId(1)),
        "the list is re-rendered"
    );

    hist.undo(&mut doc).unwrap();
    assert_eq!(items(&doc, 1), vec![2, 3]);
    assert!(doc.get(id).is_none(), "the new block is removed again");
    doc.validate().unwrap();
}

#[test]
fn add_item_inserts_at_the_front() {
    let mut doc = base();
    let mut hist = History::new();
    let (id, ops) = add_item(&doc, BlockId(1), 0, "zero").unwrap();
    hist.commit(&mut doc, ops).unwrap();
    assert_eq!(items(&doc, 1), vec![id.0, 2, 3]);
    doc.validate().unwrap();
}

#[test]
fn add_item_rejects_an_index_past_the_end() {
    let doc = base();
    assert_eq!(
        add_item(&doc, BlockId(1), 3, "x").unwrap_err(),
        EditError::IndexOutOfBounds {
            parent: BlockId(1),
            index: 3,
            len: 2
        }
    );
}

#[test]
fn remove_item_deletes_the_block_and_undo_restores_it() {
    let mut doc = base();
    let mut hist = History::new();
    let ops = remove_item(&doc, BlockId(1), 0).unwrap();
    hist.commit(&mut doc, ops).unwrap();
    assert_eq!(items(&doc, 1), vec![3]);
    assert!(doc.get(BlockId(2)).is_none());
    doc.validate().unwrap();

    hist.undo(&mut doc).unwrap();
    assert_eq!(items(&doc, 1), vec![2, 3]);
    assert_eq!(content(&doc, 2), "one");
    doc.validate().unwrap();
}

#[test]
fn remove_item_rejects_a_missing_index() {
    let doc = base();
    assert!(matches!(
        remove_item(&doc, BlockId(1), 2),
        Err(EditError::IndexOutOfBounds { len: 2, .. })
    ));
}

#[test]
fn set_kind_switches_to_numbered_and_back() {
    let mut doc = base();
    let mut hist = History::new();
    let ops = set_kind(&doc, BlockId(1), true).unwrap();
    hist.commit(&mut doc, ops).unwrap();
    assert!(matches!(
        doc.get(BlockId(1)),
        Some(Block::List {
            ordered: true,
            start: 1,
            ..
        })
    ));
    assert!(hist.last_changed().contains(&BlockId(1)));

    let ops = set_kind(&doc, BlockId(1), false).unwrap();
    hist.commit(&mut doc, ops).unwrap();
    assert!(matches!(
        doc.get(BlockId(1)),
        Some(Block::List {
            ordered: false,
            start: 1,
            ..
        })
    ));
    doc.validate().unwrap();

    hist.undo(&mut doc).unwrap();
    assert!(matches!(
        doc.get(BlockId(1)),
        Some(Block::List { ordered: true, .. })
    ));
}

#[test]
fn set_kind_to_the_same_kind_is_a_no_op() {
    let doc = base();
    assert!(set_kind(&doc, BlockId(1), false).unwrap().is_empty());
}

#[test]
fn set_kind_keeps_the_start_of_a_numbered_list() {
    let mut doc = base();
    doc.insert(
        BlockId(1),
        Block::List {
            ordered: true,
            start: 5,
            items: vec![BlockId(2), BlockId(3)],
        },
    );
    let ops = set_kind(&doc, BlockId(1), false).unwrap();
    assert!(matches!(
        ops.as_slice(),
        [Op::SetBlock {
            block: Block::List {
                ordered: false,
                start: 1,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn list_operations_reject_a_block_that_is_not_a_list() {
    let doc = base();
    assert_eq!(
        set_kind(&doc, BlockId(4), true).unwrap_err(),
        EditError::NotAList(BlockId(4))
    );
    assert_eq!(
        add_item(&doc, BlockId(4), 0, "x").unwrap_err(),
        EditError::NotAList(BlockId(4))
    );
    assert_eq!(
        remove_item(&doc, BlockId(4), 0).unwrap_err(),
        EditError::NotAList(BlockId(4))
    );
    assert_eq!(
        set_kind(&doc, BlockId(99), true).unwrap_err(),
        EditError::NoSuchBlock(BlockId(99))
    );
}

#[test]
fn a_list_holds_text_items_only() {
    let mut doc = base();
    doc.insert(BlockId(7), Block::ThematicBreak);
    let mut hist = History::new();
    let err = hist
        .commit(
            &mut doc,
            vec![Op::InsertChild {
                parent: BlockId(1),
                index: 0,
                child: BlockId(7),
            }],
        )
        .unwrap_err();
    assert_eq!(err, EditError::NotText(BlockId(7)));
    assert_eq!(
        items(&doc, 1),
        vec![2, 3],
        "the rejected step changed nothing"
    );
}

#[test]
fn remove_child_accepts_a_list_parent() {
    let mut doc = base();
    let mut hist = History::new();
    hist.commit(
        &mut doc,
        vec![Op::RemoveChild {
            parent: BlockId(1),
            index: 1,
        }],
    )
    .unwrap();
    assert_eq!(items(&doc, 1), vec![2]);
}
