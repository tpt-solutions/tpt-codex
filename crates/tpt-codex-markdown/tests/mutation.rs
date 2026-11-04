// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic mutation test for the Markdown reader and writer.
//!
//! It is a seeded stand-in for a fuzzer, so it runs on every platform with no extra tools.
//! For each mutated input it checks three things:
//! 1. the reader never panics,
//! 2. a document the reader accepts passes `Document::validate`,
//! 3. if the writer accepts that document, writing it again gives the same text, and the
//!    text reads back without error.
//!
//! Increase `ITERATIONS` locally, or set `MUTATION_ITERATIONS`, for a longer run.

use std::panic::{self, AssertUnwindSafe};

use tpt_codex_markdown::{from_markdown, to_markdown};

const FIXTURE: &str = include_str!("../../../fixtures/notes.md");

const SEEDS: &[&str] = &[
    FIXTURE,
    "---\ntitle: \"A\\\"B\"\nclass: report\n---\n\n# Head\n\ntext\n",
    "- one\n- two\n\n3) three\n4) four\n",
    "```rust\nfn main() {}\n```\n\n~~~\nplain\n~~~\n",
    "![alt](pic.png)\n\n***\n\n$$\nx^2 + y\n$$\n\n$$ a = b $$\n",
];

/// Characters that matter to the reader: block markers, delimiters, escapes and line breaks.
const ALPHABET: &[char] = &[
    '#', '-', '*', '+', '.', ')', '1', '9', ' ', '\n', '\r', '\t', '`', '~', '$', '!', '[', ']',
    '(', ')', '\\', ':', '<', '>', '|', '&', '_', '"', 'a', 'é',
];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
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

/// Applies one random edit: insert, delete, replace, or duplicate a line.
fn mutate(rng: &mut Rng, input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    match rng.below(4) {
        0 => {
            let at = rng.below(chars.len() + 1);
            chars.insert(at, rng.pick(ALPHABET));
        }
        1 if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars.remove(at);
        }
        2 if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars[at] = rng.pick(ALPHABET);
        }
        _ => {
            let text: String = chars.iter().collect();
            let lines: Vec<&str> = text.split('\n').collect();
            let i = rng.below(lines.len());
            let mut out = lines.clone();
            out.insert(i, lines[i]);
            return out.join("\n");
        }
    }
    chars.into_iter().collect()
}

/// Runs the three checks on one input. Returns a description of the first failure.
fn check(input: &str) -> Result<(), String> {
    let read = panic::catch_unwind(AssertUnwindSafe(|| from_markdown(input)))
        .map_err(|_| String::from("reader panicked"))?;
    let doc = match read {
        Ok(doc) => doc,
        Err(_) => return Ok(()),
    };
    doc.validate()
        .map_err(|e| format!("reader accepted an invalid document: {e:?}"))?;

    let written = match to_markdown(&doc) {
        Ok(text) => text,
        Err(_) => return Ok(()),
    };
    let again = from_markdown(&written)
        .map_err(|e| format!("writer output does not read back: {e} in {written:?}"))?;
    let rewritten = to_markdown(&again)
        .map_err(|e| format!("writer refused its own output: {e} in {written:?}"))?;
    if rewritten != written {
        return Err(format!(
            "writing is not a fixed point:\nfirst:  {written:?}\nsecond: {rewritten:?}"
        ));
    }
    Ok(())
}

fn iterations() -> usize {
    std::env::var("MUTATION_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4000)
}

#[test]
fn mutated_markdown_never_panics_and_round_trips() {
    // Silence the default hook so caught panics do not flood the output; failures are reported below.
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut failures = Vec::new();
    let mut accepted = 0usize;
    for i in 0..iterations() {
        let seed = SEEDS[i % SEEDS.len()];
        let mut input = seed.to_string();
        for _ in 0..=rng.below(4) {
            input = mutate(&mut rng, &input);
        }
        if from_markdown(&input).is_ok() {
            accepted += 1;
        }
        if let Err(reason) = check(&input) {
            failures.push(format!("{reason}\ninput: {input:?}"));
            if failures.len() >= 5 {
                break;
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
    // Guard against a generator that only produces rejected input.
    assert!(
        accepted > 0,
        "no mutated input was accepted; the test is not exercising the writer"
    );
}
