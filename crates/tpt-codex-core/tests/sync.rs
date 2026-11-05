// SPDX-License-Identifier: MIT OR Apache-2.0
//! Replicas converge under in-order, reordered, duplicated and offline delivery.

use std::collections::VecDeque;

use tpt_codex_core::{
    Block, BlockId, Cursor, Envelope, Message, NodeId, ROOT, Replica, SyncError, TextStyle,
    Transport,
};

/// A one-way link that queues messages until the test delivers them.
#[derive(Default)]
struct Link {
    queue: VecDeque<Message>,
}

impl Transport for Link {
    type Error = core::convert::Infallible;

    fn send(&mut self, message: &Message) -> Result<(), Self::Error> {
        self.queue.push_back(message.clone());
        Ok(())
    }

    fn receive(&mut self) -> Option<Message> {
        self.queue.pop_front()
    }
}

fn ops(envelopes: Vec<Envelope>) -> Vec<Message> {
    envelopes.into_iter().map(Message::Op).collect()
}

fn deliver(to: &mut Replica, messages: impl IntoIterator<Item = Message>) {
    for m in messages {
        to.receive(m);
    }
}

/// The text of block `node` in `replica`'s document.
fn text_of(replica: &Replica, node: NodeId) -> String {
    match replica.document().blocks.get(&BlockId(node.0)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("not a text block: {other:?}"),
    }
}

/// Two replicas sharing one text block that holds `initial`.
fn shared_text(initial: &str) -> (Replica, Replica, NodeId, Vec<Message>) {
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, initial).unwrap());
    let messages = ops(sent);
    deliver(&mut b, messages.clone());
    // The peer has the setup, so it is not pending any more.
    a.acknowledge(u64::MAX);
    (a, b, node, messages)
}

#[test]
fn in_order_delivery_converges() {
    let (a, b, node, _) = shared_text("hello");
    assert_eq!(text_of(&a, node), "hello");
    assert_eq!(text_of(&b, node), "hello");
    assert_eq!(a.document(), b.document());
}

#[test]
fn concurrent_inserts_and_deletes_converge() {
    let (mut a, mut b, node, _) = shared_text("ab");

    let from_a = a.insert_text(node, 0, "X").unwrap();
    let from_b = b.insert_text(node, 2, "Y").unwrap();
    let deleted = b.delete_text(node, 1).unwrap();

    deliver(&mut b, ops(from_a));
    deliver(&mut a, ops(from_b));
    deliver(&mut a, [Message::Op(deleted)]);

    let text_a = text_of(&a, node);
    assert_eq!(text_a, text_of(&b, node));
    assert_eq!(a.document(), b.document());
    // 'a' and 'b' are both there except the one deleted; 'X' and 'Y' are kept.
    assert!(text_a.contains('X') && text_a.contains('Y'), "{text_a}");
}

#[test]
fn reversed_delivery_is_held_until_its_dependencies_arrive() {
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, "abc").unwrap());

    // Deliver the newest operation first. Nothing can apply until the create arrives.
    let mut reversed = ops(sent);
    reversed.reverse();
    deliver(&mut b, reversed);

    assert_eq!(text_of(&b, node), "abc");
    assert_eq!(a.document(), b.document());
}

#[test]
fn duplicate_deliveries_change_nothing() {
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, "abc").unwrap());
    let messages = ops(sent);

    deliver(&mut b, messages.clone());
    deliver(&mut b, messages.clone());
    assert_eq!(text_of(&b, node), "abc");
    assert_eq!(a.document(), b.document());
}

#[test]
fn offline_edits_are_resent_and_acknowledged() {
    let (mut a, mut b, node, _) = shared_text("base");

    // Both sides edit while the link is down.
    a.insert_text(node, 4, "-a").unwrap();
    let offline_b = b.insert_text(node, 0, "b-").unwrap();
    assert_eq!(a.pending().len(), 2);
    let last_seq = a.pending().last().unwrap().seq;

    // Reconnect: a resends everything it has not had acknowledged. Some of it arrives twice.
    let mut link = Link::default();
    for env in a.pending().to_vec() {
        link.send(&Message::Op(env)).unwrap();
    }
    for env in a.pending().to_vec() {
        link.send(&Message::Op(env)).unwrap();
    }
    while let Some(m) = link.receive() {
        b.receive(m);
    }
    // b's own offline edits reach a now.
    deliver(&mut a, ops(offline_b));
    assert_eq!(a.document(), b.document());
    assert_eq!(text_of(&a, node), "b-base-a");

    // b acknowledges, and a's outbox empties.
    a.acknowledge(last_seq);
    assert!(a.pending().is_empty());
}

#[test]
fn concurrent_tree_inserts_converge() {
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let (first, sent_a) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    deliver(&mut b, ops(sent_a));

    // Both insert under the root at index 0, without seeing each other's change.
    let (from_a_node, sent_a) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let (from_b_node, sent_b) = b.create_text(ROOT, 0, TextStyle::default()).unwrap();
    deliver(&mut a, ops(sent_b));
    deliver(&mut b, ops(sent_a));

    assert_eq!(a.document(), b.document());
    for node in [first, from_a_node, from_b_node] {
        assert!(a.document().blocks.contains_key(&BlockId(node.0)));
    }
}

#[test]
fn presence_keeps_the_newest_update_and_is_not_queued() {
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let node = NodeId(42);
    let early = a.set_cursor(Some(Cursor { node, offset: 1 }));
    let late = a.set_cursor(Some(Cursor { node, offset: 5 }));
    assert!(a.pending().is_empty(), "presence is not queued");

    // The newer update arrives first, then the stale one. The stale one must not win.
    b.receive(Message::Presence(late.clone()));
    b.receive(Message::Presence(early));
    assert_eq!(b.cursor_of(1), Some(Cursor { node, offset: 5 }));
    assert_eq!(b.cursors_in(node), vec![(1, 5)]);
    assert!(b.cursors_in(NodeId(7)).is_empty());

    // A replica's own presence is not reported back to it.
    a.receive(Message::Presence(late));
    assert!(a.cursors_in(node).is_empty());
}

#[test]
fn editing_a_block_that_is_not_text_is_refused() {
    let mut a = Replica::new(1);
    assert_eq!(
        a.insert_text(ROOT, 0, "x").map(|_| ()),
        Err(SyncError::NotText(ROOT))
    );
}

#[test]
fn messages_round_trip_through_json() {
    let mut a = Replica::new(1);
    let (node, sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let mut messages = ops(sent);
    messages.extend(ops(a.insert_text(node, 0, "é!").unwrap()));
    messages.push(Message::Presence(
        a.set_cursor(Some(Cursor { node, offset: 2 })),
    ));
    for m in messages {
        let json = serde_json::to_string(&m).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }
}
