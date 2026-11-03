// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, Document, LayoutMode, Metadata};

use crate::error::SerializeError;
use crate::syntax::{heading, unsupported};

/// Writes a [`Document`] as Markdown in the supported subset.
///
/// The root must be a flow container whose children are headings or paragraphs
/// with no bold or italic flags. Anything else returns a [`SerializeError`].
/// Block ids are not written, so ids in the document are not preserved.
pub fn to_markdown(doc: &Document) -> Result<String, SerializeError> {
    let root = match doc.get(doc.root) {
        Some(Block::Container {
            layout_mode: LayoutMode::Flow,
            children,
        }) => children,
        _ => {
            return Err(SerializeError {
                index: None,
                reason: String::from("root must be a flow container"),
            });
        }
    };

    let mut out = String::new();
    write_front_matter(&doc.metadata, &mut out)?;

    let mut parts: Vec<String> = Vec::new();
    for (index, id) in root.iter().enumerate() {
        let block = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("child block {} is missing", id.0),
        })?;
        parts.push(write_block(index, block)?);
    }
    out.push_str(&parts.join("\n\n"));
    if !parts.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

fn write_front_matter(meta: &Metadata, out: &mut String) -> Result<(), SerializeError> {
    let entries = [
        ("title", &meta.title),
        ("author", &meta.author),
        ("date", &meta.date),
        ("class", &meta.class),
    ];
    if entries.iter().all(|(_, v)| v.is_none()) {
        return Ok(());
    }
    out.push_str("---\n");
    for (key, value) in entries {
        if let Some(value) = value {
            if value.contains('\n') || value.contains('\r') {
                return Err(SerializeError {
                    index: None,
                    reason: format!("front matter '{key}' contains a line break"),
                });
            }
            out.push_str(key);
            out.push_str(": \"");
            for c in value.chars() {
                if c == '"' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push_str("\"\n");
        }
    }
    out.push_str("---\n\n");
    Ok(())
}

fn write_block(index: usize, block: &Block) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let (content, style) = match block {
        Block::Text { content, style } => (content, style),
        Block::Grid { .. } => return Err(fail("live grids are not in the Markdown subset yet")),
        Block::Media { .. } => return Err(fail("media is not in the Markdown subset yet")),
        Block::Query(_) => return Err(fail("queries are not in the Markdown subset yet")),
        Block::Container { .. } => return Err(fail("nested containers are not supported yet")),
    };
    if style.bold || style.italic {
        return Err(fail(
            "bold and italic are not written yet; remove the flags",
        ));
    }
    if content.is_empty() {
        return Err(fail("empty text cannot be written"));
    }
    if content.trim() != content.as_str() {
        return Err(fail("leading or trailing spaces would be lost"));
    }

    if let Some(level) = style.heading_level {
        if !(1..=6).contains(&level) {
            return Err(fail("heading level must be 1 to 6"));
        }
        if content.contains('\n') {
            return Err(fail("headings must be a single line"));
        }
        return Ok(format!("{} {}", "#".repeat(level as usize), content));
    }

    for line in content.split('\n') {
        if line.trim().is_empty() {
            return Err(fail("paragraphs cannot contain blank lines"));
        }
        if heading(line).is_some() || unsupported(line).is_some() {
            return Err(fail("paragraph line would read back as other syntax"));
        }
    }
    Ok(content.clone())
}
