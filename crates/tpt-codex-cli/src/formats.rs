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

/// Writes `doc` to `path`. Returns the export's fidelity report when the format has one. A PDF's
/// pictures are read from the folder of `path`.
pub fn save(doc: &Document, path: &Path) -> Result<Option<FidelityReport>, String> {
    save_from(doc, path, path.parent())
}

/// Writes `doc` to `path`. A PDF's pictures are read from `base` when their addresses are relative;
/// without a base, they are placeholders. A picture file over 64 MB is not read.
pub fn save_from(
    doc: &Document,
    path: &Path,
    base: Option<&Path>,
) -> Result<Option<FidelityReport>, String> {
    let name = path.display().to_string();
    let saved =
        tpt_codex_files::save_with_images(doc, &name, &|uri: &str| picture_bytes(base, uri))?;
    fs::write(path, saved.bytes).map_err(|e| format!("{name}: cannot write: {e}"))?;
    Ok(saved.report)
}

/// The largest picture file that is read.
const MAX_PICTURE_FILE: u64 = 64 * 1024 * 1024;

/// The bytes of the file a picture's address names: relative addresses are found in `base`.
fn picture_bytes(base: Option<&Path>, uri: &str) -> Option<Vec<u8>> {
    let address = Path::new(uri);
    let path = if address.is_absolute() {
        address.to_path_buf()
    } else {
        base?.join(address)
    };
    let meta = fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_PICTURE_FILE {
        return None;
    }
    fs::read(path).ok()
}
