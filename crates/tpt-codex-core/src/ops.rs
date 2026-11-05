// SPDX-License-Identifier: MIT OR Apache-2.0
//! Editing operations on a [`Document`].
//!
//! Every operation checks its preconditions before it changes anything, so a
//! failed operation leaves the document as it was. [`apply`] returns the
//! inverse operation, which is what undo uses (see [`crate::History`]).
//!
//! Operations keep the DAG invariants: a change that would make a block reach
//! itself is rejected with [`EditError::WouldCreateCycle`]. Removing a block
//! that is still referenced is rejected, so references never dangle.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::fmt;

use alloc::string::String;

use crate::block::Block;
use crate::document::Document;
use crate::id::{BlockId, SheetId};
use crate::primitives::{Canvas, LayoutMode, Rect};
use crate::sheet::{CellInput, Sheet, is_formula_source, is_valid_address};

/// One change to a document. Child positions are indices into a container's `children`.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Add a block that is not yet referenced by any container.
    AddBlock {
        /// Id of the new block. Must not exist.
        id: BlockId,
        /// The block contents. Any children it names must exist.
        block: Block,
    },
    /// Remove a block that no container references. The root cannot be removed.
    RemoveBlock {
        /// Id of the block to remove.
        id: BlockId,
    },
    /// Replace the contents of an existing block.
    SetBlock {
        /// Id of the block to change.
        id: BlockId,
        /// The new contents.
        block: Block,
    },
    /// Add a reference from a container to an existing block.
    InsertChild {
        /// The container.
        parent: BlockId,
        /// Position in the container's children. `0..=len`.
        index: usize,
        /// The block to reference.
        child: BlockId,
    },
    /// Remove a reference from a container. The referenced block stays in the document.
    RemoveChild {
        /// The container.
        parent: BlockId,
        /// Position to remove. `0..len`.
        index: usize,
    },
    /// Set or clear one cell of a sheet. `None` clears the cell. The sheet must exist.
    SetCell {
        /// The sheet to change.
        sheet: SheetId,
        /// Canonical A1 address, for example `B4`.
        address: String,
        /// The new input, or `None` to clear the cell.
        input: Option<CellInput>,
    },
    /// Insert, replace or remove a whole sheet. `Some` inserts or replaces it, `None` removes
    /// it. A sheet that a `Block::Grid` still displays cannot be removed.
    SetSheet {
        /// The sheet to change.
        id: SheetId,
        /// The new sheet, or `None` to remove it.
        sheet: Option<Sheet>,
    },
    /// Set or clear the frame of a child of an absolute container. `None` clears it.
    SetFrame {
        /// The child block.
        id: BlockId,
        /// The new frame in points, or `None` to clear it.
        frame: Option<Rect>,
    },
    /// Set or clear the canvas size of an absolute container. `None` clears it.
    SetCanvas {
        /// The container.
        id: BlockId,
        /// The new canvas size, or `None` to clear it.
        canvas: Option<Canvas>,
    },
    /// Move a reference from one position to another, possibly in another container.
    MoveChild {
        /// Container the reference is taken from.
        from_parent: BlockId,
        /// Position it is taken from.
        from_index: usize,
        /// Container it moves to.
        to_parent: BlockId,
        /// Position in the target container after the reference is removed. `0..=len`.
        to_index: usize,
    },
}

/// Why an operation was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// A block id does not exist.
    NoSuchBlock(BlockId),
    /// `AddBlock` was given an id that already exists.
    BlockExists(BlockId),
    /// The block is not a container, so it has no children.
    NotAContainer(BlockId),
    /// A child position is outside the valid range.
    IndexOutOfBounds {
        /// The container.
        parent: BlockId,
        /// The position given.
        index: usize,
        /// The number of valid positions (exclusive upper bound for removals).
        len: usize,
    },
    /// The change would make `parent` reachable from `child`, creating a cycle.
    WouldCreateCycle {
        /// The container that would gain the reference.
        parent: BlockId,
        /// The block whose reachability would close the loop.
        child: BlockId,
    },
    /// `RemoveBlock` was asked to remove a block that is still referenced.
    StillReferenced {
        /// The block to remove.
        id: BlockId,
        /// A container that references it.
        parent: BlockId,
    },
    /// The root block cannot be removed.
    IsRoot(BlockId),
    /// A block names a child that does not exist.
    DanglingChild(BlockId),
    /// A text edit was given to a block that is not a text block.
    NotText(BlockId),
    /// A cell operation names a sheet that does not exist.
    NoSuchSheet(SheetId),
    /// A cell address is not canonical A1 notation.
    InvalidAddress(String),
    /// A formula does not start with `=`.
    InvalidFormula(String),
    /// `SetSheet` tried to remove a sheet that a grid block still displays.
    SheetReferenced {
        /// The sheet that cannot be removed.
        id: SheetId,
        /// A grid block that displays it.
        block: BlockId,
    },
    /// A frame was set on a block that is not a child of an absolute container, or a canvas on a
    /// container that is not absolute.
    NotAbsolute(BlockId),
    /// A frame or canvas has a non-finite edge, or a negative or zero size.
    InvalidGeometry(BlockId),
    /// The block has a frame or canvas, so it cannot be removed or changed to another layout.
    IsPositioned(BlockId),
    /// A character range is outside the text.
    RangeOutOfBounds {
        /// The block being edited.
        id: BlockId,
        /// Number of characters in the text.
        len: usize,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchBlock(id) => write!(f, "block {} does not exist", id.0),
            Self::BlockExists(id) => write!(f, "block {} already exists", id.0),
            Self::NotAContainer(id) => write!(f, "block {} is not a container", id.0),
            Self::IndexOutOfBounds { parent, index, len } => write!(
                f,
                "index {index} is out of bounds for container {} (length {len})",
                parent.0
            ),
            Self::WouldCreateCycle { parent, child } => write!(
                f,
                "referencing block {} from container {} would create a cycle",
                child.0, parent.0
            ),
            Self::StillReferenced { id, parent } => write!(
                f,
                "block {} is still referenced by container {}",
                id.0, parent.0
            ),
            Self::IsRoot(id) => write!(f, "block {} is the root and cannot be removed", id.0),
            Self::DanglingChild(id) => write!(f, "child block {} does not exist", id.0),
            Self::NotText(id) => write!(f, "block {} is not a text block", id.0),
            Self::NoSuchSheet(id) => write!(f, "sheet {} does not exist", id.0),
            Self::InvalidAddress(a) => write!(f, "'{a}' is not a valid cell address"),
            Self::InvalidFormula(a) => write!(f, "formula '{a}' must start with '='"),
            Self::SheetReferenced { id, block } => {
                write!(f, "sheet {} is still displayed by block {}", id.0, block.0)
            }
            Self::NotAbsolute(id) => write!(
                f,
                "block {} is not a child of an absolute container, or not an absolute container",
                id.0
            ),
            Self::InvalidGeometry(id) => write!(f, "block {} has an invalid frame or size", id.0),
            Self::IsPositioned(id) => write!(
                f,
                "block {} has a frame or canvas; clear it before changing the block",
                id.0
            ),
            Self::RangeOutOfBounds { id, len } => write!(
                f,
                "character range is outside block {} (text length {len})",
                id.0
            ),
        }
    }
}

impl core::error::Error for EditError {}

impl Op {
    /// The blocks whose own record this operation changes.
    ///
    /// For child-list operations this is the container, because its children changed
    /// and any projection of it must be rebuilt. The blocks a moved child refers to are
    /// not included, since their contents did not change.
    pub fn changed_blocks(&self) -> alloc::vec::Vec<BlockId> {
        match self {
            Op::AddBlock { id, .. } | Op::RemoveBlock { id } | Op::SetBlock { id, .. } => {
                alloc::vec![*id]
            }
            Op::InsertChild { parent, .. } | Op::RemoveChild { parent, .. } => {
                alloc::vec![*parent]
            }
            Op::SetFrame { id, .. } | Op::SetCanvas { id, .. } => alloc::vec![*id],
            Op::SetCell { .. } | Op::SetSheet { .. } => Vec::new(),
            Op::MoveChild {
                from_parent,
                to_parent,
                ..
            } => alloc::vec![*from_parent, *to_parent],
        }
    }
}

/// The blocks changed by a group of operations, without duplicates, in id order.
pub fn changed_blocks(ops: &[Op]) -> BTreeSet<BlockId> {
    ops.iter().flat_map(Op::changed_blocks).collect()
}

/// Applies `op` to `doc` and returns the inverse operation.
///
/// Applying the returned inverse restores the document to its state before `op`.
pub fn apply(doc: &mut Document, op: &Op) -> Result<Op, EditError> {
    match op {
        Op::AddBlock { id, block } => {
            if doc.blocks.contains_key(id) {
                return Err(EditError::BlockExists(*id));
            }
            check_children_exist(doc, block)?;
            doc.blocks.insert(*id, block.clone());
            Ok(Op::RemoveBlock { id: *id })
        }
        Op::RemoveBlock { id } => {
            if *id == doc.root {
                return Err(EditError::IsRoot(*id));
            }
            if doc.frames.contains_key(id) || doc.canvases.contains_key(id) {
                return Err(EditError::IsPositioned(*id));
            }
            let block = doc
                .blocks
                .get(id)
                .ok_or(EditError::NoSuchBlock(*id))?
                .clone();
            if let Some(parent) = referencing_parent(doc, *id) {
                return Err(EditError::StillReferenced { id: *id, parent });
            }
            doc.blocks.remove(id);
            Ok(Op::AddBlock { id: *id, block })
        }
        Op::SetBlock { id, block } => {
            let old = doc
                .blocks
                .get(id)
                .ok_or(EditError::NoSuchBlock(*id))?
                .clone();
            check_children_exist(doc, block)?;
            if doc.canvases.contains_key(id) && !is_absolute(block) {
                return Err(EditError::IsPositioned(*id));
            }
            for &child in children_of(block) {
                if reaches(doc, child, *id) {
                    return Err(EditError::WouldCreateCycle { parent: *id, child });
                }
            }
            doc.blocks.insert(*id, block.clone());
            Ok(Op::SetBlock {
                id: *id,
                block: old,
            })
        }
        Op::InsertChild {
            parent,
            index,
            child,
        } => {
            let len = container_len(doc, *parent)?;
            if !doc.blocks.contains_key(child) {
                return Err(EditError::NoSuchBlock(*child));
            }
            if *index > len {
                return Err(EditError::IndexOutOfBounds {
                    parent: *parent,
                    index: *index,
                    len,
                });
            }
            if reaches(doc, *child, *parent) {
                return Err(EditError::WouldCreateCycle {
                    parent: *parent,
                    child: *child,
                });
            }
            children_mut(doc, *parent)?.insert(*index, *child);
            Ok(Op::RemoveChild {
                parent: *parent,
                index: *index,
            })
        }
        Op::RemoveChild { parent, index } => {
            let len = container_len(doc, *parent)?;
            if *index >= len {
                return Err(EditError::IndexOutOfBounds {
                    parent: *parent,
                    index: *index,
                    len,
                });
            }
            let child = children_mut(doc, *parent)?.remove(*index);
            Ok(Op::InsertChild {
                parent: *parent,
                index: *index,
                child,
            })
        }
        Op::SetSheet { id, sheet } => {
            let previous = match sheet {
                Some(new) => doc.sheets.insert(*id, new.clone()),
                None => {
                    if let Some(block) = doc.blocks.iter().find_map(|(bid, b)| match b {
                        Block::Grid { sheet_id, .. } if sheet_id == id => Some(*bid),
                        _ => None,
                    }) {
                        return Err(EditError::SheetReferenced { id: *id, block });
                    }
                    doc.sheets.remove(id)
                }
            };
            Ok(Op::SetSheet {
                id: *id,
                sheet: previous,
            })
        }
        Op::SetFrame { id, frame } => {
            if !doc.blocks.contains_key(id) {
                return Err(EditError::NoSuchBlock(*id));
            }
            let parent = referencing_parent(doc, *id).ok_or(EditError::NotAbsolute(*id))?;
            let absolute = doc.blocks.get(&parent).is_some_and(is_absolute);
            if !absolute {
                return Err(EditError::NotAbsolute(*id));
            }
            if frame.is_some_and(|f| !f.is_valid()) {
                return Err(EditError::InvalidGeometry(*id));
            }
            let previous = match frame {
                Some(new) => doc.frames.insert(*id, *new),
                None => doc.frames.remove(id),
            };
            Ok(Op::SetFrame {
                id: *id,
                frame: previous,
            })
        }
        Op::SetCanvas { id, canvas } => {
            let block = doc.blocks.get(id).ok_or(EditError::NoSuchBlock(*id))?;
            if !is_absolute(block) {
                return Err(EditError::NotAbsolute(*id));
            }
            if canvas.is_some_and(|c| !c.is_valid()) {
                return Err(EditError::InvalidGeometry(*id));
            }
            let previous = match canvas {
                Some(new) => doc.canvases.insert(*id, *new),
                None => doc.canvases.remove(id),
            };
            Ok(Op::SetCanvas {
                id: *id,
                canvas: previous,
            })
        }
        Op::SetCell {
            sheet,
            address,
            input,
        } => {
            if !is_valid_address(address) {
                return Err(EditError::InvalidAddress(address.clone()));
            }
            if let Some(CellInput::Formula { source }) = input {
                if !is_formula_source(source) {
                    return Err(EditError::InvalidFormula(address.clone()));
                }
            }
            let data = doc
                .sheets
                .get_mut(sheet)
                .ok_or(EditError::NoSuchSheet(*sheet))?;
            let previous = match input {
                Some(new) => data.cells.insert(address.clone(), new.clone()),
                None => data.cells.remove(address),
            };
            Ok(Op::SetCell {
                sheet: *sheet,
                address: address.clone(),
                input: previous,
            })
        }
        Op::MoveChild {
            from_parent,
            from_index,
            to_parent,
            to_index,
        } => {
            let from_len = container_len(doc, *from_parent)?;
            let to_len = container_len(doc, *to_parent)?;
            if *from_index >= from_len {
                return Err(EditError::IndexOutOfBounds {
                    parent: *from_parent,
                    index: *from_index,
                    len: from_len,
                });
            }
            // Length of the target once the reference is taken out.
            let to_len_after = if from_parent == to_parent {
                to_len - 1
            } else {
                to_len
            };
            if *to_index > to_len_after {
                return Err(EditError::IndexOutOfBounds {
                    parent: *to_parent,
                    index: *to_index,
                    len: to_len_after,
                });
            }
            let child = children_of_id(doc, *from_parent)[*from_index];
            if reaches(doc, child, *to_parent) {
                return Err(EditError::WouldCreateCycle {
                    parent: *to_parent,
                    child,
                });
            }
            children_mut(doc, *from_parent)?.remove(*from_index);
            children_mut(doc, *to_parent)?.insert(*to_index, child);
            Ok(Op::MoveChild {
                from_parent: *to_parent,
                from_index: *to_index,
                to_parent: *from_parent,
                to_index: *from_index,
            })
        }
    }
}

fn children_of(block: &Block) -> &[BlockId] {
    match block {
        Block::Container { children, .. } => children,
        Block::List { items, .. } => items,
        _ => &[],
    }
}

fn children_of_id(doc: &Document, id: BlockId) -> &[BlockId] {
    doc.blocks.get(&id).map(children_of).unwrap_or(&[])
}

fn children_mut(doc: &mut Document, id: BlockId) -> Result<&mut Vec<BlockId>, EditError> {
    match doc.blocks.get_mut(&id) {
        Some(Block::Container { children, .. }) => Ok(children),
        Some(_) => Err(EditError::NotAContainer(id)),
        None => Err(EditError::NoSuchBlock(id)),
    }
}

fn container_len(doc: &Document, id: BlockId) -> Result<usize, EditError> {
    match doc.blocks.get(&id) {
        Some(Block::Container { children, .. }) => Ok(children.len()),
        Some(_) => Err(EditError::NotAContainer(id)),
        None => Err(EditError::NoSuchBlock(id)),
    }
}

fn check_children_exist(doc: &Document, block: &Block) -> Result<(), EditError> {
    for &child in children_of(block) {
        if !doc.blocks.contains_key(&child) {
            return Err(EditError::DanglingChild(child));
        }
    }
    Ok(())
}

/// Returns true if `target` can be reached from `from` by following child references.
/// `from == target` counts as reachable.
fn reaches(doc: &Document, from: BlockId, target: BlockId) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = Vec::new();
    stack.push(from);
    while let Some(node) = stack.pop() {
        if node == target {
            return true;
        }
        if !seen.insert(node) {
            continue;
        }
        stack.extend_from_slice(children_of_id(doc, node));
    }
    false
}

/// Returns a container that references `id`, if any.
/// Whether `block` is a container that places its children by explicit position.
fn is_absolute(block: &Block) -> bool {
    matches!(
        block,
        Block::Container {
            layout_mode: LayoutMode::Absolute,
            ..
        }
    )
}

fn referencing_parent(doc: &Document, id: BlockId) -> Option<BlockId> {
    doc.blocks
        .iter()
        .find(|(_, block)| children_of(block).contains(&id))
        .map(|(&parent, _)| parent)
}
