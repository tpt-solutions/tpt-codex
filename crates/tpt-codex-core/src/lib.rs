// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unified, headless, Block-based data model for tpt-codex.
//!
//! A document is a directed acyclic graph of [`Block`] nodes stored in a
//! [`Document`]. Presentation (flow, grid, canvas, paginated) is a projection
//! of this tree and lives in other crates. This crate is `no_std` + `alloc`.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod block;
mod document;
mod error;
mod history;
mod id;
mod ops;
mod platform;
mod primitives;
mod selection;
mod sheet;
mod text;
mod text_crdt;
mod tree_crdt;

pub use block::{Block, Query};
pub use document::{Document, Metadata, SCHEMA_VERSION};
pub use error::ValidationError;
pub use history::History;
pub use id::{BlockId, SheetId};
pub use ops::{EditError, Op, apply, changed_blocks};
pub use platform::{Clock, FixedClock, MemoryStorage, Storage};
pub use primitives::{LayoutMode, MediaSource, Rect, TextStyle};
pub use selection::{Movement, Selection, delete_backward, delete_forward, insert_text};
pub use sheet::{
    CellInput, Sheet, address_of, address_position, is_formula_source, is_valid_address,
};
pub use text::{edit_text, replace_chars};
pub use text_crdt::{Stamp, TextCrdt, TextCrdtError, TextOp, materialize};
pub use tree_crdt::{
    NodeId, ROOT, TRASH, TreeCrdt, TreeError, TreeOp, key_between, project_document,
};
