// SPDX-License-Identifier: MIT OR Apache-2.0
//! Section boxes: each section's vertical extent on a laid-out flow column.
//!
//! The flow engine lays out blocks; this module reads the positions back into the section tree,
//! so a page or a navigation pane can tell where each chapter starts and ends. A section's extent
//! covers its heading, its body and everything nested below it.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::BlockId;
use tpt_codex_views::FlowLayout;

use crate::semantic::{Section, SemanticTree};

/// A section with its extent on the column, in points from the top of the column.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionBox {
    /// Section number, such as `1.2`.
    pub number: String,
    /// Heading text.
    pub title: String,
    /// The anchor that cross-references name.
    pub anchor: String,
    /// Top and bottom edges, or `None` if none of the section's blocks is on the layout.
    pub extent: Option<(f64, f64)>,
    /// Boxes for the sections nested directly below this one.
    pub children: Vec<SectionBox>,
}

/// The section boxes of `tree` on `layout`, in reading order.
pub fn section_boxes(tree: &SemanticTree, layout: &FlowLayout) -> Vec<SectionBox> {
    let mut extents: BTreeMap<BlockId, (f64, f64)> = BTreeMap::new();
    for item in &layout.items {
        let (top, bottom) = (item.y, item.y + item.height);
        extents
            .entry(item.block)
            .and_modify(|(t, b)| {
                *t = t.min(top);
                *b = b.max(bottom);
            })
            .or_insert((top, bottom));
    }
    tree.sections
        .iter()
        .map(|s| section_box(s, &extents))
        .collect()
}

fn section_box(section: &Section, extents: &BTreeMap<BlockId, (f64, f64)>) -> SectionBox {
    let own = core::iter::once(section.heading)
        .chain(section.body.iter().copied())
        .filter_map(|id| extents.get(&id).copied().map(Some))
        .fold(None, join);
    let children: Vec<SectionBox> = section
        .children
        .iter()
        .map(|c| section_box(c, extents))
        .collect();
    let extent = children.iter().map(|c| c.extent).fold(own, join);
    SectionBox {
        number: crate::outline::number_string(&section.number),
        title: section.title.clone(),
        anchor: section.anchor.clone(),
        extent,
        children,
    }
}

/// The smallest extent that covers both `a` and `b`. A missing side is ignored.
fn join(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some((t1, b1)), Some((t2, b2))) => Some((t1.min(t2), b1.max(b2))),
        (one, None) | (None, one) => one,
    }
}
