// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{MathMetrics, check_inline_math, layout_inline_math};

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
fn inline_formulas_are_found_and_checked() {
    let doc = from_markdown("Good $x^2$ and bad $\\frac{1}{$ here.\n").unwrap();
    let checks = check_inline_math(&doc);
    assert_eq!(checks.len(), 2, "{checks:?}");
    assert_eq!(checks[0].source, "x^2");
    assert!(checks[0].error.is_none());
    assert_eq!(checks[1].source, "\\frac{1}{");
    assert!(checks[1].error.is_some());
}

#[test]
fn a_valid_inline_formula_lays_out() {
    let b = layout_inline_math("a^2", &Fixed, 10.0).unwrap();
    assert!(b.width > 0.0);
    assert!(layout_inline_math("\\frac{", &Fixed, 10.0).is_err());
}
