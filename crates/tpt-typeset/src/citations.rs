// SPDX-License-Identifier: MIT OR Apache-2.0
//! Citations and bibliography, from a CSL-JSON library.
//!
//! A citation is a link whose address starts with `@`, so `[Smith 2020](@smith2020)` cites the
//! entry `smith2020`. The Markdown reader also reads the bare form `[@smith2020]`, which shows the
//! key until the text is replaced. Entries come from a CSL-JSON array, the format that most
//! reference managers export. Only the fields used for author-year citations are read.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde_json::Value;
use tpt_codex_core::{Block, BlockId, Document};

/// One author or editor name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// Family name, as used in citations.
    pub family: String,
    /// Given names. Empty if the entry has none.
    pub given: String,
}

/// One bibliography entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The key the entry is cited by.
    pub id: String,
    /// The title. Empty if the entry has none.
    pub title: String,
    /// Authors, in order.
    pub authors: Vec<Name>,
    /// Publication year, if the entry has one.
    pub year: Option<i64>,
}

/// Why a library could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CslError(pub String);

impl fmt::Display for CslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bibliography: {}", self.0)
    }
}

impl core::error::Error for CslError {}

/// Entries by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    entries: BTreeMap<String, Reference>,
}

impl Library {
    /// Reads a CSL-JSON array. An entry without an `id` is an error, since nothing can cite it.
    pub fn from_csl_json(src: &str) -> Result<Library, CslError> {
        let value: Value = serde_json::from_str(src).map_err(|e| CslError(e.to_string()))?;
        let items = value
            .as_array()
            .ok_or_else(|| CslError("expected a JSON array of entries".into()))?;
        let mut entries = BTreeMap::new();
        for item in items {
            let reference = reference_from(item)?;
            if entries.insert(reference.id.clone(), reference).is_some() {
                return Err(CslError("two entries share an id".into()));
            }
        }
        Ok(Library { entries })
    }

    /// The entry for `key`, if the library has one.
    pub fn get(&self, key: &str) -> Option<&Reference> {
        self.entries.get(key)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the library has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn reference_from(item: &Value) -> Result<Reference, CslError> {
    let id = item
        .get("id")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CslError("an entry has no id".into()))?;
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let authors = item
        .get("author")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .map(|n| Name {
                    family: text(n, "family"),
                    given: text(n, "given"),
                })
                .filter(|n| !n.family.is_empty() || !n.given.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let year = item
        .get("issued")
        .and_then(|issued| issued.get("date-parts"))
        .and_then(Value::as_array)
        .and_then(|parts| parts.first())
        .and_then(Value::as_array)
        .and_then(|first| first.first())
        .and_then(Value::as_i64);
    Ok(Reference {
        id,
        title,
        authors,
        year,
    })
}

fn text(name: &Value, field: &str) -> String {
    name.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The author-year form of an entry: `Smith 2020`, `Smith and Jones 2020`, or `Smith et al. 2020`.
/// An entry without authors uses its title. An entry without a year says `n.d.`.
pub fn author_year(reference: &Reference) -> String {
    let who = match reference.authors.as_slice() {
        [] => reference.title.clone(),
        [one] => one.family.clone(),
        [a, b] => alloc::format!("{} and {}", a.family, b.family),
        [first, ..] => alloc::format!("{} et al.", first.family),
    };
    let when = reference.year.map_or("n.d.".to_string(), |y| y.to_string());
    alloc::format!("{who} {when}")
}

/// The bibliography for the cited keys: each known key once, sorted by first author's family name,
/// then by year, then by key. Unknown keys are left out.
pub fn bibliography(library: &Library, keys: &[&str]) -> Vec<String> {
    let mut entries: Vec<&Reference> = Vec::new();
    for key in keys {
        if let Some(reference) = library.get(key) {
            if !entries.iter().any(|e| e.id == reference.id) {
                entries.push(reference);
            }
        }
    }
    entries.sort_by(|a, b| {
        let family = |r: &Reference| {
            r.authors
                .first()
                .map(|n| n.family.clone())
                .unwrap_or_default()
        };
        family(a)
            .cmp(&family(b))
            .then(a.year.cmp(&b.year))
            .then(a.id.cmp(&b.id))
    });
    entries
        .into_iter()
        .map(|r| {
            let names: Vec<String> = r
                .authors
                .iter()
                .map(|n| {
                    if n.given.is_empty() {
                        n.family.clone()
                    } else {
                        alloc::format!("{}, {}", n.family, n.given)
                    }
                })
                .collect();
            let when = r.year.map_or("n.d.".to_string(), |y| y.to_string());
            alloc::format!("{}. {}. {}.", names.join("; "), when, r.title)
        })
        .collect()
}

/// One citation found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationCheck {
    /// The text block that holds the citation.
    pub block: BlockId,
    /// The citation key, without the `@`.
    pub key: String,
    /// The author-year text, or `None` if the library has no such entry.
    pub text: Option<String>,
}

/// Every citation in the document, in block id order, then in text order.
pub fn check_citations(doc: &Document, library: &Library) -> Vec<CitationCheck> {
    let mut out = Vec::new();
    for (&id, block) in &doc.blocks {
        let Block::Text { style, .. } = block else {
            continue;
        };
        for run in &style.runs {
            let Some(key) = run.link.as_deref().and_then(|a| a.strip_prefix('@')) else {
                continue;
            };
            out.push(CitationCheck {
                block: id,
                key: key.into(),
                text: library.get(key).map(author_year),
            });
        }
    }
    out
}
