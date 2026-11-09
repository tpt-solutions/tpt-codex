// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Document, History, InlineRun, LayoutMode, TextStyle, ValidationError,
    clip_runs, edit_text, remap_runs,
};

fn bold(start: usize, end: usize) -> InlineRun {
    InlineRun {
        start,
        end,
        bold: true,
        italic: false,
        link: None,
        math: None,
        link_title: None,
    }
}

fn link(start: usize, end: usize, url: &str) -> InlineRun {
    InlineRun {
        start,
        end,
        bold: false,
        italic: false,
        link: Some(url.into()),
        math: None,
        link_title: None,
    }
}

fn doc_with(content: &str, runs: Vec<InlineRun>) -> Document {
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
            content: content.into(),
            style: TextStyle {
                runs,
                ..TextStyle::default()
            },
        },
    );
    d
}

fn runs_of(d: &Document) -> Vec<InlineRun> {
    match d.get(BlockId(1)) {
        Some(Block::Text { style, .. }) => style.runs.clone(),
        other => panic!("not a text block: {other:?}"),
    }
}

#[test]
fn text_outside_the_edit_keeps_its_marks() {
    // "abcdef" with "cd" bold; typing "X" at the start moves the run along.
    let runs = remap_runs(&[bold(2, 4)], 0, 0, 1);
    assert_eq!(runs, vec![bold(3, 5)]);
}

#[test]
fn typing_strictly_inside_a_run_takes_its_marks() {
    let runs = remap_runs(&[bold(1, 5)], 3, 3, 2);
    assert_eq!(runs, vec![bold(1, 7)]);
}

#[test]
fn typing_at_the_edge_of_a_run_does_not_extend_it() {
    // Before the run: the inserted text stays plain.
    assert_eq!(remap_runs(&[bold(2, 4)], 2, 2, 1), vec![bold(3, 5)]);
    // After the run: the run does not grow.
    assert_eq!(remap_runs(&[bold(2, 4)], 4, 4, 1), vec![bold(2, 4)]);
}

#[test]
fn deleting_a_whole_run_drops_it() {
    assert!(remap_runs(&[bold(2, 4)], 2, 4, 0).is_empty());
}

#[test]
fn deleting_part_of_a_run_shortens_it() {
    // Delete characters 3..5 from a run covering 2..6.
    assert_eq!(remap_runs(&[bold(2, 6)], 3, 5, 0), vec![bold(2, 4)]);
}

#[test]
fn replacing_inside_a_run_keeps_the_new_text_marked() {
    // Replace "cd" (positions 2..4) inside a run covering 0..6 with "XYZ".
    assert_eq!(remap_runs(&[bold(0, 6)], 2, 4, 3), vec![bold(0, 7)]);
}

#[test]
fn a_link_is_moved_with_its_text() {
    let runs = remap_runs(&[link(0, 3, "https://x.test")], 1, 1, 2);
    assert_eq!(runs, vec![link(0, 5, "https://x.test")]);
}

#[test]
fn clip_runs_shortens_and_drops_runs_past_the_end() {
    let runs = clip_runs(&[bold(0, 3), bold(5, 9), bold(6, 8)], 6);
    assert_eq!(runs, vec![bold(0, 3), bold(5, 6)]);
}

#[test]
fn edit_text_moves_runs_with_the_text() {
    let mut d = doc_with("hello world", vec![bold(6, 11)]);
    let mut h = History::new();
    // Insert "big " before "world".
    edit_text(&mut d, &mut h, BlockId(1), 6, 6, "big ").unwrap();
    assert_eq!(runs_of(&d), vec![bold(10, 15)]);
    d.validate().unwrap();
    // Undo puts the run back.
    h.undo(&mut d).unwrap();
    assert_eq!(runs_of(&d), vec![bold(6, 11)]);
}

#[test]
fn a_run_past_the_end_of_its_text_is_invalid() {
    let d = doc_with("abc", vec![bold(1, 9)]);
    assert!(matches!(
        d.validate(),
        Err(ValidationError::InvalidRun { block: BlockId(1) })
    ));
}

#[test]
fn an_empty_run_is_invalid() {
    let d = doc_with("abc", vec![bold(2, 2)]);
    assert!(matches!(
        d.validate(),
        Err(ValidationError::InvalidRun { .. })
    ));
}

#[test]
fn plain_text_serializes_without_a_runs_field() {
    let d = doc_with("abc", Vec::new());
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("runs"), "{json}");
    assert!(!json.contains("link"), "{json}");
}

#[test]
fn runs_round_trip_through_json() {
    let d = doc_with("abcdef", vec![bold(0, 2), link(3, 5, "https://x.test")]);
    let json = serde_json::to_string(&d).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}
