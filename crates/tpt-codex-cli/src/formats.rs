// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reading and writing documents by file path. The rules are in `tpt-codex-files`.

use std::fs;
use std::path::Path;

use tpt_codex_core::Document;
use tpt_codex_files::FidelityReport;

pub use tpt_codex_files::Format;

/// The format of `path`, from its extension.
pub fn format_of(path: &Path) -> Result<Format, String> {
    tpt_codex_files::format_of(&path.display().to_string())
}

/// A document read from a file, with the import's fidelity report when the format has one.
#[derive(Debug)]
pub struct Loaded {
    /// The document.
    pub document: Document,
    /// What the import dropped or approximated. `None` for formats that are read exactly.
    pub report: Option<FidelityReport>,
}

/// Reads `path` as a document.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let name = path.display().to_string();
    format_of(path)?;
    let bytes = fs::read(path).map_err(|e| format!("{name}: cannot read: {e}"))?;
    let opened = tpt_codex_files::open(&name, &bytes)?;
    Ok(Loaded {
        document: opened.document,
        report: opened.report,
    })
}

/// Writes `doc` to `path`. Returns the export's fidelity report when the format has one.
pub fn save(doc: &Document, path: &Path) -> Result<Option<FidelityReport>, String> {
    let name = path.display().to_string();
    let saved = tpt_codex_files::save(doc, &name)?;
    fs::write(path, saved.bytes).map_err(|e| format!("{name}: cannot write: {e}"))?;
    Ok(saved.report)
}
