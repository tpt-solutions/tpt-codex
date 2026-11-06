// SPDX-License-Identifier: MIT OR Apache-2.0
//! Document classes as a typed, declarative schema. There is no macro language.

use core::fmt;

/// A document class, selected by the `class:` front matter key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentClass {
    /// Short documents: numbered sections down to level 3, TOC to level 2.
    Article,
    /// Longer documents: numbered sections down to level 3, TOC to level 3.
    Report,
}

/// The declarative rules a class applies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassSpec {
    /// Front matter name of the class.
    pub name: &'static str,
    /// Deepest heading level that gets a section number.
    pub numbered_depth: u8,
    /// Deepest heading level shown in the table of contents.
    pub toc_depth: u8,
    /// Page height in points (A4).
    pub page_height: f64,
    /// Margin above and below the content, in points.
    pub margin: f64,
    /// Fewest lines of a paragraph kept together at a page break.
    pub min_lines: usize,
}

impl DocumentClass {
    /// All known classes.
    pub const ALL: [DocumentClass; 2] = [DocumentClass::Article, DocumentClass::Report];

    /// The schema for this class.
    pub const fn spec(self) -> ClassSpec {
        match self {
            DocumentClass::Article => ClassSpec {
                name: "article",
                numbered_depth: 3,
                toc_depth: 2,
                page_height: 842.0,
                margin: 72.0,
                min_lines: 2,
            },
            DocumentClass::Report => ClassSpec {
                name: "report",
                numbered_depth: 3,
                toc_depth: 3,
                page_height: 842.0,
                margin: 90.0,
                min_lines: 3,
            },
        }
    }

    /// Parses a front matter `class:` value. Matching is case-sensitive.
    pub fn parse(name: &str) -> Result<Self, UnknownClass> {
        DocumentClass::ALL
            .into_iter()
            .find(|c| c.spec().name == name)
            .ok_or_else(|| UnknownClass {
                name: alloc::string::String::from(name),
                hint: "use one of the known classes: article, report",
            })
    }
}

/// A class name that is not one of the known classes.
///
/// The front matter does not carry a position for the value, so the error gives the name and a
/// hint, and the Markdown reader reports the line it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownClass {
    /// The name that was given.
    pub name: alloc::string::String,
    /// A suggested fix.
    pub hint: &'static str,
}

impl fmt::Display for UnknownClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown document class '{}' (hint: {})",
            self.name, self.hint
        )
    }
}

impl core::error::Error for UnknownClass {}

/// The class a document declares in its front matter, if it declares a known one.
pub fn class_of(doc: &tpt_codex_core::Document) -> Option<DocumentClass> {
    doc.metadata
        .class
        .as_deref()
        .and_then(|name| DocumentClass::parse(name).ok())
}
