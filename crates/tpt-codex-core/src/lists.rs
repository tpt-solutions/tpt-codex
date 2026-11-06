// SPDX-License-Identifier: MIT OR Apache-2.0
//! Edits to `Block::List`: add an item, remove an item, and change between bulleted and
//! numbered.
//!
//! Each function checks the document and returns the operations for one undoable step.
//! Commit them with [`History::commit`](crate::History::commit). Indenting is not offered: a
//! list item is a text block, and the model has no nested lists.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::block::Block;
use crate::document::Document;
use crate::id::BlockId;
use crate::ops::{EditError, Op};
use crate::primitives::TextStyle;

/// Adds a text item with `content` at `index` (`0..=len`) of `list`.
///
/// Returns the new item's id and the operations that create and place it.
pub fn add_item(
    doc: &Document,
    list: BlockId,
    index: usize,
    content: &str,
) -> Result<(BlockId, Vec<Op>), EditError> {
    let len = items(doc, list)?.len();
    if index > len {
        return Err(EditError::IndexOutOfBounds {
            parent: list,
            index,
            len,
        });
    }
    let id = next_block_id(doc);
    let ops = vec![
        Op::AddBlock {
            id,
            block: Block::Text {
                content: String::from(content),
                style: TextStyle::default(),
            },
        },
        Op::InsertChild {
            parent: list,
            index,
            child: id,
        },
    ];
    Ok((id, ops))
}

/// Removes the item at `index` (`0..len`) of `list` and deletes its block.
pub fn remove_item(doc: &Document, list: BlockId, index: usize) -> Result<Vec<Op>, EditError> {
    let current = items(doc, list)?;
    let Some(&item) = current.get(index) else {
        return Err(EditError::IndexOutOfBounds {
            parent: list,
            index,
            len: current.len(),
        });
    };
    Ok(vec![
        Op::RemoveChild {
            parent: list,
            index,
        },
        Op::RemoveBlock { id: item },
    ])
}

/// Makes `list` numbered (`ordered == true`) or bulleted. Returns no operations when it already
/// has that kind.
///
/// A bulleted list always starts at 1, so switching to bulleted resets the start.
pub fn set_kind(doc: &Document, list: BlockId, ordered: bool) -> Result<Vec<Op>, EditError> {
    let Some(Block::List {
        ordered: was,
        start,
        items,
    }) = doc.get(list)
    else {
        return Err(not_a_list(doc, list));
    };
    if *was == ordered {
        return Ok(Vec::new());
    }
    Ok(vec![Op::SetBlock {
        id: list,
        block: Block::List {
            ordered,
            start: if ordered { *start } else { 1 },
            items: items.clone(),
        },
    }])
}

fn items(doc: &Document, list: BlockId) -> Result<&[BlockId], EditError> {
    match doc.get(list) {
        Some(Block::List { items, .. }) => Ok(items),
        _ => Err(not_a_list(doc, list)),
    }
}

fn not_a_list(doc: &Document, id: BlockId) -> EditError {
    match doc.get(id) {
        Some(_) => EditError::NotAList(id),
        None => EditError::NoSuchBlock(id),
    }
}

fn next_block_id(doc: &Document) -> BlockId {
    BlockId(doc.blocks.keys().map(|b| b.0).max().map_or(1, |m| m + 1))
}
