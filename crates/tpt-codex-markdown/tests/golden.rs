// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden snapshot: the fixture reads to a known JSON document and writes back byte-for-byte.
//!
//! Set `UPDATE_GOLDEN=1` to regenerate `fixtures/notes.golden.json` after an intended change.
//! Each run also writes the JSON to `target/golden/`, so CI can compare it across operating systems.

use std::fs;
use std::path::PathBuf;

use tpt_codex_markdown::{from_markdown, to_markdown};

const FIXTURE: &str = include_str!("../../../fixtures/notes.md");

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

#[test]
fn fixture_matches_golden_json() {
    let doc = from_markdown(FIXTURE).unwrap();
    let json = serde_json::to_string_pretty(&doc).unwrap() + "\n";

    let golden = repo_path("fixtures/notes.golden.json");
    if std::env::var_os("UPDATE_GOLDEN").is_some() || !golden.exists() {
        fs::write(&golden, &json).unwrap();
    }
    let expected = fs::read_to_string(&golden).unwrap();
    assert_eq!(json, expected, "golden JSON differs; run with UPDATE_GOLDEN=1 if intended");

    let out_dir = repo_path("target/golden");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("notes.json"), &json).unwrap();
}

#[test]
fn fixture_writes_back_byte_for_byte() {
    let doc = from_markdown(FIXTURE).unwrap();
    assert_eq!(to_markdown(&doc).unwrap(), FIXTURE);
}
