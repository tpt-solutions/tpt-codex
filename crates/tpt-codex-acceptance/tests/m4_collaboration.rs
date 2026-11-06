// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 4 (todo.md): "Multi-user collaborative editing of a document containing live grids."
//!
//! Two replicas edit the same document through a relay: one types a paragraph and fills a grid,
//! the other edits the same paragraph and the grid at the same time. Both end with the same
//! document, with the converged text and the grid's values.

use tpt_codex_core::{
    Block, CellInput, Envelope, Message, ROOT, Relay, Replica, SheetId, TextStyle,
};

fn ops(envelopes: Vec<Envelope>) -> Vec<Message> {
    envelopes.into_iter().map(Message::Op).collect()
}

/// Publishes each message to the relay under `doc`, then hands every replica the whole log.
fn sync(relay: &mut Relay, replicas: &mut [&mut Replica], messages: Vec<Message>) {
    for message in messages {
        relay.publish("doc", message);
    }
    let log: Vec<Message> = relay.since("doc", 0).to_vec();
    for replica in replicas.iter_mut() {
        for message in log.clone() {
            replica.receive(message);
        }
    }
}

#[test]
fn two_people_edit_a_paragraph_and_a_live_grid_and_converge() {
    let mut ann = Replica::new(1);
    let mut ben = Replica::new(2);
    let mut relay = Relay::new();

    // Ann creates the paragraph and a 2 x 2 grid, then syncs.
    let (text, created) = ann.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let mut messages = ops(created);
    messages.extend(ops(ann.insert_text(text, 0, "Costs").unwrap()));
    messages.push(Message::Op(ann.set_sheet_size(SheetId(1), 2, 2, 0)));
    sync(&mut relay, &mut [&mut ben], messages);
    assert_eq!(
        ben.document(),
        ann.document(),
        "the replicas agree after the first sync"
    );

    // Both edit at once, before either sees the other's change.
    let ann_edit = ops(ann.insert_text(text, 5, " for Q3").unwrap());
    let ben_edit = ops(ben.insert_text(text, 0, "Draft: ").unwrap());
    let ann_cell = ann
        .set_cell(SheetId(1), "A1", Some(CellInput::Number { value: 21.0 }))
        .unwrap();
    let ben_cell = ben
        .set_cell(
            SheetId(1),
            "A2",
            Some(CellInput::Formula {
                source: "=A1*2".into(),
            }),
        )
        .unwrap();

    let mut both = ann_edit;
    both.extend(ben_edit);
    both.push(Message::Op(ann_cell));
    both.push(Message::Op(ben_cell));
    sync(&mut relay, &mut [&mut ann, &mut ben], both);

    assert_eq!(ann.document(), ben.document(), "the replicas converge");
    let doc = ann.document();
    // The replica's document numbers its blocks itself, so find the paragraph by its kind.
    let text_now = doc
        .blocks
        .values()
        .find_map(|b| match b {
            Block::Text { content, .. } if content.contains("Costs") => Some(content.clone()),
            _ => None,
        })
        .expect("the paragraph is in the document");
    assert!(
        text_now.contains("Costs"),
        "the typed text survives: {text_now:?}"
    );
    assert!(
        text_now.contains("Draft: "),
        "the other edit survives: {text_now:?}"
    );
    assert!(
        text_now.contains("for Q3"),
        "the first edit survives: {text_now:?}"
    );

    let results = tpt_codex_grid::evaluate_sheet(&doc.sheets[&SheetId(1)]);
    assert_eq!(
        results["A2"],
        tpt_codex_grid::CellResult::Number(42.0),
        "the grid agrees"
    );
}

#[test]
fn a_replica_that_joins_late_catches_up_from_the_relay() {
    let mut ann = Replica::new(1);
    let mut relay = Relay::new();
    let (text, created) = ann.create_text(ROOT, 0, TextStyle::default()).unwrap();
    let mut messages = ops(created);
    messages.extend(ops(ann
        .insert_text(text, 0, "written before you joined")
        .unwrap()));
    sync(&mut relay, &mut [], messages);

    let mut late = Replica::new(3);
    for message in relay.since("doc", 0).to_vec() {
        late.receive(message);
    }
    assert_eq!(late.document(), ann.document());
}
