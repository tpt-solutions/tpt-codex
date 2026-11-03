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
mod text;

pub use block::{Block, Query};
pub use document::{Document, Metadata, SCHEMA_VERSION};
pub use error::ValidationError;
pub use history::History;
pub use id::{BlockId, SheetId};
pub use ops::{EditError, Op, apply};
pub use platform::{Clock, FixedClock, MemoryStorage, Storage};
pub use primitives::{LayoutMode, MediaSource, Rect, TextStyle};
pub use text::{edit_text, replace_chars};
