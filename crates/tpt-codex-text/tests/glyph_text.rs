// SPDX-License-Identifier: MIT OR Apache-2.0
//! Each glyph carries the characters of its cluster, so a text layer can say what was drawn.

use tpt_codex_core::TextStyle;
use tpt_codex_text::FontMeasure;

/// The text of every glyph in order, joined: the first glyph of each cluster has its characters.
fn joined(text: &str) -> String {
    let measure = FontMeasure::bundled().unwrap();
    let lines = measure
        .shape_lines(text, &TextStyle::default(), None)
        .unwrap();
    lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| r.glyphs.iter())
        .map(|g| g.text.as_str())
        .collect()
}

#[test]
fn the_glyphs_of_plain_text_carry_its_characters_in_order() {
    assert_eq!(joined("Hello world"), "Hello world");
}

#[test]
fn a_ligature_or_plain_pair_keeps_its_characters_once() {
    let measure = FontMeasure::bundled().unwrap();
    let lines = measure
        .shape_lines("fi", &TextStyle::default(), None)
        .unwrap();
    let glyphs: Vec<_> = lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| r.glyphs.iter())
        .collect();
    // Either a ligature (one glyph with both characters) or two glyphs, one carrying each.
    assert_eq!(
        glyphs.iter().map(|g| g.text.as_str()).collect::<String>(),
        "fi"
    );
    assert!(glyphs.iter().all(|g| g.advance >= 0.0));
}
