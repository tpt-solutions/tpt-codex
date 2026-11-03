// SPDX-License-Identifier: MIT OR Apache-2.0
//! Semantic typesetting engine for tpt-codex (chapters, TOC, citations, math).
//!
//! Status: scaffold only. No layout or parsing is implemented yet (see todo.md, Phase 2).

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

pub use tpt_codex_core as core;
