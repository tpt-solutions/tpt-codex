// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Rect};

/// An empty document: a root with no children.
fn empty() -> Document {
    Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: Vec::new(),
        },
    )
}

fn root_children(editor: &Editor) -> Vec<BlockId> {
    match editor.document().get(editor.document().root) {
        Some(Block::Container { children, .. }) => children.clone(),
        other => panic!("root is not a container: {other:?}"),
    }
}

#[test]
fn insert_list_adds_a_list_with_one_empty_item_and_puts_the_caret_in_it() {
    let mut editor = Editor::new(empty());
    editor.update(Msg::InsertList);
    assert_eq!(editor.status(), None);

    let children = root_children(&editor);
    assert_eq!(children.len(), 1);
    let Some(Block::List { ordered, items, .. }) = editor.document().get(children[0]) else {
        panic!("not a list");
    };
    assert!(!ordered);
    assert_eq!(items.len(), 1);
    let item = items[0];
    assert!(
        matches!(editor.document().get(item), Some(Block::Text { content, .. }) if content.is_empty())
    );

    // Typing goes into the new item.
    editor.update(Msg::Type("milk".into()));
    assert!(matches!(
        editor.document().get(item),
        Some(Block::Text { content, .. }) if content == "milk"
    ));
}

#[test]
fn insert_list_is_one_undoable_step() {
    let mut editor = Editor::new(empty());
    editor.update(Msg::InsertList);
    editor.update(Msg::Undo);
    assert!(root_children(&editor).is_empty());
}

#[test]
fn insert_query_adds_a_query_block_at_the_end() {
    let mut editor = Editor::new(empty());
    editor.update(Msg::InsertQuery);
    let children = root_children(&editor);
    assert_eq!(children.len(), 1);
    assert!(matches!(
        editor.document().get(children[0]),
        Some(Block::Query(_))
    ));
    editor.update(Msg::Undo);
    assert!(root_children(&editor).is_empty());
}

/// A document with one slide, and an empty slide added to the root.
fn with_slide() -> (Document, BlockId) {
    let mut editor = Editor::new(empty());
    editor.update(Msg::InsertSlide);
    let slide = root_children(&editor)[0];
    (editor.document().clone(), slide)
}

#[test]
fn insert_slide_text_places_a_selected_text_box_on_the_slide() {
    let (doc, slide) = with_slide();
    let mut editor = Editor::new(doc);
    editor.update(Msg::InsertSlideText { slide });
    assert_eq!(editor.status(), None);

    let block = editor.selected_block().expect("the new box is selected");
    assert!(
        matches!(editor.document().get(block), Some(Block::Text { content, .. }) if content == "Text")
    );
    let frame = editor.document().frames.get(&block).copied();
    assert_eq!(
        frame,
        Some(Rect {
            x: 40.0,
            y: 40.0,
            width: 320.0,
            height: 80.0,
        })
    );
    // The slide now holds the box.
    assert_eq!(editor.document().get(slide).map_or(0, child_count), 1);
}

#[test]
fn insert_slide_text_is_one_undoable_step() {
    let (doc, slide) = with_slide();
    let mut editor = Editor::new(doc);
    editor.update(Msg::InsertSlideText { slide });
    editor.update(Msg::Undo);
    assert_eq!(
        editor.document().get(slide).map_or(usize::MAX, child_count),
        0
    );
}

#[test]
fn insert_slide_text_on_a_block_that_is_not_a_slide_is_reported() {
    let mut editor = Editor::new(empty());
    editor.update(Msg::InsertSlideText { slide: BlockId(0) });
    assert!(editor.status().unwrap().contains("is not a slide"));
}

fn child_count(block: &Block) -> usize {
    match block {
        Block::Container { children, .. } => children.len(),
        _ => 0,
    }
}
