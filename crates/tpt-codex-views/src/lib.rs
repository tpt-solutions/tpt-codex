// SPDX-License-Identifier: MIT OR Apache-2.0
//! Projection engines for tpt-codex.
//!
//! - [`layout_flow`] lays the document out as one column (the Flow view).
//! - [`paginate`] splits a flow layout into pages with orphan and widow control (the Paginated view).
//! - [`render_chrome`] fills headers, footers and page numbers; [`PaperSize`] gives standard page sizes.
//!
//! - [`layout_canvas`] places a slide's children on a fixed-size canvas, and [`render_slide_svg`]
//!   draws one slide as SVG.
//!
//! Grid rows are flow items, so pages break between rows and a grid's header rows repeat on each
//! continuation page. Long words are hyphenated with in-house rules.
//!
//! Text measurement is injected through [`TextMeasure`], so the core stays headless. The flow
//! engine is our own; `tpt-codex-text` supplies a font-backed measure from `tpt-text`, and
//! `tpt-codex-pdf` draws the pages as a PDF.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod canvas;
mod chrome;
mod flow;
mod hyphenate;
mod measure;
mod paginate;
mod svg;

pub use canvas::{CanvasError, CanvasItem, CanvasLayout, layout_canvas, slide_ids};
pub use chrome::{PageChrome, PaperSize, RenderedChrome, render_chrome, render_template};
pub use flow::{
    FlowCache, FlowItem, FlowKind, FlowLayout, FlowOptions, LIST_INDENT, LayoutError,
    grid_rows_shown, layout_flow, layout_flow_cached, layout_flow_hyphenated,
};
pub use measure::{MonospaceMeasure, TextMeasure};
pub use paginate::{
    Note, Page, PageError, PageGeometry, layout_note, paginate, paginate_with_notes,
};
pub use svg::render_slide_svg;

pub use tpt_codex_core as core;
