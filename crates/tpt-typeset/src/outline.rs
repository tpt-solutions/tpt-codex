// SPDX-License-Identifier: MIT OR Apache-2.0
//! Heading outline: numbering, stable anchors, a table of contents, and anchor lookup.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode};

/// One heading in the document, in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    /// Heading level, 1 to 6.
    pub level: u8,
    /// Heading text.
    pub title: String,
    /// The heading block.
    pub block: BlockId,
    /// Section number, such as `[1, 2]` for section 1.2. See [`number_string`].
    pub number: Vec<u32>,
    /// Unique anchor for cross-references, such as `introduction` or `introduction-2`.
    pub anchor: String,
}

/// The headings of a document, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outline {
    /// Headings in reading order.
    pub entries: Vec<OutlineEntry>,
}

impl Outline {
    /// Builds the outline of `doc`. Headings inside flow containers are included.
    pub fn of(doc: &Document) -> Self {
        let mut raw: Vec<(u8, String, BlockId)> = Vec::new();
        let mut visited = BTreeSet::new();
        collect(doc, doc.root, &mut raw, &mut visited, 0);

        let mut counters = [0u32; 6];
        let mut used_anchors: Vec<String> = Vec::new();
        let mut entries = Vec::with_capacity(raw.len());
        for (level, title, block) in raw {
            let number = next_number(&mut counters, level);
            let base = slugify(&title);
            let anchor = unique_anchor(&base, &mut used_anchors);
            entries.push(OutlineEntry {
                level,
                title,
                block,
                number,
                anchor,
            });
        }
        Self { entries }
    }

    /// Finds the heading with this anchor, as a cross-reference `[#anchor]` would.
    pub fn resolve(&self, anchor: &str) -> Option<&OutlineEntry> {
        self.entries.iter().find(|e| e.anchor == anchor)
    }

    /// Table of contents entries down to `max_depth` levels.
    pub fn toc(&self, max_depth: u8) -> Vec<TocLine> {
        self.entries
            .iter()
            .filter(|e| e.level <= max_depth)
            .map(|e| TocLine {
                depth: (e.level - 1) as usize,
                number: number_string(&e.number),
                title: e.title.clone(),
                anchor: e.anchor.clone(),
            })
            .collect()
    }
}

/// One line of a table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocLine {
    /// Indentation depth, starting at 0 for level 1.
    pub depth: usize,
    /// Section number as text, such as `1.2`.
    pub number: String,
    /// Heading text.
    pub title: String,
    /// Anchor to link to.
    pub anchor: String,
}

/// Formats a section number: `[1, 2]` becomes `"1.2"`.
pub fn number_string(number: &[u32]) -> String {
    let parts: Vec<String> = number.iter().map(|n| alloc::format!("{n}")).collect();
    parts.join(".")
}

/// Advances the counters for a heading at `level` and returns its number path.
///
/// A level that was skipped (for example a level-2 heading with no level-1 before it)
/// is numbered 1, so the path is always complete.
fn next_number(counters: &mut [u32; 6], level: u8) -> Vec<u32> {
    let l = level as usize;
    for c in counters.iter_mut().take(l - 1) {
        if *c == 0 {
            *c = 1;
        }
    }
    counters[l - 1] += 1;
    for c in counters.iter_mut().skip(l) {
        *c = 0;
    }
    counters[..l].to_vec()
}

/// Lower-case ASCII slug: letters and digits kept, every other run becomes one `-`.
/// Falls back to `section` for titles with no letters or digits.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        out.push_str("section");
    }
    out
}

fn unique_anchor(base: &str, used: &mut Vec<String>) -> String {
    let mut candidate = String::from(base);
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = alloc::format!("{base}-{n}");
        n += 1;
    }
    used.push(candidate.clone());
    candidate
}

/// Depth-first walk over flow containers, collecting headings in reading order.
fn collect(
    doc: &Document,
    id: BlockId,
    out: &mut Vec<(u8, String, BlockId)>,
    visited: &mut BTreeSet<BlockId>,
    depth: usize,
) {
    // Guard against a malformed document recursing forever. Validated documents are acyclic.
    if depth > 64 {
        return;
    }
    let Some(Block::Container {
        layout_mode: LayoutMode::Flow,
        children,
    }) = doc.get(id)
    else {
        return;
    };
    for &child in children {
        // A block can be referenced from more than one container. List it once.
        if !visited.insert(child) {
            continue;
        }
        match doc.get(child) {
            Some(Block::Text { content, style }) => {
                if let Some(level) = style.heading_level {
                    if (1..=6).contains(&level) {
                        out.push((level, content.clone(), child));
                    }
                }
            }
            Some(Block::Container {
                layout_mode: LayoutMode::Flow,
                ..
            }) => collect(doc, child, out, visited, depth + 1),
            _ => {}
        }
    }
}
