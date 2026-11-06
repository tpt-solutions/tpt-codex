// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cursors through the WebSocket relay, driven by a `Session` on each side.

use std::thread;
use std::time::{Duration, Instant};

use tpt_codex_core::{Cursor, NodeId, Replica, Session};
use tpt_codex_sync::{WsTransport, spawn};

const WAIT: Duration = Duration::from_secs(10);

fn server() -> String {
    spawn("127.0.0.1:0").unwrap().to_string()
}

/// Pumps `session` until `done` holds for its replica, or fails after `WAIT`.
fn pump_until(session: &mut Session<WsTransport>, done: impl Fn(&Replica) -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done(session.replica()) {
        assert!(Instant::now() < deadline, "timed out waiting for presence");
        if session.pump() == 0 {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

fn caret(offset: usize) -> Cursor {
    Cursor {
        node: NodeId(42),
        offset,
    }
}

#[test]
fn a_cursor_set_on_one_session_reaches_the_other_over_the_relay() {
    let addr = server();
    let mut a = Session::new(
        Replica::new(1),
        WsTransport::connect(&addr, "doc-live").unwrap(),
    );
    let mut b = Session::new(
        Replica::new(2),
        WsTransport::connect(&addr, "doc-live").unwrap(),
    );

    a.set_cursor(Some(caret(3)));
    pump_until(&mut b, |r| r.cursor_of(1) == Some(caret(3)));

    // Moving the caret sends the new position.
    a.set_cursor(Some(caret(7)));
    pump_until(&mut b, |r| r.cursor_of(1) == Some(caret(7)));
}

#[test]
fn while_offline_only_the_newest_cursor_is_kept_and_sent_on_reconnect() {
    let addr = server();
    let mut a = Session::new(
        Replica::new(1),
        WsTransport::connect(&addr, "doc-offline").unwrap(),
    );
    let mut b = Session::new(
        Replica::new(2),
        WsTransport::connect(&addr, "doc-offline").unwrap(),
    );

    a.disconnect();
    a.set_cursor(Some(caret(1)));
    a.set_cursor(Some(caret(2)));
    a.set_cursor(Some(caret(9)));
    assert!(!a.is_online());

    // Nothing has reached the other side while A was offline.
    thread::sleep(Duration::from_millis(100));
    b.pump();
    assert_eq!(b.replica().cursor_of(1), None);

    // Reconnecting sends one update: the newest cursor.
    a.reconnect(WsTransport::connect(&addr, "doc-offline").unwrap());
    assert!(a.is_online());
    pump_until(&mut b, |r| r.cursor_of(1).is_some());
    assert_eq!(b.replica().cursor_of(1), Some(caret(9)));
}
