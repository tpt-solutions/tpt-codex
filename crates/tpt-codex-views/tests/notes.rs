// SPDX-License-Identifier: MIT OR Apache-2.0
//! Footnotes at the foot of the page: the note is placed where its anchor is, and the flow makes
//! room for it.

use tpt_codex_core::{BlockId, TextStyle};
use tpt_codex_views::{
    FlowItem, FlowKind, FlowLayout, MonospaceMeasure, Note, PageGeometry, layout_note, paginate,
    paginate_with_notes,
};

const LINE: f64 = 10.0;

fn line(block: u64, y: f64) -> FlowItem {
    FlowItem {
        block: BlockId(block),
        y,
        height: LINE,
        kind: FlowKind::Line {
            text: format!("line of {block}"),
            style: TextStyle::default(),
            marker: None,
            indent: 0.0,
        },
    }
}

fn boxed(block: u64, height: f64, y: f64) -> FlowItem {
    FlowItem {
        block: BlockId(block),
        y,
        height,
        kind: FlowKind::Box,
    }
}

fn flow(items: Vec<FlowItem>) -> FlowLayout {
    let height = items.iter().map(|i| i.y + i.height).fold(0.0, f64::max);
    FlowLayout { items, height }
}

/// Page content height 40, no margins, lines of 10.
fn geometry() -> PageGeometry {
    PageGeometry {
        height: 40.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 1,
    }
}

/// A note for `anchor` of `height` points made of lines of 10 points.
fn note(anchor: u64, lines: usize) -> Note {
    let items: Vec<FlowItem> = (0..lines)
        .map(|i| FlowItem {
            block: BlockId(900 + anchor),
            y: i as f64 * LINE,
            height: LINE,
            kind: FlowKind::Line {
                text: format!("note for {anchor}"),
                style: TextStyle::default(),
                marker: None,
                indent: 0.0,
            },
        })
        .collect();
    Note {
        anchor: BlockId(anchor),
        items,
        height: lines as f64 * LINE,
    }
}

#[test]
fn with_no_notes_the_result_is_the_plain_pagination() {
    let f = flow(vec![line(1, 0.0), line(1, 10.0), line(2, 20.0)]);
    assert_eq!(
        paginate_with_notes(&f, geometry(), &[]).unwrap(),
        paginate(&f, geometry()).unwrap()
    );
}

#[test]
fn a_note_sits_at_the_foot_of_the_page_where_its_anchor_is() {
    // Three lines of one paragraph. The note of two lines is reserved on the first page as soon
    // as the paragraph starts there, so only two lines fit above it.
    let f = flow(vec![line(1, 0.0), line(1, 10.0), line(1, 20.0)]);
    let pages = paginate_with_notes(&f, geometry(), &[note(1, 2)]).unwrap();

    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].items.len(), 2);
    assert_eq!(pages[1].items.len(), 1);
    assert!(pages[1].notes.is_empty());

    let ys: Vec<f64> = pages[0].notes.iter().map(|n| n.y).collect();
    assert_eq!(ys, vec![20.0, 30.0], "the note fills the bottom 20 points");
    assert!(pages[0].notes.iter().all(|n| n.block == BlockId(901)));
}

#[test]
fn a_note_pushes_the_next_block_to_the_next_page() {
    // A 25-point box with a 20-point note leaves no room for another 10-point box.
    let f = flow(vec![boxed(1, 25.0, 0.0), boxed(2, 10.0, 25.0)]);
    let pages = paginate_with_notes(&f, geometry(), &[note(1, 2)]).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].items.len(), 1);
    assert_eq!(pages[0].notes.len(), 2);
    assert_eq!(pages[1].items[0].block, BlockId(2));
}

#[test]
fn a_note_whose_anchor_is_not_in_the_flow_is_not_placed() {
    let f = flow(vec![line(1, 0.0)]);
    let pages = paginate_with_notes(&f, geometry(), &[note(7, 2)]).unwrap();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].notes.is_empty());
    assert_eq!(pages[0].items.len(), 1);
}

#[test]
fn notes_on_one_anchor_stack_in_order_from_the_top_of_the_note_area() {
    // Two notes for the same paragraph: one line and two lines, 30 points in all. The note area
    // starts at 40 - 30 = 10, so the notes sit at 10, 20 and 30, and the paragraph's second line
    // moves to the next page.
    let f = flow(vec![line(1, 0.0), line(1, 10.0)]);
    let pages = paginate_with_notes(&f, geometry(), &[note(1, 1), note(1, 2)]).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].items.len(), 1);
    let ys: Vec<f64> = pages[0].notes.iter().map(|n| n.y).collect();
    assert_eq!(ys, vec![10.0, 20.0, 30.0]);
    assert_eq!(pages[1].items.len(), 1);
}

#[test]
fn layout_note_wraps_the_text_and_keeps_its_height() {
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 10.0,
    };
    let n = layout_note(
        BlockId(1),
        BlockId(50),
        "one two three four",
        &TextStyle::default(),
        &measure,
        // Room for about two words of 3-5 characters.
        30.0,
    );
    assert!(n.items.len() >= 2, "the note wraps");
    assert_eq!(n.height, n.items.len() as f64 * 10.0);
    assert_eq!(n.anchor, BlockId(1));
    assert!(n.items.iter().all(|i| i.block == BlockId(50)));
}
