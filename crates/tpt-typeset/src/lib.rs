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
//! Not yet implemented: footnotes, citations and bibliography, figure and table
//! numbering, cross-references inside text, a font-backed math layout (MATH table), and `tpt-layout` box conversion.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod citations;
mod class;
mod footnotes;
mod math;
mod math_check;
mod math_layout;
mod math_svg;
mod outline;
mod query;
mod references;

pub use citations::{
    CitationCheck, CslError, Library, Name, Reference, author_year, bibliography, check_citations,
};
pub use class::{ClassSpec, DocumentClass, UnknownClass, class_of};
pub use footnotes::{
    FootnoteRef, check_footnotes, footnote_numbers, reading_order, unreferenced_footnotes,
};
pub use math::{MAX_DEPTH, MathError, MathNode, parse_math, symbol_glyph, to_plain_text};
pub use math_check::{MathBlockError, check_math_blocks};
pub use math_layout::{MathBox, MathItem, MathMetrics, layout_math};
pub use math_svg::math_svg;
pub use outline::{Outline, OutlineEntry, TocLine, number_string, slugify};
pub use query::{QueryRunError, document_entities, entity_type, run_query};
pub use references::{ReferenceCheck, check_references};

pub use tpt_codex_core as core;
