// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, MediaSource, Metadata, TextStyle};

use crate::error::SerializeError;
use crate::inline::write_inline;
use crate::syntax::{MAX_LIST_NUMBER, image_line, list_item, math_closes, starts_block};

/// Writes a [`Document`] as Markdown in the supported subset.
///
/// The root must be a flow container. Its children may be headings, paragraphs,
/// lists, fenced code blocks, thematic breaks, images with a URI and display math. Anything
/// else returns a [`SerializeError`]. Block ids are not written, so ids in the
/// document are not preserved.
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
    let mut prev: Option<&Block> = None;
    for (index, id) in root.iter().enumerate() {
        let block = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("child block {} is missing", id.0),
        })?;
        parts.push(write_block(doc, index, block, prev)?);
        prev = Some(block);
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

fn write_block(
    doc: &Document,
    index: usize,
    block: &Block,
    prev: Option<&Block>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    match block {
        Block::Text { content, style } => write_text(index, content, style),
        Block::List {
            ordered,
            start,
            items,
        } => write_list(doc, index, *ordered, *start, items, prev),
        Block::Code { language, code } => write_code(index, language.as_deref(), code),
        Block::Math { source } => write_math(index, source),
        Block::ThematicBreak => Ok(String::from("***")),
        Block::Media { source, alt_text } => write_image(index, source, alt_text.as_deref()),
        Block::Grid { .. } => Err(fail("live grids are not in the Markdown subset yet")),
        Block::Query(_) => Err(fail("queries are not in the Markdown subset yet")),
        Block::Container { .. } => Err(fail("nested containers are not supported yet")),
    }
}

fn write_text(index: usize, content: &str, style: &TextStyle) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if style.bold || style.italic {
        return Err(fail(
            "bold and italic are not written yet; remove the flags",
        ));
    }
    if content.is_empty() {
        return Err(fail("empty text cannot be written"));
    }
    if content.trim() != content {
        return Err(fail("leading or trailing spaces would be lost"));
    }
    let escaped = write_inline(content, &style.runs).map_err(fail)?;

    if let Some(level) = style.heading_level {
        if !(1..=6).contains(&level) {
            return Err(fail("heading level must be 1 to 6"));
        }
        if content.contains('\n') {
            return Err(fail("headings must be a single line"));
        }
        return Ok(format!("{} {}", "#".repeat(level as usize), escaped));
    }

    for line in escaped.split('\n') {
        if line.trim().is_empty() {
            return Err(fail("paragraphs cannot contain blank lines"));
        }
        if starts_block(line) {
            return Err(fail("paragraph line would read back as other syntax"));
        }
    }
    Ok(escaped)
}

fn write_list(
    doc: &Document,
    index: usize,
    ordered: bool,
    start: u32,
    items: &[BlockId],
    prev: Option<&Block>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if items.is_empty() {
        return Err(fail("an empty list cannot be written"));
    }
    if !ordered && start != 1 {
        return Err(fail("bulleted lists always start at 1"));
    }
    if ordered && (start > MAX_LIST_NUMBER || (items.len() as u32 - 1) > MAX_LIST_NUMBER - start) {
        return Err(fail("list numbers are out of range"));
    }
    if let Some(Block::List {
        ordered: prev_ordered,
        ..
    }) = prev
    {
        if *prev_ordered == ordered {
            return Err(fail(
                "two adjacent lists of the same kind would read back as one list; put a paragraph or other block between them",
            ));
        }
    }
    if ordered && start != 1 {
        if let Some(Block::Text { style, .. }) = prev {
            if style.heading_level.is_none() {
                return Err(fail(
                    "a numbered list that does not start at 1 cannot follow a paragraph",
                ));
            }
        }
    }

    let mut lines: Vec<String> = Vec::with_capacity(items.len());
    for (i, id) in items.iter().enumerate() {
        let item = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("list item {} is missing", id.0),
        })?;
        let (text, style) = match item {
            Block::Text { content, style } => (content, style),
            _ => return Err(fail("list items must be text")),
        };
        if style.bold || style.italic || style.heading_level.is_some() {
            return Err(fail("list items must be plain text"));
        }
        let content = text.as_str();
        if content.is_empty() || content.trim() != content {
            return Err(fail(
                "list items must be non-empty, without leading or trailing spaces",
            ));
        }
        if content.contains(['\n', '\r']) {
            return Err(fail("list items must be single lines"));
        }
        let escaped = write_inline(content, &style.runs).map_err(fail)?;
        // "- - -" would read back as a thematic break, not as an item.
        if escaped
            .bytes()
            .all(|b| matches!(b, b'-' | b'*' | b'_' | b' '))
        {
            return Err(fail(
                "list item text made only of rule characters would read back as a break",
            ));
        }
        let line = if ordered {
            format!("{}. {}", start + i as u32, escaped)
        } else {
            format!("- {escaped}")
        };
        match list_item(&line) {
            Some(marker) if marker.text == escaped.as_str() && marker.ordered == ordered => {}
            _ => return Err(fail("list item would read back as other syntax")),
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

fn write_code(index: usize, language: Option<&str>, code: &str) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if code.contains('\r') {
        return Err(fail("code cannot contain carriage returns"));
    }
    if let Some(lang) = language {
        if lang.is_empty() || lang.trim() != lang {
            return Err(fail(
                "code language must be non-empty, without leading or trailing spaces",
            ));
        }
        if lang.contains(['\n', '`', '\\', '&']) {
            return Err(fail(
                "code language cannot contain line breaks, backticks, backslashes or entities",
            ));
        }
    }
    // A fence longer than any backtick run in the code can never be closed by a code line.
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(core::cmp::max(3, longest + 1));
    let mut out = String::new();
    out.push_str(&fence);
    if let Some(lang) = language {
        out.push_str(lang);
    }
    out.push('\n');
    out.push_str(code);
    out.push('\n');
    out.push_str(&fence);
    Ok(out)
}

fn write_math(index: usize, source: &str) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if source.is_empty() {
        return Err(fail("math block is empty"));
    }
    if source.contains('\r') {
        return Err(fail("math source cannot contain carriage returns"));
    }
    // A source line that reads as the closing delimiter would end the block early.
    if source.split('\n').any(math_closes) {
        return Err(fail(
            "math source cannot contain a line with only $$, which would close the block",
        ));
    }
    Ok(format!("$$\n{source}\n$$"))
}

fn write_image(
    index: usize,
    source: &MediaSource,
    alt: Option<&str>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let uri = match source {
        MediaSource::Uri { uri } => uri,
        MediaSource::Svg { .. } => {
            return Err(fail("inline SVG is not in the Markdown subset yet"));
        }
    };
    if uri.is_empty()
        || uri.contains(|c: char| c.is_whitespace() || c.is_control())
        || uri.contains(['(', ')', '<', '>', '\\', '&'])
    {
        return Err(fail(
            "image uri cannot contain spaces, parentheses, angle brackets, backslashes or entities",
        ));
    }
    if alt == Some("") {
        return Err(fail("empty alt text is read back as no alt text; use None"));
    }
    let alt = alt.unwrap_or("");
    if alt.contains(['[', ']', '\\', '&', '\n', '\r']) {
        return Err(fail(
            "image alt text cannot contain brackets, backslashes, entities or line breaks",
        ));
    }
    let line = format!("![{alt}]({uri})");
    match image_line(&line) {
        Some(Ok((a, u))) if a == alt && u == uri => Ok(line),
        _ => Err(fail("image would read back differently")),
    }
}
