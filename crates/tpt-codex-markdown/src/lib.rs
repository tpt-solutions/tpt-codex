// SPDX-License-Identifier: MIT OR Apache-2.0
//! Markdown and Codex Script reader and writer for tpt-codex.
//!
//! Markdown and Codex Script reader and writer for tpt-codex.
//!
//! The supported subset is described in `docs/markdown-grammar.md`. It covers:
//!
//! - YAML-style front matter with `title`, `author`, `date` and `class`,
//! - ATX headings (`#` to `######`) and paragraphs,
//! - bulleted and numbered lists, with nested sub-lists,
//! - block quotes (`>`),
//! - fenced code blocks (backtick or tilde fences),
//! - thematic breaks,
//! - standalone images (`![alt](uri)`),
//! - pipe tables outside a grid (text cells only),
//! - inline emphasis (`*`, `**`, `***`), links, and `[[name]]` bindings.
//!
//! Any other syntax returns a [`ParseError`] with a line, a column and a hint.
//! [`to_markdown`] refuses
//! documents it cannot write back in this subset, so a round trip never drops data.
//!
//! Block ids are not stored in Markdown. A document read by [`from_markdown`]
//! has the root `BlockId(0)`, and every other block gets the next id in the
//! order it is read. A list's items get ids before the list.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

mod error;
mod inline;
mod migrate;
mod read;
mod syntax;
mod write;

pub use error::{ParseError, SerializeError};
pub use migrate::{Migrated, migrate_legacy_markdown};
pub use read::from_markdown;
pub use write::to_markdown;
