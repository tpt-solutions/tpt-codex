// SPDX-License-Identifier: MIT OR Apache-2.0
//! Kerning and standard ligatures come from shaping with the bundled font. These tests pin
//! that: a kerned pair measures narrower than its letters apart, and `fi` shapes to one glyph.

use tpt_codex_core::TextStyle as CoreStyle;
use tpt_codex_text::{FAMILY, FontMeasure, NOTO_SANS_REGULAR};
use tpt_codex_views::TextMeasure;
use tpt_text::font::FontRegistry;
use tpt_text::fx::Fx;
use tpt_text::layout::{LayoutOptions, layout_paragraph_with};
use tpt_text::style::TextStyle;

/// Shaped glyph count and total advance of `text`, laid out on one line.
fn shaped(text: &str, style: &TextStyle) -> (usize, f32) {
    let registry = FontRegistry::new();
    registry
        .load_from_memory(NOTO_SANS_REGULAR.to_vec(), 0, FAMILY, 400, false)
        .unwrap();
    let para = layout_paragraph_with(
        &registry,
        text,
        style,
        &[],
        Fx::from_f32(1000.0),
        &LayoutOptions::default(),
    )
    .unwrap();
    let runs = || para.lines.iter().flat_map(|l| l.runs.iter());
    let glyphs = runs().map(|r| r.glyphs.len()).sum();
    let width = runs().map(|r| r.width().to_f32()).sum();
    (glyphs, width)
}

fn body() -> TextStyle {
    TextStyle::new(FAMILY, Fx::from_f32(11.0))
}

#[test]
fn kerned_pairs_measure_narrower_than_their_letters_apart() {
    let measure = FontMeasure::bundled().unwrap();
    let style = CoreStyle::default();
    for pair in ["AV", "To"] {
        let mut letters = pair.chars().map(|c| measure.width(&c.to_string(), &style));
        let apart = letters.next().unwrap() + letters.next().unwrap();
        let together = measure.width(pair, &style);
        assert!(
            together < apart - 0.2,
            "{pair}: kerned {together:.3} should be narrower than {apart:.3}"
        );
    }
}

#[test]
fn kerning_is_on_by_default_and_can_be_turned_off() {
    let kerned = shaped("AV", &body()).1;
    let unkerned = shaped("AV", &body().with_feature(*b"kern", 0)).1;
    assert!(
        kerned < unkerned - 0.2,
        "kerned {kerned} vs unkerned {unkerned}"
    );
}

#[test]
fn standard_ligatures_shape_to_one_glyph_by_default() {
    for text in ["fi", "fl", "ffi"] {
        let (default, _) = shaped(text, &body());
        let (plain, _) = shaped(text, &body().with_feature(*b"liga", 0));
        assert_eq!(default, 1, "{text} should shape to one ligature glyph");
        assert!(
            plain > default,
            "{text} without liga should be {plain} glyphs"
        );
    }
}
