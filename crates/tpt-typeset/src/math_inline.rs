// SPDX-License-Identifier: MIT OR Apache-2.0
//! Inline math: formulas inside a paragraph, from `$…$` in Markdown.
//!
//! An inline formula is an inline run whose `math` field holds its TeX source. This module parses
//! each one and reports the ones that do not parse, so a bad formula is found before layout.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};

use crate::math::{MathError, parse_math};
use crate::math_layout::{MathBox, MathMetrics, layout_math};

/// One inline formula in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineMathCheck {
    /// The text block that holds the formula.
    pub block: BlockId,
    /// The TeX source.
    pub source: String,
    /// Why it does not parse, or `None` if it does.
    pub error: Option<MathError>,
}

/// Every inline formula in the document, in block id order, then in text order.
pub fn check_inline_math(doc: &Document) -> Vec<InlineMathCheck> {
    let mut out = Vec::new();
    for (&id, block) in &doc.blocks {
        let Block::Text { style, .. } = block else {
            continue;
        };
        for run in &style.runs {
            let Some(source) = run.math.as_deref() else {
                continue;
            };
            out.push(InlineMathCheck {
                block: id,
                source: source.into(),
                error: parse_math(source).err(),
            });
        }
    }
    out
}

/// Lays out one inline formula at `size`. Fails with the parse error, if there is one.
pub fn layout_inline_math(
    source: &str,
    metrics: &dyn MathMetrics,
    size: f64,
) -> Result<MathBox, MathError> {
    Ok(layout_math(&parse_math(source)?, metrics, size))
}
