// SPDX-License-Identifier: MIT OR Apache-2.0
//! Markdown and Codex Script reader and writer for tpt-codex.
//!
//! This is the first slice of Phase 2. It supports:
//!
//! - YAML-style front matter with `title`, `author`, `date` and `class`,
//! - ATX headings (`#` to `######`),
//! - paragraphs (consecutive non-blank lines, joined with `\n`).
//!
//! Any other syntax returns a [`ParseError`] with a line, a column and a hint.
//! Inline emphasis is kept as literal text. [`to_markdown`] refuses documents
//! it cannot write back in this subset, so a round trip never drops data.
//!
//! Block ids are not stored in Markdown. A document read by [`from_markdown`]
//! has the root `BlockId(0)` and children `BlockId(1)..` in order.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod error;
mod read;
mod syntax;
mod write;

pub use error::{ParseError, SerializeError};
pub use read::from_markdown;
pub use write::to_markdown;
