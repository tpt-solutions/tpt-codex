// SPDX-License-Identifier: MIT OR Apache-2.0
//! Import/export adapters for legacy formats (.docx, .xlsx, .pdf, .pptx).
//!
//! All adapters are in-house, on [`Archive`] (a ZIP reader) and [`ArchiveWriter`]:
//! - [`import_docx`] and [`export_docx`];
//! - [`import_xlsx`] and [`export_xlsx`], with [`place_sheet`] to put an imported sheet in a document;
//! - [`import_pptx`] (data only).
//!
//! Every importer fills in a [`FidelityReport`] that lists what it dropped or approximated.
//! Not done: `.pdf` import (see `todo.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod archive;
mod docx;
mod docx_write;
mod fidelity;
mod place;
mod pptx;
mod xlsx;

pub use archive::{Archive, ArchiveWriter, ZipError};
pub use docx::{DocxError, DocxImport, import_docx};
pub use docx_write::{DocxExport, export_docx};
pub use fidelity::{Fidelity, FidelityItem, FidelityReport};
pub use place::{PlaceError, SheetPlacement, place_sheet};
pub use pptx::{PptxImport, import_pptx};
pub use xlsx::{
    XlsxError, XlsxImport, XlsxSheet, XlsxWorkbook, export_xlsx, import_xlsx, import_xlsx_workbook,
};

pub use tpt_codex_core as core;
