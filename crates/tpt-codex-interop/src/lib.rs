// SPDX-License-Identifier: MIT OR Apache-2.0
//! Import/export adapters for legacy formats (.docx, .xlsx, .pdf, .pptx).
//!
//! Implemented so far: the [`FidelityReport`] that every importer fills in.
//!
//! Status: no format adapter yet. Adapters are planned for Phase 5, after v1, and depend on
//! the TPT document crates, which are not yet audited (see `docs/tpt-dependency-audit.md`).

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
pub use xlsx::{XlsxError, XlsxImport, export_xlsx, import_xlsx};

pub use tpt_codex_core as core;
