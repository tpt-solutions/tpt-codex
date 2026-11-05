// SPDX-License-Identifier: MIT OR Apache-2.0
//! Version history: labelled snapshots of a document, with a block-level diff between them.
//!
//! This is an in-house log. The planned `tpt-version` crate is not wired in, because its
//! dependencies are not available. Snapshots are whole documents, so a log is cheap to read and
//! restore, and not space-efficient for long histories.

use alloc::string::String;
use alloc::vec::Vec;

use crate::block::Block;
use crate::document::Document;
use crate::id::BlockId;

/// One saved version.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    /// Position in the log, from 0.
    pub index: usize,
    /// What the author called this version.
    pub label: String,
    /// The document as it was saved.
    pub document: Document,
}

/// How a block differs between two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Present only in the later version.
    Added,
    /// Present only in the earlier version.
    Removed,
    /// Present in both, with different contents.
    Changed,
}

/// One block that differs between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockChange {
    /// The block.
    pub block: BlockId,
    /// How it changed.
    pub kind: ChangeKind,
}

/// An ordered list of saved versions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionLog {
    versions: Vec<Version>,
}

impl VersionLog {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Saves `document` under `label`. Returns the new version's index.
    pub fn save(&mut self, label: impl Into<String>, document: &Document) -> usize {
        let index = self.versions.len();
        self.versions.push(Version {
            index,
            label: label.into(),
            document: document.clone(),
        });
        index
    }

    /// The number of saved versions.
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// Whether nothing has been saved.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    /// All versions, oldest first.
    pub fn versions(&self) -> &[Version] {
        &self.versions
    }

    /// The version at `index`, if it exists.
    pub fn get(&self, index: usize) -> Option<&Version> {
        self.versions.get(index)
    }

    /// A copy of the document as saved at `index`. The log is not changed.
    pub fn restore(&self, index: usize) -> Option<Document> {
        self.get(index).map(|v| v.document.clone())
    }

    /// The blocks that differ between versions `from` and `to`, in id order. Returns `None` if
    /// either version does not exist.
    pub fn diff(&self, from: usize, to: usize) -> Option<Vec<BlockChange>> {
        let (a, b) = (&self.get(from)?.document, &self.get(to)?.document);
        Some(diff_documents(a, b))
    }
}

/// The blocks that differ between two documents, in id order. Structure (which blocks a
/// container holds) counts as a change to that container.
pub fn diff_documents(a: &Document, b: &Document) -> Vec<BlockChange> {
    let mut ids: Vec<BlockId> = a.blocks.keys().chain(b.blocks.keys()).copied().collect();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .filter_map(|block| {
            let kind = match (a.blocks.get(&block), b.blocks.get(&block)) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Removed,
                (Some(x), Some(y)) if !same_block(x, y) => ChangeKind::Changed,
                _ => return None,
            };
            Some(BlockChange { block, kind })
        })
        .collect()
}

fn same_block(a: &Block, b: &Block) -> bool {
    a == b
}
