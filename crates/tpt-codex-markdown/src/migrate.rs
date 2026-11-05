// SPDX-License-Identifier: MIT OR Apache-2.0
//! Migration of Markdown written before inline syntax was read.
//!
//! Before inline syntax was read, `*`, `[`, `]` and backslashes in text were literal. Reading them
//! now would change the meaning of those files. [`migrate_legacy_markdown`] escapes them, so the
//! text reads back exactly as it did before. It changes text lines only: front matter, code
//! fences, math blocks, rules, images and blank lines are copied as they are.

use alloc::string::String;
use alloc::vec::Vec;

use crate::inline::escape;
use crate::syntax::{
    fence_closes, fence_open, heading, image_line, list_item, math_closes, math_open,
    thematic_break, unsupported,
};

/// The result of a migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migrated {
    /// The migrated Markdown.
    pub text: String,
    /// 1-based numbers of the lines that changed, in order.
    pub changed_lines: Vec<usize>,
}

/// Escapes the inline markers in the text of `src`, so the text reads back as literal.
pub fn migrate_legacy_markdown(src: &str) -> Migrated {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut changed_lines = Vec::new();
    let mut i = 0;

    // Front matter: copied as it is.
    if lines.first() == Some(&"---") {
        if let Some(close) = lines.iter().skip(1).position(|l| *l == "---") {
            for line in &lines[..close + 2] {
                out.push((*line).into());
            }
            i = close + 2;
        }
    }

    while i < lines.len() {
        let line = lines[i];
        let lineno = i + 1;
        if let Some(fence) = fence_open(line) {
            // Copy the fence and its contents through the closing line.
            out.push(line.into());
            i += 1;
            while i < lines.len() && !fence_closes(lines[i], fence.ch, fence.len) {
                out.push(lines[i].into());
                i += 1;
            }
            if i < lines.len() {
                out.push(lines[i].into());
                i += 1;
            }
            continue;
        }
        if let Some(Ok(_)) = math_open(line) {
            if !line.trim_start().ends_with("$$") || line.trim() == "$$" {
                out.push(line.into());
                i += 1;
                while i < lines.len() && !math_closes(lines[i]) {
                    out.push(lines[i].into());
                    i += 1;
                }
                if i < lines.len() {
                    out.push(lines[i].into());
                    i += 1;
                }
                continue;
            }
        }
        let migrated = migrate_line(line);
        if migrated != line {
            changed_lines.push(lineno);
        }
        out.push(migrated);
        i += 1;
    }

    Migrated {
        text: out.join("\n"),
        changed_lines,
    }
}

/// Migrates one line that is not part of a fence, math block or front matter.
fn migrate_line(line: &str) -> String {
    if line.trim().is_empty()
        || thematic_break(line).is_some()
        || image_line(line).is_some()
        || unsupported(line).is_some()
    {
        return line.into();
    }
    if let Some((level, text)) = heading(line) {
        if text.is_empty() {
            return line.into();
        }
        return alloc::format!("{} {}", "#".repeat(level as usize), escape(text));
    }
    if let Some(marker) = list_item(line) {
        if marker.text.is_empty() {
            return line.into();
        }
        // The item text is a slice of the line, so its offset locates it.
        let start = marker.text.as_ptr() as usize - line.as_ptr() as usize;
        let end = start + marker.text.len();
        return alloc::format!("{}{}{}", &line[..start], escape(marker.text), &line[end..]);
    }
    escape(line)
}
