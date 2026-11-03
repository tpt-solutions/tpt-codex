// SPDX-License-Identifier: MIT OR Apache-2.0
//! Randomized property tests for the document model.
//!
//! Uses a small deterministic PRNG (splitmix64) instead of a property-testing
//! crate, so the test graph stays free of extra dependencies. Failures print
//! the seed, so any case can be replayed.

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle, ValidationError};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish value in `0..n`. `n` must be non-zero.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const CASES: u64 = 300;

fn container(children: Vec<BlockId>) -> Block {
    Block::Container {
        layout_mode: LayoutMode::Flow,
        children,
    }
}

/// A chain `0 -> 1 -> ... -> n-1` plus random forward-only extra edges.
/// Forward-only edges can never form a cycle.
fn random_dag(rng: &mut Rng, n: usize) -> Document {
    let mut doc = Document::new(BlockId(0), container(vec![]));
    for i in 0..n {
        let mut children = Vec::new();
        if i + 1 < n {
            children.push(BlockId(i as u64 + 1));
        }
        let extra = rng.below(3);
        for _ in 0..extra {
            if i + 1 < n {
                let j = i + 1 + rng.below(n - i - 1);
                let id = BlockId(j as u64);
                if !children.contains(&id) {
                    children.push(id);
                }
            }
        }
        let block = if i + 1 == n && rng.below(2) == 0 {
            Block::Text {
                content: format!("leaf {i}"),
                style: TextStyle::default(),
            }
        } else {
            container(children)
        };
        doc.insert(BlockId(i as u64), block);
    }
    doc
}

#[test]
fn random_forward_dags_are_valid() {
    for seed in 0..CASES {
        let mut rng = Rng(seed);
        let n = 1 + rng.below(40);
        let doc = random_dag(&mut rng, n);
        assert_eq!(doc.validate(), Ok(()), "seed {seed}, n {n}");
    }
}

#[test]
fn any_back_edge_on_a_chain_is_a_cycle() {
    for seed in 0..CASES {
        let mut rng = Rng(seed);
        let n = 2 + rng.below(40);
        let mut doc = random_dag(&mut rng, n);
        // Chain edges i -> i+1 exist for all i < n-1, so a back edge j -> i
        // (i < j) closes a cycle through i..=j.
        let j = 1 + rng.below(n - 1);
        let i = rng.below(j);
        let mut children = match doc.get(BlockId(j as u64)).unwrap() {
            Block::Container { children, .. } => children.clone(),
            _ => Vec::new(),
        };
        children.push(BlockId(i as u64));
        doc.insert(BlockId(j as u64), container(children));
        let result = doc.validate();
        assert!(
            matches!(result, Err(ValidationError::Cycle(_))),
            "seed {seed}, back edge {j} -> {i}, got {result:?}"
        );
    }
}

#[test]
fn removing_a_referenced_block_is_detected() {
    for seed in 0..CASES {
        let mut rng = Rng(seed);
        let n = 2 + rng.below(40);
        let mut doc = random_dag(&mut rng, n);
        let victim = BlockId(1 + rng.below(n - 1) as u64);
        doc.blocks.remove(&victim);
        // The chain always references every node 1..n-1, so the parent of the
        // victim must now report a dangling child.
        let result = doc.validate();
        assert!(
            matches!(result, Err(ValidationError::DanglingChild { child, .. }) if child == victim),
            "seed {seed}, removed {victim:?}, got {result:?}"
        );
    }
}

#[test]
fn serialization_round_trips_and_is_deterministic() {
    for seed in 0..CASES {
        let mut rng = Rng(seed);
        let n = 1 + rng.below(40);
        let doc = random_dag(&mut rng, n);
        let json = serde_json::to_string(&doc).unwrap();
        let again = serde_json::to_string(&doc).unwrap();
        assert_eq!(json, again, "seed {seed}: serialization not deterministic");
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(doc, back, "seed {seed}: round trip changed the document");
    }
}
