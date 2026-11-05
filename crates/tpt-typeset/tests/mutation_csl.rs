// SPDX-License-Identifier: MIT OR Apache-2.0
//! Seeded mutation test for the CSL-JSON reader: it must never panic on damaged input, and every
//! library it accepts must format each entry.

use std::panic::{self, AssertUnwindSafe};

use tpt_typeset::{Library, author_year, bibliography};

const SEED: &str = r#"[{"id":"a1","title":"Notes","author":[{"family":"Smith","given":"Ann"}],"issued":{"date-parts":[[2020]]}},{"id":"b2","author":[{"family":"Jones"},{"family":"Lee"}]}]"#;

const ALPHABET: &[char] = &[
    '[', ']', '{', '}', '"', ':', ',', '0', '9', '-', ' ', 'a', 'n', 'u', 'l', '\\', 'é',
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
}

fn mutate(rng: &mut Rng, input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    let at = rng.below(chars.len() + 1);
    match rng.below(3) {
        0 => chars.insert(at.min(chars.len()), ALPHABET[rng.below(ALPHABET.len())]),
        1 if !chars.is_empty() => {
            chars.remove(at.min(chars.len() - 1));
        }
        _ if !chars.is_empty() => {
            let i = at.min(chars.len() - 1);
            chars[i] = ALPHABET[rng.below(ALPHABET.len())];
        }
        _ => {}
    }
    chars.into_iter().collect()
}

#[test]
fn damaged_libraries_never_panic_and_accepted_ones_format() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut failures = Vec::new();
    let mut accepted = 0usize;
    for _ in 0..3000 {
        let input = mutate(&mut rng, SEED);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            if let Ok(lib) = Library::from_csl_json(&input) {
                for key in ["a1", "b2"] {
                    if let Some(entry) = lib.get(key) {
                        let _ = author_year(entry);
                    }
                }
                let _ = bibliography(&lib, &["a1", "b2"]);
                return true;
            }
            false
        }));
        match result {
            Ok(true) => accepted += 1,
            Ok(false) => {}
            Err(_) => failures.push(input),
        }
    }
    panic::set_hook(previous);
    assert!(
        failures.is_empty(),
        "panicked on {} inputs, e.g. {:?}",
        failures.len(),
        failures.first()
    );
    // The seed itself is valid, and some mutations should leave it valid.
    assert!(accepted > 0);
}
