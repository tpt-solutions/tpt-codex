// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::error::ValidationError;
use crate::id::BlockId;

/// The schema version written by this build.
pub const SCHEMA_VERSION: u32 = 1;

/// Front-matter style metadata for a document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    /// Document title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Author name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Date as written by the author (free text, e.g. `2026-11-04`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Document class, e.g. `article`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
}

/// A tpt-codex document: a DAG of blocks with a single root.
///
/// Blocks are stored in a `BTreeMap` so that serialization order is
/// deterministic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// Schema version of the serialized form.
    pub schema_version: u32,
    /// Document metadata.
    #[serde(default)]
    pub metadata: Metadata,
    /// The root block of the tree.
    pub root: BlockId,
    /// All blocks, keyed by id.
    pub blocks: BTreeMap<BlockId, Block>,
}

impl Document {
    /// Creates a document containing a single root block.
    pub fn new(root_id: BlockId, root: Block) -> Self {
        let mut blocks = BTreeMap::new();
        blocks.insert(root_id, root);
        Self {
            schema_version: SCHEMA_VERSION,
            metadata: Metadata::default(),
            root: root_id,
            blocks,
        }
    }

    /// Returns the block with the given id, if present.
    pub fn get(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(&id)
    }

    /// Inserts or replaces a block, returning the previous block at that id.
    ///
    /// This does not check invariants; call [`Document::validate`] after editing.
    pub fn insert(&mut self, id: BlockId, block: Block) -> Option<Block> {
        self.blocks.insert(id, block)
    }

    /// Checks the structural invariants of the document.
    ///
    /// Verifies the schema version, that the root exists, that every container
    /// child exists, and that the graph is acyclic.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ValidationError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        if !self.blocks.contains_key(&self.root) {
            return Err(ValidationError::MissingRoot(self.root));
        }
        for (&parent, block) in &self.blocks {
            for &child in children_of(block) {
                if !self.blocks.contains_key(&child) {
                    return Err(ValidationError::DanglingChild { parent, child });
                }
            }
        }
        if let Some(id) = self.find_cycle() {
            return Err(ValidationError::Cycle(id));
        }
        Ok(())
    }

    /// Iterative three-colour DFS. Returns a block on a cycle, if any.
    fn find_cycle(&self) -> Option<BlockId> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            InProgress,
            Done,
        }

        let mut marks: BTreeMap<BlockId, Mark> = BTreeMap::new();
        // Each frame is (node, index of the next child to visit).
        let mut stack: Vec<(BlockId, usize)> = Vec::new();

        for &start in self.blocks.keys() {
            if marks.contains_key(&start) {
                continue;
            }
            marks.insert(start, Mark::InProgress);
            stack.push((start, 0));

            while let Some(frame) = stack.last_mut() {
                let node = frame.0;
                let children = self.blocks.get(&node).map(children_of).unwrap_or(&[]);
                if frame.1 < children.len() {
                    let child = children[frame.1];
                    frame.1 += 1;
                    match marks.get(&child) {
                        Some(Mark::InProgress) => return Some(child),
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(child, Mark::InProgress);
                            stack.push((child, 0));
                        }
                    }
                } else {
                    marks.insert(node, Mark::Done);
                    stack.pop();
                }
            }
        }
        None
    }
}

fn children_of(block: &Block) -> &[BlockId] {
    match block {
        Block::Container { children, .. } => children,
        Block::List { items, .. } => items,
        _ => &[],
    }
}
