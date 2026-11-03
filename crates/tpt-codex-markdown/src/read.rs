// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Metadata, TextStyle};

use crate::error::ParseError;
use crate::syntax::{heading, unsupported};

/// Reads the supported Markdown subset into a [`Document`].
///
/// The result always passes [`Document::validate`]. Errors carry a 1-based
/// line and column.
pub fn from_markdown(src: &str) -> Result<Document, ParseError> {
    let lines: Vec<&str> = src.lines().collect();
    let mut metadata = Metadata::default();
    let body_start = read_front_matter(&lines, &mut metadata)?;

    let mut children: Vec<Block> = Vec::new();
    let mut paragraph: Option<String> = None;

    for (offset, line) in lines[body_start..].iter().enumerate() {
        let lineno = body_start + offset + 1;
        if line.trim().is_empty() {
            flush_paragraph(&mut paragraph, &mut children);
        } else if let Some((level, text)) = heading(line) {
            flush_paragraph(&mut paragraph, &mut children);
            if text.is_empty() {
                return Err(ParseError {
                    line: lineno,
                    column: 1,
                    message: String::from("heading has no text"),
                    suggestion: Some("add text after the hashes, or write a paragraph instead"),
                });
            }
            children.push(Block::Text {
                content: String::from(text),
                style: TextStyle {
                    bold: false,
                    italic: false,
                    heading_level: Some(level),
                },
            });
        } else if let Some(reason) = unsupported(line) {
            return Err(ParseError {
                line: lineno,
                column: 1,
                message: alloc::format!("{reason} is not supported yet"),
                suggestion: Some("rewrite as plain paragraphs and headings"),
            });
        } else {
            match &mut paragraph {
                Some(text) => {
                    text.push('\n');
                    text.push_str(line);
                }
                None => paragraph = Some(String::from(*line)),
            }
        }
    }
    flush_paragraph(&mut paragraph, &mut children);

    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=children.len() as u64).map(BlockId).collect(),
        },
    );
    doc.metadata = metadata;
    for (i, block) in children.into_iter().enumerate() {
        doc.insert(BlockId(i as u64 + 1), block);
    }
    Ok(doc)
}

fn flush_paragraph(paragraph: &mut Option<String>, children: &mut Vec<Block>) {
    if let Some(content) = paragraph.take() {
        children.push(Block::Text {
            content,
            style: TextStyle::default(),
        });
    }
}

/// Reads an optional front-matter block. Returns the index of the first body line.
fn read_front_matter(lines: &[&str], metadata: &mut Metadata) -> Result<usize, ParseError> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Ok(0);
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        let lineno = i + 1;
        if line.trim_end() == "---" {
            return Ok(i + 1);
        }
        if line.trim().is_empty() {
            continue;
        }
        let (key, raw) = line.split_once(':').ok_or_else(|| ParseError {
            line: lineno,
            column: 1,
            message: String::from("front matter line is not 'key: value'"),
            suggestion: Some("write each entry as key: value"),
        })?;
        let value = parse_value(raw.trim()).map_err(|message| ParseError {
            line: lineno,
            column: raw.len() - raw.trim_start().len() + key.len() + 2,
            message,
            suggestion: Some("wrap the value in double quotes; escape \\\" and \\\\"),
        })?;
        let slot = match key.trim() {
            "title" => &mut metadata.title,
            "author" => &mut metadata.author,
            "date" => &mut metadata.date,
            "class" => &mut metadata.class,
            other => {
                return Err(ParseError {
                    line: lineno,
                    column: 1,
                    message: alloc::format!("unknown front matter key '{other}'"),
                    suggestion: Some("supported keys: title, author, date, class"),
                });
            }
        };
        *slot = Some(value);
    }
    Err(ParseError {
        line: lines.len(),
        column: 1,
        message: String::from("front matter is not closed"),
        suggestion: Some("add a '---' line after the last front matter entry"),
    })
}

/// Parses a bare or double-quoted value. Quoted values accept `\"` and `\\`.
fn parse_value(raw: &str) -> Result<String, String> {
    if !raw.starts_with('"') {
        return Ok(String::from(raw));
    }
    let inner = raw[1..]
        .strip_suffix('"')
        .ok_or_else(|| String::from("quoted value is missing its closing quote"))?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(e @ ('"' | '\\')) => out.push(e),
                _ => return Err(String::from("invalid escape in quoted value")),
            }
        } else if c == '"' {
            return Err(String::from("unescaped quote inside quoted value"));
        } else {
            out.push(c);
        }
    }
    Ok(out)
}
