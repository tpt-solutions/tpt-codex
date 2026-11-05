// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_typeset::{MathMetrics, layout_math, math_svg, parse_math};

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

#[test]
fn a_fraction_renders_its_glyphs_and_bar() {
    let b = layout_math(&parse_math(r"\frac{1}{2}").unwrap(), &Fixed, 10.0);
    let svg = math_svg(&b, "Noto Sans");
    assert!(svg.starts_with("<svg"), "{svg}");
    assert_eq!(svg.matches("<text ").count(), 2, "{svg}");
    assert_eq!(svg.matches("<rect ").count(), 1, "{svg}");
    assert!(svg.contains("font-family=\"Noto Sans\""), "{svg}");
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn operators_and_family_names_are_escaped() {
    let b = layout_math(&parse_math("a<b").unwrap(), &Fixed, 10.0);
    let svg = math_svg(&b, "A&B");
    assert!(svg.contains("&lt;"), "{svg}");
    assert!(svg.contains("A&amp;B"), "{svg}");
    assert!(!svg.contains("a<b"), "{svg}");
}
