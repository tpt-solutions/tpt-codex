// SPDX-License-Identifier: MIT OR Apache-2.0
//! Caret and selection inside one `Block::Text`, with the edit commands that use them.
//!
//! Positions count Unicode scalar values, like [`crate::edit_text`]. A [`Selection`] is a
//! caret when `anchor == focus`. Every command that changes text goes through [`History`],
//! so it can be undone.

use crate::block::Block;
use crate::document::Document;
use crate::history::History;
use crate::id::BlockId;
use crate::ops::EditError;
use crate::text::edit_text;

/// A caret or range inside one text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The text block the selection is in.
    pub block: BlockId,
    /// Where the selection started. Stays fixed while extending.
    pub anchor: usize,
    /// The moving end. This is where the caret is.
    pub focus: usize,
}

/// A caret movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// One character left.
    Left,
    /// One character right.
    Right,
    /// To the start of the text.
    Start,
    /// To the end of the text.
    End,
}

impl Selection {
    /// A collapsed caret at `pos` in `block`.
    pub fn caret(block: BlockId, pos: usize) -> Self {
        Self {
            block,
            anchor: pos,
            focus: pos,
        }
    }

    /// A range from `anchor` to `focus`, in either order.
    pub fn range(block: BlockId, anchor: usize, focus: usize) -> Self {
        Self {
            block,
            anchor,
            focus,
        }
    }

    /// `true` when there is no selected text, only a caret.
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    /// The selected range as `(start, end)` with `start <= end`.
    pub fn bounds(&self) -> (usize, usize) {
        (self.anchor.min(self.focus), self.anchor.max(self.focus))
    }

    /// Moves the caret. With `extend`, only the focus moves and the range grows or shrinks.
    /// Without it, a non-empty range collapses to the side the movement points at.
    ///
    /// Fails if the block is missing or is not text.
    pub fn move_caret(
        &mut self,
        doc: &Document,
        movement: Movement,
        extend: bool,
    ) -> Result<(), EditError> {
        let len = text_len(doc, self.block)?;
        let (start, end) = self.bounds();
        let target = match movement {
            Movement::Left if !extend && !self.is_collapsed() => start,
            Movement::Right if !extend && !self.is_collapsed() => end,
            Movement::Left => self.focus.saturating_sub(1),
            Movement::Right => (self.focus + 1).min(len),
            Movement::Start => 0,
            Movement::End => len,
        };
        self.focus = target.min(len);
        if !extend {
            self.anchor = self.focus;
        }
        Ok(())
    }

    /// Selects the whole text of the block.
    pub fn select_all(&mut self, doc: &Document) -> Result<(), EditError> {
        let len = text_len(doc, self.block)?;
        self.anchor = 0;
        self.focus = len;
        Ok(())
    }
}

/// Replaces the selected range with `text` and puts the caret after it.
///
/// An empty `text` deletes the selection. A collapsed selection inserts at the caret.
pub fn insert_text(
    doc: &mut Document,
    history: &mut History,
    selection: &mut Selection,
    text: &str,
) -> Result<(), EditError> {
    let (start, end) = selection.bounds();
    edit_text(doc, history, selection.block, start, end, text)?;
    let caret = start + text.chars().count();
    *selection = Selection::caret(selection.block, caret);
    Ok(())
}

/// Deletes the selected range, or the character before a collapsed caret.
///
/// Returns `Ok(false)` when there was nothing to delete (caret at the start).
pub fn delete_backward(
    doc: &mut Document,
    history: &mut History,
    selection: &mut Selection,
) -> Result<bool, EditError> {
    if selection.is_collapsed() {
        if selection.focus == 0 {
            return Ok(false);
        }
        *selection = Selection::range(selection.block, selection.focus - 1, selection.focus);
    }
    insert_text(doc, history, selection, "")?;
    Ok(true)
}

/// Deletes the selected range, or the character after a collapsed caret.
///
/// Returns `Ok(false)` when there was nothing to delete (caret at the end).
pub fn delete_forward(
    doc: &mut Document,
    history: &mut History,
    selection: &mut Selection,
) -> Result<bool, EditError> {
    if selection.is_collapsed() {
        let len = text_len(doc, selection.block)?;
        if selection.focus >= len {
            return Ok(false);
        }
        *selection = Selection::range(selection.block, selection.focus, selection.focus + 1);
    }
    insert_text(doc, history, selection, "")?;
    Ok(true)
}

/// The number of characters in a text block.
fn text_len(doc: &Document, id: BlockId) -> Result<usize, EditError> {
    match doc.get(id) {
        Some(Block::Text { content, .. }) => Ok(content.chars().count()),
        Some(_) => Err(EditError::NotText(id)),
        None => Err(EditError::NoSuchBlock(id)),
    }
}
