// SPDX-License-Identifier: MIT OR Apache-2.0
//! A replicated tree of nodes with move operations, for the block tree.
//!
//! This follows the move-operation design of Kleppmann, Mulligan, Gomes and Beresford (2021). Each
//! node has one parent. A move sets the parent of a node, and moves are ordered by a Lamport
//! [`Stamp`]. When a replica receives a move older than some it already applied, it undoes the newer
//! moves, applies the old one, and redoes the rest. A move that would make a node its own ancestor is
//! skipped on every replica, so the structure stays a tree. Replicas that have seen the same set of
//! operations have the same tree, whatever order they arrived in.
//!
//! Other choices:
//! - Children are ordered by a fractional position key, then by stamp. Inserting at an index picks a key
//!   between the neighbours, so no sequence numbers are needed.
//! - Payloads are last-writer-wins by stamp.
//! - Deleting a node moves it under the trash node. A node in the trash is not reachable from the root,
//!   and its own children move with it. Undoing a delete is a move back out of the trash.
//!
//! [`TreeCrdt::apply`] is idempotent and does not need causal delivery.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use serde::{Deserialize, Serialize};

use crate::text_crdt::Stamp;

/// Identity of a node. Allocated by the replica that creates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// The root of the tree. Always present, and never moved.
pub const ROOT: NodeId = NodeId(0);
/// The trash. Nodes under it are deleted. It is never moved, and it is not reachable from the root.
pub const TRASH: NodeId = NodeId(1);

/// One change to the tree. These are what replicas exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "P: Serialize + for<'a> Deserialize<'a>")]
pub enum TreeOp<P> {
    /// Create a node with a payload. It is not in the tree until a move places it.
    Create {
        /// When the node was created.
        stamp: Stamp,
        /// The new node's id.
        node: NodeId,
        /// Its initial payload.
        payload: P,
    },
    /// Replace a node's payload. Last writer wins.
    Update {
        /// When the update was made.
        stamp: Stamp,
        /// The node to update.
        node: NodeId,
        /// The new payload.
        payload: P,
    },
    /// Put `node` under `parent`, at the position given by `key`.
    Move {
        /// When the move was made. Moves are applied in stamp order.
        stamp: Stamp,
        /// The node to move.
        node: NodeId,
        /// Its new parent: a node, [`ROOT`], or [`TRASH`].
        parent: NodeId,
        /// Its position among its new siblings. Siblings are ordered by key, then by stamp.
        key: Vec<u64>,
    },
}

impl<P> TreeOp<P> {
    /// When the operation was made.
    pub fn stamp(&self) -> Stamp {
        match self {
            TreeOp::Create { stamp, .. }
            | TreeOp::Update { stamp, .. }
            | TreeOp::Move { stamp, .. } => *stamp,
        }
    }
}

/// Why a local edit could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    /// The node is not in the tree (it was never created, or it is in the trash).
    NotInTree(NodeId),
    /// The index is past the end of the children.
    IndexOutOfBounds {
        /// The index given.
        index: usize,
        /// Number of children that could be placed before it.
        len: usize,
    },
    /// The move would make the node its own ancestor.
    WouldCreateCycle(NodeId),
    /// The node is the root, or the trash, which cannot be moved.
    Fixed(NodeId),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInTree(n) => write!(f, "node {} is not in the tree", n.0),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} is outside the children (length {len})")
            }
            Self::WouldCreateCycle(n) => {
                write!(f, "moving node {} would make it its own ancestor", n.0)
            }
            Self::Fixed(n) => write!(f, "node {} cannot be moved", n.0),
        }
    }
}

impl core::error::Error for TreeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Placement {
    parent: NodeId,
    key: Vec<u64>,
    stamp: Stamp,
}

#[derive(Debug, Clone)]
struct LogEntry {
    stamp: Stamp,
    node: NodeId,
    parent: NodeId,
    key: Vec<u64>,
    /// The placement before this move, to restore on undo. `None` if the node had none.
    before: Option<Placement>,
    /// Whether the move changed the tree. A move that would create a cycle does not.
    applied: bool,
}

/// One replica's view of the tree.
#[derive(Debug, Clone)]
pub struct TreeCrdt<P> {
    replica: u64,
    clock: u64,
    next_node: u64,
    payloads: BTreeMap<NodeId, (Stamp, P)>,
    placements: BTreeMap<NodeId, Placement>,
    log: BTreeMap<Stamp, LogEntry>,
    seen: BTreeSet<Stamp>,
}

impl<P: Clone> TreeCrdt<P> {
    /// An empty tree for `replica`. Each replica must use a different id.
    pub fn new(replica: u64) -> Self {
        Self {
            replica,
            clock: 0,
            next_node: 0,
            payloads: BTreeMap::new(),
            placements: BTreeMap::new(),
            log: BTreeMap::new(),
            seen: BTreeSet::new(),
        }
    }

    fn stamp(&mut self) -> Stamp {
        self.clock += 1;
        Stamp {
            lamport: self.clock,
            replica: self.replica,
        }
    }

    fn fresh_node(&mut self) -> NodeId {
        self.next_node += 1;
        // Ids of different replicas never collide, and ROOT and TRASH (0 and 1) are never allocated.
        NodeId(((self.replica + 1) << 32) | self.next_node)
    }

    /// The payload of a node, if it has one.
    pub fn payload(&self, node: NodeId) -> Option<&P> {
        self.payloads.get(&node).map(|(_, p)| p)
    }

    /// Where a node is placed: its parent, if it has a placement.
    pub fn parent_of(&self, node: NodeId) -> Option<NodeId> {
        self.placements.get(&node).map(|p| p.parent)
    }

    /// The children of `parent`, in order.
    pub fn children(&self, parent: NodeId) -> Vec<NodeId> {
        let mut kids: Vec<(&Vec<u64>, Stamp, NodeId)> = self
            .placements
            .iter()
            .filter(|(_, p)| p.parent == parent)
            .map(|(n, p)| (&p.key, p.stamp, *n))
            .collect();
        kids.sort();
        kids.into_iter().map(|(_, _, n)| n).collect()
    }

    /// The nodes reachable from the root, depth first, with their depth. The root itself is not included.
    pub fn flatten(&self) -> Vec<(NodeId, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = self
            .children(ROOT)
            .into_iter()
            .rev()
            .map(|n| (n, 0))
            .collect();
        while let Some((node, depth)) = stack.pop() {
            out.push((node, depth));
            for child in self.children(node).into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        out
    }

    /// `true` when every placed node reaches the root or the trash. This is what the move rules guarantee.
    pub fn is_acyclic(&self) -> bool {
        self.placements.keys().all(|&start| {
            let mut x = start;
            for _ in 0..=self.placements.len() {
                if x == ROOT || x == TRASH {
                    return true;
                }
                match self.placements.get(&x) {
                    Some(p) => x = p.parent,
                    None => return true,
                }
            }
            false
        })
    }

    /// Creates a node under `parent` at `index` among its children, with `payload`.
    ///
    /// Returns the operations to send: the create, then the move.
    pub fn insert_local(
        &mut self,
        parent: NodeId,
        index: usize,
        payload: P,
    ) -> Result<Vec<TreeOp<P>>, TreeError> {
        let node = self.fresh_node();
        let create_stamp = self.stamp();
        let create = TreeOp::Create {
            stamp: create_stamp,
            node,
            payload,
        };
        self.apply(&create);
        let key = self.key_at(parent, index, None)?;
        let move_op = self.move_op(node, parent, key)?;
        Ok(alloc::vec![create, move_op])
    }

    /// Moves `node` under `parent` at `index` among its children (not counting `node` itself).
    pub fn move_local(
        &mut self,
        node: NodeId,
        parent: NodeId,
        index: usize,
    ) -> Result<TreeOp<P>, TreeError> {
        if node == ROOT || node == TRASH {
            return Err(TreeError::Fixed(node));
        }
        if !self.placements.contains_key(&node) {
            return Err(TreeError::NotInTree(node));
        }
        let key = self.key_at(parent, index, Some(node))?;
        self.move_op(node, parent, key)
    }

    /// Deletes `node` (and everything under it) by moving it to the trash.
    pub fn delete_local(&mut self, node: NodeId) -> Result<TreeOp<P>, TreeError> {
        if node == ROOT || node == TRASH {
            return Err(TreeError::Fixed(node));
        }
        if !self.placements.contains_key(&node) {
            return Err(TreeError::NotInTree(node));
        }
        self.move_op(node, TRASH, Vec::new())
    }

    /// Replaces the payload of `node`.
    pub fn update_local(&mut self, node: NodeId, payload: P) -> TreeOp<P> {
        let stamp = self.stamp();
        let op = TreeOp::Update {
            stamp,
            node,
            payload,
        };
        self.apply(&op);
        op
    }

    fn move_op(
        &mut self,
        node: NodeId,
        parent: NodeId,
        key: Vec<u64>,
    ) -> Result<TreeOp<P>, TreeError> {
        if self.would_cycle(node, parent) {
            return Err(TreeError::WouldCreateCycle(node));
        }
        let stamp = self.stamp();
        let op = TreeOp::Move {
            stamp,
            node,
            parent,
            key,
        };
        self.apply(&op);
        Ok(op)
    }

    /// Applies an operation from this or another replica. Applying it again does nothing.
    pub fn apply(&mut self, op: &TreeOp<P>) {
        let stamp = op.stamp();
        self.clock = self.clock.max(stamp.lamport);
        if !self.seen.insert(stamp) {
            return;
        }
        match op {
            TreeOp::Create {
                stamp,
                node,
                payload,
            } => {
                self.next_node_for(*node);
                self.set_payload(*node, *stamp, payload.clone());
            }
            TreeOp::Update {
                stamp,
                node,
                payload,
            } => {
                self.set_payload(*node, *stamp, payload.clone());
            }
            TreeOp::Move {
                stamp,
                node,
                parent,
                key,
            } => self.apply_move(*stamp, *node, *parent, key.clone()),
        }
    }

    /// Keeps locally allocated ids ahead of any id a remote replica has used for this replica.
    fn next_node_for(&mut self, node: NodeId) {
        if node.0 >> 32 == self.replica + 1 {
            self.next_node = self.next_node.max(node.0 & 0xFFFF_FFFF);
        }
    }

    fn set_payload(&mut self, node: NodeId, stamp: Stamp, payload: P) {
        match self.payloads.get(&node) {
            Some((existing, _)) if *existing >= stamp => {}
            _ => {
                self.payloads.insert(node, (stamp, payload));
            }
        }
    }

    /// The Kleppmann move: undo the newer moves, apply this one, then redo them.
    fn apply_move(&mut self, stamp: Stamp, node: NodeId, parent: NodeId, key: Vec<u64>) {
        // Undo every logged move newer than this one, newest first. The state is then as it was
        // just before this move would have applied.
        let newer: Vec<LogEntry> = self
            .log
            .range((
                core::ops::Bound::Excluded(stamp),
                core::ops::Bound::Unbounded,
            ))
            .map(|(_, e)| e.clone())
            .collect();
        for entry in newer.iter().rev() {
            self.undo(entry);
        }

        let before = self.placements.get(&node).cloned();
        let applied = self.place(node, parent, &key, stamp);
        self.log.insert(
            stamp,
            LogEntry {
                stamp,
                node,
                parent,
                key,
                before,
                applied,
            },
        );

        // Redo the newer moves in stamp order, against the new state.
        for entry in newer {
            self.redo(entry);
        }
    }

    /// Places `node` under `parent` unless that would be a cycle or touches a fixed node.
    /// Returns whether the tree changed.
    fn place(&mut self, node: NodeId, parent: NodeId, key: &[u64], stamp: Stamp) -> bool {
        if node == ROOT || node == TRASH || self.would_cycle(node, parent) {
            return false;
        }
        self.placements.insert(
            node,
            Placement {
                parent,
                key: key.to_vec(),
                stamp,
            },
        );
        true
    }

    fn undo(&mut self, entry: &LogEntry) {
        if !entry.applied {
            return;
        }
        match &entry.before {
            Some(p) => {
                self.placements.insert(entry.node, p.clone());
            }
            None => {
                self.placements.remove(&entry.node);
            }
        }
    }

    fn redo(&mut self, mut entry: LogEntry) {
        entry.before = self.placements.get(&entry.node).cloned();
        entry.applied = self.place(entry.node, entry.parent, &entry.key, entry.stamp);
        self.log.insert(entry.stamp, entry);
    }

    /// `true` if putting `node` under `parent` would make `node` its own ancestor.
    fn would_cycle(&self, node: NodeId, parent: NodeId) -> bool {
        let mut x = parent;
        for _ in 0..=self.placements.len() {
            if x == node {
                return true;
            }
            if x == ROOT || x == TRASH {
                return false;
            }
            match self.placements.get(&x) {
                Some(p) => x = p.parent,
                None => return false,
            }
        }
        // A walk longer than the tree means a cycle already exists; refuse the move.
        true
    }

    /// A position key for `index` among the children of `parent`, leaving out `exclude`.
    fn key_at(
        &self,
        parent: NodeId,
        index: usize,
        exclude: Option<NodeId>,
    ) -> Result<Vec<u64>, TreeError> {
        let siblings: Vec<NodeId> = self
            .children(parent)
            .into_iter()
            .filter(|n| Some(*n) != exclude)
            .collect();
        if index > siblings.len() {
            return Err(TreeError::IndexOutOfBounds {
                index,
                len: siblings.len(),
            });
        }
        let before = if index == 0 {
            Vec::new()
        } else {
            self.placements[&siblings[index - 1]].key.clone()
        };
        let after = siblings.get(index).map(|n| self.placements[n].key.clone());
        Ok(key_between(&before, after.as_deref()))
    }
}

/// A key strictly between `lo` and `hi` (`None` means no upper bound). `lo` may be empty, meaning the start.
///
/// Keys are compared lexicographically, so the result sorts after `lo` and before `hi`.
///
/// Precondition: `lo < hi`, and `hi` does not consist only of zeros. Nothing sorts below `[0]`, so
/// no key lies between the empty key and `[0]`. Keys this function returns end in a non-zero digit,
/// so keys made here always satisfy the precondition.
pub fn key_between(lo: &[u64], hi: Option<&[u64]>) -> Vec<u64> {
    let mut out = Vec::new();
    let mut bounded = hi.is_some();
    let mut i = 0;
    loop {
        let low = lo.get(i).copied().unwrap_or(0);
        let high = match hi {
            Some(h) if bounded => h.get(i).copied().unwrap_or(u64::MAX),
            _ => u64::MAX,
        };
        if high > low.saturating_add(1) {
            out.push(low + (high - low) / 2);
            return out;
        }
        out.push(low);
        if high > low {
            // The prefix is now below the upper bound, so it no longer constrains later digits.
            bounded = false;
        }
        i += 1;
    }
}

/// Projects a tree of blocks to a [`Document`](crate::Document).
///
/// Each node becomes the block with the same id, and the children of containers and lists come
/// from the tree's order. A node's own `children` field in its payload is replaced. Nodes in the
/// trash are not included. The result is a snapshot: project again after the tree changes.
pub fn project_document(tree: &TreeCrdt<crate::Block>) -> crate::Document {
    use crate::{Block, BlockId, Document, LayoutMode};

    let root = Block::Container {
        layout_mode: LayoutMode::Flow,
        children: tree
            .children(ROOT)
            .into_iter()
            .map(|n| BlockId(n.0))
            .collect(),
    };
    let mut doc = Document::new(BlockId(ROOT.0), root);
    for (node, _) in tree.flatten() {
        let Some(payload) = tree.payload(node) else {
            // Placed but never created: nothing to show.
            continue;
        };
        let child_ids: Vec<BlockId> = tree
            .children(node)
            .into_iter()
            .map(|n| BlockId(n.0))
            .collect();
        let block = match payload.clone() {
            Block::Container { layout_mode, .. } => Block::Container {
                layout_mode,
                children: child_ids,
            },
            Block::List { ordered, start, .. } => Block::List {
                ordered,
                start,
                items: child_ids,
            },
            other => other,
        };
        doc.insert(BlockId(node.0), block);
    }
    doc
}
