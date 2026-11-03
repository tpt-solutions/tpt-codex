// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Document, EditError, History, LayoutMode, Op, TextStyle, apply,
};

fn text(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

fn container(children: Vec<BlockId>) -> Block {
    Block::Container {
        layout_mode: LayoutMode::Flow,
        children,
    }
}

/// Root 0 -> [1, 2]; 1 is a container -> [3]; 2 and 3 are text.
fn base() -> Document {
    let mut doc = Document::new(BlockId(0), container(vec![BlockId(1), BlockId(2)]));
    doc.insert(BlockId(1), container(vec![BlockId(3)]));
    doc.insert(BlockId(2), text("two"));
    doc.insert(BlockId(3), text("three"));
    doc
}

fn children(doc: &Document, id: u64) -> Vec<u64> {
    match doc.get(BlockId(id)).unwrap() {
        Block::Container { children, .. } => children.iter().map(|c| c.0).collect(),
        _ => panic!("not a container"),
    }
}

#[test]
fn add_insert_and_remove_child() {
    let mut doc = base();
    let mut hist = History::new();
    hist.commit(
        &mut doc,
        vec![
            Op::AddBlock {
                id: BlockId(10),
                block: text("new"),
            },
            Op::InsertChild {
                parent: BlockId(0),
                index: 1,
                child: BlockId(10),
            },
        ],
    )
    .unwrap();
    assert_eq!(children(&doc, 0), vec![1, 10, 2]);
    assert_eq!(doc.validate(), Ok(()));

    hist.commit(
        &mut doc,
        vec![Op::RemoveChild {
            parent: BlockId(0),
            index: 1,
        }],
    )
    .unwrap();
    assert_eq!(children(&doc, 0), vec![1, 2]);
    // The block stays in the document until it is explicitly removed.
    assert!(doc.get(BlockId(10)).is_some());
}

#[test]
fn move_child_within_and_across_containers() {
    let mut doc = base();
    let mut hist = History::new();
    // Move block 2 from root position 1 into container 1 at position 0.
    hist.commit(
        &mut doc,
        vec![Op::MoveChild {
            from_parent: BlockId(0),
            from_index: 1,
            to_parent: BlockId(1),
            to_index: 0,
        }],
    )
    .unwrap();
    assert_eq!(children(&doc, 0), vec![1]);
    assert_eq!(children(&doc, 1), vec![2, 3]);
    assert_eq!(doc.validate(), Ok(()));

    // Reorder within container 1: move position 0 to position 1 (after removal, len is 1).
    hist.commit(
        &mut doc,
        vec![Op::MoveChild {
            from_parent: BlockId(1),
            from_index: 0,
            to_parent: BlockId(1),
            to_index: 1,
        }],
    )
    .unwrap();
    assert_eq!(children(&doc, 1), vec![3, 2]);
}

#[test]
fn move_that_would_create_a_cycle_is_rejected_and_leaves_doc_unchanged() {
    let mut doc = base();
    let before = doc.clone();
    let mut hist = History::new();
    // Moving container 1 into itself.
    let err = hist
        .commit(
            &mut doc,
            vec![Op::MoveChild {
                from_parent: BlockId(0),
                from_index: 0,
                to_parent: BlockId(1),
                to_index: 0,
            }],
        )
        .unwrap_err();
    assert!(matches!(err, EditError::WouldCreateCycle { .. }));
    assert_eq!(doc, before);
    assert_eq!(hist.undo_len(), 0);
}

#[test]
fn insert_child_that_would_create_a_cycle_is_rejected() {
    let mut doc = base();
    let err = apply(
        &mut doc,
        &Op::InsertChild {
            parent: BlockId(3),
            index: 0,
            child: BlockId(0),
        },
    );
    // Block 3 is text, so this is NotAContainer rather than a cycle. Use a container instead.
    assert_eq!(err, Err(EditError::NotAContainer(BlockId(3))));

    let err = apply(
        &mut doc,
        &Op::InsertChild {
            parent: BlockId(1),
            index: 0,
            child: BlockId(0),
        },
    );
    assert!(matches!(err, Err(EditError::WouldCreateCycle { .. })));
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn set_block_that_would_create_a_cycle_is_rejected() {
    let mut doc = base();
    let err = apply(
        &mut doc,
        &Op::SetBlock {
            id: BlockId(3),
            block: container(vec![BlockId(1)]),
        },
    );
    assert!(matches!(err, Err(EditError::WouldCreateCycle { .. })));
}

#[test]
fn set_block_replaces_contents_and_inverse_restores_them() {
    let mut doc = base();
    let original = doc.clone();
    let inverse = apply(
        &mut doc,
        &Op::SetBlock {
            id: BlockId(2),
            block: Block::Text {
                content: "TWO".into(),
                style: TextStyle {
                    bold: true,
                    italic: false,
                    heading_level: None,
                },
            },
        },
    )
    .unwrap();
    assert_ne!(doc, original);
    apply(&mut doc, &inverse).unwrap();
    assert_eq!(doc, original);
}

#[test]
fn remove_block_requires_no_references() {
    let mut doc = base();
    let err = apply(&mut doc, &Op::RemoveBlock { id: BlockId(3) });
    assert_eq!(
        err,
        Err(EditError::StillReferenced {
            id: BlockId(3),
            parent: BlockId(1)
        })
    );
    // Remove the reference first, then the block.
    apply(
        &mut doc,
        &Op::RemoveChild {
            parent: BlockId(1),
            index: 0,
        },
    )
    .unwrap();
    apply(&mut doc, &Op::RemoveBlock { id: BlockId(3) }).unwrap();
    assert!(doc.get(BlockId(3)).is_none());
    assert_eq!(doc.validate(), Ok(()));
}

#[test]
fn root_cannot_be_removed() {
    let mut doc = base();
    assert_eq!(
        apply(&mut doc, &Op::RemoveBlock { id: BlockId(0) }),
        Err(EditError::IsRoot(BlockId(0)))
    );
}

#[test]
fn add_block_rejects_existing_id_and_dangling_children() {
    let mut doc = base();
    assert_eq!(
        apply(
            &mut doc,
            &Op::AddBlock {
                id: BlockId(2),
                block: text("x")
            }
        ),
        Err(EditError::BlockExists(BlockId(2)))
    );
    assert_eq!(
        apply(
            &mut doc,
            &Op::AddBlock {
                id: BlockId(9),
                block: container(vec![BlockId(77)])
            }
        ),
        Err(EditError::DanglingChild(BlockId(77)))
    );
}

#[test]
fn index_and_container_errors() {
    let mut doc = base();
    assert!(matches!(
        apply(
            &mut doc,
            &Op::InsertChild {
                parent: BlockId(0),
                index: 5,
                child: BlockId(3)
            }
        ),
        Err(EditError::IndexOutOfBounds { .. })
    ));
    assert!(matches!(
        apply(
            &mut doc,
            &Op::RemoveChild {
                parent: BlockId(1),
                index: 1
            }
        ),
        Err(EditError::IndexOutOfBounds { .. })
    ));
    assert_eq!(
        apply(
            &mut doc,
            &Op::RemoveChild {
                parent: BlockId(42),
                index: 0
            }
        ),
        Err(EditError::NoSuchBlock(BlockId(42)))
    );
}

#[test]
fn failed_transaction_rolls_back_earlier_operations() {
    let mut doc = base();
    let before = doc.clone();
    let mut hist = History::new();
    let result = hist.commit(
        &mut doc,
        vec![
            Op::AddBlock {
                id: BlockId(10),
                block: text("kept?"),
            },
            Op::InsertChild {
                parent: BlockId(0),
                index: 0,
                child: BlockId(10),
            },
            // Fails: index 99 is out of range.
            Op::RemoveChild {
                parent: BlockId(0),
                index: 99,
            },
        ],
    );
    assert!(result.is_err());
    assert_eq!(doc, before, "partial effects must be rolled back");
    assert_eq!(hist.undo_len(), 0);
}

#[test]
fn undo_and_redo_restore_each_state() {
    let mut doc = base();
    let initial = doc.clone();
    let mut hist = History::new();

    hist.commit(
        &mut doc,
        vec![Op::MoveChild {
            from_parent: BlockId(0),
            from_index: 1,
            to_parent: BlockId(1),
            to_index: 1,
        }],
    )
    .unwrap();
    let after_move = doc.clone();

    hist.commit(
        &mut doc,
        vec![Op::SetBlock {
            id: BlockId(3),
            block: text("changed"),
        }],
    )
    .unwrap();
    let after_set = doc.clone();

    assert!(hist.undo(&mut doc).unwrap());
    assert_eq!(doc, after_move);
    assert!(hist.undo(&mut doc).unwrap());
    assert_eq!(doc, initial);
    assert!(!hist.undo(&mut doc).unwrap());

    assert!(hist.redo(&mut doc).unwrap());
    assert_eq!(doc, after_move);
    assert!(hist.redo(&mut doc).unwrap());
    assert_eq!(doc, after_set);
    assert!(!hist.redo(&mut doc).unwrap());
}

#[test]
fn new_commit_clears_redo() {
    let mut doc = base();
    let mut hist = History::new();
    hist.commit(
        &mut doc,
        vec![Op::RemoveChild {
            parent: BlockId(0),
            index: 0,
        }],
    )
    .unwrap();
    hist.undo(&mut doc).unwrap();
    assert_eq!(hist.redo_len(), 1);
    hist.commit(
        &mut doc,
        vec![Op::RemoveChild {
            parent: BlockId(0),
            index: 1,
        }],
    )
    .unwrap();
    assert_eq!(hist.redo_len(), 0);
}

#[test]
fn every_error_has_a_message() {
    let samples = [
        EditError::NoSuchBlock(BlockId(1)),
        EditError::BlockExists(BlockId(1)),
        EditError::NotAContainer(BlockId(1)),
        EditError::IndexOutOfBounds {
            parent: BlockId(1),
            index: 2,
            len: 1,
        },
        EditError::WouldCreateCycle {
            parent: BlockId(1),
            child: BlockId(2),
        },
        EditError::StillReferenced {
            id: BlockId(1),
            parent: BlockId(2),
        },
        EditError::IsRoot(BlockId(1)),
        EditError::DanglingChild(BlockId(1)),
    ];
    for e in samples {
        assert!(!e.to_string().is_empty());
    }
}

// Randomized: every accepted edit keeps the document valid, every rejected
// edit leaves it unchanged, and undoing everything returns to the start.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_op(rng: &mut Rng, doc: &Document, next_id: &mut u64) -> Op {
    let ids: Vec<BlockId> = doc.blocks.keys().copied().collect();
    let pick = |rng: &mut Rng| ids[rng.below(ids.len())];
    let containers: Vec<BlockId> = ids
        .iter()
        .copied()
        .filter(|id| matches!(doc.get(*id), Some(Block::Container { .. })))
        .collect();
    if containers.is_empty() {
        *next_id += 1;
        return Op::AddBlock {
            id: BlockId(*next_id),
            block: text("r"),
        };
    }
    match rng.below(6) {
        0 => {
            *next_id += 1;
            Op::AddBlock {
                id: BlockId(*next_id),
                block: text("r"),
            }
        }
        1 => Op::RemoveBlock { id: pick(rng) },
        2 => Op::SetBlock {
            id: pick(rng),
            block: if rng.below(2) == 0 {
                text("s")
            } else {
                container(vec![pick(rng)])
            },
        },
        3 => {
            let parent = containers[rng.below(containers.len())];
            Op::InsertChild {
                parent,
                index: rng.below(3),
                child: pick(rng),
            }
        }
        4 => {
            let parent = containers[rng.below(containers.len())];
            Op::RemoveChild {
                parent,
                index: rng.below(3),
            }
        }
        _ => Op::MoveChild {
            from_parent: containers[rng.below(containers.len())],
            from_index: rng.below(3),
            to_parent: containers[rng.below(containers.len())],
            to_index: rng.below(3),
        },
    }
}

#[test]
fn random_edits_keep_documents_valid_and_undo_is_exact() {
    let mut accepted_total = 0;
    for seed in 0..200u64 {
        let mut rng = Rng(seed);
        let mut doc = base();
        let initial = doc.clone();
        let mut hist = History::new();
        let mut next_id = 100;
        let mut states = vec![doc.clone()];

        for _ in 0..40 {
            let op = random_op(&mut rng, &doc, &mut next_id);
            let before = doc.clone();
            match hist.commit(&mut doc, vec![op.clone()]) {
                Ok(()) => {
                    accepted_total += 1;
                    assert_eq!(
                        doc.validate(),
                        Ok(()),
                        "seed {seed}: accepted {op:?} left an invalid document"
                    );
                    states.push(doc.clone());
                }
                Err(_) => {
                    assert_eq!(
                        doc, before,
                        "seed {seed}: rejected {op:?} changed the document"
                    );
                }
            }
        }

        // Undo everything, checking each intermediate state on the way back.
        let mut expected = states.len();
        while hist.undo(&mut doc).unwrap() {
            expected -= 1;
            assert_eq!(doc, states[expected - 1], "seed {seed}: undo mismatch");
        }
        assert_eq!(
            doc, initial,
            "seed {seed}: full undo did not restore the start"
        );

        // Redo everything.
        while hist.redo(&mut doc).unwrap() {}
        assert_eq!(
            doc,
            *states.last().unwrap(),
            "seed {seed}: full redo mismatch"
        );
    }
    assert!(
        accepted_total > 500,
        "too few edits were accepted: {accepted_total}"
    );
}
