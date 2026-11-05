// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic mutation test for document JSON.
//!
//! Mutated JSON must never panic the deserializer or `validate`. A document that parses and
//! validates must serialize and parse back to the same document.
//!
//! Set `MUTATION_ITERATIONS` for a longer run.

use std::panic::{self, AssertUnwindSafe};

use tpt_codex_core::{
    Block, BlockId, CellInput, Document, LayoutMode, MediaSource, Metadata, Rect, Sheet, SheetId,
    TextStyle,
};

fn seed_documents() -> Vec<String> {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)],
        },
    );
    doc.metadata = Metadata {
        title: Some("T".into()),
        ..Metadata::default()
    };
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "héllo".into(),
            style: TextStyle {
                bold: true,
                italic: false,
                heading_level: Some(2),
                runs: Vec::new(),
            },
        },
    );
    doc.insert(
        BlockId(2),
        Block::Grid {
            sheet_id: SheetId(7),
            view_bounds: Some(Rect {
                x: 0.0,
                y: 1.5,
                width: 2.0,
                height: 3.0,
            }),
        },
    );
    let mut sheet = Sheet::default();
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 2.5 });
    sheet.cells.insert(
        "B1".into(),
        CellInput::Formula {
            source: "=A1*2".into(),
        },
    );
    doc.insert_sheet(SheetId(7), sheet);
    doc.insert(
        BlockId(3),
        Block::Media {
            source: MediaSource::Uri {
                uri: "a.png".into(),
            },
            alt_text: None,
        },
    );
    doc.insert(
        BlockId(4),
        Block::Code {
            language: Some("rust".into()),
            code: "fn f() {}".into(),
        },
    );
    vec![serde_json::to_string(&doc).unwrap()]
}

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

/// Characters that matter to JSON and to the document format.
const ALPHABET: &[char] = &[
    '{', '}', '[', ']', '"', ':', ',', ' ', '0', '1', '7', '-', '.', 'e', 'n', 't', 'a', '\\', 'é',
];

fn mutate(rng: &mut Rng, input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    match rng.below(3) {
        0 => {
            let at = rng.below(chars.len() + 1);
            chars.insert(at, ALPHABET[rng.below(ALPHABET.len())]);
        }
        1 if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars.remove(at);
        }
        _ if !chars.is_empty() => {
            let at = rng.below(chars.len());
            chars[at] = ALPHABET[rng.below(ALPHABET.len())];
        }
        _ => {}
    }
    chars.into_iter().collect()
}

fn check(input: &str) -> Result<bool, String> {
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| serde_json::from_str::<Document>(input)))
        .map_err(|_| String::from("deserializer panicked"))?;
    let Ok(doc) = parsed else {
        return Ok(false);
    };
    let valid = panic::catch_unwind(AssertUnwindSafe(|| doc.validate()))
        .map_err(|_| String::from("validate panicked"))?;
    if valid.is_err() {
        return Ok(false);
    }
    let written = serde_json::to_string(&doc).map_err(|e| format!("cannot write: {e}"))?;
    let back: Document = serde_json::from_str(&written)
        .map_err(|e| format!("written JSON does not read back: {e}"))?;
    if back != doc {
        return Err(String::from("valid document changed on a JSON round trip"));
    }
    Ok(true)
}

fn iterations() -> usize {
    std::env::var("MUTATION_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000)
}

#[test]
fn mutated_documents_never_panic_and_valid_ones_round_trip() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));

    let seeds = seed_documents();
    let mut rng = Rng(0x8CB9_2BA7_2F3D_8DD7);
    let mut failures = Vec::new();
    let mut accepted = 0usize;
    for i in 0..iterations() {
        let mut input = seeds[i % seeds.len()].clone();
        for _ in 0..=rng.below(3) {
            input = mutate(&mut rng, &input);
        }
        match check(&input) {
            Ok(true) => accepted += 1,
            Ok(false) => {}
            Err(reason) => {
                failures.push(format!("{reason}\ninput: {input}"));
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
    // Single-character edits to the seed rarely stay valid, so accept a small floor.
    assert!(
        accepted > 0,
        "no mutated document was accepted; the test is not exercising validation"
    );
}
