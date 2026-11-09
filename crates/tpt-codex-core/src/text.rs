// SPDX-License-Identifier: MIT OR Apache-2.0
//! Text editing on `Block::Text`, by character position.
//!
//! Positions and ranges count Unicode scalar values (`char`), not bytes or grapheme clusters.
//! Grapheme-aware caret movement belongs to the host, which knows the shaping rules.

use alloc::string::String;
use alloc::vec::Vec;

use crate::block::Block;
use crate::document::Document;
use crate::history::History;
use crate::id::BlockId;
use crate::ops::{EditError, Op};
use crate::primitives::InlineRun;

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

/// Returns the inline runs that still apply after `start..end` is replaced by `inserted_len`
/// characters.
///
/// Text outside the replaced range keeps its marks. Text typed strictly inside a run takes
/// that run's marks. Text typed at a run's edge does not, so typing before a link does not
/// extend it. Runs left empty by the edit are dropped.
pub fn remap_runs(
    runs: &[InlineRun],
    start: usize,
    end: usize,
    inserted_len: usize,
) -> Vec<InlineRun> {
    let pure_insert = start == end;
    let delta = inserted_len as isize - (end - start) as isize;
    let shift = |x: usize| (x as isize + delta) as usize;
    let map_start = |x: usize| {
        if x < start {
            x
        } else if x == start {
            // A pure insertion at a run's start goes before the run.
            if pure_insert { x + inserted_len } else { x }
        } else if x < end {
            start + inserted_len
        } else {
            shift(x)
        }
    };
    let map_end = |x: usize| {
        if x <= start {
            x
        } else if x < end {
            start
        } else {
            shift(x)
        }
    };
    runs.iter()
        .filter_map(|run| {
            let s = map_start(run.start);
            let e = map_end(run.end);
            (s < e).then(|| InlineRun {
                start: s,
                end: e,
                math: None,
                link_title: None,
                ..run.clone()
            })
        })
        .collect()
}

/// Returns `runs` clipped to a text of `len` characters. Runs past the end are shortened or
/// dropped.
pub fn clip_runs(runs: &[InlineRun], len: usize) -> Vec<InlineRun> {
    runs.iter()
        .filter_map(|run| {
            let s = run.start.min(len);
            let e = run.end.min(len);
            (s < e).then(|| InlineRun {
                start: s,
                end: e,
                math: None,
                link_title: None,
                ..run.clone()
            })
        })
        .collect()
}

/// Replaces `start..end` of text block `id` with `insert` as one undoable step.
///
/// The paragraph style is kept, and inline runs are moved with the text (see [`remap_runs`]).
/// Use [`Op::SetBlock`] directly to change the style.
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
    let mut style = style.clone();
    style.runs = remap_runs(&style.runs, start, end, insert.chars().count());
    let block = Block::Text {
        content: new_content,
        style,
    };
    history.commit(doc, alloc::vec![Op::SetBlock { id, block }])
}
