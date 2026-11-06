// SPDX-License-Identifier: MIT OR Apache-2.0
//! The server side of replication: an ordered operation log per document.
//!
//! A relay keeps every operation it receives, in arrival order. A replica that was offline sends
//! its outbox again, so the relay drops an operation it already has (same sender and sequence
//! number). A replica that joins late asks for everything after its cursor. Presence is not
//! logged, so it is never replayed.
//!
//! This is the logic only. A network server wraps it, and the choice of server is still open.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::sync::{Message, ReplicaId};

/// One document's log.
#[derive(Debug, Clone, Default)]
struct Log {
    messages: Vec<Message>,
    seen: BTreeSet<(ReplicaId, u64)>,
}

/// Operation logs for many documents.
#[derive(Debug, Clone, Default)]
pub struct Relay {
    docs: BTreeMap<String, Log>,
}

/// What [`Relay::publish`] did with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publish {
    /// The message was new and is now in the log at this position.
    Stored(usize),
    /// The message was already in the log, so it was dropped.
    Duplicate,
    /// Presence is not logged. Forward it to the document's other replicas instead.
    NotLogged,
}

impl Relay {
    /// An empty relay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts `message` for document `doc`.
    pub fn publish(&mut self, doc: &str, message: Message) -> Publish {
        let log = self.docs.entry(doc.into()).or_default();
        match &message {
            Message::Presence(_) | Message::Ack(_) => Publish::NotLogged,
            Message::Op(envelope) => {
                if !log.seen.insert((envelope.from, envelope.seq)) {
                    return Publish::Duplicate;
                }
                log.messages.push(message);
                Publish::Stored(log.messages.len() - 1)
            }
        }
    }

    /// The operations logged for `doc` at positions `cursor..`, in order. A replica that has
    /// seen `cursor` operations asks with that number.
    pub fn since(&self, doc: &str, cursor: usize) -> &[Message] {
        match self.docs.get(doc) {
            Some(log) => log.messages.get(cursor..).unwrap_or(&[]),
            None => &[],
        }
    }

    /// How many operations are logged for `doc`.
    pub fn len(&self, doc: &str) -> usize {
        self.docs.get(doc).map_or(0, |log| log.messages.len())
    }
}
