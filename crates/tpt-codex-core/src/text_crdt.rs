// SPDX-License-Identifier: MIT OR Apache-2.0
//! Concurrent editing of one text block: a replicated growable array (RGA) of characters.
//!
//! Every character has a [`Stamp`] (a Lamport clock value and the replica that made it). An
//! insert names the character it goes after. Concurrent inserts after the same character are
//! ordered by stamp, the larger first, so every replica places them the same way. Deletes leave
//! tombstones, so a concurrent insert that names a deleted character still finds its place.
//!
//! Operations must be applied in causal order: an insert needs the character it names, and a
//! delete needs its target. [`TextCrdt::apply`] reports a missing dependency as
//! [`TextCrdtError::Missing`], and the caller holds the operation until it can apply it. Applying
//! the same operation twice does nothing.
//!
//! This is a sequence CRDT for `Block::Text` content only. Structural changes to the block tree
//! are not covered by it.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

/// Unique, totally ordered identity of one inserted character.
///
/// Ordering is by `lamport` first, then by `replica`, so a character inserted later (in causal
/// order) always has a larger stamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stamp {
    /// Lamport clock value at the time of the insert.
    pub lamport: u64,
    /// The replica that made the insert. Breaks ties between concurrent inserts.
    pub replica: u64,
}

/// One change to a text block. Operations are what replicas exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOp {
    /// Insert `value` with identity `id`, after the character `after` (or at the start).
    Insert {
        /// Identity of the new character.
        id: Stamp,
        /// The character it goes after, or `None` for the start of the text.
        after: Option<Stamp>,
        /// The character.
        value: char,
    },
    /// Delete the character `target`.
    Delete {
        /// Identity of the character to delete.
        target: Stamp,
    },
}

/// Why an operation could not be applied yet, or at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextCrdtError {
    /// The operation names a character this replica has not seen. Apply it after that one.
    Missing(Stamp),
    /// A local index is past the end of the visible text.
    IndexOutOfBounds {
        /// The index given.
        index: usize,
        /// Number of visible characters.
        len: usize,
    },
}

impl fmt::Display for TextCrdtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(s) => write!(f, "operation depends on unseen character {s:?}"),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} is outside the text (length {len})")
            }
        }
    }
}

impl core::error::Error for TextCrdtError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    id: Stamp,
    value: char,
    deleted: bool,
}

/// The replicated text of one block, as seen by one replica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCrdt {
    replica: u64,
    /// The largest Lamport value this replica has seen or made.
    clock: u64,
    /// Every character ever inserted, in document order, tombstones included.
    elements: Vec<Element>,
}

impl TextCrdt {
    /// An empty text for replica `replica`. Each replica must use a different id.
    pub fn new(replica: u64) -> Self {
        Self {
            replica,
            clock: 0,
            elements: Vec::new(),
        }
    }

    /// This replica's id.
    pub fn replica(&self) -> u64 {
        self.replica
    }

    /// The visible text.
    pub fn text(&self) -> String {
        self.elements
            .iter()
            .filter(|e| !e.deleted)
            .map(|e| e.value)
            .collect()
    }

    /// Number of visible characters.
    pub fn len(&self) -> usize {
        self.elements.iter().filter(|e| !e.deleted).count()
    }

    /// `true` when no character is visible.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Inserts `value` at visible position `index` (in characters) and returns the operation to
    /// send to the other replicas.
    pub fn insert_local(&mut self, index: usize, value: char) -> Result<TextOp, TextCrdtError> {
        let len = self.len();
        if index > len {
            return Err(TextCrdtError::IndexOutOfBounds { index, len });
        }
        let after = if index == 0 {
            None
        } else {
            Some(self.visible_id(index - 1))
        };
        self.clock += 1;
        let id = Stamp {
            lamport: self.clock,
            replica: self.replica,
        };
        let op = TextOp::Insert { id, after, value };
        self.apply(&op)?;
        Ok(op)
    }

    /// Deletes the visible character at `index` and returns the operation to send.
    pub fn delete_local(&mut self, index: usize) -> Result<TextOp, TextCrdtError> {
        let len = self.len();
        if index >= len {
            return Err(TextCrdtError::IndexOutOfBounds { index, len });
        }
        let op = TextOp::Delete {
            target: self.visible_id(index),
        };
        self.apply(&op)?;
        Ok(op)
    }

    /// Applies an operation made by this or another replica. Applying it again changes nothing.
    pub fn apply(&mut self, op: &TextOp) -> Result<(), TextCrdtError> {
        match op {
            TextOp::Insert { id, after, value } => {
                if self.position(*id).is_some() {
                    return Ok(());
                }
                let mut pos = match after {
                    None => 0,
                    Some(a) => self.position(*a).ok_or(TextCrdtError::Missing(*a))? + 1,
                };
                // Skip characters that were placed after `after` with a larger stamp: they were
                // inserted later, or concurrently with priority, and must stay in front.
                while pos < self.elements.len() && self.elements[pos].id > *id {
                    pos += 1;
                }
                self.elements.insert(
                    pos,
                    Element {
                        id: *id,
                        value: *value,
                        deleted: false,
                    },
                );
                self.clock = self.clock.max(id.lamport);
                Ok(())
            }
            TextOp::Delete { target } => {
                let pos = self
                    .position(*target)
                    .ok_or(TextCrdtError::Missing(*target))?;
                self.elements[pos].deleted = true;
                Ok(())
            }
        }
    }

    fn position(&self, id: Stamp) -> Option<usize> {
        self.elements.iter().position(|e| e.id == id)
    }

    /// Identity of the visible character at `index`. The caller checks the bound.
    fn visible_id(&self, index: usize) -> Stamp {
        self.elements
            .iter()
            .filter(|e| !e.deleted)
            .nth(index)
            .expect("index was checked against len")
            .id
    }
}

/// Writes the replica's visible text into text block `id` as one undoable step.
///
/// The block keeps its style. Use this after applying remote operations, so the document shows
/// the converged text and the local user can undo it like any other edit.
pub fn materialize(
    doc: &mut crate::Document,
    history: &mut crate::History,
    id: crate::BlockId,
    crdt: &TextCrdt,
) -> Result<(), crate::EditError> {
    use crate::{Block, EditError, Op};
    let mut style = match doc.get(id) {
        Some(Block::Text { style, .. }) => style.clone(),
        Some(_) => return Err(EditError::NotText(id)),
        None => return Err(EditError::NoSuchBlock(id)),
    };
    let content = crdt.text();
    // The merged text is not aligned with the old runs, so keep only the marks that still fit.
    style.runs = crate::text::clip_runs(&style.runs, content.chars().count());
    let block = Block::Text { content, style };
    history.commit(doc, alloc::vec![Op::SetBlock { id, block }])
}
