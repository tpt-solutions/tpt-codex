// SPDX-License-Identifier: MIT OR Apache-2.0
//! The checks behind `tpt check`: everything the typesetter can verify about a document.

use tpt_codex_core::{Block, BlockId, Document};
use tpt_typeset::{
    Library, check_citations, check_figure_references, check_footnotes, check_inline_math,
    check_label_references, check_math_blocks, check_references, unreferenced_footnotes,
};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The document will not typeset as intended.
    Error,
    /// Probably a mistake, but the document still typesets.
    Warning,
}

/// One problem found in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// The block it is in.
    pub block: BlockId,
    /// What is wrong, and how to fix it.
    pub message: String,
}

/// A short quotation of a block's text, to help find it in the source.
pub fn excerpt(doc: &Document, id: BlockId) -> String {
    let text = match doc.get(id) {
        Some(Block::Text { content, .. }) => content.as_str(),
        Some(Block::Math { source }) => source.as_str(),
        _ => return format!("block {}", id.0),
    };
    let line = text.lines().next().unwrap_or("");
    let mut short: String = line.chars().take(40).collect();
    if short.chars().count() < line.chars().count() {
        short.push('…');
    }
    format!("block {} \"{short}\"", id.0)
}

/// Checks `doc`. Citations are checked only when a bibliography is given.
pub fn check_document(doc: &Document, library: Option<&Library>) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut add = |severity, block, message: String| {
        out.push(Finding {
            severity,
            block,
            message,
        })
    };

    for e in check_math_blocks(doc) {
        add(
            Severity::Error,
            e.block,
            format!(
                "math, line {} column {}: {} (hint: {})",
                e.error.line, e.error.column, e.error.message, e.error.hint
            ),
        );
    }
    for m in check_inline_math(doc) {
        if let Some(e) = m.error {
            add(
                Severity::Error,
                m.block,
                format!(
                    "inline math `{}`: {} at column {} (hint: {})",
                    m.source, e.message, e.column, e.hint
                ),
            );
        }
    }
    for r in check_references(doc) {
        if r.number.is_none() {
            add(
                Severity::Error,
                r.block,
                format!(
                    "cross-reference to #{} but no heading has that anchor",
                    r.target
                ),
            );
        }
    }
    for r in check_figure_references(doc) {
        if r.number.is_none() {
            add(
                Severity::Error,
                r.block,
                format!(
                    "reference to figure #{} but no figure has that label",
                    r.label
                ),
            );
        }
    }
    for r in check_label_references(doc) {
        if r.number.is_none() {
            add(
                Severity::Error,
                r.block,
                format!("reference to #{} but nothing has that label", r.label),
            );
        }
    }
    for f in check_footnotes(doc) {
        if f.definition.is_none() {
            add(
                Severity::Error,
                f.block,
                format!("footnote [^{}] has no definition", f.label),
            );
        }
    }
    for id in unreferenced_footnotes(doc) {
        add(
            Severity::Warning,
            id,
            "footnote is never referenced".to_string(),
        );
    }
    if let Some(library) = library {
        for c in check_citations(doc, library) {
            if c.text.is_none() {
                add(
                    Severity::Error,
                    c.block,
                    format!("citation [@{}] is not in the bibliography", c.key),
                );
            }
        }
    }
    out
}

/// The number of citations in `doc`, for telling the user how many went unchecked.
pub fn citation_count(doc: &Document) -> usize {
    check_citations(doc, &Library::default()).len()
}
