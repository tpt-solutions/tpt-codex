// SPDX-License-Identifier: MIT OR Apache-2.0
//! Flow projection: lays the document out as one continuous column.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, SheetId, TextStyle};

use crate::measure::TextMeasure;

/// Options for the flow projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowOptions {
    /// Column width in points. Must be positive.
    pub width: f64,
    /// Space added after each block, in points.
    pub block_spacing: f64,
    /// Height of a placeholder for blocks that are not text (media, queries, grids whose sheet
    /// is missing).
    pub box_height: f64,
    /// Height of one grid row, in points.
    pub row_height: f64,
}

/// What a flow item draws.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowKind {
    /// One line of wrapped text.
    Line {
        /// The text of this line.
        text: String,
        /// The style it is set in.
        style: TextStyle,
    },
    /// A non-text block, reserved as an opaque box.
    Box,
    /// One row of a grid sheet. The cell values are the host's to draw.
    GridRow {
        /// The sheet the row belongs to.
        sheet: SheetId,
        /// 1-based row number.
        row: u32,
        /// Whether the row is a header, repeated on continuation pages.
        header: bool,
    },
}

/// A positioned piece of the flow.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowItem {
    /// The block this item came from.
    pub block: BlockId,
    /// Top edge, measured from the top of the column.
    pub y: f64,
    /// Height of the item.
    pub height: f64,
    /// What to draw.
    pub kind: FlowKind,
}

/// The result of a flow layout.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowLayout {
    /// Items in reading order.
    pub items: Vec<FlowItem>,
    /// Total height of the column, including trailing spacing.
    pub height: f64,
}

/// Why a layout could not be produced.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutError {
    /// The column width is not a positive number.
    InvalidWidth(f64),
    /// The document root is not a flow container.
    RootNotFlow,
    /// A child id does not exist.
    MissingBlock(BlockId),
}

impl core::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidWidth(w) => write!(f, "column width {w} must be positive"),
            Self::RootNotFlow => f.write_str("document root must be a flow container"),
            Self::MissingBlock(id) => write!(f, "block {} does not exist", id.0),
        }
    }
}

impl core::error::Error for LayoutError {}

/// Remembers how each text block was wrapped, so that a later layout re-measures only the blocks
/// that changed. A block is reused when its content, style and the column width are the same.
#[derive(Debug, Clone, Default)]
pub struct FlowCache {
    blocks: BTreeMap<BlockId, CachedBlock>,
}

#[derive(Debug, Clone)]
struct CachedBlock {
    content: String,
    style: TextStyle,
    width: f64,
    lines: Vec<String>,
    line_height: f64,
}

impl FlowCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of blocks cached.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Lays out `doc` as a single flow column.
pub fn layout_flow(
    doc: &Document,
    measure: &dyn TextMeasure,
    options: FlowOptions,
) -> Result<FlowLayout, LayoutError> {
    layout_flow_cached(doc, measure, options, &mut FlowCache::new())
}

/// Lays out `doc` as a single flow column, reusing `cache` for blocks that have not changed.
/// The result is the same as [`layout_flow`]. Cache entries for blocks that no longer exist are
/// dropped.
pub fn layout_flow_cached(
    doc: &Document,
    measure: &dyn TextMeasure,
    options: FlowOptions,
    cache: &mut FlowCache,
) -> Result<FlowLayout, LayoutError> {
    if options.width.is_nan() || options.width <= 0.0 {
        return Err(LayoutError::InvalidWidth(options.width));
    }
    match doc.get(doc.root) {
        Some(Block::Container {
            layout_mode: LayoutMode::Flow,
            ..
        }) => {}
        _ => return Err(LayoutError::RootNotFlow),
    }
    let mut out = FlowLayout {
        items: Vec::new(),
        height: 0.0,
    };
    flow_children(doc, doc.root, measure, options, cache, &mut out)?;
    cache.blocks.retain(|id, _| doc.blocks.contains_key(id));
    Ok(out)
}

/// The wrapped lines and line height of text block `id`, from the cache when it is current.
fn wrapped_text(
    id: BlockId,
    content: &str,
    style: &TextStyle,
    measure: &dyn TextMeasure,
    width: f64,
    cache: &mut FlowCache,
) -> (Vec<String>, f64) {
    if let Some(hit) = cache.blocks.get(&id) {
        if hit.content == content && hit.style == *style && hit.width == width {
            return (hit.lines.clone(), hit.line_height);
        }
    }
    let lines = wrap(content, style, measure, width);
    let line_height = measure.line_height(style);
    cache.blocks.insert(
        id,
        CachedBlock {
            content: String::from(content),
            style: style.clone(),
            width,
            lines: lines.clone(),
            line_height,
        },
    );
    (lines, line_height)
}

fn flow_children(
    doc: &Document,
    container: BlockId,
    measure: &dyn TextMeasure,
    options: FlowOptions,
    cache: &mut FlowCache,
    out: &mut FlowLayout,
) -> Result<(), LayoutError> {
    let children = match doc.get(container) {
        Some(Block::Container { children, .. }) => children,
        _ => return Ok(()),
    };
    for &id in children {
        let block = doc.get(id).ok_or(LayoutError::MissingBlock(id))?;
        match block {
            Block::Text { content, style } => {
                let (lines, height) =
                    wrapped_text(id, content, style, measure, options.width, cache);
                for text in lines {
                    out.items.push(FlowItem {
                        block: id,
                        y: out.height,
                        height,
                        kind: FlowKind::Line {
                            text,
                            style: style.clone(),
                        },
                    });
                    out.height += height;
                }
                out.height += options.block_spacing;
            }
            Block::Container {
                layout_mode: LayoutMode::Flow,
                ..
            } => {
                flow_children(doc, id, measure, options, cache, out)?;
            }
            Block::Grid { sheet_id, .. } if doc.sheets.contains_key(sheet_id) => {
                let sheet = &doc.sheets[sheet_id];
                let (rows, _) = sheet.extent();
                let header_rows = sheet.header_rows;
                for row in 1..=rows {
                    out.items.push(FlowItem {
                        block: id,
                        y: out.height,
                        height: options.row_height,
                        kind: FlowKind::GridRow {
                            sheet: *sheet_id,
                            row,
                            header: row <= header_rows,
                        },
                    });
                    out.height += options.row_height;
                }
                out.height += options.block_spacing;
            }
            _ => {
                out.items.push(FlowItem {
                    block: id,
                    y: out.height,
                    height: options.box_height,
                    kind: FlowKind::Box,
                });
                out.height += options.box_height + options.block_spacing;
            }
        }
    }
    Ok(())
}

/// Greedy word wrap. Hard line breaks in `content` are kept. Words wider than the
/// column are split between characters.
pub(crate) fn wrap(
    content: &str,
    style: &TextStyle,
    measure: &dyn TextMeasure,
    width: f64,
) -> Vec<String> {
    let mut lines = Vec::new();
    for hard in content.split('\n') {
        let mut current = String::new();
        for word in hard.split_whitespace() {
            let candidate = if current.is_empty() {
                String::from(word)
            } else {
                let mut c = current.clone();
                c.push(' ');
                c.push_str(word);
                c
            };
            if measure.width(&candidate, style) <= width {
                current = candidate;
                continue;
            }
            if !current.is_empty() {
                lines.push(core::mem::take(&mut current));
            }
            if measure.width(word, style) <= width {
                current = String::from(word);
                continue;
            }
            // The word alone is too wide: split it by characters.
            let mut chunk = String::new();
            for ch in word.chars() {
                chunk.push(ch);
                if chunk.chars().count() > 1 && measure.width(&chunk, style) > width {
                    chunk.pop();
                    lines.push(core::mem::take(&mut chunk));
                    chunk.push(ch);
                }
            }
            current = chunk;
        }
        lines.push(current);
    }
    lines
}
