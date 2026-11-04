// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::BTreeSet;

use tpt_codex_core::{
    Block, BlockId, Document, EditError, History, LayoutMode, Movement, Op, Selection, TextStyle,
    changed_blocks, delete_backward, delete_forward, insert_text,
};

fn doc(text: &str) -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    d.insert(
        BlockId(1),
        Block::Text {
            content: text.into(),
            style: TextStyle::default(),
        },
    );
    d
}

fn text(d: &Document) -> String {
    match d.get(BlockId(1)).unwrap() {
        Block::Text { content, .. } => content.clone(),
        _ => panic!("not text"),
    }
}

#[test]
fn insert_at_caret_moves_caret_past_inserted_text() {
    let mut d = doc("héllo");
    let mut h = History::new();
    let mut sel = Selection::caret(BlockId(1), 1);
    insert_text(&mut d, &mut h, &mut sel, "XY").unwrap();
    assert_eq!(text(&d), "hXYéllo");
    assert_eq!(sel, Selection::caret(BlockId(1), 3));
}

#[test]
fn insert_over_range_replaces_it() {
    let mut d = doc("abcdef");
    let mut h = History::new();
    // Backwards range (focus before anchor) is normalised.
    let mut sel = Selection::range(BlockId(1), 4, 2);
    insert_text(&mut d, &mut h, &mut sel, "-").unwrap();
    assert_eq!(text(&d), "ab-ef");
    assert_eq!(sel, Selection::caret(BlockId(1), 3));
}

#[test]
fn insert_empty_text_deletes_selection() {
    let mut d = doc("abcdef");
    let mut h = History::new();
    let mut sel = Selection::range(BlockId(1), 1, 3);
    insert_text(&mut d, &mut h, &mut sel, "").unwrap();
    assert_eq!(text(&d), "adef");
    assert!(sel.is_collapsed());
}

#[test]
fn backspace_removes_previous_character_and_stops_at_start() {
    let mut d = doc("aé");
    let mut h = History::new();
    let mut sel = Selection::caret(BlockId(1), 2);
    assert!(delete_backward(&mut d, &mut h, &mut sel).unwrap());
    assert_eq!(text(&d), "a");
    assert_eq!(sel, Selection::caret(BlockId(1), 1));
    assert!(delete_backward(&mut d, &mut h, &mut sel).unwrap());
    assert_eq!(text(&d), "");
    assert!(!delete_backward(&mut d, &mut h, &mut sel).unwrap());
    assert_eq!(text(&d), "");
}

#[test]
fn delete_key_removes_next_character_and_stops_at_end() {
    let mut d = doc("ab");
    let mut h = History::new();
    let mut sel = Selection::caret(BlockId(1), 0);
    assert!(delete_forward(&mut d, &mut h, &mut sel).unwrap());
    assert_eq!(text(&d), "b");
    assert!(delete_forward(&mut d, &mut h, &mut sel).unwrap());
    assert!(!delete_forward(&mut d, &mut h, &mut sel).unwrap());
    assert_eq!(text(&d), "");
}

#[test]
fn each_edit_is_one_undo_step() {
    let mut d = doc("abc");
    let mut h = History::new();
    let mut sel = Selection::caret(BlockId(1), 3);
    insert_text(&mut d, &mut h, &mut sel, "d").unwrap();
    delete_backward(&mut d, &mut h, &mut sel).unwrap();
    insert_text(&mut d, &mut h, &mut sel, "xyz").unwrap();
    assert_eq!(text(&d), "abcxyz");
    assert_eq!(h.undo_len(), 3);
    h.undo(&mut d).unwrap();
    assert_eq!(text(&d), "abc");
    h.undo(&mut d).unwrap();
    assert_eq!(text(&d), "abcd");
    h.redo(&mut d).unwrap();
    assert_eq!(text(&d), "abc");
}

#[test]
fn movement_without_extend_collapses_range_to_its_edges() {
    let d = doc("abcdef");
    let mut sel = Selection::range(BlockId(1), 4, 1);
    sel.move_caret(&d, Movement::Right, false).unwrap();
    assert_eq!(sel, Selection::caret(BlockId(1), 4));

    let mut sel = Selection::range(BlockId(1), 4, 1);
    sel.move_caret(&d, Movement::Left, false).unwrap();
    assert_eq!(sel, Selection::caret(BlockId(1), 1));
}

#[test]
fn movement_is_clamped_to_text() {
    let d = doc("ab");
    let mut sel = Selection::caret(BlockId(1), 0);
    sel.move_caret(&d, Movement::Left, false).unwrap();
    assert_eq!(sel.focus, 0);
    sel.move_caret(&d, Movement::Right, false).unwrap();
    sel.move_caret(&d, Movement::Right, false).unwrap();
    sel.move_caret(&d, Movement::Right, false).unwrap();
    assert_eq!(sel.focus, 2);
    sel.move_caret(&d, Movement::Start, false).unwrap();
    assert_eq!(sel.focus, 0);
    sel.move_caret(&d, Movement::End, false).unwrap();
    assert_eq!(sel.focus, 2);
}

#[test]
fn extend_keeps_anchor_and_grows_range() {
    let d = doc("abcdef");
    let mut sel = Selection::caret(BlockId(1), 2);
    sel.move_caret(&d, Movement::Right, true).unwrap();
    sel.move_caret(&d, Movement::Right, true).unwrap();
    assert_eq!(sel.anchor, 2);
    assert_eq!(sel.bounds(), (2, 4));
    sel.move_caret(&d, Movement::Start, true).unwrap();
    assert_eq!(sel.bounds(), (0, 2));
}

#[test]
fn select_all_covers_whole_text() {
    let d = doc("héllo");
    let mut sel = Selection::caret(BlockId(1), 2);
    sel.select_all(&d).unwrap();
    assert_eq!(sel.bounds(), (0, 5));
}

#[test]
fn non_text_and_missing_blocks_are_errors() {
    let d = doc("a");
    let mut sel = Selection::caret(BlockId(0), 0);
    assert_eq!(
        sel.move_caret(&d, Movement::Left, false),
        Err(EditError::NotText(BlockId(0)))
    );
    let mut sel = Selection::caret(BlockId(9), 0);
    assert_eq!(
        sel.move_caret(&d, Movement::Left, false),
        Err(EditError::NoSuchBlock(BlockId(9)))
    );
}

#[test]
fn changed_blocks_names_the_container_for_child_operations() {
    let ops = vec![
        Op::SetBlock {
            id: BlockId(1),
            block: Block::ThematicBreak,
        },
        Op::MoveChild {
            from_parent: BlockId(0),
            from_index: 0,
            to_parent: BlockId(2),
            to_index: 0,
        },
    ];
    let expected: BTreeSet<BlockId> = [BlockId(0), BlockId(1), BlockId(2)].into();
    assert_eq!(changed_blocks(&ops), expected);
}

#[test]
fn history_reports_changes_of_last_step_including_undo() {
    let mut d = doc("abc");
    let mut h = History::new();
    assert!(h.last_changed().is_empty());

    let mut sel = Selection::caret(BlockId(1), 3);
    insert_text(&mut d, &mut h, &mut sel, "d").unwrap();
    assert_eq!(h.last_changed(), &[BlockId(1)].into());

    h.undo(&mut d).unwrap();
    assert_eq!(h.last_changed(), &[BlockId(1)].into());

    // A failed commit leaves the previous step's record alone.
    let failed = h.commit(&mut d, vec![Op::RemoveBlock { id: BlockId(1) }]);
    assert!(failed.is_err());
    assert_eq!(h.last_changed(), &[BlockId(1)].into());
}
