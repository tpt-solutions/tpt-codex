// SPDX-License-Identifier: MIT OR Apache-2.0
//! Figures: labelled images, numbered in reading order, and the cross-references that name them.
//!
//! An image with a figure label, `![Plan](plan.png) {#fig:plan}`, is a figure. A link to it,
//! `[the plan](#fig:plan)`, is a figure cross-reference. [`check_figure_references`] resolves
//! each one to the figure's number, and reports the ones with no figure.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};

use crate::footnotes::reading_order;

/// The prefix of figure labels and of figure link targets, such as `fig:plan`.
pub(crate) const FIGURE_PREFIX: &str = "fig:";

/// One figure: a labelled image and its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureEntry {
    /// The image block.
    pub block: BlockId,
    /// The label, such as `fig:plan`.
    pub label: String,
    /// The figure number, from 1, in reading order.
    pub number: u32,
}

/// The labelled images of the document, numbered from 1 in reading order.
///
/// Unlabelled images are not figures and are not numbered.
pub fn figure_entries(doc: &Document) -> Vec<FigureEntry> {
    let mut out = Vec::new();
    for id in reading_order(doc) {
        if !matches!(doc.get(id), Some(Block::Media { .. })) {
            continue;
        }
        let Some(label) = doc.figures.get(&id).and_then(|f| f.id.as_deref()) else {
            continue;
        };
        let number = out.len() as u32 + 1;
        out.push(FigureEntry {
            block: id,
            label: String::from(label),
            number,
        });
    }
    out
}

/// One figure cross-reference found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureReferenceCheck {
    /// The text block that holds the reference.
    pub block: BlockId,
    /// The label it names, such as `fig:plan`.
    pub label: String,
    /// The figure's number, or `None` if no figure has that label.
    pub number: Option<u32>,
}

/// Every figure cross-reference in the document, in block id order, then in text order.
pub fn check_figure_references(doc: &Document) -> Vec<FigureReferenceCheck> {
    let numbers: BTreeMap<String, u32> = figure_entries(doc)
        .into_iter()
        .map(|figure| (figure.label, figure.number))
        .collect();
    let mut out = Vec::new();
    for (&id, block) in &doc.blocks {
        let Block::Text { style, .. } = block else {
            continue;
        };
        for run in &style.runs {
            let Some(target) = run.link.as_deref().and_then(|a| a.strip_prefix('#')) else {
                continue;
            };
            if !target.starts_with(FIGURE_PREFIX) {
                continue;
            }
            out.push(FigureReferenceCheck {
                block: id,
                label: String::from(target),
                number: numbers.get(target).copied(),
            });
        }
    }
    out
}
