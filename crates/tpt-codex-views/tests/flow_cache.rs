// SPDX-License-Identifier: MIT OR Apache-2.0
//! The cached flow layout matches the plain one, and re-measures only what changed.

use std::cell::Cell;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{
    FlowCache, FlowOptions, MonospaceMeasure, TextMeasure, layout_flow, layout_flow_cached,
};

/// Counts how many times the width is measured.
struct Counting {
    inner: MonospaceMeasure,
    calls: Cell<usize>,
}

impl TextMeasure for Counting {
    fn width(&self, text: &str, style: &TextStyle) -> f64 {
        self.calls.set(self.calls.get() + 1);
        self.inner.width(text, style)
    }
    fn line_height(&self, style: &TextStyle) -> f64 {
        self.inner.line_height(style)
    }
}

fn doc(paragraphs: &[&str]) -> Document {
    let children: Vec<BlockId> = (1..=paragraphs.len() as u64).map(BlockId).collect();
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children,
        },
    );
    for (i, text) in paragraphs.iter().enumerate() {
        d.insert(
            BlockId(i as u64 + 1),
            Block::Text {
                content: (*text).into(),
                style: TextStyle::default(),
            },
        );
    }
    d
}

fn options() -> FlowOptions {
    FlowOptions {
        width: 120.0,
        block_spacing: 4.0,
        box_height: 10.0,
        row_height: 10.0,
    }
}

fn measure() -> Counting {
    Counting {
        inner: MonospaceMeasure {
            char_width: 6.0,
            line_height: 14.0,
        },
        calls: Cell::new(0),
    }
}

#[test]
fn the_cached_layout_is_the_same_as_the_plain_one() {
    let d = doc(&["one two three four five", "short", "six seven eight nine"]);
    let m = measure();
    let plain = layout_flow(&d, &m, options()).unwrap();
    let cached = layout_flow_cached(&d, &m, options(), &mut FlowCache::new()).unwrap();
    assert_eq!(plain, cached);
}

#[test]
fn an_unchanged_document_is_not_measured_again() {
    let d = doc(&["one two three four five", "short"]);
    let m = measure();
    let mut cache = FlowCache::new();
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    let first = m.calls.get();
    assert!(first > 0);
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    assert_eq!(m.calls.get(), first, "a second layout re-measured text");
}

#[test]
fn only_the_edited_block_is_measured_again() {
    let mut d = doc(&["one two three four five", "short", "last"]);
    let m = measure();
    let mut cache = FlowCache::new();
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();

    d.insert(
        BlockId(2),
        Block::Text {
            content: "shorter now".into(),
            style: TextStyle::default(),
        },
    );
    let before = m.calls.get();
    let cached = layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    let edited_cost = m.calls.get() - before;

    let plain_cost = {
        let fresh = measure();
        layout_flow(&d, &fresh, options()).unwrap();
        fresh.calls.get()
    };
    assert!(
        edited_cost > 0 && edited_cost < plain_cost,
        "{edited_cost} vs {plain_cost}"
    );
    assert_eq!(cached, layout_flow(&d, &m, options()).unwrap());
}

#[test]
fn a_width_change_invalidates_the_cache() {
    let d = doc(&["one two three four five"]);
    let m = measure();
    let mut cache = FlowCache::new();
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    let before = m.calls.get();
    let narrow = FlowOptions {
        width: 60.0,
        ..options()
    };
    let cached = layout_flow_cached(&d, &m, narrow, &mut cache).unwrap();
    assert!(m.calls.get() > before);
    assert_eq!(cached, layout_flow(&d, &m, narrow).unwrap());
}

#[test]
fn removed_blocks_leave_the_cache() {
    let mut d = doc(&["a", "b"]);
    let m = measure();
    let mut cache = FlowCache::new();
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    assert_eq!(cache.len(), 2);
    d.blocks.remove(&BlockId(2));
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.retain(|c| *c != BlockId(2));
    }
    layout_flow_cached(&d, &m, options(), &mut cache).unwrap();
    assert_eq!(cache.len(), 1);
}
