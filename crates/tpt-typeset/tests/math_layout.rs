// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_typeset::{MathItem, MathMetrics, layout_math, parse_math};

/// Every character is half the size wide, and glyphs reach 0.8 above and 0.2 below.
struct Fixed;

impl MathMetrics for Fixed {
    fn advance(&self, _ch: char, size: f64) -> f64 {
        0.5 * size
    }
    fn ascent(&self, size: f64) -> f64 {
        0.8 * size
    }
    fn descent(&self, size: f64) -> f64 {
        0.2 * size
    }
}

fn layout(src: &str) -> tpt_typeset::MathBox {
    layout_math(&parse_math(src).unwrap(), &Fixed, 10.0)
}

fn rules(b: &tpt_typeset::MathBox) -> Vec<&MathItem> {
    b.items
        .iter()
        .filter(|i| matches!(i, MathItem::Rule { .. }))
        .collect()
}

#[test]
fn numbers_lay_out_side_by_side() {
    let b = layout("12");
    assert_eq!(b.width, 10.0);
    assert_eq!((b.ascent, b.descent), (8.0, 2.0));
    let xs: Vec<f64> = b
        .items
        .iter()
        .map(|i| match i {
            MathItem::Glyph { x, .. } => *x,
            _ => panic!(),
        })
        .collect();
    assert_eq!(xs, vec![0.0, 5.0]);
}

#[test]
fn operators_have_space_on_both_sides() {
    let plain = layout("a");
    let op = layout("a+a");
    // Two letters and an operator with 2 * 0.2 * size of space around it.
    assert!(
        (op.width - (plain.width * 2.0 + 0.5 * 10.0 + 4.0)).abs() < 1e-9,
        "{}",
        op.width
    );
}

#[test]
fn a_fraction_has_a_bar_and_stacks_its_parts() {
    let frac = layout(r"\frac{1}{2}");
    let bars = rules(&frac);
    assert_eq!(bars.len(), 1);
    // The numerator is above the bar and the denominator below it.
    let (bar_y, bar_h) = match bars[0] {
        MathItem::Rule { y, height, .. } => (*y, *height),
        _ => unreachable!(),
    };
    let heights: Vec<f64> = frac
        .items
        .iter()
        .filter_map(|i| match i {
            MathItem::Glyph { y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    assert_eq!(heights.len(), 2);
    assert!(heights[0] > bar_y + bar_h, "numerator above the bar");
    assert!(heights[1] < bar_y, "denominator below the bar");
    assert!(frac.ascent > 8.0 && frac.descent > 2.0);
}

#[test]
fn scripts_are_smaller_and_raised_or_lowered() {
    let b = layout("x^2_3");
    let glyphs: Vec<(char, f64, f64)> = b
        .items
        .iter()
        .filter_map(|i| match i {
            MathItem::Glyph { ch, y, size, .. } => Some((*ch, *y, *size)),
            _ => None,
        })
        .collect();
    let base = glyphs.iter().find(|g| g.0 == 'x').unwrap();
    let sup = glyphs.iter().find(|g| g.0 == '2').unwrap();
    let sub = glyphs.iter().find(|g| g.0 == '3').unwrap();
    assert_eq!(base.2, 10.0);
    assert!((sup.2 - 7.0).abs() < 1e-9 && (sub.2 - 7.0).abs() < 1e-9);
    assert!(sup.1 > 0.0 && sub.1 < 0.0);
    assert!(b.ascent > 8.0 && b.descent > 2.0);
}

#[test]
fn a_root_has_a_sign_and_an_overline() {
    let b = layout(r"\sqrt{x}");
    assert!(
        b.items
            .iter()
            .any(|i| matches!(i, MathItem::Glyph { ch: '√', .. }))
    );
    assert_eq!(rules(&b).len(), 1);
    assert!(b.ascent > 8.0);
}

#[test]
fn layout_is_deterministic_and_items_stay_inside_the_box() {
    let src = r"\frac{a^2+b}{\sqrt{c}} = 3_1";
    let a = layout(src);
    assert_eq!(a, layout(src));
    for item in &a.items {
        if let MathItem::Glyph { x, .. } = item {
            assert!(*x >= 0.0 && *x <= a.width, "{x} outside {}", a.width);
        }
    }
}
