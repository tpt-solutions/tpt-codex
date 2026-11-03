// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Document, EditError, History, LayoutMode, TextStyle, edit_text, replace_chars,
};

fn doc() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    d.insert(
        BlockId(1),
        Block::Text {
            content: "héllo wörld".into(),
            style: TextStyle {
                bold: true,
                italic: false,
                heading_level: None,
            },
        },
    );
    d.insert(
        BlockId(2),
        Block::Text {
            content: "x".into(),
            style: TextStyle::default(),
        },
    );
    d
}

fn text_of(d: &Document, id: u64) -> String {
    match d.get(BlockId(id)).unwrap() {
        Block::Text { content, .. } => content.clone(),
        _ => panic!(),
    }
}

#[test]
fn replace_counts_characters_not_bytes() {
    assert_eq!(replace_chars("héllo", 1, 2, "e").as_deref(), Some("hello"));
    assert_eq!(replace_chars("abc", 3, 3, "d").as_deref(), Some("abcd"));
    assert_eq!(replace_chars("abc", 0, 3, "").as_deref(), Some(""));
    assert_eq!(replace_chars("abc", 2, 1, "x"), None);
    assert_eq!(replace_chars("abc", 0, 4, "x"), None);
}

#[test]
fn edit_text_keeps_style_and_is_undoable() {
    let mut d = doc();
    let mut h = History::new();
    edit_text(&mut d, &mut h, BlockId(1), 6, 11, "there").unwrap();
    assert_eq!(text_of(&d, 1), "héllo there");
    match d.get(BlockId(1)).unwrap() {
        Block::Text { style, .. } => assert!(style.bold),
        _ => panic!(),
    }
    assert!(h.undo(&mut d).unwrap());
    assert_eq!(text_of(&d, 1), "héllo wörld");
}

#[test]
fn edit_text_rejects_wrong_block_and_bad_range() {
    let mut d = doc();
    let mut h = History::new();
    assert_eq!(
        edit_text(&mut d, &mut h, BlockId(0), 0, 0, "x"),
        Err(EditError::NotText(BlockId(0)))
    );
    assert_eq!(
        edit_text(&mut d, &mut h, BlockId(99), 0, 0, "x"),
        Err(EditError::NoSuchBlock(BlockId(99)))
    );
    assert_eq!(
        edit_text(&mut d, &mut h, BlockId(2), 0, 5, "x"),
        Err(EditError::RangeOutOfBounds {
            id: BlockId(2),
            len: 1
        })
    );
    assert_eq!(text_of(&d, 2), "x");
    assert_eq!(h.undo_len(), 0);
}
