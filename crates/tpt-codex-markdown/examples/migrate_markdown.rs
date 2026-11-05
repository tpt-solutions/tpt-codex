// SPDX-License-Identifier: MIT OR Apache-2.0
//! Escapes inline markers in Markdown written before inline syntax was read.
//!
//! Usage: `cargo run -p tpt-codex-markdown --example migrate_markdown -- <input.md> <output.md>`
//! The changed line numbers are printed. Run it once per file, on files written before inline
//! syntax was read: a second run escapes the escapes.

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let [_, input, output] = args.as_slice() else {
        eprintln!("usage: migrate_markdown <input.md> <output.md>");
        return ExitCode::from(2);
    };
    let source = match fs::read_to_string(input) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let migrated = tpt_codex_markdown::migrate_legacy_markdown(&source);
    if let Err(e) = fs::write(output, &migrated.text) {
        eprintln!("cannot write {output}: {e}");
        return ExitCode::FAILURE;
    }
    println!("changed lines: {:?}", migrated.changed_lines);
    ExitCode::SUCCESS
}
