// SPDX-License-Identifier: MIT OR Apache-2.0
//! Projection engines for tpt-codex.
//!
//! - [`layout_flow`] lays the document out as one column (the Flow view).
//! - [`paginate`] splits a flow layout into pages with orphan and widow control (the Paginated view).
//!
//! Text measurement is injected through [`TextMeasure`], so the core stays headless.
//! The real shaping and layout come from `tpt-text` and `tpt-layout` once those dependencies are approved.
//!
//! Not yet implemented: the Grid and Canvas views, header and footer content,
//! repeated table headers (there are no tables in the model yet), and PDF output.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod flow;
mod measure;
mod paginate;

pub use flow::{FlowItem, FlowKind, FlowLayout, FlowOptions, LayoutError, layout_flow};
pub use measure::{MonospaceMeasure, TextMeasure};
pub use paginate::{Page, PageError, PageGeometry, paginate};

pub use tpt_codex_core as core;
