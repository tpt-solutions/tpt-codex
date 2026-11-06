// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cross-references: links to heading anchors inside the document.
//!
//! A link whose address starts with `#` points at the heading with that anchor, so
//! `[see details](#details)` is a cross-reference. The model needs no new field for this.
//! [`check_references`] resolves each one against the outline, and reports the ones that have
//! no target.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};

use crate::figures::FIGURE_PREFIX;
use crate::outline::{Outline, number_string};

/// One cross-reference found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCheck {
    /// The text block that holds the reference.
    pub block: BlockId,
    /// The anchor it names, without the `#`.
    pub target: String,
    /// The target heading's section number (for example `1.2`), or `None` if there is no such
    /// heading.
    pub number: Option<String>,
}

/// Every cross-reference in the document, in block id order, then in text order.
pub fn check_references(doc: &Document) -> Vec<ReferenceCheck> {
    let outline = Outline::of(doc);
    let mut out = Vec::new();
    for (&id, block) in &doc.blocks {
        let Block::Text { style, .. } = block else {
            continue;
        };
        for run in &style.runs {
            let Some(address) = run.link.as_deref() else {
                continue;
            };
            let Some(target) = address.strip_prefix('#') else {
                continue;
            };
            // Figure links are resolved by `check_figure_references`.
            if target.starts_with(FIGURE_PREFIX) {
                continue;
            }
            let number = outline
                .resolve(target)
                .map(|entry| number_string(&entry.number));
            out.push(ReferenceCheck {
                block: id,
                target: target.into(),
                number,
            });
        }
    }
    out
}
