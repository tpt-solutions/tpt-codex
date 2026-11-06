// SPDX-License-Identifier: MIT OR Apache-2.0
//! Equation and table labels, numbered in reading order, and the cross-references that name them.
//!
//! A display equation labelled `$$ ... $$ {#eq:id}` is an equation, and a grid labelled
//! `::: grid { id="1" label="tab:id" }` is a table. Each kind is numbered from 1 in reading order.
//! A link `[text](#eq:id)` or `[text](#tab:id)` is a cross-reference: [`check_label_references`]
//! resolves each one to its number, and reports the ones with no label. Figures are in
//! [`crate::figures`].

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, SheetId};

use crate::footnotes::reading_order;

/// The prefix of equation labels and of equation link targets, such as `eq:euler`.
pub(crate) const EQUATION_PREFIX: &str = "eq:";
/// The prefix of table labels and of table link targets, such as `tab:results`.
pub(crate) const TABLE_PREFIX: &str = "tab:";

/// What a label names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelTarget {
    /// A display equation, by its math block.
    Equation(BlockId),
    /// A table, by its sheet.
    Table(SheetId),
}

/// One labelled equation or table, and its number among labels of its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelEntry {
    /// The block or sheet the label is on.
    pub target: LabelTarget,
    /// The label, such as `eq:euler` or `tab:results`.
    pub label: String,
    /// The number among equations, or among tables, from 1, in reading order.
    pub number: u32,
}

/// The labelled equations and tables of the document. Equations and tables are numbered
/// separately, each from 1 in reading order. Unlabelled ones are not numbered.
pub fn label_entries(doc: &Document) -> Vec<LabelEntry> {
    let mut out = Vec::new();
    let mut equations = 0u32;
    let mut tables = 0u32;
    let mut seen_tables = BTreeSet::new();
    for id in reading_order(doc) {
        match doc.get(id) {
            Some(Block::Math { .. }) => {
                let Some(label) = doc.equation_labels.get(&id) else {
                    continue;
                };
                equations += 1;
                out.push(LabelEntry {
                    target: LabelTarget::Equation(id),
                    label: label.clone(),
                    number: equations,
                });
            }
            Some(Block::Grid { sheet_id, .. }) => {
                let Some(label) = doc.table_labels.get(sheet_id) else {
                    continue;
                };
                // A sheet shown twice is one table.
                if !seen_tables.insert(*sheet_id) {
                    continue;
                }
                tables += 1;
                out.push(LabelEntry {
                    target: LabelTarget::Table(*sheet_id),
                    label: label.clone(),
                    number: tables,
                });
            }
            _ => {}
        }
    }
    out
}

/// One equation or table cross-reference found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelReferenceCheck {
    /// The text block that holds the reference.
    pub block: BlockId,
    /// The label it names, such as `eq:euler`.
    pub label: String,
    /// The number of the labelled equation or table, or `None` if no label has that name.
    pub number: Option<u32>,
}

/// Every equation and table cross-reference in the document, in block id order, then in text
/// order.
pub fn check_label_references(doc: &Document) -> Vec<LabelReferenceCheck> {
    let numbers: BTreeMap<String, u32> = label_entries(doc)
        .into_iter()
        .map(|entry| (entry.label, entry.number))
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
            if !is_label_target(target) {
                continue;
            }
            out.push(LabelReferenceCheck {
                block: id,
                label: String::from(target),
                number: numbers.get(target).copied(),
            });
        }
    }
    out
}

/// Whether a link target names an equation or a table.
pub(crate) fn is_label_target(target: &str) -> bool {
    target.starts_with(EQUATION_PREFIX) || target.starts_with(TABLE_PREFIX)
}
