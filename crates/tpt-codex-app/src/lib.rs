// SPDX-License-Identifier: MIT OR Apache-2.0
//! Application shell for tpt-codex.
//!
//! Implemented so far: local persistence ([`DocumentStore`]) over any [`Storage`],
//! and a file-system backend ([`FileStorage`]).
//!
//! Not yet implemented: the tpt-appfront UI shell and WASM build. Both depend on
//! `tpt-appfront` and the rendering crates, which are not yet approved (see `docs/tpt-dependency-audit.md`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod file_storage;
mod persistence;

pub use file_storage::FileStorage;
pub use persistence::{DocumentStore, PersistError};

pub use tpt_codex_core as core;
