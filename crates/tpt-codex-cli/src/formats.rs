// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reading and writing documents by file extension.

use std::fs;
use std::path::Path;

use tpt_codex_core::{Block, BlockId, Document, History, LayoutMode, Op, TextStyle};
use tpt_codex_interop::{
    FidelityReport, export_docx, export_xlsx, import_docx, import_pptx, import_xlsx_workbook,
    place_sheet,
};
use tpt_codex_markdown::{from_markdown, to_markdown};

/// A file format the CLI knows, found from the file's extension.
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
    /// PDF (`.pdf`), written only.
    Pdf,
}

/// The format of `path`, from its extension.
pub fn format_of(path: &Path) -> Result<Format, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" | "cdx" => Ok(Format::Markdown),
        "docx" => Ok(Format::Docx),
        "xlsx" => Ok(Format::Xlsx),
        "pptx" => Ok(Format::Pptx),
        "json" => Ok(Format::Json),
        "pdf" => Ok(Format::Pdf),
        _ => Err(format!(
            "{}: unknown file type (use .cdx, .md, .docx, .xlsx, .pptx, .json or .pdf)",
            path.display()
        )),
    }
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
    let format = format_of(path)?;
    let name = path.display();
    let bytes = fs::read(path).map_err(|e| format!("{name}: cannot read: {e}"))?;
    let loaded = match format {
        Format::Markdown => {
            let text = String::from_utf8(bytes).map_err(|_| format!("{name}: not UTF-8 text"))?;
            let document = from_markdown(&text).map_err(|e| {
                let hint = e
                    .suggestion
                    .map(|h| format!(" (hint: {h})"))
                    .unwrap_or_default();
                format!("{name}:{}:{}: error: {}{hint}", e.line, e.column, e.message)
            })?;
            Loaded {
                document,
                report: None,
            }
        }
        Format::Json => {
            let document: Document = serde_json::from_slice(&bytes)
                .map_err(|e| format!("{name}: not a document: {e}"))?;
            Loaded {
                document,
                report: None,
            }
        }
        Format::Docx => {
            let import = import_docx(&bytes).map_err(|e| format!("{name}: {e}"))?;
            Loaded {
                document: import.document,
                report: Some(import.report),
            }
        }
        Format::Pptx => {
            let import = import_pptx(&bytes).map_err(|e| format!("{name}: {e}"))?;
            Loaded {
                document: import.document,
                report: Some(import.report),
            }
        }
        Format::Xlsx => workbook_document(&bytes).map_err(|e| format!("{name}: {e}"))?,
        Format::Pdf => {
            return Err(format!(
                "{name}: PDF files cannot be read yet (PDF is an output format)"
            ));
        }
    };
    loaded
        .document
        .validate()
        .map_err(|e| format!("{name}: the document is not valid: {e}"))?;
    Ok(loaded)
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
fn workbook_document(bytes: &[u8]) -> Result<Loaded, String> {
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
    Ok(Loaded {
        document: doc,
        report: Some(workbook.report),
    })
}

/// Writes `doc` to `path`. Returns the export's fidelity report when the format has one.
pub fn save(doc: &Document, path: &Path) -> Result<Option<FidelityReport>, String> {
    let format = format_of(path)?;
    let name = path.display();
    let (bytes, report) = match format {
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
            let export = export_docx(doc).map_err(|e| format!("{name}: {e}"))?;
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
            tpt_codex_pdf::export_with_defaults(doc).map_err(|e| format!("{name}: {e}"))?,
            None,
        ),
        Format::Pptx => {
            return Err(format!("{name}: .pptx cannot be written"));
        }
    };
    fs::write(path, bytes).map_err(|e| format!("{name}: cannot write: {e}"))?;
    Ok(report)
}
