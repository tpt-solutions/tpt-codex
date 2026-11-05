// SPDX-License-Identifier: MIT OR Apache-2.0
//! Application shell for tpt-codex.
//!
//! Implemented so far:
//! - local persistence ([`DocumentStore`]) over any [`Storage`], and a file-system backend
//!   ([`FileStorage`]);
//! - the editor's state and UI tree ([`Editor`], [`Msg`]), built on `tpt-appfront`. The tree renders
//!   to semantic HTML with `tpt-appfront-html` and runs natively, so it can be tested without a browser.
//! - Mounting the editor in a browser page ([`web::mount`], `wasm32` only) with `tpt-appfront-dom`.
//!   The WASM build compiles; it has not been run in a browser.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod editor;
mod file_storage;
mod persistence;

pub use editor::{Arrow, CellFocus, Editor, Msg, RemoteCursor};
pub use file_storage::FileStorage;
pub use persistence::{DocumentStore, PersistError};

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use tpt_codex_core as core;
