// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};

/// Root 0 -> [1, 4]; 1 is a bulleted list of [2, 3]; 4 is a paragraph.
fn document() -> Document {
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
    doc.insert(
        BlockId(2),
        Block::Text {
            content: "one".into(),
            style: TextStyle::default(),
        },
    );
    doc.insert(
        BlockId(3),
        Block::Text {
            content: "two".into(),
            style: TextStyle::default(),
        },
    );
    doc.insert(
        BlockId(4),
        Block::Text {
            content: "after".into(),
            style: TextStyle::default(),
        },
    );
    doc
}

fn items(editor: &Editor) -> Vec<u64> {
    match editor.document().get(BlockId(1)).unwrap() {
        Block::List { items, .. } => items.iter().map(|i| i.0).collect(),
        other => panic!("not a list: {other:?}"),
    }
}

fn ordered(editor: &Editor) -> bool {
    match editor.document().get(BlockId(1)).unwrap() {
        Block::List { ordered, .. } => *ordered,
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn add_item_appends_an_empty_item_with_the_caret_in_it_and_typing_fills_it() {
    let mut editor = Editor::new(document());
    editor.update(Msg::AddListItem { list: BlockId(1) });
    let new_items = items(&editor);
    assert_eq!(new_items.len(), 3);
    let new = BlockId(new_items[2]);
    assert_eq!(editor.caret().map(|s| s.block), Some(new));

    editor.update(Msg::Type("three".into()));
    match editor.document().get(new) {
        Some(Block::Text { content, .. }) => assert_eq!(content, "three"),
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn undo_removes_an_added_item_in_one_step() {
    let mut editor = Editor::new(document());
    editor.update(Msg::AddListItem { list: BlockId(1) });
    editor.update(Msg::Undo);
    assert_eq!(items(&editor), vec![2, 3]);
}

#[test]
fn toggle_switches_between_bulleted_and_numbered_and_back() {
    let mut editor = Editor::new(document());
    assert!(!ordered(&editor));
    editor.update(Msg::ToggleListKind { list: BlockId(1) });
    assert!(ordered(&editor));
    editor.update(Msg::ToggleListKind { list: BlockId(1) });
    assert!(!ordered(&editor));
    editor.update(Msg::Undo);
    assert!(ordered(&editor));
}

#[test]
fn remove_deletes_the_item_and_its_caret_is_dropped_with_it() {
    let mut editor = Editor::new(document());
    editor.update(Msg::Focus(BlockId(2)));
    editor.update(Msg::RemoveListItem {
        list: BlockId(1),
        index: 0,
    });
    assert_eq!(items(&editor), vec![3]);
    assert!(editor.document().get(BlockId(2)).is_none());
    assert_eq!(editor.caret(), None, "the caret was in the removed item");

    editor.update(Msg::Undo);
    assert_eq!(items(&editor), vec![2, 3]);
}

#[test]
fn list_commands_on_a_paragraph_report_an_error_and_change_nothing() {
    let mut editor = Editor::new(document());
    editor.update(Msg::ToggleListKind { list: BlockId(4) });
    assert!(editor.status().is_some_and(|s| s.contains("not a list")));
    editor.update(Msg::AddListItem { list: BlockId(4) });
    assert_eq!(items(&editor), vec![2, 3]);
    assert!(!ordered(&editor));
}

#[test]
fn the_view_shows_list_controls_and_a_cached_view_matches_a_full_render() {
    let mut editor = Editor::new(document());
    editor.update(Msg::AddListItem { list: BlockId(1) });
    editor.update(Msg::Type("x".into()));
    let cached = format!("{:?}", editor.view());
    assert!(cached.contains("Add item"));
    assert!(cached.contains("Make numbered"));
    assert!(cached.contains("Remove item"));

    editor.invalidate_view();
    assert_eq!(format!("{:?}", editor.view()), cached);
}
