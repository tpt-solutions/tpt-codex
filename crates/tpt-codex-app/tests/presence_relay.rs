// SPDX-License-Identifier: MIT OR Apache-2.0
//! Carets from one editor reach another editor's page through the WebSocket relay, as presence
//! messages, and are drawn there.

use std::thread;
use std::time::{Duration, Instant};

use tpt_appfront_html::render;
use tpt_codex_app::{Editor, Msg, Presence, RemoteCursor};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Message, TextStyle, Transport};
use tpt_codex_sync::{WsTransport, spawn};

const WAIT: Duration = Duration::from_secs(10);

/// Root 0 -> [1, 2]: two paragraphs.
fn document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    for (id, text) in [(1, "First"), (2, "Second")] {
        doc.insert(
            BlockId(id),
            Block::Text {
                content: text.into(),
                style: TextStyle::default(),
            },
        );
    }
    doc
}

/// Sends this editor's caret, if it has moved since the last send.
fn announce(presence: &mut Presence, editor: &Editor, transport: &mut WsTransport) {
    if let Some(message) = presence.outbound_for(editor.caret()) {
        transport.send(&message).expect("send presence");
    }
}

/// Reads from `transport` into `presence` until `done` holds, or fails after `WAIT`.
fn receive_until(
    presence: &mut Presence,
    transport: &mut WsTransport,
    done: impl Fn(&Presence) -> bool,
) {
    let deadline = Instant::now() + WAIT;
    while !done(presence) {
        assert!(Instant::now() < deadline, "timed out waiting for a caret");
        match transport.receive() {
            Some(message) => presence.receive(message),
            None => thread::sleep(Duration::from_millis(2)),
        }
    }
}

fn remote_in(presence: &Presence, block: BlockId) -> Option<RemoteCursor> {
    presence
        .remote_cursors()
        .into_iter()
        .find(|r| r.block == block)
}

#[test]
fn a_caret_moved_on_one_page_is_drawn_on_the_other() {
    let addr = spawn("127.0.0.1:0").unwrap().to_string();
    let mut a_link = WsTransport::connect(&addr, "presence-draw").unwrap();
    let mut b_link = WsTransport::connect(&addr, "presence-draw").unwrap();
    let mut a = Presence::new(1);
    let mut b = Presence::new(2);
    let mut a_editor = Editor::new(document());
    let mut b_editor = Editor::new(document());

    a_editor.update(Msg::Focus(BlockId(2)));
    announce(&mut a, &a_editor, &mut a_link);
    receive_until(&mut b, &mut b_link, |p| remote_in(p, BlockId(2)).is_some());

    let expected = RemoteCursor {
        label: "Replica 1".into(),
        block: BlockId(2),
        offset: a_editor.caret().unwrap().focus,
    };
    assert_eq!(remote_in(&b, BlockId(2)), Some(expected.clone()));

    b_editor.set_remote_cursors(b.remote_cursors());
    let page = render(&b_editor.view());
    assert!(
        page.contains("remote-cursor"),
        "the other page draws the caret: {page}"
    );
    assert!(
        page.contains("Replica 1"),
        "the caret names its replica: {page}"
    );

    // Moving the caret moves it on the other page.
    a_editor.update(Msg::Focus(BlockId(1)));
    announce(&mut a, &a_editor, &mut a_link);
    receive_until(&mut b, &mut b_link, |p| remote_in(p, BlockId(1)).is_some());
    b_editor.set_remote_cursors(b.remote_cursors());
    assert!(
        remote_in(&b, BlockId(2)).is_none(),
        "a peer has one caret, so the old paragraph no longer shows it"
    );
}

#[test]
fn a_page_that_joins_late_learns_the_caret_once_the_peer_announces_again() {
    let addr = spawn("127.0.0.1:0").unwrap().to_string();
    let mut a_link = WsTransport::connect(&addr, "presence-late").unwrap();
    let mut a = Presence::new(1);
    let mut a_editor = Editor::new(document());
    a_editor.update(Msg::Focus(BlockId(1)));
    announce(&mut a, &a_editor, &mut a_link);

    // The relay keeps no presence, so a page that joins now hears nothing until A speaks again.
    let mut c_link = WsTransport::connect(&addr, "presence-late").unwrap();
    let mut c = Presence::new(3);
    thread::sleep(Duration::from_millis(100));
    while let Some(message) = c_link.receive() {
        c.receive(message);
    }
    assert!(c.remote_cursors().is_empty(), "nothing is replayed");

    // A page that has just joined asks its peers to announce; A does, and C sees the caret.
    a.announce_again();
    announce(&mut a, &a_editor, &mut a_link);
    receive_until(&mut c, &mut c_link, |p| remote_in(p, BlockId(1)).is_some());
}

#[test]
fn a_message_that_is_not_presence_does_not_move_any_caret() {
    let mut p = Presence::new(1);
    p.receive(Message::Ack(tpt_codex_core::Ack {
        to: 1,
        from: 2,
        seq: 9,
    }));
    assert!(p.remote_cursors().is_empty());
}
