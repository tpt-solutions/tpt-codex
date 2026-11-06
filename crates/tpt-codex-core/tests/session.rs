// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sessions over an in-memory link: outbox resend on reconnect, acknowledgements, and newest-only
//! presence while offline.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use tpt_codex_core::{
    Block, BlockId, Cursor, Message, NodeId, ROOT, Replica, Session, TextStyle, Transport,
};

/// One connection between two sessions. A new connection is a new `Link`, so messages in flight
/// on a dropped one are lost, as they are on a real socket.
#[derive(Default)]
struct Link {
    to_b: RefCell<VecDeque<Message>>,
    to_a: RefCell<VecDeque<Message>>,
    up: Cell<bool>,
}

/// One end of a [`Link`]. `a` is true for the first end.
#[derive(Clone)]
struct End {
    link: Rc<Link>,
    a: bool,
}

fn connect() -> (Rc<Link>, End, End) {
    let link = Rc::new(Link::default());
    link.up.set(true);
    let a = End {
        link: Rc::clone(&link),
        a: true,
    };
    let b = End {
        link: Rc::clone(&link),
        a: false,
    };
    (link, a, b)
}

impl Transport for End {
    type Error = &'static str;

    fn send(&mut self, message: &Message) -> Result<(), Self::Error> {
        if !self.link.up.get() {
            return Err("link is down");
        }
        let queue = if self.a {
            &self.link.to_b
        } else {
            &self.link.to_a
        };
        queue.borrow_mut().push_back(message.clone());
        Ok(())
    }

    fn receive(&mut self) -> Option<Message> {
        let queue = if self.a {
            &self.link.to_a
        } else {
            &self.link.to_b
        };
        queue.borrow_mut().pop_front()
    }

    fn is_connected(&self) -> bool {
        self.link.up.get()
    }
}

fn text_of(replica: &Replica, node: NodeId) -> String {
    match replica.document().blocks.get(&BlockId(node.0)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("not a text block: {other:?}"),
    }
}

/// Two sessions on one live link, sharing one text block that holds `initial`.
fn paired(initial: &str) -> (Session<End>, Session<End>, NodeId, Rc<Link>) {
    let (link, end_a, end_b) = connect();
    let mut a = Replica::new(1);
    let b = Replica::new(2);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, initial).unwrap());
    let mut sa = Session::new(a, end_a);
    let mut sb = Session::new(b, end_b);
    sa.send(&sent);
    sb.pump();
    sa.pump();
    assert!(
        sa.replica().pending().is_empty(),
        "the peer acknowledged the setup"
    );
    (sa, sb, node, link)
}

#[test]
fn acknowledged_edits_leave_the_outbox_and_reach_the_peer() {
    let (mut sa, mut sb, node, _link) = paired("base");
    let sent = sa.replica_mut().insert_text(node, 4, "!").unwrap();
    sa.send(&sent);
    assert_eq!(sb.pump(), 1);
    assert_eq!(text_of(sb.replica(), node), "base!");
    sa.pump();
    assert!(sa.replica().pending().is_empty());
}

#[test]
fn offline_edits_stay_pending_and_resend_on_reconnect() {
    let (mut sa, mut sb, node, link) = paired("base");

    // The link drops. Edits are made but cannot be sent.
    link.up.set(false);
    sa.disconnect();
    let offline = sa.replica_mut().insert_text(node, 4, "-a").unwrap();
    sa.send(&offline);
    assert!(!sa.is_online());
    assert_eq!(sa.replica().pending().len(), 2);

    // The peer edits too, on its side, and its edit is also lost with the link.
    let lost = sb.replica_mut().insert_text(node, 0, "b-").unwrap();
    sb.send(&lost);
    assert_eq!(sb.pump(), 0);

    // A new connection: a fresh link, and both sessions reconnect on it.
    let (_new_link, end_a, end_b) = connect();
    sa.reconnect(end_a);
    sb.reconnect(end_b);
    assert!(sa.is_online() && sb.is_online());

    // Everything that was pending goes out on reconnect, and each side acknowledges.
    for _ in 0..3 {
        sb.pump();
        sa.pump();
    }
    assert!(sa.replica().pending().is_empty(), "outbox emptied by acks");
    assert_eq!(text_of(sa.replica(), node), text_of(sb.replica(), node));
    assert_eq!(text_of(sb.replica(), node), "b-base-a");
}

#[test]
fn a_lost_acknowledgement_is_repeated_when_the_operation_arrives_again() {
    let (mut sa, mut sb, node, link) = paired("base");
    let sent = sa.replica_mut().insert_text(node, 4, "x").unwrap();
    sa.send(&sent);
    // b applies it, but its ack is lost: the link drops before a reads it.
    sb.pump();
    link.up.set(false);
    sa.pump();
    assert_eq!(sa.replica().pending().len(), 1, "no ack arrived");

    // Reconnect and resend: b sees a duplicate, applies nothing, and acknowledges again.
    let (_new, end_a, end_b) = connect();
    sa.reconnect(end_a);
    sb.reconnect(end_b);
    sb.pump();
    sa.pump();
    assert!(sa.replica().pending().is_empty());
    assert_eq!(text_of(sb.replica(), node), "basex");
    assert_eq!(sa.replica().document(), sb.replica().document());
}

#[test]
fn an_outbox_entry_waits_for_every_known_peer() {
    // Replica 1 talks to replicas 2 and 3. Only 2 has acknowledged, so 3 may still need it.
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let mut c = Replica::new(3);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, "hi").unwrap());
    for m in sent.iter().cloned().map(Message::Op) {
        b.receive(m.clone());
        c.receive(m);
    }
    // c hears from b so that b and c are known peers of a.
    c.receive(Message::Presence(b.set_cursor(None)));
    a.receive(Message::Presence(b.set_cursor(None)));
    a.receive(Message::Presence(c.set_cursor(None)));

    let ack_b = b.ack(1).unwrap();
    a.receive(Message::Ack(ack_b));
    assert!(!a.pending().is_empty(), "c has not acknowledged");

    let ack_c = c.ack(1).unwrap();
    a.receive(Message::Ack(ack_c));
    assert!(a.pending().is_empty(), "both peers acknowledged");
}

#[test]
fn acknowledgements_for_other_replicas_are_ignored() {
    let mut a = Replica::new(1);
    let (_node, _) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    a.receive(Message::Ack(tpt_codex_core::Ack {
        to: 9,
        from: 2,
        seq: u64::MAX,
    }));
    assert!(!a.pending().is_empty(), "the ack was for replica 9");
}

#[test]
fn only_the_newest_cursor_is_queued_while_offline_and_sent_on_reconnect() {
    let (mut sa, mut sb, node, link) = paired("text");
    link.up.set(false);
    sa.disconnect();
    sa.set_cursor(Some(Cursor { node, offset: 1 }));
    sa.set_cursor(Some(Cursor { node, offset: 3 }));
    sa.set_cursor(Some(Cursor { node, offset: 2 }));
    assert_eq!(sb.replica().cursor_of(1), None, "nothing was sent");

    let (_new, end_a, end_b) = connect();
    sa.reconnect(end_a);
    sb.reconnect(end_b);
    sb.pump();
    assert_eq!(sb.replica().cursor_of(1), Some(Cursor { node, offset: 2 }));
    assert_eq!(sb.replica().cursors_in(node), vec![(1, 2)]);
}

#[test]
fn cursor_updates_while_online_are_sent_at_once() {
    let (mut sa, mut sb, node, _link) = paired("text");
    sa.set_cursor(Some(Cursor { node, offset: 4 }));
    sb.pump();
    assert_eq!(sb.replica().cursor_of(1), Some(Cursor { node, offset: 4 }));
}

#[test]
fn a_failed_send_takes_the_session_offline_and_keeps_the_edit() {
    let (mut sa, _sb, node, link) = paired("base");
    link.up.set(false);
    let sent = sa.replica_mut().insert_text(node, 4, "?").unwrap();
    sa.send(&sent);
    assert!(!sa.is_online());
    assert_eq!(sa.replica().pending().len(), 1);
}

#[test]
fn messages_from_a_dropped_link_are_not_read() {
    let (mut sa, mut sb, node, link) = paired("base");
    let sent = sb.replica_mut().insert_text(node, 0, "x").unwrap();
    sb.send(&sent);
    link.up.set(false);
    assert_eq!(sa.pump(), 0, "a closed link delivers nothing");
    assert!(!sa.is_online());
}

#[test]
fn a_received_envelope_is_not_applied_twice() {
    let (mut sa, mut sb, node, _link) = paired("base");
    let sent = sa.replica_mut().insert_text(node, 4, "!").unwrap();
    sa.send(&sent);
    sb.pump();
    assert_eq!(text_of(sb.replica(), node), "base!");
    // Replaying the same envelopes, as a resend would, changes nothing.
    for envelope in &sent {
        sb.replica_mut().receive(Message::Op(envelope.clone()));
    }
    assert_eq!(text_of(sb.replica(), node), "base!");
}
