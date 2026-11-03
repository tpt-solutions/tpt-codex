// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_typeset::{MAX_DEPTH, MathNode, parse_math, to_plain_text};

fn ok(src: &str) -> String {
    to_plain_text(&parse_math(src).unwrap_or_else(|e| panic!("{src:?}: {e}")))
}

fn err(src: &str) -> tpt_typeset::MathError {
    match parse_math(src) {
        Ok(nodes) => panic!("{src:?} should fail, got {nodes:?}"),
        Err(e) => e,
    }
}

#[test]
fn parses_the_quadratic_formula() {
    let src = r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}";
    let nodes = parse_math(src).unwrap();
    assert_eq!(nodes.len(), 3);
    assert!(matches!(nodes[2], MathNode::Frac { .. }));
    assert_eq!(ok(src), "x=(-b±√(b^(2)-4ac))/(2a)");
}

#[test]
fn numbers_identifiers_and_operators() {
    assert_eq!(ok("3.14 r + 2"), "3.14r+2");
    assert_eq!(ok("a_1^2"), "a^(2)_(1)");
}

#[test]
fn symbols_render_as_glyphs() {
    assert_eq!(ok(r"\alpha \leq \infty"), "α≤∞");
}

#[test]
fn scripts_take_braced_or_single_arguments() {
    let nodes = parse_math("x^{10}_i").unwrap();
    match &nodes[0] {
        MathNode::Script {
            sup: Some(sup),
            sub: Some(sub),
            ..
        } => {
            assert_eq!(sup, &vec![MathNode::Number("10".into())]);
            assert_eq!(sub, &vec![MathNode::Ident('i')]);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn groups_nest() {
    let nodes = parse_math("{a {b}}").unwrap();
    assert!(matches!(&nodes[0], MathNode::Group(items) if items.len() == 2));
}

#[test]
fn errors_point_at_the_problem() {
    let e = err("a +\n  {b");
    assert_eq!((e.line, e.column), (2, 3));
    assert!(e.message.contains("unclosed group"));
    assert!(e.to_string().starts_with("line 2, column 3:"));

    let e = err("x }");
    assert!(e.message.contains("unexpected '}'"));
    assert_eq!((e.line, e.column), (1, 3));

    let e = err(r"\frac{a}");
    assert!(e.message.contains("denominator"), "{}", e.message);

    let e = err(r"\bogus");
    assert!(e.message.contains("unknown command"));
    assert!(e.hint.contains("supported"));

    let e = err("^2");
    assert!(e.message.contains("no base"));

    let e = err("x^");
    assert!(e.message.contains("no argument"));

    let e = err("x^1^2");
    assert!(e.message.contains("duplicate superscript"));

    let e = err("1.2.3");
    assert!(e.message.contains("decimal point"));

    let e = err("a # b");
    assert!(e.message.contains("unexpected character '#'"));
}

#[test]
fn nesting_is_bounded() {
    let deep = "{".repeat(MAX_DEPTH + 10) + &"}".repeat(MAX_DEPTH + 10);
    let e = err(&deep);
    assert!(e.message.contains("nested too deeply"));
}

#[test]
fn hostile_inputs_never_panic() {
    let alphabet: Vec<char> = r"{}^_\frac sqrt pi0.9a+#\n".chars().collect();
    let mut state = 99u64;
    for _ in 0..3000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let len = (state >> 40) as usize % 40;
        let mut src = String::new();
        for _ in 0..len {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            src.push(alphabet[(state >> 33) as usize % alphabet.len()]);
        }
        let _ = parse_math(&src);
    }
}
