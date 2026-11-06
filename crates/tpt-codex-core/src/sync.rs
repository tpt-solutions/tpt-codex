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
//! protocol. A [`Session`] puts a replica on a transport: it acknowledges operations, resends the
//! outbox on [`Session::reconnect`], and keeps only the newest cursor update while offline.
//! Presence (cursors) is not replicated as document state: it is sent with
//! [`Message::Presence`], and only the newest update from each replica counts.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::block::Block;
use crate::document::Document;
use crate::id::BlockId;
use crate::id::SheetId;
use crate::primitives::TextStyle;
use crate::sheet::{CellInput, Sheet, is_formula_source, is_valid_address};
use crate::text_crdt::{Stamp, TextCrdt, TextCrdtError, TextOp};
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
    /// A cell of a live grid. The newest stamp wins, so a cell written at the same time on two
    /// replicas ends up the same on both.
    Cell {
        /// The sheet holding the cell.
        sheet: SheetId,
        /// Canonical A1 address.
        address: String,
        /// When the cell was written.
        stamp: Stamp,
        /// The new value, or `None` to clear the cell.
        input: Option<CellInput>,
    },
    /// The minimum size and header rows of a live grid. The newest stamp wins.
    SheetSize {
        /// The sheet.
        sheet: SheetId,
        /// When the size was set.
        stamp: Stamp,
        /// Minimum rows.
        rows: u32,
        /// Minimum columns.
        cols: u32,
        /// Leading rows that are headers.
        header_rows: u32,
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

/// Tells replica `to` that replica `from` has applied its operations up to and including `seq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    /// The replica whose operations were applied.
    pub to: ReplicaId,
    /// The replica that applied them.
    pub from: ReplicaId,
    /// The highest sequence number applied in order.
    pub seq: u64,
}

/// What travels between replicas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    /// A sequenced change. It is retried until acknowledged.
    Op(Envelope),
    /// A presence update. It is not retried.
    Presence(PresenceUpdate),
    /// An acknowledgement of applied operations. It is not retried: the receiver of an operation
    /// acknowledges it again each time it arrives.
    Ack(Ack),
}

/// The boundary to the network. An implementation decides how bytes move.
pub trait Transport {
    /// Why sending failed.
    type Error;

    /// Sends one message to the peer.
    fn send(&mut self, message: &Message) -> Result<(), Self::Error>;

    /// The next message from the peer, if one has arrived.
    fn receive(&mut self) -> Option<Message>;

    /// Whether the connection is still up. A transport that cannot tell says `true`.
    fn is_connected(&self) -> bool {
        true
    }
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
    /// The cell address is not canonical A1 notation.
    InvalidAddress(String),
    /// A formula does not start with `=`.
    InvalidFormula(String),
}

impl core::fmt::Display for SyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tree(e) => write!(f, "{e}"),
            Self::Text(e) => write!(f, "{e}"),
            Self::NotText(n) => write!(f, "node {} is not a text block", n.0),
            Self::InvalidAddress(a) => write!(f, "'{a}' is not a valid cell address"),
            Self::InvalidFormula(a) => write!(f, "formula in cell '{a}' must start with '='"),
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
    /// Local operations that some peer has not acknowledged, in sequence order.
    unacked: Vec<Envelope>,
    /// Highest sequence number applied from each sender, in order.
    applied: BTreeMap<ReplicaId, u64>,
    /// Every other replica heard from so far. An operation is dropped from the outbox only when
    /// all of them have acknowledged it.
    peers: BTreeSet<ReplicaId>,
    /// Highest sequence number each peer has acknowledged from this replica.
    peer_acks: BTreeMap<ReplicaId, u64>,
    /// Received operations that are not applied yet.
    held: Vec<Envelope>,
    /// Newest presence update from each replica.
    presence: BTreeMap<ReplicaId, PresenceUpdate>,
    presence_counter: u64,
    /// Lamport clock for stamps made here, advanced by every stamp seen.
    clock: u64,
    /// Live grids, with the stamp of each cell and of the size.
    sheets: BTreeMap<SheetId, ReplicaSheet>,
}

#[derive(Debug, Clone, Default)]
struct ReplicaSheet {
    data: Sheet,
    cell_stamps: BTreeMap<String, Stamp>,
    size_stamp: Option<Stamp>,
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
            peers: BTreeSet::new(),
            peer_acks: BTreeMap::new(),
            held: Vec::new(),
            presence: BTreeMap::new(),
            presence_counter: 0,
            clock: 0,
            sheets: BTreeMap::new(),
        }
    }

    /// This replica's id.
    pub fn id(&self) -> ReplicaId {
        self.id
    }

    /// The document as this replica sees it, with each text block's characters from its CRDT.
    pub fn document(&self) -> Document {
        let mut doc = project_document(&self.tree);
        for (id, sheet) in &self.sheets {
            doc.sheets.insert(*id, sheet.data.clone());
        }
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

    /// Sets cell `address` of live grid `sheet` to `input`, or clears it with `None`.
    pub fn set_cell(
        &mut self,
        sheet: SheetId,
        address: &str,
        input: Option<CellInput>,
    ) -> Result<Envelope, SyncError> {
        if !is_valid_address(address) {
            return Err(SyncError::InvalidAddress(address.into()));
        }
        if let Some(CellInput::Formula { source }) = &input {
            if !is_formula_source(source) {
                return Err(SyncError::InvalidFormula(address.into()));
            }
        }
        let stamp = self.next_stamp();
        let op = SyncOp::Cell {
            sheet,
            address: address.into(),
            stamp,
            input,
        };
        self.try_apply(&op);
        Ok(self.emit(op))
    }

    /// Sets the minimum size of live grid `sheet`.
    pub fn set_sheet_size(
        &mut self,
        sheet: SheetId,
        rows: u32,
        cols: u32,
        header_rows: u32,
    ) -> Envelope {
        let stamp = self.next_stamp();
        let op = SyncOp::SheetSize {
            sheet,
            stamp,
            rows,
            cols,
            header_rows,
        };
        self.try_apply(&op);
        self.emit(op)
    }

    fn next_stamp(&mut self) -> Stamp {
        self.clock += 1;
        Stamp {
            lamport: self.clock,
            replica: self.id,
        }
    }

    /// Operations this replica made that the peer has not acknowledged, in order. After a
    /// reconnect, send these again. The peer ignores any it already has.
    pub fn pending(&self) -> &[Envelope] {
        &self.unacked
    }

    /// Records that every peer has applied every operation up to and including `seq` from this
    /// replica. The outbox drops them.
    pub fn acknowledge(&mut self, seq: u64) {
        self.unacked.retain(|e| e.seq > seq);
    }

    /// The highest sequence number applied in order from `sender`, or 0 if none.
    pub fn applied_from(&self, sender: ReplicaId) -> u64 {
        self.applied.get(&sender).copied().unwrap_or(0)
    }

    /// The acknowledgement to send `sender` for what this replica has applied from it, or
    /// `None` if nothing has been applied yet.
    pub fn ack(&self, sender: ReplicaId) -> Option<Ack> {
        match self.applied_from(sender) {
            0 => None,
            seq => Some(Ack {
                to: sender,
                from: self.id,
                seq,
            }),
        }
    }

    /// Takes in one message from the peer and applies whatever is ready.
    pub fn receive(&mut self, message: Message) {
        match message {
            Message::Op(envelope) => {
                self.note_peer(envelope.from);
                self.receive_op(envelope);
            }
            Message::Presence(update) => {
                self.note_peer(update.from);
                self.receive_presence(update);
            }
            Message::Ack(ack) => {
                self.note_peer(ack.from);
                if ack.to == self.id {
                    let known = self.peer_acks.entry(ack.from).or_insert(0);
                    *known = (*known).max(ack.seq);
                    self.drop_acknowledged();
                }
            }
        }
    }

    fn note_peer(&mut self, from: ReplicaId) {
        if from != self.id {
            self.peers.insert(from);
        }
    }

    /// Drops the outbox entries that every known peer has acknowledged. With no known peer,
    /// nothing is dropped.
    fn drop_acknowledged(&mut self) {
        let floor = self
            .peers
            .iter()
            .map(|p| self.peer_acks.get(p).copied().unwrap_or(0))
            .min();
        if let Some(floor) = floor {
            self.unacked.retain(|e| e.seq > floor);
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
            SyncOp::Cell {
                sheet,
                address,
                stamp,
                input,
            } => {
                self.clock = self.clock.max(stamp.lamport);
                let entry = self.sheets.entry(*sheet).or_default();
                if entry
                    .cell_stamps
                    .get(address)
                    .is_some_and(|seen| seen >= stamp)
                {
                    return true;
                }
                entry.cell_stamps.insert(address.clone(), *stamp);
                match input {
                    Some(value) => {
                        entry.data.cells.insert(address.clone(), value.clone());
                    }
                    None => {
                        entry.data.cells.remove(address);
                    }
                }
                true
            }
            SyncOp::SheetSize {
                sheet,
                stamp,
                rows,
                cols,
                header_rows,
            } => {
                self.clock = self.clock.max(stamp.lamport);
                let entry = self.sheets.entry(*sheet).or_default();
                if entry.size_stamp.is_some_and(|seen| seen >= *stamp) {
                    return true;
                }
                entry.size_stamp = Some(*stamp);
                entry.data.rows = *rows;
                entry.data.cols = *cols;
                entry.data.header_rows = *header_rows;
                true
            }
            SyncOp::Tree(tree_op) => {
                self.clock = self.clock.max(tree_op.stamp().lamport);
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

/// A replica connected through a [`Transport`].
///
/// Edits made while the connection is down stay in the replica's outbox. Presence updates made
/// while it is down are kept only as the newest one. [`Session::reconnect`] takes a new
/// transport and sends the outbox and that presence update again. Each received operation is
/// acknowledged, including a duplicate, so the sender can empty its outbox even when an earlier
/// acknowledgement was lost.
#[derive(Debug)]
pub struct Session<T: Transport> {
    replica: Replica,
    transport: T,
    online: bool,
    /// The newest presence update made while offline. Only it is sent on reconnect.
    queued_presence: Option<PresenceUpdate>,
}

impl<T: Transport> Session<T> {
    /// Starts a session on `transport`, which is taken to be connected.
    pub fn new(replica: Replica, transport: T) -> Self {
        Self {
            replica,
            transport,
            online: true,
            queued_presence: None,
        }
    }

    /// The replica, as this session sees it.
    pub fn replica(&self) -> &Replica {
        &self.replica
    }

    /// The replica, for making local changes. Send what they return with [`Session::send`].
    pub fn replica_mut(&mut self) -> &mut Replica {
        &mut self.replica
    }

    /// Whether messages are going out now.
    pub fn is_online(&self) -> bool {
        self.online && self.transport.is_connected()
    }

    /// Marks the connection as down, for example after the host sees it close. Nothing is sent
    /// until [`Session::reconnect`].
    pub fn disconnect(&mut self) {
        self.online = false;
    }

    /// Sends `envelopes` now if the connection is up. Otherwise they stay in the outbox, and
    /// [`Session::reconnect`] sends them.
    pub fn send(&mut self, envelopes: &[Envelope]) {
        if !self.is_online() {
            return;
        }
        for envelope in envelopes {
            if self.transport.send(&Message::Op(envelope.clone())).is_err() {
                self.online = false;
                return;
            }
        }
    }

    /// Makes a cursor update. It is sent now if the connection is up, and kept otherwise.
    pub fn set_cursor(&mut self, cursor: Option<Cursor>) {
        let update = self.replica.set_cursor(cursor);
        if self.is_online() {
            if self
                .transport
                .send(&Message::Presence(update.clone()))
                .is_ok()
            {
                self.queued_presence = None;
                return;
            }
            self.online = false;
        }
        self.queued_presence = Some(update);
    }

    /// Receives every message that has arrived, applies it, and acknowledges operations. Returns
    /// how many messages were received. A failed send marks the connection as down.
    pub fn pump(&mut self) -> usize {
        let mut received = 0;
        while self.is_online() {
            let Some(message) = self.transport.receive() else {
                break;
            };
            received += 1;
            let sender = match &message {
                Message::Op(envelope) => Some(envelope.from),
                _ => None,
            };
            self.replica.receive(message);
            if let Some(ack) = sender.and_then(|s| self.replica.ack(s)) {
                if self.transport.send(&Message::Ack(ack)).is_err() {
                    self.online = false;
                }
            }
        }
        received
    }

    /// Replaces the transport with a new connection, and sends the outbox and any queued
    /// presence update over it. Messages that fail to send leave the session offline again.
    pub fn reconnect(&mut self, transport: T) {
        self.transport = transport;
        self.online = true;
        let pending = self.replica.pending().to_vec();
        self.send(&pending);
        if self.is_online() {
            if let Some(update) = self.queued_presence.take() {
                if self
                    .transport
                    .send(&Message::Presence(update.clone()))
                    .is_err()
                {
                    self.online = false;
                    self.queued_presence = Some(update);
                }
            }
        }
    }
}
