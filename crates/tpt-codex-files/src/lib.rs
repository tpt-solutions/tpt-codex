// SPDX-License-Identifier: MIT OR Apache-2.0
//! Opens and saves documents by file name, from and to bytes, so the `tpt` command and the
//! browser page share one set of rules. The format is the file name's extension:
//!
//! | Extension | Open | Save |
//! | --- | --- | --- |
//! | `.cdx`, `.md`, `.markdown` | yes | yes |
//! | `.docx` | yes, with a fidelity report | yes, with a fidelity report |
//! | `.xlsx` | yes, every worksheet as a grid | yes, the first grid |
//! | `.pptx` | yes, text only | no |
//! | `.json` | yes | yes |
//! | `.pdf` | yes, its text, with a fidelity report | yes |
//!
//! Saving as `.docx` or `.pdf` replaces `[[sheet1.B4]]` with that cell's computed value, and a
//! `.docx` grid's formula cells with their values, because those files have no live values.

use tpt_codex_core::{Block, BlockId, Document, History, LayoutMode, Op, TextStyle};
use tpt_codex_interop::{
    export_docx, export_xlsx, import_docx, import_pdf, import_pptx, import_xlsx_workbook,
    place_sheet,
};
use tpt_codex_markdown::{from_markdown, to_markdown};

pub use tpt_codex_interop::{Fidelity, FidelityReport};

/// A file format, found from a file name's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Markdown or Codex Script (`.md`, `.markdown`, `.cdx`).
    Markdown,
    /// Word (`.docx`).
    Docx,
    /// Excel (`.xlsx`).
    Xlsx,
    /// PowerPoint (`.pptx`), read only.
    Pptx,
    /// The document model as JSON (`.json`).
    Json,
    /// PDF (`.pdf`): its text is read, and it is written with a text layer.
    Pdf,
}

/// The format of a file called `name`.
pub fn format_of(name: &str) -> Result<Format, String> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" | "cdx" => Ok(Format::Markdown),
        "docx" => Ok(Format::Docx),
        "xlsx" => Ok(Format::Xlsx),
        "pptx" => Ok(Format::Pptx),
        "json" => Ok(Format::Json),
        "pdf" => Ok(Format::Pdf),
        _ => Err(format!(
            "{name}: unknown file type (use .cdx, .md, .docx, .xlsx, .pptx, .pdf or .json)"
        )),
    }
}

/// A document that was opened, with the import's fidelity report when the format has one.
#[derive(Debug)]
pub struct Opened {
    /// The document. It has passed [`Document::validate`].
    pub document: Document,
    /// What the import dropped or approximated. `None` for formats that are read exactly.
    pub report: Option<FidelityReport>,
}

/// Reads a document from the bytes of a file called `name`. Errors start with `name`, and a
/// Markdown error continues `line:column: error: message (hint: …)`.
pub fn open(name: &str, bytes: &[u8]) -> Result<Opened, String> {
    let opened = match format_of(name)? {
        Format::Markdown => {
            let text = std::str::from_utf8(bytes).map_err(|_| format!("{name}: not UTF-8 text"))?;
            let document = from_markdown(text).map_err(|e| {
                let hint = e
                    .suggestion
                    .map(|h| format!(" (hint: {h})"))
                    .unwrap_or_default();
                format!("{name}:{}:{}: error: {}{hint}", e.line, e.column, e.message)
            })?;
            Opened {
                document,
                report: None,
            }
        }
        Format::Json => Opened {
            document: serde_json::from_slice(bytes)
                .map_err(|e| format!("{name}: not a document: {e}"))?,
            report: None,
        },
        Format::Docx => {
            let import = import_docx(bytes).map_err(|e| format!("{name}: {e}"))?;
            Opened {
                document: import.document,
                report: Some(import.report),
            }
        }
        Format::Pptx => {
            let import = import_pptx(bytes).map_err(|e| format!("{name}: {e}"))?;
            Opened {
                document: import.document,
                report: Some(import.report),
            }
        }
        Format::Xlsx => workbook_document(bytes).map_err(|e| format!("{name}: {e}"))?,
        Format::Pdf => {
            let import = import_pdf(bytes).map_err(|e| format!("{name}: {e}"))?;
            Opened {
                document: import.document,
                report: Some(import.report),
            }
        }
    };
    opened
        .document
        .validate()
        .map_err(|e| format!("{name}: the document is not valid: {e}"))?;
    Ok(opened)
}

fn next_block_id(doc: &Document) -> BlockId {
    BlockId(doc.blocks.keys().map(|b| b.0).max().map_or(1, |m| m + 1))
}

fn child_count(doc: &Document) -> usize {
    match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children.len(),
        _ => 0,
    }
}

/// A workbook as a document: a heading and a grid for each worksheet.
fn workbook_document(bytes: &[u8]) -> Result<Opened, String> {
    let workbook = import_xlsx_workbook(bytes).map_err(|e| e.to_string())?;
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: Vec::new(),
        },
    );
    let mut history = History::new();
    for entry in workbook.sheets {
        let heading = next_block_id(&doc);
        let index = child_count(&doc);
        history
            .commit(
                &mut doc,
                vec![
                    Op::AddBlock {
                        id: heading,
                        block: Block::Text {
                            content: entry.name,
                            style: TextStyle {
                                heading_level: Some(2),
                                ..TextStyle::default()
                            },
                        },
                    },
                    Op::InsertChild {
                        parent: BlockId(0),
                        index,
                        child: heading,
                    },
                ],
            )
            .map_err(|e| e.to_string())?;
        let index = child_count(&doc);
        place_sheet(&mut doc, &mut history, entry.sheet, BlockId(0), index)
            .map_err(|e| e.to_string())?;
    }
    Ok(Opened {
        document: doc,
        report: Some(workbook.report),
    })
}

/// A document written as a file's bytes, with the export's fidelity report when the format has
/// one.
#[derive(Debug)]
pub struct Saved {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// What the export wrote in a closer form, or left out.
    pub report: Option<FidelityReport>,
}

/// Writes `doc` in the format of a file called `name`. Errors start with `name`. A PDF's pictures
/// are placeholders; use [`save_with_images`] to draw them.
pub fn save(doc: &Document, name: &str) -> Result<Saved, String> {
    save_with_images(doc, name, &no_images)
}

fn no_images(_: &str) -> Option<Vec<u8>> {
    None
}

/// Writes `doc` in the format of a file called `name`, as [`save`] does. A PDF draws each picture
/// whose URI `images` gives the bytes of, in a PNG or JPEG format.
pub fn save_with_images(
    doc: &Document,
    name: &str,
    images: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Saved, String> {
    let (bytes, report) = match format_of(name)? {
        Format::Markdown => (
            to_markdown(doc)
                .map_err(|e| format!("{name}: cannot be written as Markdown: {e}"))?
                .into_bytes(),
            None,
        ),
        Format::Json => (
            serde_json::to_vec_pretty(doc).map_err(|e| format!("{name}: {e}"))?,
            None,
        ),
        Format::Docx => {
            let resolved = tpt_codex_grid::resolve_document(doc);
            let computed = tpt_codex_grid::with_computed_cells(resolved.as_ref().unwrap_or(doc));
            let export = export_docx(&computed).map_err(|e| format!("{name}: {e}"))?;
            (export.bytes, Some(export.report))
        }
        Format::Xlsx => {
            let sheet =
                doc.sheets.values().next().ok_or_else(|| {
                    format!("{name}: the document has no grid to write as a sheet")
                })?;
            (
                export_xlsx(sheet).map_err(|e| format!("{name}: {e}"))?,
                None,
            )
        }
        Format::Pdf => (
            tpt_codex_pdf::export_with_images(doc, images).map_err(|e| format!("{name}: {e}"))?,
            None,
        ),
        Format::Pptx => return Err(format!("{name}: .pptx cannot be written")),
    };
    Ok(Saved { bytes, report })
}
