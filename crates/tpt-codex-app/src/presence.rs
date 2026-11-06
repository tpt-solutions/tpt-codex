// SPDX-License-Identifier: MIT OR Apache-2.0
//! Other people's carets, from the presence messages a relay connection delivers.
//!
//! The page owns the connection. It hands each received message in, and sends back whatever
//! [`Presence::outbound_for`] returns. This module keeps the newest cursor from each replica and
//! names it for the editor. Operations and acknowledgements are not presence, so they are
//! ignored here.

use std::collections::BTreeSet;

use tpt_codex_core::{BlockId, Cursor, Message, NodeId, Replica, ReplicaId, Selection};

use crate::editor::RemoteCursor;

/// The presence of one editing session: its own caret, and the newest caret of each peer.
#[derive(Debug)]
pub struct Presence {
    replica: Replica,
    peers: BTreeSet<ReplicaId>,
    /// The caret last announced, or `None` before the first announcement.
    announced: Option<Option<Cursor>>,
}

impl Presence {
    /// Presence for the replica `id`. Every session on a document needs its own id.
    pub fn new(id: ReplicaId) -> Self {
        Presence {
            replica: Replica::new(id),
            peers: BTreeSet::new(),
            announced: None,
        }
    }

    /// The replica this presence belongs to.
    pub fn id(&self) -> ReplicaId {
        self.replica.id()
    }

    /// Takes in one message from the relay. Only presence changes anything.
    pub fn receive(&mut self, message: Message) {
        if let Message::Presence(update) = message {
            if update.from != self.id() {
                self.peers.insert(update.from);
            }
            self.replica.receive(Message::Presence(update));
        }
    }

    /// The presence update to send for this editor's `caret`, if the caret has moved since the
    /// last one sent. The first call always returns an update, so a peer that joins late learns
    /// where this editor is.
    pub fn outbound_for(&mut self, caret: Option<Selection>) -> Option<Message> {
        let cursor = caret.map(|s| Cursor {
            node: NodeId(s.block.0),
            offset: s.focus,
        });
        if self.announced == Some(cursor) {
            return None;
        }
        self.announced = Some(cursor);
        Some(Message::Presence(self.replica.set_cursor(cursor)))
    }

    /// Forgets the last announcement, so the next [`outbound_for`](Self::outbound_for) sends the
    /// caret again. The relay does not keep presence, so a page that has just joined a document
    /// calls this to let the peers already there learn where it is.
    pub fn announce_again(&mut self) {
        self.announced = None;
    }

    /// The other replicas' carets, in replica order, for the editor to draw.
    pub fn remote_cursors(&self) -> Vec<RemoteCursor> {
        self.peers
            .iter()
            .filter_map(|&peer| {
                let cursor = self.replica.cursor_of(peer)?;
                Some(RemoteCursor {
                    label: format!("Replica {peer}"),
                    block: BlockId(cursor.node.0),
                    offset: cursor.offset,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_codex_core::Selection;

    fn caret(block: u64, offset: usize) -> Option<Selection> {
        Some(Selection::caret(BlockId(block), offset))
    }

    #[test]
    fn the_first_announcement_is_sent_and_an_unchanged_caret_is_not() {
        let mut p = Presence::new(1);
        assert!(p.outbound_for(caret(2, 3)).is_some());
        assert!(p.outbound_for(caret(2, 3)).is_none());
        assert!(p.outbound_for(caret(2, 4)).is_some());
        assert!(
            p.outbound_for(None).is_some(),
            "losing the caret is announced"
        );
        assert!(p.outbound_for(None).is_none());
    }

    #[test]
    fn a_peers_update_becomes_a_remote_caret_in_its_block() {
        let mut peer = Presence::new(2);
        let Some(message) = peer.outbound_for(caret(7, 5)) else {
            panic!("the first announcement is sent");
        };
        let mut me = Presence::new(1);
        me.receive(message);
        assert_eq!(
            me.remote_cursors(),
            vec![RemoteCursor {
                label: "Replica 2".into(),
                block: BlockId(7),
                offset: 5,
            }]
        );
    }

    #[test]
    fn only_the_newest_update_from_a_peer_counts() {
        let mut peer = Presence::new(2);
        let first = peer.outbound_for(caret(7, 1)).unwrap();
        let second = peer.outbound_for(caret(7, 9)).unwrap();
        let mut me = Presence::new(1);
        me.receive(second);
        me.receive(first);
        assert_eq!(me.remote_cursors()[0].offset, 9);
    }

    #[test]
    fn my_own_echo_and_non_presence_messages_are_ignored() {
        let mut me = Presence::new(1);
        let own = me.outbound_for(caret(3, 0)).unwrap();
        me.receive(own);
        assert!(me.remote_cursors().is_empty());
        me.receive(Message::Ack(tpt_codex_core::Ack {
            to: 1,
            from: 2,
            seq: 4,
        }));
        assert!(me.remote_cursors().is_empty());
    }

    #[test]
    fn a_peer_with_no_caret_is_not_drawn() {
        let mut peer = Presence::new(2);
        let gone = peer.outbound_for(None).unwrap();
        let mut me = Presence::new(1);
        me.receive(gone);
        assert!(me.remote_cursors().is_empty());
    }
}
