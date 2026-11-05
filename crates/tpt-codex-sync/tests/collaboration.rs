// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 4: several people edit one document that holds text and live grids, through a relay,
//! and every copy ends up the same.

use std::thread;
use std::time::{Duration, Instant};

use tpt_codex_core::{
    Block, BlockId, CellInput, Envelope, Message, NodeId, ROOT, Replica, SheetId, TextStyle,
    Transport,
};
use tpt_codex_sync::{WsTransport, spawn};

const WAIT: Duration = Duration::from_secs(10);

fn send_all(transport: &mut WsTransport, envelopes: Vec<Envelope>) {
    for env in envelopes {
        transport.send(&Message::Op(env)).unwrap();
    }
}

/// Receives until `done` holds, or fails after `WAIT`.
fn pump_until(transport: &mut WsTransport, replica: &mut Replica, done: impl Fn(&Replica) -> bool) {
    let deadline = Instant::now() + WAIT;
    while !done(replica) {
        assert!(Instant::now() < deadline, "timed out");
        match transport.receive() {
            Some(m) => replica.receive(m),
            None => thread::sleep(Duration::from_millis(2)),
        }
    }
}

fn text_of(replica: &Replica, node: NodeId) -> String {
    match replica.document().blocks.get(&BlockId(node.0)) {
        Some(Block::Text { content, .. }) => content.clone(),
        _ => String::new(),
    }
}

fn cell_of(replica: &Replica, sheet: SheetId, address: &str) -> Option<CellInput> {
    replica
        .document()
        .sheets
        .get(&sheet)
        .and_then(|s| s.cells.get(address).cloned())
}

#[test]
fn people_editing_text_and_grids_together_end_up_with_the_same_document() {
    let addr = spawn("127.0.0.1:0").unwrap().to_string();
    let mut ta = WsTransport::connect(&addr, "report").unwrap();
    let mut tb = WsTransport::connect(&addr, "report").unwrap();
    let mut a = Replica::new(1);
    let mut b = Replica::new(2);
    let sheet = SheetId(1);

    // Ann creates a paragraph and a 2 by 2 grid with a header row.
    let (text, mut sent) = a.create_text(ROOT, 0, TextStyle::default()).unwrap();
    sent.extend(a.insert_text(text, 0, "Totals").unwrap());
    sent.push(a.set_sheet_size(sheet, 2, 2, 1));
    let (_grid, grid_ops) = a
        .create_block(
            ROOT,
            1,
            Block::Grid {
                sheet_id: sheet,
                view_bounds: None,
            },
        )
        .unwrap();
    sent.extend(grid_ops);
    sent.push(
        a.set_cell(sheet, "A1", Some(CellInput::Number { value: 3.0 }))
            .unwrap(),
    );
    send_all(&mut ta, sent);
    pump_until(&mut tb, &mut b, |r| {
        text_of(r, text) == "Totals" && cell_of(r, sheet, "A1").is_some()
    });

    // Both edit at once, while not yet seeing each other's changes.
    let mut from_a = a.insert_text(text, 6, " for Q3").unwrap();
    from_a.push(
        a.set_cell(sheet, "B2", Some(CellInput::Text { value: "ok".into() }))
            .unwrap(),
    );
    let mut from_b = b.insert_text(text, 0, "Report: ").unwrap();
    from_b.push(
        b.set_cell(sheet, "A1", Some(CellInput::Number { value: 7.0 }))
            .unwrap(),
    );
    send_all(&mut ta, from_a);
    send_all(&mut tb, from_b);

    // Wait for everything each side should end with, not just the text: grid writes come later
    // in each sender's stream.
    let expected = "Report: Totals for Q3";
    pump_until(&mut tb, &mut b, |r| {
        text_of(r, text) == expected && cell_of(r, sheet, "B2").is_some()
    });
    pump_until(&mut ta, &mut a, |r| {
        text_of(r, text) == expected
            && cell_of(r, sheet, "A1") == Some(CellInput::Number { value: 7.0 })
    });

    // Every copy agrees on the text and on the grid, and the grid's later write won.
    assert_eq!(a.document(), b.document());
    let doc = a.document();
    doc.validate().unwrap();
    assert!(doc.sheets[&sheet].cells.contains_key("B2"));
    assert_eq!(cell_of(&a, sheet, "A1"), cell_of(&b, sheet, "A1"));
    assert_eq!(doc.sheets[&sheet].header_rows, 1);
}
