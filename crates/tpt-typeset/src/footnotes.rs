// SPDX-License-Identifier: MIT OR Apache-2.0
//! Footnotes: references in the text, their definitions, and numbering.
//!
//! A reference is a link whose address starts with `^`, so `[^note]` (bare) or `[the note](^note)`
//! refers to the footnote labelled `note`. A definition is a text block recorded in
//! `Document::footnotes`. Footnotes are numbered in the order they are first referenced, reading
//! the document from the top, so each reference gets the number of its footnote.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};
use tpt_codex_views::{Note, TextMeasure, layout_note};

/// One footnote reference found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteRef {
    /// The text block that holds the reference.
    pub block: BlockId,
    /// The label it names.
    pub label: String,
    /// The text block that holds the footnote, or `None` if no footnote has this label.
    pub definition: Option<BlockId>,
    /// The footnote's number, from 1 in order of first reference. `None` if there is no footnote.
    pub number: Option<usize>,
}

/// Block ids in reading order: the root's children from the top, each container's children in
/// turn. A block reached twice is listed once.
pub fn reading_order(doc: &Document) -> Vec<BlockId> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = alloc::vec![doc.root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        out.push(id);
        let children: &[BlockId] = match doc.get(id) {
            Some(Block::Container { children, .. }) => children,
            Some(Block::List { items, .. }) => items,
            _ => &[],
        };
        // Push in reverse so the first child is visited first.
        stack.extend(children.iter().rev().copied());
    }
    out
}

/// The footnote numbers, by label, in order of first reference. Only labels with a definition
/// are numbered.
pub fn footnote_numbers(doc: &Document) -> BTreeMap<String, usize> {
    let mut numbers = BTreeMap::new();
    for id in reading_order(doc) {
        let Some(Block::Text { style, .. }) = doc.get(id) else {
            continue;
        };
        for run in &style.runs {
            let Some(label) = run.link.as_deref().and_then(|a| a.strip_prefix('^')) else {
                continue;
            };
            // Only a footnote that exists gets a number.
            if doc.footnotes.values().any(|l| l == label) {
                let next = numbers.len() + 1;
                numbers.entry(String::from(label)).or_insert(next);
            }
        }
    }
    numbers
}

/// Every footnote reference in the document, in reading order.
pub fn check_footnotes(doc: &Document) -> Vec<FootnoteRef> {
    let numbers = footnote_numbers(doc);
    let mut out = Vec::new();
    for id in reading_order(doc) {
        let Some(Block::Text { style, .. }) = doc.get(id) else {
            continue;
        };
        for run in &style.runs {
            let Some(label) = run.link.as_deref().and_then(|a| a.strip_prefix('^')) else {
                continue;
            };
            let definition = doc
                .footnotes
                .iter()
                .find(|(_, l)| l.as_str() == label)
                .map(|(block, _)| *block);
            out.push(FootnoteRef {
                block: id,
                label: label.into(),
                definition,
                number: numbers.get(label).copied(),
            });
        }
    }
    out
}

/// Footnote definitions that no reference names, in label order.
pub fn unreferenced_footnotes(doc: &Document) -> Vec<BlockId> {
    let referenced: BTreeSet<String> = check_footnotes(doc).into_iter().map(|r| r.label).collect();
    let mut out: Vec<(String, BlockId)> = doc
        .footnotes
        .iter()
        .filter(|(_, label)| !referenced.contains(label.as_str()))
        .map(|(block, label)| (label.clone(), *block))
        .collect();
    out.sort();
    out.into_iter().map(|(_, block)| block).collect()
}

/// The footnotes of `doc` as notes for the foot of their pages, laid out at `width`.
///
/// Each footnote is one note, anchored at its first reference, and numbered as in
/// [`footnote_numbers`]. A footnote that is referenced again is not repeated. A note's text is
/// its number, a period, a space, and the definition's text. Footnotes with no reference, and
/// references with no definition, give no note.
pub fn footnote_notes(doc: &Document, measure: &dyn TextMeasure, width: f64) -> Vec<Note> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut notes = Vec::new();
    for reference in check_footnotes(doc) {
        let (Some(definition), Some(number)) = (reference.definition, reference.number) else {
            continue;
        };
        if !seen.insert(reference.label.clone()) {
            continue;
        }
        let Some(Block::Text { content, style }) = doc.get(definition) else {
            continue;
        };
        let text = alloc::format!("{number}. {content}");
        notes.push(layout_note(
            reference.block,
            definition,
            &text,
            style,
            measure,
            width,
        ));
    }
    notes
}
