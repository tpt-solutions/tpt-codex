// SPDX-License-Identifier: MIT OR Apache-2.0
//! Semantic typesetting layer for tpt-codex.
//!
//! This crate sits between the document model and layout. It knows about headings,
//! sections, the table of contents, anchors for cross-references, and document classes.
//! It does not lay anything out. [`tpt_codex_views`] and `tpt-layout` do that.
//!
//! Implemented so far:
//! - [`Outline`]: section numbers, unique anchors, table of contents, anchor lookup.
//! - [`DocumentClass`]: `article` and `report` as a typed schema.
//! - [`parse_math`]: a LaTeX-style math subset with structured errors.
//! - [`layout_math`]: boxes and rules for a parsed formula, from injected character metrics.
//! - [`check_math_blocks`]: parses every `Block::Math` in a document and reports the failures.
//! - [`run_query`]: runs a `Block::Query` with `tpt-query` against a host-supplied entity source.
//!
//! - [`semantic_tree`]: chapters and sections with their blocks, footnotes and citations, and a TOC.
//! - [`Bindings`]: `[[name]]` values for text blocks, such as grid cells and query results.
//! - [`section_boxes`]: each section's extent on a laid-out flow column.
//!
//! Not yet implemented: hyphenation and kerning (they need `tpt-text` APIs that are not wired), a
//! font-backed math layout (the bundled font has no MATH table), equation and table labels (the
//! model has no label on them), and `tpt-layout` box conversion.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod bibtex;
mod bindings;
mod boxes;
mod citations;
mod class;
mod figures;
mod footnotes;
mod labels;
mod math;
mod math_check;
mod math_font;
mod math_inline;
mod math_layout;
mod math_svg;
mod outline;
mod query;
mod references;
mod sections;
mod semantic;

pub use bindings::{BindingCheck, Bindings, bound_text, check_bindings, grid_cell_name};
pub use boxes::{SectionBox, section_boxes};
pub use citations::{
    CitationCheck, CslError, Library, Name, Reference, author_year, bibliography, check_citations,
};
pub use class::{ClassSpec, DocumentClass, UnknownClass, class_of};
pub use figures::{FigureEntry, FigureReferenceCheck, check_figure_references, figure_entries};
pub use footnotes::{
    FootnoteRef, check_footnotes, footnote_notes, footnote_numbers, reading_order,
    unreferenced_footnotes,
};
pub use labels::{
    LabelEntry, LabelReferenceCheck, LabelTarget, check_label_references, label_entries,
};
pub use math::{MAX_DEPTH, MathError, MathNode, parse_math, symbol_glyph, to_plain_text};
pub use math_check::{MathBlockError, check_math_blocks};
pub use math_font::{STIX_TWO_MATH, math_constants};
pub use math_inline::{InlineMathCheck, check_inline_math, layout_inline_math};
pub use math_layout::{MathBox, MathConstants, MathItem, MathMetrics, layout_math};
pub use math_svg::math_svg;
pub use outline::{Outline, OutlineEntry, TocLine, number_string, slugify};
pub mod query_builder;
pub use query::{QueryRunError, document_entities, entity_type, run_query};
pub use references::{ReferenceCheck, check_references};
pub use sections::{SectionGroup, slide_sections};
pub use semantic::{Section, SemanticTree, semantic_tree};

pub use tpt_codex_core as core;
