// SPDX-License-Identifier: MIT OR Apache-2.0
//! The relay's limits and clean stop, over real loopback sockets.

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use tpt_codex_core::{Message, ROOT, Replica, TextStyle, Transport};
use tpt_codex_sync::{Limits, WsTransport, serve_until, spawn_with};

const WAIT: Duration = Duration::from_secs(10);

fn server(limits: Limits) -> String {
    spawn_with("127.0.0.1:0", limits).unwrap().to_string()
}

/// Receives until `done` holds for the transport, or fails after `WAIT`.
fn receive_until(transport: &mut WsTransport, done: impl Fn(&WsTransport) -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done(transport) {
        assert!(Instant::now() < deadline, "timed out");
        let _ = transport.receive();
        thread::sleep(Duration::from_millis(2));
    }
}

/// Everything `replica` edits in the test, as operations in send order.
fn text_ops(replica: &mut Replica, words: &[&str]) -> Vec<tpt_codex_core::Envelope> {
    let (node, mut ops) = replica.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let mut at = 0;
    for word in words {
        ops.extend(replica.insert_text(node, at, word).unwrap());
        at += word.chars().count();
    }
    ops
}

#[test]
fn a_message_over_the_size_cap_closes_the_connection() {
    let addr = server(Limits {
        max_message_bytes: 2048,
        max_ops_per_doc: 1000,
    });

    // A raw client, so the frame can be larger than any operation the replica would send. The
    // frame is not even a message: the cap applies before the relay parses anything.
    let stream = TcpStream::connect(&addr).unwrap();
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let (mut raw, _) = tungstenite::client(format!("ws://{addr}/"), stream).unwrap();
    raw.send(tungstenite::Message::Text(r#"{"join":"size"}"#.into()))
        .unwrap();
    // The relay acknowledges the join before anything else.
    assert!(matches!(raw.read(), Ok(tungstenite::Message::Text(_))));

    let oversized = format!(r#"{{"junk":"{}"}}"#, "x".repeat(8000));
    // The send may succeed locally; the relay then closes the connection.
    let _ = raw.send(tungstenite::Message::Text(oversized));
    let closed = loop {
        match raw.read() {
            Ok(tungstenite::Message::Close(_)) | Err(_) => break true,
            Ok(_) => {}
        }
    };
    assert!(closed, "the relay kept an oversized connection open");

    // The oversized operation was never stored: a late joiner gets none of it, and an
    // operation within the cap still reaches it.
    let mut small = WsTransport::connect(&addr, "size").unwrap();
    let mut other = WsTransport::connect(&addr, "size").unwrap();
    let mut second = Replica::new(2);
    let (_, ops) = second.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let sent = ops.len();
    for op in ops {
        other.send(&Message::Op(op)).unwrap();
    }
    let mut received = 0;
    let deadline = Instant::now() + WAIT;
    while received == 0 {
        assert!(
            Instant::now() < deadline,
            "the small operation never arrived"
        );
        if let Some(Message::Op(_)) = small.receive() {
            received += 1;
        }
    }
    // Nothing else arrives: the oversized operation is not in the log.
    let quiet_until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < quiet_until {
        if let Some(Message::Op(_)) = small.receive() {
            received += 1;
        }
    }
    assert_eq!(
        received, sent,
        "only the operations sent after the oversized one arrive"
    );
}

#[test]
fn operations_stop_being_stored_at_the_per_document_cap() {
    let addr = server(Limits {
        max_message_bytes: 1024 * 1024,
        max_ops_per_doc: 2,
    });
    let mut sender = WsTransport::connect(&addr, "cap").unwrap();
    let mut replica = Replica::new(1);
    let ops = text_ops(&mut replica, &["a", "b", "c", "d"]);
    assert!(ops.len() > 2, "the test needs more operations than the cap");
    for op in &ops {
        sender.send(&Message::Op(op.clone())).unwrap();
    }

    // The sender is told, and only the sender.
    receive_until(&mut sender, |t| t.relay_error().is_some());
    assert_eq!(sender.relay_error(), Some("document is full"));
    assert!(
        sender.is_connected(),
        "a refusal does not close the connection"
    );

    // A late joiner gets exactly the first two operations, in order.
    let mut late = WsTransport::connect(&addr, "cap").unwrap();
    let mut got = Vec::new();
    let quiet_until = Instant::now() + Duration::from_millis(500);
    while Instant::now() < quiet_until {
        if let Some(Message::Op(op)) = late.receive() {
            got.push(op);
        }
    }
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], ops[0]);
    assert_eq!(got[1], ops[1]);
}

#[test]
fn the_cap_is_per_document() {
    let addr = server(Limits {
        max_message_bytes: 1024 * 1024,
        max_ops_per_doc: 1,
    });
    let mut replica = Replica::new(1);
    let ops = text_ops(&mut replica, &["a", "b"]);
    let mut first = WsTransport::connect(&addr, "one").unwrap();
    first.send(&Message::Op(ops[0].clone())).unwrap();
    let mut second = WsTransport::connect(&addr, "two").unwrap();
    second.send(&Message::Op(ops[0].clone())).unwrap();

    // Each document has room for its own first operation, so neither sender is refused.
    let quiet_until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < quiet_until {
        let _ = first.receive();
        let _ = second.receive();
    }
    assert_eq!(first.relay_error(), None);
    assert_eq!(second.relay_error(), None);
}

#[test]
fn the_relay_stops_accepting_once_asked_to_stop() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let (done_tx, done_rx) = mpsc::channel();
    let flag = Arc::clone(&stop);
    thread::spawn(move || {
        let result = serve_until(listener, Limits::default(), flag);
        let _ = done_tx.send(result.is_ok());
    });

    // It serves while running.
    let working = WsTransport::connect(&addr, "stop").unwrap();
    assert!(working.is_connected());

    stop.store(true, Ordering::SeqCst);
    let ok = done_rx
        .recv_timeout(WAIT)
        .expect("the accept loop did not stop");
    assert!(ok, "stopping should return Ok");

    // The listener is gone, so new connections are refused.
    assert!(TcpStream::connect(&addr).is_err());
}
