// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, ChangeKind, Document, History, LayoutMode, Op, TextStyle, VersionLog, edit_text,
};

fn doc() -> Document {
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
            content: "one".into(),
            style: TextStyle::default(),
        },
    );
    d
}

#[test]
fn restore_returns_the_saved_document_and_the_log_is_unchanged() {
    let mut log = VersionLog::new();
    let mut d = doc();
    let first = log.save("draft", &d);

    let mut h = History::new();
    edit_text(&mut d, &mut h, BlockId(1), 3, 3, " two").unwrap();
    let second = log.save("edited", &d);

    assert_eq!((first, second, log.len()), (0, 1, 2));
    assert_eq!(log.restore(first).unwrap(), doc());
    assert_eq!(log.restore(second).unwrap(), d);
    assert!(log.restore(9).is_none());
    assert_eq!(log.get(1).unwrap().label, "edited");
}

#[test]
fn diff_names_changed_added_and_removed_blocks() {
    let mut log = VersionLog::new();
    let mut d = doc();
    log.save("a", &d);

    let mut h = History::new();
    edit_text(&mut d, &mut h, BlockId(1), 0, 0, "x").unwrap();
    d.insert(
        BlockId(5),
        Block::Text {
            content: "new".into(),
            style: TextStyle::default(),
        },
    );
    h.commit(
        &mut d,
        vec![Op::AddBlock {
            id: BlockId(6),
            block: Block::ThematicBreak,
        }],
    )
    .unwrap();
    log.save("b", &d);

    let changes = log.diff(0, 1).unwrap();
    assert!(changes.contains(&tpt_codex_core::BlockChange {
        block: BlockId(1),
        kind: ChangeKind::Changed
    }));
    assert!(changes.contains(&tpt_codex_core::BlockChange {
        block: BlockId(6),
        kind: ChangeKind::Added
    }));
    // Going back the other way, the same blocks are removed.
    let back = log.diff(1, 0).unwrap();
    assert!(back.contains(&tpt_codex_core::BlockChange {
        block: BlockId(6),
        kind: ChangeKind::Removed
    }));
    assert!(log.diff(0, 0).unwrap().is_empty());
    assert!(log.diff(0, 9).is_none());
}

#[test]
fn word_diff_marks_only_the_changed_words() {
    use tpt_codex_core::{TextChange, diff_text};
    let changes = diff_text("the quick fox", "the slow fox");
    assert_eq!(
        changes,
        vec![
            TextChange::Same("the ".into()),
            TextChange::Removed("quick".into()),
            TextChange::Added("slow".into()),
            TextChange::Same(" fox".into()),
        ]
    );
}

#[test]
fn word_diff_pieces_join_back_into_both_texts() {
    use tpt_codex_core::{TextChange, diff_text};
    let a = "one  two\nthree four";
    let b = "two three five";
    let changes = diff_text(a, b);
    let old: String = changes
        .iter()
        .filter_map(|c| match c {
            TextChange::Same(t) | TextChange::Removed(t) => Some(t.as_str()),
            TextChange::Added(_) => None,
        })
        .collect();
    let new: String = changes
        .iter()
        .filter_map(|c| match c {
            TextChange::Same(t) | TextChange::Added(t) => Some(t.as_str()),
            TextChange::Removed(_) => None,
        })
        .collect();
    assert_eq!(old, a);
    assert_eq!(new, b);
    assert!(diff_text("", "").is_empty());
}

#[test]
fn the_log_diffs_one_text_block_and_refuses_others() {
    let mut log = VersionLog::new();
    let mut d = doc();
    log.save("a", &d);
    let mut h = History::new();
    edit_text(&mut d, &mut h, BlockId(1), 3, 3, " big").unwrap();
    log.save("b", &d);
    let changes = log.text_diff(0, 1, BlockId(1)).unwrap();
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, tpt_codex_core::TextChange::Added(t) if t.contains("big")))
    );
    // Block 0 is the container, not text.
    assert!(log.text_diff(0, 1, BlockId(0)).is_none());
}
