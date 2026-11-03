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
//!
//! Not yet implemented: footnotes, citations and bibliography, figure and table
//! numbering, cross-references inside text, math, and `tpt-layout` box conversion.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod class;
mod outline;

pub use class::{ClassSpec, DocumentClass, UnknownClass};
pub use outline::{Outline, OutlineEntry, TocLine, number_string, slugify};

pub use tpt_codex_core as core;
