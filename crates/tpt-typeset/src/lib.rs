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
//! - [`parse_math`]: a LaTeX-style math subset with structured errors (no layout yet).
//! - [`check_math_blocks`]: parses every `Block::Math` in a document and reports the failures.
//! - [`run_query`]: runs a `Block::Query` with `tpt-query` against a host-supplied entity source.
//!
//! Not yet implemented: footnotes, citations and bibliography, figure and table
//! numbering, cross-references inside text, math layout, and `tpt-layout` box conversion.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod class;
mod math;
mod math_check;
mod outline;
mod query;

pub use class::{ClassSpec, DocumentClass, UnknownClass};
pub use math::{MAX_DEPTH, MathError, MathNode, parse_math, symbol_glyph, to_plain_text};
pub use math_check::{MathBlockError, check_math_blocks};
pub use outline::{Outline, OutlineEntry, TocLine, number_string, slugify};
pub use query::{QueryRunError, document_entities, entity_type, run_query};

pub use tpt_codex_core as core;
