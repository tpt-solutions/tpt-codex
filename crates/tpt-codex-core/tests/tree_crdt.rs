// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{NodeId, ROOT, TRASH, TreeCrdt, TreeError, TreeOp, key_between};

struct Rng(u64);

impl Rng {
    fn next(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

fn names(t: &TreeCrdt<String>) -> Vec<(String, usize)> {
    t.flatten()
        .into_iter()
        .map(|(n, d)| (t.payload(n).cloned().unwrap_or_default(), d))
        .collect()
}

fn text(s: &str) -> String {
    s.to_string()
}

#[test]
fn key_between_is_strictly_between_its_bounds() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..5000 {
        let a: Vec<u64> = (0..rng.next(4)).map(|_| rng.next(4)).collect();
        // Bounds are keys, which end in a non-zero digit (see `key_between`).
        let mut b: Vec<u64> = (1..=rng.next(3) + 1).map(|_| rng.next(4)).collect();
        if let Some(last) = b.last_mut() {
            *last = (*last).max(1);
        }
        if a == b {
            continue;
        }
        if a > b {
            // Swapping could put a zero-ended key in the upper slot; skip the pair instead.
            continue;
        }
        let k = key_between(&a, Some(&b));
        assert!(a < k && k < b, "{a:?} < {k:?} < {b:?}");
        let open = key_between(&a, None);
        assert!(a < open, "{a:?} < {open:?}");
    }
}

#[test]
fn children_keep_their_positions_and_deletes_hide_nodes() {
    let mut t: TreeCrdt<String> = TreeCrdt::new(1);
    let mut ops = Vec::new();
    ops.extend(t.insert_local(ROOT, 0, text("a")).unwrap());
    ops.extend(t.insert_local(ROOT, 1, text("b")).unwrap());
    ops.extend(t.insert_local(ROOT, 2, text("c")).unwrap());
    assert_eq!(names(&t), [(text("a"), 0), (text("b"), 0), (text("c"), 0)]);

    // Move "c" to the front.
    let c = t.children(ROOT)[2];
    t.move_local(c, ROOT, 0).unwrap();
    assert_eq!(names(&t), [(text("c"), 0), (text("a"), 0), (text("b"), 0)]);

    // Nest "a" under "c".
    let a = t.children(ROOT)[1];
    t.move_local(a, c, 0).unwrap();
    assert_eq!(names(&t), [(text("c"), 0), (text("a"), 1), (text("b"), 0)]);

    // Deleting "c" hides its subtree; the nodes stay in the trash.
    t.delete_local(c).unwrap();
    assert_eq!(names(&t), [(text("b"), 0)]);
    assert_eq!(t.parent_of(c), Some(TRASH));
    assert!(t.is_acyclic());

    // Moving "c" back restores its subtree.
    t.move_local(c, ROOT, 1).unwrap();
    assert_eq!(names(&t), [(text("b"), 0), (text("c"), 0), (text("a"), 1)]);
}

#[test]
fn local_errors_are_reported_and_change_nothing() {
    let mut t: TreeCrdt<String> = TreeCrdt::new(1);
    let node = t.insert_local(ROOT, 0, text("x")).unwrap()[1].clone();
    let TreeOp::Move { node: n, .. } = node else {
        panic!()
    };
    let before = names(&t);
    assert_eq!(
        t.insert_local(ROOT, 5, text("y")).err(),
        Some(TreeError::IndexOutOfBounds { index: 5, len: 1 })
    );
    assert_eq!(
        t.move_local(ROOT, ROOT, 0).err(),
        Some(TreeError::Fixed(ROOT))
    );
    assert_eq!(t.delete_local(TRASH).err(), Some(TreeError::Fixed(TRASH)));
    assert_eq!(
        t.move_local(NodeId(999), ROOT, 0).err(),
        Some(TreeError::NotInTree(NodeId(999)))
    );
    assert_eq!(names(&t), before);
    // A node cannot be moved under itself.
    assert_eq!(
        t.move_local(n, n, 0).err(),
        Some(TreeError::WouldCreateCycle(n))
    );
}

#[test]
fn concurrent_moves_that_would_form_a_cycle_leave_a_tree() {
    let mut r1: TreeCrdt<String> = TreeCrdt::new(1);
    let mut r2: TreeCrdt<String> = TreeCrdt::new(2);
    let mut setup = Vec::new();
    setup.extend(r1.insert_local(ROOT, 0, text("A")).unwrap());
    setup.extend(r1.insert_local(ROOT, 1, text("B")).unwrap());
    for op in &setup {
        r2.apply(op);
    }
    let a = r1.children(ROOT)[0];
    let b = r1.children(ROOT)[1];

    // Concurrently: replica 1 puts A under B, replica 2 puts B under A.
    let op1 = r1.move_local(a, b, 0).unwrap();
    let op2 = r2.move_local(b, a, 0).unwrap();
    r1.apply(&op2);
    r2.apply(&op1);

    assert!(r1.is_acyclic() && r2.is_acyclic());
    assert_eq!(names(&r1), names(&r2));
    // Exactly one of the two nodes is at the root now.
    assert_eq!(r1.flatten().iter().filter(|(_, d)| *d == 0).count(), 1);
}

#[test]
fn a_late_move_is_applied_in_stamp_order() {
    // Replica 1 moves a node twice. Replica 2 receives the second move first.
    let mut r1: TreeCrdt<String> = TreeCrdt::new(1);
    let mut r2: TreeCrdt<String> = TreeCrdt::new(2);
    let mut setup = Vec::new();
    setup.extend(r1.insert_local(ROOT, 0, text("x")).unwrap());
    setup.extend(r1.insert_local(ROOT, 1, text("y")).unwrap());
    for op in &setup {
        r2.apply(op);
    }
    let x = r1.children(ROOT)[0];
    let y = r1.children(ROOT)[1];
    let first = r1.move_local(x, y, 0).unwrap();
    let second = r1.move_local(x, ROOT, 0).unwrap();

    r2.apply(&second);
    r2.apply(&first);
    assert_eq!(names(&r1), names(&r2));
    assert_eq!(r2.parent_of(x), Some(ROOT), "the later move wins");
}

#[test]
fn updates_are_last_writer_wins() {
    let mut r1: TreeCrdt<String> = TreeCrdt::new(1);
    let mut r2: TreeCrdt<String> = TreeCrdt::new(2);
    let ops = r1.insert_local(ROOT, 0, text("v0")).unwrap();
    for op in &ops {
        r2.apply(op);
    }
    let n = r1.children(ROOT)[0];
    let u1 = r1.update_local(n, text("from one"));
    let u2 = r2.update_local(n, text("from two"));
    r1.apply(&u2);
    r2.apply(&u1);
    assert_eq!(r1.payload(n), r2.payload(n));
}

#[test]
fn applying_an_operation_twice_changes_nothing() {
    let mut r1: TreeCrdt<String> = TreeCrdt::new(1);
    let mut r2: TreeCrdt<String> = TreeCrdt::new(2);
    let ops = r1.insert_local(ROOT, 0, text("once")).unwrap();
    for op in ops.iter().chain(ops.iter()) {
        r2.apply(op);
    }
    assert_eq!(names(&r2), [(text("once"), 0)]);
}

#[test]
fn three_replicas_with_random_edits_and_reordered_delivery_converge() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for round in 0..150u64 {
        let n = 3usize;
        let mut replicas: Vec<TreeCrdt<String>> =
            (0..n).map(|i| TreeCrdt::new(i as u64 + 1)).collect();
        // links[from][to]: operations from `from` not yet delivered to `to`.
        let mut links: Vec<Vec<Vec<TreeOp<String>>>> = vec![vec![Vec::new(); n]; n];

        for step in 0..(30 + round % 25) {
            let r = rng.next(n as u64) as usize;
            let nodes: Vec<NodeId> = replicas[r].flatten().into_iter().map(|(x, _)| x).collect();
            let roll = rng.next(10);
            let produced: Vec<TreeOp<String>> = if nodes.is_empty() || roll < 4 {
                let parent = if nodes.is_empty() || rng.next(3) == 0 {
                    ROOT
                } else {
                    nodes[rng.next(nodes.len() as u64) as usize]
                };
                let index = rng.next(replicas[r].children(parent).len() as u64 + 1) as usize;
                replicas[r]
                    .insert_local(parent, index, format!("n{round}-{step}"))
                    .unwrap_or_default()
            } else if roll < 7 {
                let node = nodes[rng.next(nodes.len() as u64) as usize];
                let parent = if rng.next(4) == 0 {
                    ROOT
                } else {
                    nodes[rng.next(nodes.len() as u64) as usize]
                };
                let index = rng.next(replicas[r].children(parent).len() as u64 + 1) as usize;
                // Some moves are refused locally (cycles, bad index). Refused moves send nothing.
                replicas[r]
                    .move_local(node, parent, index)
                    .map(|op| vec![op])
                    .unwrap_or_default()
            } else if roll < 8 {
                let node = nodes[rng.next(nodes.len() as u64) as usize];
                replicas[r]
                    .delete_local(node)
                    .map(|op| vec![op])
                    .unwrap_or_default()
            } else {
                let node = nodes[rng.next(nodes.len() as u64) as usize];
                vec![replicas[r].update_local(node, format!("u{round}-{step}"))]
            };
            for op in produced {
                for (to, link) in links[r].iter_mut().enumerate() {
                    if to != r {
                        link.push(op.clone());
                    }
                }
            }
            // Deliver a few operations from random links, out of order.
            for _ in 0..rng.next(4) {
                let from = rng.next(n as u64) as usize;
                let to = rng.next(n as u64) as usize;
                if from != to && !links[from][to].is_empty() {
                    let i = rng.next(links[from][to].len() as u64) as usize;
                    let op = links[from][to].remove(i);
                    replicas[to].apply(&op);
                }
            }
            for r in &replicas {
                assert!(
                    r.is_acyclic(),
                    "round {round} step {step}: a cycle appeared"
                );
            }
        }

        // Deliver everything that is left, in random order, then compare.
        for from in links.iter_mut() {
            for (to, link) in from.iter_mut().enumerate() {
                while !link.is_empty() {
                    let i = rng.next(link.len() as u64) as usize;
                    let op = link.remove(i);
                    replicas[to].apply(&op);
                }
            }
        }
        let first = names(&replicas[0]);
        for r in &replicas[1..] {
            assert_eq!(names(r), first, "round {round}: replicas diverged");
            assert!(r.is_acyclic());
        }
    }
}

#[test]
fn a_replicated_tree_projects_to_a_valid_document_that_matches_on_every_replica() {
    use tpt_codex_core::{Block, BlockId, LayoutMode, TextStyle, project_document};

    fn text(s: &str) -> Block {
        Block::Text {
            content: s.into(),
            style: TextStyle::default(),
        }
    }
    let section = Block::Container {
        layout_mode: LayoutMode::Flow,
        children: vec![],
    };

    let mut r1: TreeCrdt<Block> = TreeCrdt::new(1);
    let mut r2: TreeCrdt<Block> = TreeCrdt::new(2);
    let mut ops = Vec::new();
    ops.extend(r1.insert_local(ROOT, 0, text("intro")).unwrap());
    ops.extend(r1.insert_local(ROOT, 1, section.clone()).unwrap());
    let sec = r1.children(ROOT)[1];
    ops.extend(r1.insert_local(sec, 0, text("inside")).unwrap());
    for op in &ops {
        r2.apply(op);
    }

    // Concurrently: replica 2 adds a paragraph to the section; replica 1 deletes the intro.
    let inner_op = r2.insert_local(sec, 1, text("second")).unwrap();
    let intro = r1.children(ROOT)[0];
    let del = r1.delete_local(intro).unwrap();
    for op in &inner_op {
        r1.apply(op);
    }
    r2.apply(&del);

    let d1 = project_document(&r1);
    let d2 = project_document(&r2);
    d1.validate().unwrap();
    assert_eq!(d1, d2, "replicas must project the same document");

    let Some(Block::Container { children, .. }) = d1.get(d1.root) else {
        panic!("root");
    };
    assert_eq!(
        children.as_slice(),
        [BlockId(sec.0)],
        "the intro is deleted"
    );
    let Some(Block::Container {
        children: inner, ..
    }) = d1.get(BlockId(sec.0))
    else {
        panic!("section");
    };
    assert_eq!(inner.len(), 2);
}
