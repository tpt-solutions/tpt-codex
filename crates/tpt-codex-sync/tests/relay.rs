// SPDX-License-Identifier: MIT OR Apache-2.0
//! Replicas talk through a real relay on the loopback interface.

use std::thread;
use std::time::{Duration, Instant};

use tpt_codex_core::{Block, BlockId, Cursor, Message, ROOT, Replica, TextStyle, Transport};
use tpt_codex_sync::{WsTransport, spawn};

const WAIT: Duration = Duration::from_secs(10);

/// Receives until `done` holds for `replica`, or fails after `WAIT`.
fn pump_until(transport: &mut WsTransport, replica: &mut Replica, done: impl Fn(&Replica) -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done(replica) {
        assert!(Instant::now() < deadline, "timed out");
        match transport.receive() {
            Some(message) => replica.receive(message),
            None => thread::sleep(Duration::from_millis(2)),
        }
    }
}

fn text_of(replica: &Replica, node: tpt_codex_core::NodeId) -> String {
    match replica.document().blocks.get(&BlockId(node.0)) {
        Some(Block::Text { content, .. }) => content.clone(),
        _ => String::new(),
    }
}

fn server() -> String {
    spawn("127.0.0.1:0").unwrap().to_string()
}

#[test]
fn two_replicas_converge_over_the_relay() {
    let addr = server();
    let mut ta = WsTransport::connect(&addr, "doc-a").unwrap();
    let mut tb = WsTransport::connect(&addr, "doc-a").unwrap();
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);

    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, "hello").unwrap());
    for env in sent {
        ta.send(&Message::Op(env)).unwrap();
    }
    pump_until(&mut tb, &mut b, |r| text_of(r, node) == "hello");

    // And back the other way.
    for env in b.insert_text(node, 5, "!").unwrap() {
        tb.send(&Message::Op(env)).unwrap();
    }
    pump_until(&mut ta, &mut a, |r| text_of(r, node) == "hello!");
    assert_eq!(a.document(), b.document());
}

#[test]
fn a_late_joiner_catches_up_from_the_log() {
    let addr = server();
    let mut ta = WsTransport::connect(&addr, "doc-late").unwrap();
    let mut a = Replica::new(1);
    let (node, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(node, 0, "history").unwrap());
    for env in sent {
        ta.send(&Message::Op(env)).unwrap();
    }
    // Make sure the relay has the operations before the late client joins.
    thread::sleep(Duration::from_millis(100));

    let mut tc = WsTransport::connect(&addr, "doc-late").unwrap();
    let mut c = Replica::new(3);
    pump_until(&mut tc, &mut c, |r| text_of(r, node) == "history");
    assert_eq!(a.document(), c.document());
}

#[test]
fn documents_are_kept_apart() {
    let addr = server();
    let mut t1 = WsTransport::connect(&addr, "doc-one").unwrap();
    let mut t2 = WsTransport::connect(&addr, "doc-two").unwrap();
    let mut a = Replica::new(1);
    let mut other = Replica::new(2);
    let (node, sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    for env in sent {
        t1.send(&Message::Op(env)).unwrap();
    }
    // The other document must not see it.
    thread::sleep(Duration::from_millis(200));
    while let Some(m) = t2.receive() {
        other.receive(m);
    }
    assert!(!other.document().blocks.contains_key(&BlockId(node.0)));
}

#[test]
fn presence_reaches_the_other_replica_and_is_not_replayed() {
    let addr = server();
    let mut ta = WsTransport::connect(&addr, "doc-p").unwrap();
    let mut tb = WsTransport::connect(&addr, "doc-p").unwrap();
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let cursor = Cursor {
        node: tpt_codex_core::NodeId(42),
        offset: 3,
    };
    ta.send(&Message::Presence(a.set_cursor(Some(cursor))))
        .unwrap();

    let deadline = Instant::now() + WAIT;
    while b.cursor_of(1).is_none() {
        assert!(Instant::now() < deadline, "presence did not arrive");
        match tb.receive() {
            Some(m) => b.receive(m),
            None => thread::sleep(Duration::from_millis(2)),
        }
    }
    assert_eq!(b.cursor_of(1), Some(cursor));

    // A client that joins later is not sent the old cursor: presence is not logged.
    let mut tc = WsTransport::connect(&addr, "doc-p").unwrap();
    thread::sleep(Duration::from_millis(200));
    let mut c = Replica::new(3);
    while let Some(m) = tc.receive() {
        c.receive(m);
    }
    assert_eq!(c.cursor_of(1), None);
}

#[test]
fn a_repeated_send_is_not_forwarded_twice() {
    let addr = server();
    let mut ta = WsTransport::connect(&addr, "doc-dup").unwrap();
    let mut tb = WsTransport::connect(&addr, "doc-dup").unwrap();
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let (node, sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    // Send everything twice, as an offline replica does after reconnecting.
    for env in sent.iter().chain(sent.iter()) {
        ta.send(&Message::Op(env.clone())).unwrap();
    }
    pump_until(&mut tb, &mut b, |r| {
        text_of(r, node).is_empty() && r.document().blocks.contains_key(&BlockId(node.0))
    });
    thread::sleep(Duration::from_millis(200));
    let mut extra = 0;
    while let Some(m) = tb.receive() {
        extra += 1;
        b.receive(m);
    }
    assert_eq!(extra, 0, "the relay forwarded a repeated operation");
}
