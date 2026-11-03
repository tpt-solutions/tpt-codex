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

use crate::block::Block;
use crate::document::Document;
use crate::id::BlockId;

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
        }
    }
}

impl core::error::Error for EditError {}

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
fn referencing_parent(doc: &Document, id: BlockId) -> Option<BlockId> {
    doc.blocks
        .iter()
        .find(|(_, block)| children_of(block).contains(&id))
        .map(|(&parent, _)| parent)
}
