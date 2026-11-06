// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_markdown::from_markdown;
use tpt_codex_views::{
    FlowKind, FlowOptions, MonospaceMeasure, PageGeometry, layout_flow, paginate_with_notes,
};
use tpt_typeset::{check_footnotes, footnote_notes};

fn measure() -> MonospaceMeasure {
    MonospaceMeasure {
        char_width: 6.0,
        line_height: 10.0,
    }
}

const MD: &str = "First[^b] then second[^a] and first again[^b].\n\n\
[^a]: Text of a.\n\n[^b]: Text of b.\n";

#[test]
fn each_footnote_gets_one_note_numbered_by_first_reference() {
    let doc = from_markdown(MD).unwrap();
    let notes = footnote_notes(&doc, &measure(), 500.0);

    assert_eq!(notes.len(), 2, "one note per footnote, not per reference");
    let anchor = check_footnotes(&doc)[0].block;
    assert!(notes.iter().all(|n| n.anchor == anchor));

    let texts: Vec<String> = notes
        .iter()
        .map(|n| {
            n.items
                .iter()
                .filter_map(|i| match &i.kind {
                    FlowKind::Line { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(texts, vec!["1. Text of b.", "2. Text of a."]);
}

#[test]
fn notes_sit_at_the_foot_of_the_page_and_definitions_stay_out_of_the_body() {
    let doc = from_markdown(MD).unwrap();
    let flow = layout_flow(
        &doc,
        &measure(),
        FlowOptions {
            width: 500.0,
            block_spacing: 0.0,
            box_height: 10.0,
            row_height: 10.0,
        },
    )
    .unwrap();

    let notes = footnote_notes(&doc, &measure(), 500.0);
    let geometry = PageGeometry {
        height: 200.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 1,
    };
    let pages = paginate_with_notes(&flow, geometry, &notes).unwrap();
    assert_eq!(pages.len(), 1);

    // The definitions are shown as notes, not as body text.
    for item in &pages[0].items {
        if let FlowKind::Line { text, .. } = &item.kind {
            assert!(!text.contains("Text of"), "definition in body: {text}");
        }
    }
    assert_eq!(pages[0].notes.len(), 2, "both notes on the page");
    let note_top = pages[0].notes.iter().map(|n| n.y).fold(f64::MAX, f64::min);
    assert_eq!(
        note_top,
        200.0 - 20.0,
        "the two notes fill the bottom 20 points"
    );
    assert!(
        pages[0].items.iter().all(|i| i.y + i.height <= note_top),
        "body items stay above the notes"
    );
}

#[test]
fn a_footnote_with_no_reference_gives_no_note() {
    let doc = from_markdown("Text.\n\n[^lonely]: Nobody cites me.\n").unwrap();
    assert!(footnote_notes(&doc, &measure(), 500.0).is_empty());
}
