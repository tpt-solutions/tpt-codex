// SPDX-License-Identifier: MIT OR Apache-2.0
//! Application shell for tpt-codex.
//!
//! Implemented so far:
//! - local persistence ([`DocumentStore`]) over any [`Storage`], and a file-system backend
//!   ([`FileStorage`]);
//! - the editor's state and UI tree ([`Editor`], [`Msg`]), built on `tpt-appfront`. The tree renders
//!   to semantic HTML with `tpt-appfront-html` and runs natively, so it can be tested without a browser.
//! - Mounting the editor in a browser page ([`web::mount`], `wasm32` only) with `tpt-appfront-dom`.
//!   It is run in Chrome by the checks in `crates/tpt-codex-web/browser`.
//! - PDF export ([`export::export_pdf`]) and remote presence ([`Presence`]).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod editor;
pub mod export;
mod file_storage;
mod persistence;
pub mod presence;
pub mod query_form;

pub use editor::{Arrow, CellFocus, Editor, Msg, RemoteCursor};
pub use file_storage::FileStorage;
pub use persistence::{DocumentStore, PersistError};
pub use presence::Presence;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use tpt_codex_core as core;
