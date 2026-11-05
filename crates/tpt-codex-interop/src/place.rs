// SPDX-License-Identifier: MIT OR Apache-2.0
//! Places an imported sheet in a document as a grid block, as one undoable step.

use core::fmt;

use tpt_codex_core::{Block, BlockId, Document, History, Op, Sheet, SheetId};

/// Where an imported sheet ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetPlacement {
    /// The sheet's id in the document.
    pub sheet: SheetId,
    /// The grid block that shows it.
    pub block: BlockId,
}

/// Why a sheet could not be placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceError {
    /// The parent block does not exist or is not a container.
    NotAContainer(BlockId),
    /// The index is past the end of the parent's children.
    IndexOutOfBounds {
        /// The parent.
        parent: BlockId,
        /// The index given.
        index: usize,
    },
    /// The change was refused by the document.
    Edit(String),
}

impl fmt::Display for PlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAContainer(id) => write!(f, "block {} cannot hold a grid", id.0),
            Self::IndexOutOfBounds { parent, index } => {
                write!(f, "index {index} is outside block {}", parent.0)
            }
            Self::Edit(message) => f.write_str(message),
        }
    }
}

impl core::error::Error for PlaceError {}

/// Adds `sheet` to `doc` and shows it in a new grid block at `index` in `parent`. The three steps
/// (sheet, block, position) are one undoable step.
pub fn place_sheet(
    doc: &mut Document,
    history: &mut History,
    sheet: Sheet,
    parent: BlockId,
    index: usize,
) -> Result<SheetPlacement, PlaceError> {
    let len = match doc.get(parent) {
        Some(Block::Container { children, .. }) => children.len(),
        _ => return Err(PlaceError::NotAContainer(parent)),
    };
    if index > len {
        return Err(PlaceError::IndexOutOfBounds { parent, index });
    }
    let sheet_id = SheetId(doc.sheets.keys().map(|s| s.0).max().map_or(1, |m| m + 1));
    let block = BlockId(doc.blocks.keys().map(|b| b.0).max().map_or(1, |m| m + 1));
    let ops = [
        Op::SetSheet {
            id: sheet_id,
            sheet: Some(sheet),
        },
        Op::AddBlock {
            id: block,
            block: Block::Grid {
                sheet_id,
                view_bounds: None,
            },
        },
        Op::InsertChild {
            parent,
            index,
            child: block,
        },
    ];
    history
        .commit(doc, ops.to_vec())
        .map_err(|e| PlaceError::Edit(e.to_string()))?;
    Ok(SheetPlacement {
        sheet: sheet_id,
        block,
    })
}
