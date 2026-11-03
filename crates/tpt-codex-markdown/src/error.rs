// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use core::fmt;

/// A problem found while reading Markdown, with a 1-based position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number (1 for whole-line problems).
    pub column: usize,
    /// What went wrong.
    pub message: String,
    /// A suggested fix, when one is known.
    pub suggestion: Option<&'static str>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )?;
        if let Some(hint) = self.suggestion {
            write!(f, " (hint: {hint})")?;
        }
        Ok(())
    }
}

impl core::error::Error for ParseError {}

/// A document that cannot be written as the supported Markdown subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializeError {
    /// Index of the child block in the root container, when the problem is with a block.
    pub index: Option<usize>,
    /// What cannot be represented.
    pub reason: String,
}

impl fmt::Display for SerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.index {
            Some(i) => write!(f, "block {i}: {}", self.reason),
            None => f.write_str(&self.reason),
        }
    }
}

impl core::error::Error for SerializeError {}
