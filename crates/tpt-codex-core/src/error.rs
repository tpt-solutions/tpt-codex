// SPDX-License-Identifier: MIT OR Apache-2.0
use core::fmt;

use crate::id::{BlockId, SheetId};

/// A structural problem found by [`Document::validate`](crate::Document::validate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The document was written with a schema version this build does not read.
    UnsupportedSchemaVersion {
        /// Version found in the document.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },
    /// The root id does not refer to a block in the document.
    MissingRoot(BlockId),
    /// A container references a child id that does not exist.
    DanglingChild {
        /// The container holding the reference.
        parent: BlockId,
        /// The missing child id.
        child: BlockId,
    },
    /// The block graph contains a cycle through this block.
    Cycle(BlockId),
    /// A text block has an inline run that is empty, reversed, or past the end of its text.
    InvalidRun {
        /// The text block.
        block: BlockId,
    },
    /// A frame is recorded for a block that does not exist.
    FrameOfMissingBlock(BlockId),
    /// A frame has a non-finite edge or a negative size.
    InvalidFrame(BlockId),
    /// A canvas is recorded for a block that is not an absolute container.
    CanvasNotAbsolute(BlockId),
    /// A canvas has a non-finite or non-positive size.
    InvalidCanvas(BlockId),
    /// A `Block::Grid` refers to a sheet that does not exist.
    MissingSheet {
        /// The grid block.
        block: BlockId,
        /// The missing sheet.
        sheet: SheetId,
    },
    /// A sheet has a cell address that is not canonical A1 notation.
    InvalidAddress {
        /// The sheet holding the cell.
        sheet: SheetId,
        /// The bad address.
        address: alloc::string::String,
    },
    /// A formula cell has a source that does not start with `=`.
    InvalidFormula {
        /// The sheet holding the cell.
        sheet: SheetId,
        /// The cell's address.
        address: alloc::string::String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "unsupported schema version {found} (this build supports {supported})"
            ),
            Self::MissingRoot(id) => write!(f, "root block {} does not exist", id.0),
            Self::DanglingChild { parent, child } => write!(
                f,
                "block {} references missing child block {}",
                parent.0, child.0
            ),
            Self::Cycle(id) => write!(f, "block {} is part of a cycle", id.0),
            Self::InvalidRun { block } => write!(
                f,
                "text block {} has an inline run outside its text or with no characters",
                block.0
            ),
            Self::FrameOfMissingBlock(id) => {
                write!(
                    f,
                    "a frame is recorded for block {} which does not exist",
                    id.0
                )
            }
            Self::InvalidFrame(id) => write!(f, "block {} has an invalid frame", id.0),
            Self::CanvasNotAbsolute(id) => write!(
                f,
                "block {} has a canvas but is not an absolute container",
                id.0
            ),
            Self::InvalidCanvas(id) => write!(f, "block {} has an invalid canvas size", id.0),
            Self::MissingSheet { block, sheet } => write!(
                f,
                "grid block {} refers to missing sheet {}",
                block.0, sheet.0
            ),
            Self::InvalidAddress { sheet, address } => {
                write!(
                    f,
                    "sheet {} has an invalid cell address '{address}'",
                    sheet.0
                )
            }
            Self::InvalidFormula { sheet, address } => write!(
                f,
                "sheet {} cell '{address}' is a formula that does not start with '='",
                sheet.0
            ),
        }
    }
}

impl core::error::Error for ValidationError {}
