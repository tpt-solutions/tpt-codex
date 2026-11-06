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

    /// The word-level change to text block `block` between versions `from` and `to`. `None` if a
    /// version does not exist, or the block is not a text block in one of them.
    pub fn text_diff(&self, from: usize, to: usize, block: BlockId) -> Option<Vec<TextChange>> {
        let text = |doc: &Document| match doc.get(block) {
            Some(Block::Text { content, .. }) => Some(content.clone()),
            _ => None,
        };
        let a = text(&self.get(from)?.document)?;
        let b = text(&self.get(to)?.document)?;
        Some(diff_text(&a, &b))
    }
}

/// One stretch of a word-level diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextChange {
    /// Text present in both versions.
    Same(String),
    /// Text only in the later version.
    Added(String),
    /// Text only in the earlier version.
    Removed(String),
}

/// A word-level diff of `a` to `b`. Words are compared as whole runs of non-space characters, and
/// the spaces between them are compared too, so the pieces join back into each text exactly.
///
/// The longest common subsequence of the tokens is found with a table of their lengths, so the
/// cost grows with the product of the two token counts. That is fine for paragraphs, and slow for
/// very long texts.
pub fn diff_text(a: &str, b: &str) -> Vec<TextChange> {
    let x = tokens(a);
    let y = tokens(b);
    let (n, m) = (x.len(), y.len());
    // lcs[i][j]: length of the common subsequence of x[i..] and y[j..].
    let mut lcs = alloc::vec![alloc::vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if x[i] == y[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut out: Vec<TextChange> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && x[i] == y[j] {
            push(&mut out, TextChange::Same(x[i].into()));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            // Removals come before additions when both are possible, as in a unified diff.
            push(&mut out, TextChange::Removed(x[i].into()));
            i += 1;
        } else {
            push(&mut out, TextChange::Added(y[j].into()));
            j += 1;
        }
    }
    out
}

/// Words and the spaces between them, in order. Joining them gives the original text.
pub(crate) fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = None;
    for (i, c) in text.char_indices() {
        let space = c.is_whitespace();
        match in_space {
            Some(s) if s != space => {
                out.push(&text[start..i]);
                start = i;
                in_space = Some(space);
            }
            None => in_space = Some(space),
            _ => {}
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Adds `change` to `out`, joining it to the last piece when both are the same kind.
fn push(out: &mut Vec<TextChange>, change: TextChange) {
    let join = |last: &mut String, add: &str| last.push_str(add);
    match (out.last_mut(), &change) {
        (Some(TextChange::Same(last)), TextChange::Same(add))
        | (Some(TextChange::Added(last)), TextChange::Added(add))
        | (Some(TextChange::Removed(last)), TextChange::Removed(add)) => join(last, add),
        _ => out.push(change),
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
