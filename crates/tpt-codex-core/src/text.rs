// SPDX-License-Identifier: MIT OR Apache-2.0
//! Text editing on `Block::Text`, by character position.
//!
//! Positions and ranges count Unicode scalar values (`char`), not bytes or grapheme clusters.
//! Grapheme-aware caret movement belongs to the host, which knows the shaping rules.

use alloc::string::String;

use crate::block::Block;
use crate::document::Document;
use crate::history::History;
use crate::id::BlockId;
use crate::ops::{EditError, Op};

/// Returns `content` with the characters in `start..end` replaced by `insert`.
///
/// Returns `None` if `start > end` or `end` is past the end of the text.
pub fn replace_chars(content: &str, start: usize, end: usize, insert: &str) -> Option<String> {
    let len = content.chars().count();
    if start > end || end > len {
        return None;
    }
    let mut out = String::with_capacity(content.len() + insert.len());
    out.extend(content.chars().take(start));
    out.push_str(insert);
    out.extend(content.chars().skip(end));
    Some(out)
}

/// Replaces `start..end` of text block `id` with `insert` as one undoable step.
///
/// The paragraph style is kept. Use [`Op::SetBlock`] directly to change the style.
pub fn edit_text(
    doc: &mut Document,
    history: &mut History,
    id: BlockId,
    start: usize,
    end: usize,
    insert: &str,
) -> Result<(), EditError> {
    let (content, style) = match doc.get(id) {
        Some(Block::Text { content, style }) => (content, style),
        Some(_) => return Err(EditError::NotText(id)),
        None => return Err(EditError::NoSuchBlock(id)),
    };
    let len = content.chars().count();
    let new_content = replace_chars(content, start, end, insert)
        .ok_or(EditError::RangeOutOfBounds { id, len })?;
    let block = Block::Text {
        content: new_content,
        style: style.clone(),
    };
    history.commit(doc, alloc::vec![Op::SetBlock { id, block }])
}
