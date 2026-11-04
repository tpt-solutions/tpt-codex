// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::document::Document;
use crate::id::BlockId;
use crate::ops::{EditError, Op, apply, changed_blocks};

/// A group of operations that succeed or fail together.
#[derive(Debug, Clone, PartialEq)]
struct Transaction {
    /// The operations, in the order they were applied.
    ops: Vec<Op>,
    /// The inverse of each operation, in the same order as `ops`.
    inverses: Vec<Op>,
}

/// Undo and redo for document edits.
///
/// Changes go through [`History::commit`]. Undo applies the stored inverses in
/// reverse order and redo re-applies the original operations. A new commit
/// clears the redo stack.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    last_changed: BTreeSet<BlockId>,
}

impl History {
    /// Creates an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `ops` to `doc` as one undoable step.
    ///
    /// If any operation fails, the operations already applied are reverted and
    /// the document is left unchanged.
    pub fn commit(&mut self, doc: &mut Document, ops: Vec<Op>) -> Result<(), EditError> {
        let inverses = apply_all(doc, &ops)?;
        self.last_changed = changed_blocks(&ops);
        self.undo.push(Transaction { ops, inverses });
        self.redo.clear();
        Ok(())
    }

    /// Reverts the most recent step. Returns `Ok(false)` when there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<bool, EditError> {
        let Some(tx) = self.undo.pop() else {
            return Ok(false);
        };
        for inverse in tx.inverses.iter().rev() {
            apply(doc, inverse)
                .expect("undo: stored inverse must apply to the state it was made for");
        }
        self.last_changed = changed_blocks(&tx.ops);
        self.redo.push(tx);
        Ok(true)
    }

    /// Re-applies the most recently undone step. Returns `Ok(false)` when there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<bool, EditError> {
        let Some(tx) = self.redo.pop() else {
            return Ok(false);
        };
        match apply_all(doc, &tx.ops) {
            Ok(_) => {
                self.last_changed = changed_blocks(&tx.ops);
                self.undo.push(tx);
                Ok(true)
            }
            Err(e) => {
                self.redo.push(tx);
                Err(e)
            }
        }
    }

    /// The blocks changed by the most recent commit, undo or redo.
    ///
    /// A view uses this to rebuild only the projections that changed. It is empty
    /// before the first step, and after a step that changed nothing.
    pub fn last_changed(&self) -> &BTreeSet<BlockId> {
        &self.last_changed
    }

    /// Number of steps that can be undone.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Number of steps that can be redone.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

/// Applies every operation, or none. Returns the inverses in application order.
fn apply_all(doc: &mut Document, ops: &[Op]) -> Result<Vec<Op>, EditError> {
    let mut inverses = Vec::with_capacity(ops.len());
    for op in ops {
        match apply(doc, op) {
            Ok(inverse) => inverses.push(inverse),
            Err(e) => {
                for inverse in inverses.iter().rev() {
                    apply(doc, inverse)
                        .expect("rollback: stored inverse must apply to the state it was made for");
                }
                return Err(e);
            }
        }
    }
    Ok(inverses)
}
