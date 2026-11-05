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

#[test]
fn grid_cells_converge_and_the_newest_write_wins() {
    use tpt_codex_core::{CellInput, SheetId};

    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let sheet = SheetId(1);

    let sent = a.set_sheet_size(sheet, 2, 2, 1);
    let (_, mut block_ops) = a
        .create_block(
            ROOT,
            0,
            Block::Grid {
                sheet_id: sheet,
                view_bounds: None,
            },
        )
        .unwrap();
    block_ops.insert(0, sent);
    deliver(&mut b, ops(block_ops));

    // Both replicas write A1 while apart. The later write (by stamp) must win on both.
    let from_a = a
        .set_cell(sheet, "A1", Some(CellInput::Number { value: 1.0 }))
        .unwrap();
    let from_b = b
        .set_cell(sheet, "A1", Some(CellInput::Number { value: 2.0 }))
        .unwrap();
    // a writes B2 and then clears it; b must end up with it cleared.
    let set_b2 = a
        .set_cell(sheet, "B2", Some(CellInput::Text { value: "hi".into() }))
        .unwrap();
    let clear_b2 = a.set_cell(sheet, "B2", None).unwrap();

    deliver(&mut b, [Message::Op(from_a)]);
    deliver(&mut a, [Message::Op(from_b)]);
    deliver(&mut b, [Message::Op(set_b2), Message::Op(clear_b2)]);

    let doc_a = a.document();
    let doc_b = b.document();
    assert_eq!(doc_a, doc_b);
    doc_a.validate().unwrap();
    let cells = &doc_a.sheets[&sheet].cells;
    assert!(cells.contains_key("A1"));
    assert!(!cells.contains_key("B2"));
    assert_eq!(doc_a.sheets[&sheet].rows, 2);
}

#[test]
fn a_stale_cell_write_does_not_overwrite_a_newer_one() {
    use tpt_codex_core::{CellInput, SheetId};

    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let sheet = SheetId(7);
    let old = a
        .set_cell(sheet, "C3", Some(CellInput::Number { value: 1.0 }))
        .unwrap();
    // b sees a's write, then writes a newer value.
    b.receive(Message::Op(old.clone()));
    let newer = b
        .set_cell(sheet, "C3", Some(CellInput::Number { value: 9.0 }))
        .unwrap();
    // Replaying the old write on b must not win.
    b.receive(Message::Op(old));
    a.receive(Message::Op(newer));
    assert_eq!(a.document(), b.document());
    assert_eq!(
        a.document().sheets[&sheet].cells.get("C3"),
        Some(&CellInput::Number { value: 9.0 })
    );
}

#[test]
fn invalid_cell_writes_are_refused_before_anything_is_sent() {
    use tpt_codex_core::{CellInput, SheetId};

    let mut a = Replica::new(1);
    assert!(matches!(
        a.set_cell(SheetId(1), "a1", None),
        Err(tpt_codex_core::SyncError::InvalidAddress(_))
    ));
    assert!(matches!(
        a.set_cell(
            SheetId(1),
            "A1",
            Some(CellInput::Formula {
                source: "1+1".into()
            })
        ),
        Err(tpt_codex_core::SyncError::InvalidFormula(_))
    ));
    assert!(a.pending().is_empty());
}

#[test]
fn a_relay_keeps_one_copy_of_each_operation_and_serves_late_replicas() {
    use tpt_codex_core::{Publish, Relay};

    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let mut relay = Relay::new();
    let (node, sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let mut sent_ops = ops(sent);
    sent_ops.extend(ops(a.insert_text(node, 0, "relayed").unwrap()));

    // a sends, then sends its outbox again after a reconnect: the relay keeps one copy.
    for m in sent_ops.clone() {
        assert!(matches!(relay.publish("doc-1", m), Publish::Stored(_)));
    }
    for m in sent_ops.clone() {
        assert_eq!(relay.publish("doc-1", m), Publish::Duplicate);
    }
    assert_eq!(relay.len("doc-1"), sent_ops.len());

    // A presence update is forwarded, not logged.
    let presence = Message::Presence(a.set_cursor(None));
    assert_eq!(relay.publish("doc-1", presence), Publish::NotLogged);
    assert_eq!(relay.len("doc-1"), sent_ops.len());

    // b joins late and catches up from the start.
    for m in relay.since("doc-1", 0).to_vec() {
        b.receive(m);
    }
    assert_eq!(b.document(), a.document());
    assert!(relay.since("doc-1", 999).is_empty());
    assert!(relay.since("other", 0).is_empty());
}
