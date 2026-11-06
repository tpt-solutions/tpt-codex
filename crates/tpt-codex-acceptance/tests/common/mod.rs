// SPDX-License-Identifier: MIT OR Apache-2.0
//! Helpers shared by the acceptance tests: fixture paths, an in-memory store, and the deterministic
//! text measure used everywhere a layout is needed.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::convert::Infallible;

use tpt_codex_core::Storage;
use tpt_codex_views::MonospaceMeasure;

/// The fixed text measure: every character is 6 points wide, every line 14 points high.
pub const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 6.0,
    line_height: 14.0,
};

/// Number of page objects in a PDF written by `tpt-canvas-pdf`.
pub fn pdf_page_count(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(pdf).matches("/Type /Page ").count()
}

/// Reads a file from the repository's `fixtures/office` directory.
pub fn office_fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../fixtures/office/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

/// A store that keeps everything in memory.
#[derive(Default)]
pub struct MemStorage(pub BTreeMap<String, Vec<u8>>);

impl Storage for MemStorage {
    type Error = Infallible;

    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, Infallible> {
        Ok(self.0.get(key).cloned())
    }

    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), Infallible> {
        self.0.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<(), Infallible> {
        self.0.remove(key);
        Ok(())
    }
}
