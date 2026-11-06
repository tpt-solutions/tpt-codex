// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hyphenated line breaking in the flow layout: a long word is broken at a hyphenation point when
//! that keeps lines inside the column, and the letters are all still there.

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{
    FlowKind, FlowLayout, FlowOptions, MonospaceMeasure, TextMeasure, layout_flow,
    layout_flow_hyphenated,
};

/// Six points per character.
fn measure() -> MonospaceMeasure {
    MonospaceMeasure {
        char_width: 6.0,
        line_height: 10.0,
    }
}

fn options(width: f64) -> FlowOptions {
    FlowOptions {
        width,
        block_spacing: 0.0,
        box_height: 10.0,
        row_height: 10.0,
    }
}

fn paragraph(text: &str) -> Document {
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
            content: text.to_string(),
            style: TextStyle::default(),
        },
    );
    doc
}

/// The text of each line, in order.
fn lines(layout: &FlowLayout) -> Vec<String> {
    layout
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            FlowKind::Line { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_long_word_is_broken_with_a_hyphen_when_that_fits_the_line() {
    let doc = paragraph("a extraordinarily");
    let width = 50.0; // eight characters
    let plain = lines(&layout_flow(&doc, &measure(), options(width)).unwrap());
    let hyphenated = lines(&layout_flow_hyphenated(&doc, &measure(), options(width)).unwrap());

    // The plain layout splits the word between letters; the hyphenated one breaks it at points.
    assert!(plain.iter().all(|l| !l.ends_with('-')));
    assert_eq!(
        hyphenated[0], "a ex-",
        "the first line takes the head of the word"
    );
    assert_eq!(hyphenated[1], "traordi-");
    assert_eq!(hyphenated[2], "narily");
}

#[test]
fn every_hyphenated_line_fits_the_column() {
    let doc = paragraph("a extraordinarily long encyclopedia entry about hyphenation");
    let width = 60.0;
    let m = measure();
    let layout = layout_flow_hyphenated(&doc, &m, options(width)).unwrap();
    for line in lines(&layout) {
        assert!(
            m.width(&line, &TextStyle::default()) <= width,
            "line too wide: {line:?}"
        );
    }
}

#[test]
fn breaking_only_adds_hyphens_and_spaces_to_the_letters() {
    let text = "a extraordinarily long encyclopedia entry";
    let doc = paragraph(text);
    let layout = layout_flow_hyphenated(&doc, &measure(), options(60.0)).unwrap();
    let joined: String = lines(&layout).concat();
    let letters = |s: &str| -> String { s.chars().filter(|c| c.is_alphabetic()).collect() };
    assert_eq!(letters(&joined), letters(text));
}

#[test]
fn words_that_fit_are_not_touched() {
    let doc = paragraph("short words only here");
    let plain = lines(&layout_flow(&doc, &measure(), options(200.0)).unwrap());
    let hyphenated = lines(&layout_flow_hyphenated(&doc, &measure(), options(200.0)).unwrap());
    assert_eq!(plain, hyphenated);
}

#[test]
fn a_plain_cache_gives_the_same_lines_as_layout_flow() {
    use tpt_codex_views::{FlowCache, layout_flow_cached};
    let doc = paragraph("a extraordinarily");
    let mut cache = FlowCache::new();
    let plain = lines(&layout_flow_cached(&doc, &measure(), options(50.0), &mut cache).unwrap());
    let plain_again = lines(&layout_flow(&doc, &measure(), options(50.0)).unwrap());
    assert_eq!(plain, plain_again);
}
