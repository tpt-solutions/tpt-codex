// SPDX-License-Identifier: MIT OR Apache-2.0
//! Replication between replicas, independent of any server or network.
//!
//! A [`Replica`] holds a copy of the document as a tree CRDT, with one text CRDT per text block.
//! Every local change is turned into a [`SyncOp`] with a sequence number and kept in an outbox
//! until the peer acknowledges it. So a replica edits offline, and on reconnect it sends
//! [`Replica::pending`] again. Receivers apply each sender's operations in sequence order,
//! ignore duplicates, and hold an operation until its dependencies have arrived.
//!
//! [`Transport`] is the boundary to the network. This crate does not choose a server or a
//! protocol. Presence (cursors) is not replicated as document state: it is sent with
//! [`Message::Presence`], is not queued, and only the newest update from each replica counts.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::document::Document;
use crate::id::BlockId;
use crate::primitives::TextStyle;
use crate::text_crdt::{TextCrdt, TextCrdtError, TextOp};
use crate::tree_crdt::{NodeId, TreeCrdt, TreeError, TreeOp, project_document};

/// Identity of a replica. Each editing session has its own.
pub type ReplicaId = u64;

/// One change to the replicated document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SyncOp {
    /// A change to the block tree.
    Tree(TreeOp<Block>),
    /// A change to the characters of one text block.
    Text {
        /// The text block, by its tree node.
        node: NodeId,
        /// The character change.
        op: TextOp,
    },
}

/// A sequenced operation. `seq` counts from 1 for each sender, in the order it was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    /// The replica that made the operation.
    pub from: ReplicaId,
    /// Position in that replica's sequence.
    pub seq: u64,
    /// The change.
    pub op: SyncOp,
}

/// Where a replica's caret is: a character offset in a text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// The text block.
    pub node: NodeId,
    /// Character offset in its text.
    pub offset: usize,
}

/// A presence update. Only the highest `counter` from each replica is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceUpdate {
    /// The replica the cursor belongs to.
    pub from: ReplicaId,
    /// Increases with each update from `from`.
    pub counter: u64,
    /// Where the replica's caret is, or `None` if it has none.
    pub cursor: Option<Cursor>,
}

/// What travels between replicas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    /// A sequenced change. It is retried until acknowledged.
    Op(Envelope),
    /// A presence update. It is not retried.
    Presence(PresenceUpdate),
}

/// The boundary to the network. An implementation decides how bytes move.
pub trait Transport {
    /// Why sending failed.
    type Error;

    /// Sends one message to the peer.
    fn send(&mut self, message: &Message) -> Result<(), Self::Error>;

    /// The next message from the peer, if one has arrived.
    fn receive(&mut self) -> Option<Message>;
}

/// Why a local edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The tree refused the change.
    Tree(TreeError),
    /// The text refused the change.
    Text(TextCrdtError),
    /// The node is not a text block that this replica knows.
    NotText(NodeId),
}

impl core::fmt::Display for SyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tree(e) => write!(f, "{e}"),
            Self::Text(e) => write!(f, "{e}"),
            Self::NotText(n) => write!(f, "node {} is not a text block", n.0),
        }
    }
}

impl core::error::Error for SyncError {}

/// One replica of a document.
#[derive(Debug, Clone)]
pub struct Replica {
    id: ReplicaId,
    tree: TreeCrdt<Block>,
    texts: BTreeMap<NodeId, TextCrdt>,
    next_seq: u64,
    /// Local operations the peer has not acknowledged, in sequence order.
    unacked: Vec<Envelope>,
    /// Highest sequence number applied from each sender, in order.
    applied: BTreeMap<ReplicaId, u64>,
    /// Received operations that are not applied yet.
    held: Vec<Envelope>,
    /// Newest presence update from each replica.
    presence: BTreeMap<ReplicaId, PresenceUpdate>,
    presence_counter: u64,
}

impl Replica {
    /// An empty replica with the given id. Its document has only the root.
    pub fn new(id: ReplicaId) -> Self {
        Self {
            id,
            tree: TreeCrdt::new(id),
            texts: BTreeMap::new(),
            next_seq: 1,
            unacked: Vec::new(),
            applied: BTreeMap::new(),
            held: Vec::new(),
            presence: BTreeMap::new(),
            presence_counter: 0,
        }
    }

    /// This replica's id.
    pub fn id(&self) -> ReplicaId {
        self.id
    }

    /// The document as this replica sees it, with each text block's characters from its CRDT.
    pub fn document(&self) -> Document {
        let mut doc = project_document(&self.tree);
        for (node, text) in &self.texts {
            if let Some(Block::Text { content, .. }) = doc.blocks.get_mut(&BlockId(node.0)) {
                *content = text.text();
            }
        }
        doc
    }

    /// Creates a block under `parent` at `index`. A text block starts empty; fill it with
    /// [`Replica::insert_text`]. Returns the new block's node.
    pub fn create_block(
        &mut self,
        parent: NodeId,
        index: usize,
        block: Block,
    ) -> Result<(NodeId, Vec<Envelope>), SyncError> {
        let payload = match block {
            Block::Text { style, .. } => Block::Text {
                content: String::new(),
                style,
            },
            other => other,
        };
        let is_text = matches!(payload, Block::Text { .. });
        let ops = self
            .tree
            .insert_local(parent, index, payload)
            .map_err(SyncError::Tree)?;
        let node = match ops.first() {
            Some(TreeOp::Create { node, .. }) => *node,
            _ => unreachable!("insert_local always creates a node first"),
        };
        if is_text {
            self.texts.insert(node, TextCrdt::new(self.id));
        }
        let envelopes = ops
            .into_iter()
            .map(|op| self.emit(SyncOp::Tree(op)))
            .collect();
        Ok((node, envelopes))
    }

    /// Creates a text block with `style`, under `parent` at `index`.
    pub fn create_text(
        &mut self,
        parent: NodeId,
        index: usize,
        style: TextStyle,
    ) -> Result<(NodeId, Vec<Envelope>), SyncError> {
        self.create_block(
            parent,
            index,
            Block::Text {
                content: String::new(),
                style,
            },
        )
    }

    /// Inserts `text` into the text block `node` at character `index`.
    pub fn insert_text(
        &mut self,
        node: NodeId,
        index: usize,
        text: &str,
    ) -> Result<Vec<Envelope>, SyncError> {
        let mut out = Vec::new();
        for (at, ch) in (index..).zip(text.chars()) {
            let crdt = self.texts.get_mut(&node).ok_or(SyncError::NotText(node))?;
            let op = crdt.insert_local(at, ch).map_err(SyncError::Text)?;
            out.push(self.emit(SyncOp::Text { node, op }));
        }
        Ok(out)
    }

    /// Deletes the character at `index` in the text block `node`.
    pub fn delete_text(&mut self, node: NodeId, index: usize) -> Result<Envelope, SyncError> {
        let crdt = self.texts.get_mut(&node).ok_or(SyncError::NotText(node))?;
        let op = crdt.delete_local(index).map_err(SyncError::Text)?;
        Ok(self.emit(SyncOp::Text { node, op }))
    }

    /// Operations this replica made that the peer has not acknowledged, in order. After a
    /// reconnect, send these again. The peer ignores any it already has.
    pub fn pending(&self) -> &[Envelope] {
        &self.unacked
    }

    /// Records that the peer has applied every operation up to and including `seq` from this
    /// replica. The outbox drops them.
    pub fn acknowledge(&mut self, seq: u64) {
        self.unacked.retain(|e| e.seq > seq);
    }

    /// Takes in one message from the peer and applies whatever is ready.
    pub fn receive(&mut self, message: Message) {
        match message {
            Message::Op(envelope) => self.receive_op(envelope),
            Message::Presence(update) => self.receive_presence(update),
        }
    }

    /// Holds `envelope` and applies whatever is ready.
    fn receive_op(&mut self, envelope: Envelope) {
        let applied = self.applied.get(&envelope.from).copied().unwrap_or(0);
        // Old duplicates, and an operation already waiting, are dropped.
        if envelope.seq <= applied
            || self
                .held
                .iter()
                .any(|h| h.from == envelope.from && h.seq == envelope.seq)
        {
            return;
        }
        self.held.push(envelope);
        self.drain();
    }

    /// Applies held operations until none can be applied. An operation is ready when it is the
    /// next in its sender's sequence and its dependencies are here.
    fn drain(&mut self) {
        loop {
            let mut progressed = false;
            let mut i = 0;
            while i < self.held.len() {
                let (from, seq) = (self.held[i].from, self.held[i].seq);
                let applied = self.applied.get(&from).copied().unwrap_or(0);
                if seq == applied + 1 && self.try_apply(&self.held[i].op.clone()) {
                    self.held.remove(i);
                    self.applied.insert(from, seq);
                    progressed = true;
                } else {
                    i += 1;
                }
            }
            if !progressed {
                return;
            }
        }
    }

    /// Applies `op` if its dependencies are here. A failed apply changes nothing.
    fn try_apply(&mut self, op: &SyncOp) -> bool {
        match op {
            SyncOp::Tree(tree_op) => {
                self.tree.apply(tree_op);
                if let TreeOp::Create {
                    node,
                    payload: Block::Text { .. },
                    ..
                } = tree_op
                {
                    self.texts
                        .entry(*node)
                        .or_insert_with(|| TextCrdt::new(self.id));
                }
                true
            }
            SyncOp::Text { node, op } => match self.texts.get_mut(node) {
                Some(text) => text.apply(op).is_ok(),
                None => false,
            },
        }
    }

    fn emit(&mut self, op: SyncOp) -> Envelope {
        let envelope = Envelope {
            from: self.id,
            seq: self.next_seq,
            op,
        };
        self.next_seq += 1;
        self.unacked.push(envelope.clone());
        envelope
    }

    /// Sets this replica's caret. The update is not queued; send it now.
    pub fn set_cursor(&mut self, cursor: Option<Cursor>) -> PresenceUpdate {
        self.presence_counter += 1;
        let update = PresenceUpdate {
            from: self.id,
            counter: self.presence_counter,
            cursor,
        };
        self.presence.insert(self.id, update.clone());
        update
    }

    fn receive_presence(&mut self, update: PresenceUpdate) {
        if update.from == self.id {
            return;
        }
        let newer = self
            .presence
            .get(&update.from)
            .is_none_or(|known| update.counter > known.counter);
        if newer {
            self.presence.insert(update.from, update);
        }
    }

    /// The newest known cursor of `replica`, if it has one.
    pub fn cursor_of(&self, replica: ReplicaId) -> Option<Cursor> {
        self.presence.get(&replica).and_then(|u| u.cursor)
    }

    /// The other replicas whose cursors are in text block `node`, with their offsets.
    pub fn cursors_in(&self, node: NodeId) -> Vec<(ReplicaId, usize)> {
        self.presence
            .values()
            .filter(|u| u.from != self.id)
            .filter_map(|u| match u.cursor {
                Some(c) if c.node == node => Some((u.from, c.offset)),
                _ => None,
            })
            .collect()
    }
}
