// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::id::{BlockId, SheetId};
use crate::primitives::{LayoutMode, MediaSource, Rect, TextStyle};

/// A node in the document DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A paragraph or text run, shaped by `tpt-text`.
    Text {
        /// The text content.
        content: String,
        /// Styling applied to the content.
        style: TextStyle,
    },
    /// A live, evaluatable grid, inline or as a full-sheet reference.
    Grid {
        /// The sheet this block displays.
        sheet_id: SheetId,
        /// Visible region when embedded inline; `None` shows the whole sheet.
        view_bounds: Option<Rect>,
    },
    /// Images, SVGs, or embedded drawing commands.
    Media {
        /// Where the media comes from.
        source: MediaSource,
        /// Accessibility text.
        alt_text: Option<String>,
    },
    /// Groups blocks, e.g. a slide, a column, or a section.
    Container {
        /// How the children are laid out.
        layout_mode: LayoutMode,
        /// Child blocks, in order.
        children: Vec<BlockId>,
    },
    /// A dynamically evaluated database or data query, evaluated by `tpt-query`.
    Query(Query),
    /// An ordered or bulleted list. Each item is a `Block::Text` with default style.
    List {
        /// `true` for a numbered list, `false` for a bulleted list.
        ordered: bool,
        /// Number of the first item. Always `1` for bulleted lists.
        start: u32,
        /// The item blocks, in order.
        items: Vec<BlockId>,
    },
    /// A fenced code block. The content is stored exactly as written.
    Code {
        /// The info string's language, if any.
        language: Option<String>,
        /// The code, lines joined with `\n`, with no trailing line break.
        code: String,
    },
    /// A horizontal rule.
    ThematicBreak,
}

/// A query whose result is projected into the document.
///
/// The concrete query language belongs to `tpt-query`; this crate stores the
/// source text verbatim so documents round-trip without interpreting it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    /// Query source text in the `tpt-query` language.
    pub source: String,
}
