// SPDX-License-Identifier: MIT OR Apache-2.0
//! Math layout with the constants from the bundled font's MATH table.

use tpt_typeset::{
    MathBox, MathConstants, MathItem, MathMetrics, STIX_TWO_MATH, layout_math, math_constants,
    math_svg, parse_math,
};

/// The same fixed character metrics as the other math tests, with the given constants.
struct Metrics {
    constants: Option<MathConstants>,
}

impl MathMetrics for Metrics {
    fn advance(&self, _ch: char, size: f64) -> f64 {
        0.5 * size
    }
    fn ascent(&self, size: f64) -> f64 {
        0.8 * size
    }
    fn descent(&self, size: f64) -> f64 {
        0.2 * size
    }
    fn math_constants(&self) -> Option<MathConstants> {
        self.constants
    }
}

fn font_metrics() -> Metrics {
    Metrics {
        constants: math_constants(STIX_TWO_MATH),
    }
}

fn layout(src: &str, metrics: &Metrics) -> MathBox {
    layout_math(&parse_math(src).unwrap(), metrics, 10.0)
}

/// The baseline height of the glyph `ch`, if the box draws it.
fn glyph_y(b: &MathBox, ch: char) -> Option<f64> {
    b.items.iter().find_map(|i| match i {
        MathItem::Glyph { y, ch: c, .. } if *c == ch => Some(*y),
        _ => None,
    })
}

#[test]
fn the_font_constants_are_read() {
    let k = font_metrics()
        .constants
        .expect("MATH table in the bundled font");
    assert!(k.numerator_shift_up.is_some_and(|s| s > 0.0), "{k:?}");
    assert!(k.denominator_shift_down.is_some_and(|s| s > 0.0), "{k:?}");
    assert!(
        k.superscript_shift_up > 0.0 && k.subscript_shift_down > 0.0,
        "{k:?}"
    );
    assert!(k.script_scale > 0.0 && k.script_scale < 1.0, "{k:?}");
}

#[test]
fn a_superscript_is_raised_by_the_font_shift() {
    let metrics = font_metrics();
    let k = metrics.constants.unwrap();
    let b = layout("x^2", &metrics);
    // The script is drawn at the font's superscript shift, in the size of the formula.
    let y = glyph_y(&b, '2').expect("the superscript is drawn");
    assert!(
        (y - k.superscript_shift_up * 10.0).abs() < 1e-9,
        "{y} vs {k:?}"
    );
}

#[test]
fn a_fraction_parts_clear_the_bar_and_respect_the_font_shifts() {
    let metrics = font_metrics();
    let k = metrics.constants.unwrap();
    let b = layout(r"\frac{1}{2}", &metrics);
    let num = glyph_y(&b, '1').expect("numerator");
    let den = glyph_y(&b, '2').expect("denominator");
    // Numerator sits at least at the font's numerator shift; the denominator at most at the
    // font's denominator shift below the baseline.
    assert!(
        num >= k.numerator_shift_up.unwrap() * 10.0 - 1e-9,
        "num {num}"
    );
    assert!(
        den <= -k.denominator_shift_down.unwrap() * 10.0 + 1e-9,
        "den {den}"
    );
    // The bar is centred on the font's axis height.
    let bar = b
        .items
        .iter()
        .find_map(|i| match i {
            MathItem::Rule { y, height, .. } => Some((*y, *height)),
            _ => None,
        })
        .expect("bar");
    let centre = bar.0 + bar.1 / 2.0;
    assert!(
        (centre - k.axis_height * 10.0).abs() < 1e-9,
        "centre {centre}"
    );
}

#[test]
fn without_constants_the_layout_is_the_default() {
    let plain = Metrics { constants: None };
    let explicit = Metrics {
        constants: Some(MathConstants::default()),
    };
    for src in [r"\frac{a+1}{b}", "x^2_i", r"\sqrt{2}", "12 + 3"] {
        assert_eq!(layout(src, &plain), layout(src, &explicit), "{src}");
    }
}

#[test]
fn the_font_changes_the_layout_of_a_fraction_with_a_superscript() {
    let font = layout(r"\frac{x^2}{3}", &font_metrics());
    let fixed = layout(r"\frac{x^2}{3}", &Metrics { constants: None });
    assert_ne!(font, fixed);
    assert!(font.ascent > 0.0 && font.descent > 0.0);
}

#[test]
fn a_formula_with_a_fraction_and_a_superscript_matches_its_golden_svg() {
    let svg = math_svg(&layout(r"\frac{x^2}{3}", &font_metrics()), "STIX Two Math");
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/frac_sup.svg");
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden")).unwrap();
        std::fs::write(path, &svg).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!("missing golden file {path} ({e}); run with TPT_UPDATE_GOLDEN=1 to create it")
    });
    assert_eq!(expected, svg, "golden mismatch for {path}");
}
