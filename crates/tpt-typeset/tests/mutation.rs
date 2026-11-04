// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic mutation test for the math parser, the counterpart of the Markdown one.
//!
//! For each mutated formula the parser must not panic. If it accepts the formula, plain-text
//! rendering must not panic either, and the same source must give the same result twice.
//!
//! Set `MUTATION_ITERATIONS` for a longer run.

use std::panic::{self, AssertUnwindSafe};

use tpt_typeset::{parse_math, to_plain_text};

const SEEDS: &[&str] = &[
    r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
    "x^{10}_i",
    "{a {b}}",
    r"\sum_{i=1}^{n} a_i",
    "3.14 r + 2",
    "a_1^2",
];

/// Characters that matter to the math grammar: braces, scripts, commands, spaces and operators.
const ALPHABET: &[char] = &[
    '{', '}', '^', '_', '\\', ' ', '(', ')', '[', ']', '+', '-', '=', '/', '.', ',', '&', '$', 'a',
    'x', '1', '9', 'é', '\n',
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick(&mut self, chars: &[char]) -> char {
        chars[self.below(chars.len())]
    }
}

fn mutate(rng: &mut Rng, input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    match rng.below(3) {
        0 => {
            let at = rng.below(chars.len() + 1);
            chars.insert(at, rng.pick(ALPHABET));
        }
        1 if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars.remove(at);
        }
        _ if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars[at] = rng.pick(ALPHABET);
        }
        _ => chars.push(rng.pick(ALPHABET)),
    }
    chars.into_iter().collect()
}

fn check(input: &str) -> Result<bool, String> {
    let first = panic::catch_unwind(AssertUnwindSafe(|| parse_math(input)))
        .map_err(|_| String::from("parser panicked"))?;
    let second = parse_math(input);
    match (&first, &second) {
        (Ok(a), Ok(b)) if a == b => {
            panic::catch_unwind(AssertUnwindSafe(|| to_plain_text(a)))
                .map_err(|_| String::from("to_plain_text panicked on an accepted formula"))?;
            Ok(true)
        }
        (Err(a), Err(b)) if a == b => Ok(false),
        _ => Err(String::from(
            "parsing the same input twice gave different results",
        )),
    }
}

fn iterations() -> usize {
    std::env::var("MUTATION_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000)
}

#[test]
fn mutated_math_never_panics_and_parses_deterministically() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut failures = Vec::new();
    let mut accepted = 0usize;
    for i in 0..iterations() {
        let mut input = SEEDS[i % SEEDS.len()].to_string();
        for _ in 0..=rng.below(4) {
            input = mutate(&mut rng, &input);
        }
        match check(&input) {
            Ok(true) => accepted += 1,
            Ok(false) => {}
            Err(reason) => {
                failures.push(format!("{reason}\ninput: {input:?}"));
                if failures.len() >= 5 {
                    break;
                }
            }
        }
    }

    panic::set_hook(previous);
    assert!(
        failures.is_empty(),
        "{} failure(s):\n{}",
        failures.len(),
        failures.join("\n---\n")
    );
    assert!(
        accepted > 0,
        "no mutated formula was accepted; the test is not exercising rendering"
    );
}
