// SPDX-License-Identifier: MIT OR Apache-2.0
//! Saving and loading documents through a [`Storage`] backend.
//!
//! Documents are stored as JSON, which keeps block ids and the schema version. Markdown is
//! the readable interchange format and is handled by `tpt-codex-markdown`.
//! Every save and load checks [`Document::validate`], so an invalid document is never written
//! and never returned.

use std::fmt;

use tpt_codex_core::{Document, Storage, ValidationError};

/// Why a save or load failed.
#[derive(Debug)]
pub enum PersistError<E> {
    /// The document name is empty or has characters outside `[A-Za-z0-9_-]`.
    InvalidName(String),
    /// No document with this name is stored.
    NotFound(String),
    /// The storage backend failed.
    Storage(E),
    /// The document could not be encoded or decoded as JSON.
    Json(serde_json::Error),
    /// The document is structurally invalid.
    Invalid(ValidationError),
}

impl<E: fmt::Display> fmt::Display for PersistError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(n) => write!(
                f,
                "invalid document name '{n}': use letters, digits, '-' and '_'"
            ),
            Self::NotFound(n) => write!(f, "no document named '{n}'"),
            Self::Storage(e) => write!(f, "storage error: {e}"),
            Self::Json(e) => write!(f, "document is not valid JSON: {e}"),
            Self::Invalid(e) => write!(f, "document is invalid: {e}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for PersistError<E> {}

/// Saves and loads named documents.
#[derive(Debug)]
pub struct DocumentStore<S> {
    storage: S,
}

const PREFIX: &str = "documents/";
const SUFFIX: &str = ".json";

impl<S: Storage> DocumentStore<S> {
    /// Wraps a storage backend.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Returns the backend, for example to inspect it in tests.
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Validates and saves `doc` under `name`, replacing any previous version.
    pub fn save(&mut self, name: &str, doc: &Document) -> Result<(), PersistError<S::Error>> {
        check_name(name)?;
        doc.validate().map_err(PersistError::Invalid)?;
        let bytes = serde_json::to_vec_pretty(doc).map_err(PersistError::Json)?;
        self.storage
            .write(&key(name), &bytes)
            .map_err(PersistError::Storage)
    }

    /// Loads the document saved under `name` and validates it.
    pub fn load(&self, name: &str) -> Result<Document, PersistError<S::Error>> {
        check_name(name)?;
        let bytes = self
            .storage
            .read(&key(name))
            .map_err(PersistError::Storage)?
            .ok_or_else(|| PersistError::NotFound(name.to_string()))?;
        let doc: Document = serde_json::from_slice(&bytes).map_err(PersistError::Json)?;
        doc.validate().map_err(PersistError::Invalid)?;
        Ok(doc)
    }

    /// Removes the document saved under `name`. Removing a missing document is not an error.
    pub fn delete(&mut self, name: &str) -> Result<(), PersistError<S::Error>> {
        check_name(name)?;
        self.storage
            .remove(&key(name))
            .map_err(PersistError::Storage)
    }
}

fn key(name: &str) -> String {
    format!("{PREFIX}{name}{SUFFIX}")
}

fn check_name<E>(name: &str) -> Result<(), PersistError<E>> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(PersistError::InvalidName(name.to_string()))
    }
}
