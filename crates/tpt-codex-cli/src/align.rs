// SPDX-License-Identifier: MIT OR Apache-2.0
//! Lines up the blocks of two documents with a base, so they can be merged.
//!
//! A document read from Markdown numbers its blocks by position, so a file that gains a paragraph
//! has a different id on every later block. The merge in `tpt-codex-core` compares blocks by id,
//! so before merging, each side's blocks are given the id of the base block they correspond to.
//! Blocks with no counterpart get fresh ids, which are different on the two sides.

use std::collections::{BTreeMap, BTreeSet};

use tpt_codex_core::{Block, BlockId, Document};

/// Blocks that match with a score below this are treated as different blocks.
const MIN_SCORE: f64 = 0.4;

/// How alike two blocks are, from 0 (different) to 1 (the same).
fn similarity(base: &Document, a: BlockId, other: &Document, b: BlockId) -> f64 {
    let (Some(x), Some(y)) = (base.get(a), other.get(b)) else {
        return 0.0;
    };
    // A footnote only matches the footnote with its label.
    match (base.footnotes.get(&a), other.footnotes.get(&b)) {
        (Some(l), Some(m)) => return if l == m { 1.0 } else { 0.0 },
        (None, None) => {}
        _ => return 0.0,
    }
    match (x, y) {
        (
            Block::Text {
                content: c,
                style: s,
            },
            Block::Text {
                content: d,
                style: t,
            },
        ) => {
            if s.heading_level != t.heading_level {
                return 0.0;
            }
            overlap(c, d)
        }
        (
            Block::Code {
                language: l,
                code: c,
            },
            Block::Code {
                language: m,
                code: d,
            },
        ) => {
            if l != m {
                0.0
            } else {
                overlap(c, d).max(0.5)
            }
        }
        (Block::Math { source: c }, Block::Math { source: d }) => overlap(c, d).max(0.5),
        (Block::Grid { sheet_id: s, .. }, Block::Grid { sheet_id: t, .. }) => {
            if s == t {
                1.0
            } else {
                0.0
            }
        }
        (Block::Media { source: s, .. }, Block::Media { source: t, .. }) => {
            if s == t {
                1.0
            } else {
                0.0
            }
        }
        (Block::Query(p), Block::Query(q)) => {
            if p == q {
                1.0
            } else {
                0.0
            }
        }
        (Block::ThematicBreak, Block::ThematicBreak) => 0.5,
        (
            Block::Container {
                layout_mode: p,
                children: c,
            },
            Block::Container {
                layout_mode: q,
                children: d,
            },
        ) => {
            if p != q {
                0.0
            } else {
                child_overlap(base, c, other, d)
            }
        }
        (Block::List { items: c, .. }, Block::List { items: d, .. }) => {
            child_overlap(base, c, other, d)
        }
        _ => 0.0,
    }
}

/// The share of words two texts have in common, as a fraction of the longer one.
fn overlap(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let x: BTreeSet<&str> = a.split_whitespace().collect();
    let y: BTreeSet<&str> = b.split_whitespace().collect();
    let longest = x.len().max(y.len());
    if longest == 0 {
        return 0.0;
    }
    x.intersection(&y).count() as f64 / longest as f64
}

/// How alike two child lists are, by whether their first children are alike. A container with
/// no children is as good a match as any other empty one.
fn child_overlap(base: &Document, a: &[BlockId], other: &Document, b: &[BlockId]) -> f64 {
    match (a.first(), b.first()) {
        (None, None) => 0.5,
        (Some(&x), Some(&y)) => similarity(base, x, other, y).max(0.45),
        _ => 0.45,
    }
}

fn children(block: &Block) -> Option<&[BlockId]> {
    match block {
        Block::Container { children, .. } => Some(children),
        Block::List { items, .. } => Some(items),
        _ => None,
    }
}

/// Matches two child lists in order, so that the total similarity is as high as possible.
fn match_lists(
    base: &Document,
    a: &[BlockId],
    other: &Document,
    b: &[BlockId],
    out: &mut BTreeMap<BlockId, BlockId>,
) {
    let (n, m) = (a.len(), b.len());
    let score = |i: usize, j: usize| {
        let s = similarity(base, a[i], other, b[j]);
        if s >= MIN_SCORE { s } else { 0.0 }
    };
    // best[i][j]: the best total for a[i..] against b[j..].
    let mut best = vec![vec![0.0f64; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let matched = match score(i, j) {
                s if s > 0.0 => s + best[i + 1][j + 1],
                _ => 0.0,
            };
            best[i][j] = matched.max(best[i + 1][j]).max(best[i][j + 1]);
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        let s = score(i, j);
        if s > 0.0 && (s + best[i + 1][j + 1] - best[i][j]).abs() < 1e-9 {
            if !out.values().any(|&used| used == a[i]) && !out.contains_key(&b[j]) {
                out.insert(b[j], a[i]);
                if let (Some(x), Some(y)) = (base.get(a[i]), other.get(b[j]))
                    && let (Some(p), Some(q)) = (children(x), children(y))
                {
                    match_lists(base, p, other, q, out);
                }
            }
            i += 1;
            j += 1;
        } else if best[i + 1][j] >= best[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
}

/// A copy of `other` whose blocks have the ids of the matching blocks of `base`. Blocks with no
/// match get ids from `next_id`, which is advanced, so ids given to different documents differ.
pub fn align(base: &Document, other: &Document, next_id: &mut u64) -> Document {
    let mut map: BTreeMap<BlockId, BlockId> = BTreeMap::new();
    map.insert(other.root, base.root);
    if let (Some(x), Some(y)) = (base.get(base.root), other.get(other.root))
        && let (Some(p), Some(q)) = (children(x), children(y))
    {
        match_lists(base, p, other, q, &mut map);
    }
    for &id in other.blocks.keys() {
        map.entry(id).or_insert_with(|| {
            let fresh = BlockId(*next_id);
            *next_id += 1;
            fresh
        });
    }
    relabel(other, &map)
}

fn relabel(doc: &Document, map: &BTreeMap<BlockId, BlockId>) -> Document {
    let to = |id: &BlockId| map.get(id).copied().unwrap_or(*id);
    let mut out = doc.clone();
    out.root = to(&doc.root);
    out.blocks = doc
        .blocks
        .iter()
        .map(|(id, block)| {
            let mut block = block.clone();
            match &mut block {
                Block::Container { children, .. } => children.iter_mut().for_each(|c| *c = to(c)),
                Block::List { items, .. } => items.iter_mut().for_each(|c| *c = to(c)),
                _ => {}
            }
            (to(id), block)
        })
        .collect();
    out.frames = doc.frames.iter().map(|(k, v)| (to(k), *v)).collect();
    out.canvases = doc.canvases.iter().map(|(k, v)| (to(k), *v)).collect();
    out.footnotes = doc
        .footnotes
        .iter()
        .map(|(k, v)| (to(k), v.clone()))
        .collect();
    out.sections = doc
        .sections
        .iter()
        .map(|(k, v)| (to(k), v.clone()))
        .collect();
    out.figures = doc
        .figures
        .iter()
        .map(|(k, v)| (to(k), v.clone()))
        .collect();
    out.equation_labels = doc
        .equation_labels
        .iter()
        .map(|(k, v)| (to(k), v.clone()))
        .collect();
    out
}

/// The first id that no block of any of the documents has.
pub fn first_free_id(docs: &[&Document]) -> u64 {
    docs.iter()
        .flat_map(|d| d.blocks.keys())
        .map(|id| id.0)
        .max()
        .map_or(1, |m| m + 1)
}
