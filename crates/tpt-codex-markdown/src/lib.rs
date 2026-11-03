// SPDX-License-Identifier: MIT OR Apache-2.0
//! Markdown and Codex Script parser/serializer for tpt-codex.
//!
//! Status: scaffold only. `from_markdown` and `to_markdown` are not implemented yet (see todo.md, Phase 2).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub use tpt_codex_core as core;
