// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::VecDeque;

use tpt_codex_core::{Stamp, TextCrdt, TextCrdtError, TextOp};

/// Applies whatever is applicable from `pending`, repeatedly, until nothing more can be applied.
fn drain(replica: &mut TextCrdt, pending: &mut Vec<TextOp>) {
    loop {
        let before = pending.len();
        let mut rest = Vec::new();
        for op in pending.drain(..) {
            match replica.apply(&op) {
                Ok(()) => {}
                Err(TextCrdtError::Missing(_)) => rest.push(op),
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
        *pending = rest;
        if pending.len() == before {
            break;
        }
    }
}

#[test]
fn local_edits_read_back_in_order() {
    let mut a = TextCrdt::new(1);
    a.insert_local(0, 'h').unwrap();
    a.insert_local(1, 'é').unwrap();
    a.insert_local(2, 'y').unwrap();
    a.insert_local(1, '字').unwrap();
    assert_eq!(a.text(), "h字éy");
    a.delete_local(0).unwrap();
    assert_eq!(a.text(), "字éy");
    assert_eq!(a.len(), 3);
}

#[test]
fn index_bounds_are_checked() {
    let mut a = TextCrdt::new(1);
    assert_eq!(
        a.insert_local(1, 'x'),
        Err(TextCrdtError::IndexOutOfBounds { index: 1, len: 0 })
    );
    assert_eq!(
        a.delete_local(0),
        Err(TextCrdtError::IndexOutOfBounds { index: 0, len: 0 })
    );
}

#[test]
fn concurrent_inserts_at_the_same_place_converge() {
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    let op_a = a.insert_local(0, 'a').unwrap();
    let op_b = b.insert_local(0, 'b').unwrap();
    b.apply(&op_a).unwrap();
    a.apply(&op_b).unwrap();
    assert_eq!(a.text(), b.text());
    assert_eq!(a.text().len(), 2);
}

#[test]
fn an_insert_after_a_deleted_character_still_places_correctly() {
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    let first = a.insert_local(0, 'x').unwrap();
    b.apply(&first).unwrap();
    // a deletes x while b, concurrently, types after it.
    let del = a.delete_local(0).unwrap();
    let ins = b.insert_local(1, 'y').unwrap();
    a.apply(&ins).unwrap();
    b.apply(&del).unwrap();
    assert_eq!(a.text(), "y");
    assert_eq!(b.text(), "y");
}

#[test]
fn a_dependency_on_an_unseen_character_is_reported_then_applies() {
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    let first = a.insert_local(0, 'p').unwrap();
    let second = a.insert_local(1, 'q').unwrap();
    // b receives the second operation first.
    assert_eq!(
        b.apply(&second),
        Err(TextCrdtError::Missing(Stamp {
            lamport: 1,
            replica: 1
        }))
    );
    b.apply(&first).unwrap();
    b.apply(&second).unwrap();
    assert_eq!(b.text(), "pq");
}

#[test]
fn applying_an_operation_twice_changes_nothing() {
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    let op = a.insert_local(0, 'z').unwrap();
    b.apply(&op).unwrap();
    b.apply(&op).unwrap();
    assert_eq!(b.text(), "z");
    let del = a.delete_local(0).unwrap();
    b.apply(&del).unwrap();
    b.apply(&del).unwrap();
    assert!(b.is_empty());
}

#[test]
fn clocks_advance_past_remote_stamps() {
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    for i in 0..5 {
        let op = a.insert_local(i, 'a').unwrap();
        b.apply(&op).unwrap();
    }
    // b's next insert must sort after everything it has seen.
    let op = b.insert_local(5, 'b').unwrap();
    let TextOp::Insert { id, .. } = op else {
        panic!("expected insert");
    };
    assert!(id.lamport > 5);
}

#[test]
fn three_replicas_with_random_edits_and_reordered_delivery_converge() {
    // A small seeded generator keeps the run reproducible without extra dependencies.
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    const CHARS: [char; 6] = ['a', 'b', 'é', '字', '🙂', 'z'];

    for round in 0..200 {
        let n = 3usize;
        let mut replicas: Vec<TextCrdt> = (0..n).map(|i| TextCrdt::new(i as u64 + 1)).collect();
        // links[from][to] is the FIFO of operations from `from` not yet delivered to `to`.
        let mut links: Vec<Vec<VecDeque<TextOp>>> = vec![vec![VecDeque::new(); n]; n];
        let mut pending: Vec<Vec<TextOp>> = vec![Vec::new(); n];

        for _ in 0..(40 + round % 30) {
            let r = next(n as u64) as usize;
            if next(10) < 6 || replicas[r].is_empty() {
                let index = next(replicas[r].len() as u64 + 1) as usize;
                let ch = CHARS[next(CHARS.len() as u64) as usize];
                let op = replicas[r].insert_local(index, ch).unwrap();
                for (to, link) in links[r].iter_mut().enumerate() {
                    if to != r {
                        link.push_back(op.clone());
                    }
                }
            } else {
                let index = next(replicas[r].len() as u64) as usize;
                let op = replicas[r].delete_local(index).unwrap();
                for (to, link) in links[r].iter_mut().enumerate() {
                    if to != r {
                        link.push_back(op.clone());
                    }
                }
            }
            // Deliver a few messages from random links, so ops arrive out of causal order.
            for _ in 0..next(4) {
                let from = next(n as u64) as usize;
                let to = next(n as u64) as usize;
                if from == to {
                    continue;
                }
                if let Some(op) = links[from][to].pop_front() {
                    pending[to].push(op);
                    drain(&mut replicas[to], &mut pending[to]);
                }
            }
        }

        // Deliver everything that is left, in arbitrary link order, then drain the buffers.
        for from in links.iter_mut() {
            for (to, link) in from.iter_mut().enumerate() {
                while let Some(op) = link.pop_front() {
                    pending[to].push(op);
                }
            }
        }
        for to in 0..n {
            drain(&mut replicas[to], &mut pending[to]);
            assert!(
                pending[to].is_empty(),
                "round {round}: replica {to} still has undeliverable operations"
            );
        }
        let first = replicas[0].text();
        for r in &replicas[1..] {
            assert_eq!(r.text(), first, "round {round}: replicas diverged");
        }
    }
}

#[test]
fn materialize_writes_converged_text_as_one_undoable_step() {
    use tpt_codex_core::{Block, BlockId, Document, History, LayoutMode, TextStyle, materialize};
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: String::new(),
            style: TextStyle {
                bold: true,
                italic: false,
                heading_level: Some(2),
            },
        },
    );
    let mut h = History::new();
    let mut a = TextCrdt::new(1);
    let mut b = TextCrdt::new(2);
    let op = a.insert_local(0, 'h').unwrap();
    b.apply(&op).unwrap();
    let op = b.insert_local(1, 'i').unwrap();
    a.apply(&op).unwrap();

    materialize(&mut doc, &mut h, BlockId(1), &a).unwrap();
    assert_eq!(h.undo_len(), 1);
    match doc.get(BlockId(1)) {
        Some(Block::Text { content, style }) => {
            assert_eq!(content, "hi");
            assert!(style.bold, "style must be kept");
            assert_eq!(style.heading_level, Some(2));
        }
        other => panic!("{other:?}"),
    }
    h.undo(&mut doc).unwrap();
    assert!(matches!(doc.get(BlockId(1)), Some(Block::Text { content, .. }) if content.is_empty()));
}
