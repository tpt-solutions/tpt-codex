// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headless Milestone 1 path: a blank document is typed into with a caret, an
//! equation block is inserted as one undoable step, and the result is saved, reloaded
//! and written as Markdown.

use std::fs;
use std::path::PathBuf;

use tpt_codex_app::{DocumentStore, FileStorage};
use tpt_codex_core::{
    Block, BlockId, Document, History, LayoutMode, Movement, Op, Selection, TextStyle,
    delete_backward, insert_text,
};
use tpt_codex_markdown::to_markdown;
use tpt_typeset::check_math_blocks;

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("tpt-codex-typing-{}-{}", std::process::id(), name))
}

fn text_of(doc: &Document, id: u64) -> String {
    match doc.get(BlockId(id)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn typing_with_caret_undo_math_and_persistence() {
    let dir = scratch("session");
    let _ = fs::remove_dir_all(&dir);

    // A blank document: a flow container holding one empty paragraph.
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: String::new(),
            style: TextStyle::default(),
        },
    );
    let mut history = History::new();
    let mut caret = Selection::caret(BlockId(1), 0);

    // Type "Helo" (four steps), step back, add the missing "l", then delete one character at the end.
    for ch in ["H", "e", "l", "o"] {
        insert_text(&mut doc, &mut history, &mut caret, ch).unwrap();
    }
    caret.move_caret(&doc, Movement::Left, false).unwrap();
    insert_text(&mut doc, &mut history, &mut caret, "l").unwrap();
    assert_eq!(text_of(&doc, 1), "Hello");
    caret.move_caret(&doc, Movement::End, false).unwrap();
    assert!(delete_backward(&mut doc, &mut history, &mut caret).unwrap());
    assert_eq!(text_of(&doc, 1), "Hell");
    assert_eq!(history.last_changed(), &[BlockId(1)].into());

    // Undo the deletion, then undo the "l" insertion, then redo it.
    history.undo(&mut doc).unwrap();
    assert_eq!(text_of(&doc, 1), "Hello");
    history.undo(&mut doc).unwrap();
    assert_eq!(text_of(&doc, 1), "Helo");
    history.redo(&mut doc).unwrap();
    assert_eq!(text_of(&doc, 1), "Hello");

    // Insert an equation block after the paragraph as one undoable step.
    history
        .commit(
            &mut doc,
            vec![
                Op::AddBlock {
                    id: BlockId(2),
                    block: Block::Math {
                        source: r"\frac{a}{b} = c".into(),
                    },
                },
                Op::InsertChild {
                    parent: BlockId(0),
                    index: 1,
                    child: BlockId(2),
                },
            ],
        )
        .unwrap();
    assert_eq!(history.last_changed(), &[BlockId(0), BlockId(2)].into());
    doc.validate().unwrap();
    assert!(check_math_blocks(&doc).is_empty());

    // Save, reload, and compare.
    let mut store = DocumentStore::new(FileStorage::new(&dir));
    store.save("session", &doc).unwrap();
    let reloaded = store.load("session").unwrap();
    assert_eq!(reloaded, doc);
    fs::remove_dir_all(&dir).ok();

    // The Markdown form has the typed paragraph and the equation.
    assert_eq!(
        to_markdown(&reloaded).unwrap(),
        "Hello\n\n$$\n\\frac{a}{b} = c\n$$\n"
    );
}
