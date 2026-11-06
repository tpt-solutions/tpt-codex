// SPDX-License-Identifier: MIT OR Apache-2.0
//! The semantic tree: chapters and sections built from the heading outline, each holding the
//! blocks under its heading, and the footnotes and citations those blocks refer to.
//!
//! A section runs from its heading to the next heading of the same or a higher level. A heading
//! of a lower level inside it becomes a child section. Blocks before the first heading form the
//! preamble.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};

use crate::footnotes::reading_order;
use crate::outline::{Outline, OutlineEntry, TocLine, number_string};

/// One section: a heading, the blocks under it, and the sections nested below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Heading level, 1 to 6.
    pub level: u8,
    /// Section number path, such as `[1, 2]` for section 1.2.
    pub number: Vec<u32>,
    /// Heading text.
    pub title: String,
    /// The anchor that cross-references name.
    pub anchor: String,
    /// The heading block.
    pub heading: BlockId,
    /// Blocks under the heading and above any child section, in reading order.
    pub body: Vec<BlockId>,
    /// Footnote labels referenced in `body`, in order of first reference.
    pub footnotes: Vec<String>,
    /// Citation keys cited in `body`, in order of first citation.
    pub citations: Vec<String>,
    /// Sections nested directly below this one.
    pub children: Vec<Section>,
}

impl Section {
    /// Footnote labels referenced in this section or any section below it.
    pub fn reachable_footnotes(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect(&mut out, |s| &s.footnotes);
        out
    }

    /// Citation keys cited in this section or any section below it.
    pub fn reachable_citations(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect(&mut out, |s| &s.citations);
        out
    }

    fn collect(&self, out: &mut Vec<String>, field: fn(&Section) -> &Vec<String>) {
        push_unique(out, field(self));
        for child in &self.children {
            child.collect(out, field);
        }
    }
}

/// The document as chapters and sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticTree {
    /// Blocks before the first heading, in reading order.
    pub preamble: Vec<BlockId>,
    /// The top-level sections, in order.
    pub sections: Vec<Section>,
    /// Every footnote label referenced in the document, in order of first reference.
    pub footnotes: Vec<String>,
    /// Every citation key cited in the document, in order of first citation.
    pub citations: Vec<String>,
}

impl SemanticTree {
    /// Table of contents lines for sections down to `max_depth` levels, in reading order.
    /// The same lines as [`Outline::toc`], nested sections included.
    pub fn toc(&self, max_depth: u8) -> Vec<TocLine> {
        let mut out = Vec::new();
        for section in &self.sections {
            push_toc(section, max_depth, &mut out);
        }
        out
    }

    /// The section with this anchor, at any depth.
    pub fn find(&self, anchor: &str) -> Option<&Section> {
        fn walk<'a>(sections: &'a [Section], anchor: &str) -> Option<&'a Section> {
            for s in sections {
                if s.anchor == anchor {
                    return Some(s);
                }
                if let Some(found) = walk(&s.children, anchor) {
                    return Some(found);
                }
            }
            None
        }
        walk(&self.sections, anchor)
    }
}

/// Builds the semantic tree of `doc`.
pub fn semantic_tree(doc: &Document) -> SemanticTree {
    let outline = Outline::of(doc);
    let headings: BTreeMap<BlockId, &OutlineEntry> =
        outline.entries.iter().map(|e| (e.block, e)).collect();

    let mut tree = SemanticTree::default();
    let mut roots: Vec<Section> = Vec::new();
    let mut open: Vec<Section> = Vec::new();

    for id in reading_order(doc) {
        if let Some(entry) = headings.get(&id) {
            while open.last().is_some_and(|top| top.level >= entry.level) {
                if let Some(done) = open.pop() {
                    attach(done, &mut open, &mut roots);
                }
            }
            open.push(Section {
                level: entry.level,
                number: entry.number.clone(),
                title: entry.title.clone(),
                anchor: entry.anchor.clone(),
                heading: id,
                body: Vec::new(),
                footnotes: Vec::new(),
                citations: Vec::new(),
                children: Vec::new(),
            });
            continue;
        }
        if matches!(doc.get(id), None | Some(Block::Container { .. })) {
            continue;
        }
        let (footnotes, citations) = references_in(doc, id);
        push_unique(&mut tree.footnotes, &footnotes);
        push_unique(&mut tree.citations, &citations);
        match open.last_mut() {
            Some(top) => {
                top.body.push(id);
                push_unique(&mut top.footnotes, &footnotes);
                push_unique(&mut top.citations, &citations);
            }
            None => tree.preamble.push(id),
        }
    }
    while let Some(done) = open.pop() {
        attach(done, &mut open, &mut roots);
    }
    tree.sections = roots;
    tree
}

/// Puts a finished section under the open section below it, or at the top level.
fn attach(done: Section, open: &mut [Section], roots: &mut Vec<Section>) {
    match open.last_mut() {
        Some(parent) => parent.children.push(done),
        None => roots.push(done),
    }
}

/// The footnote labels and citation keys a text block refers to, in text order.
fn references_in(doc: &Document, id: BlockId) -> (Vec<String>, Vec<String>) {
    let mut footnotes = Vec::new();
    let mut citations = Vec::new();
    if let Some(Block::Text { style, .. }) = doc.get(id) {
        for run in &style.runs {
            let Some(address) = run.link.as_deref() else {
                continue;
            };
            if let Some(label) = address.strip_prefix('^') {
                push_unique(&mut footnotes, &[String::from(label)]);
            } else if let Some(key) = address.strip_prefix('@') {
                push_unique(&mut citations, &[String::from(key)]);
            }
        }
    }
    (footnotes, citations)
}

/// Appends each item of `items` that is not already in `out`, keeping order.
fn push_unique(out: &mut Vec<String>, items: &[String]) {
    for item in items {
        if !out.contains(item) {
            out.push(item.clone());
        }
    }
}

fn push_toc(section: &Section, max_depth: u8, out: &mut Vec<TocLine>) {
    if section.level <= max_depth {
        out.push(TocLine {
            depth: (section.level - 1) as usize,
            number: number_string(&section.number),
            title: section.title.clone(),
            anchor: section.anchor.clone(),
        });
    }
    for child in &section.children {
        push_toc(child, max_depth, out);
    }
}
