// SPDX-License-Identifier: MIT OR Apache-2.0
//! Flow projection: lays the document out as one continuous column.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};

use crate::measure::TextMeasure;

/// Options for the flow projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowOptions {
    /// Column width in points. Must be positive.
    pub width: f64,
    /// Space added after each block, in points.
    pub block_spacing: f64,
    /// Height of a placeholder for blocks that are not text (grids, media, queries).
    pub box_height: f64,
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

/// Lays out `doc` as a single flow column.
pub fn layout_flow(
    doc: &Document,
    measure: &dyn TextMeasure,
    options: FlowOptions,
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
    flow_children(doc, doc.root, measure, options, &mut out)?;
    Ok(out)
}

fn flow_children(
    doc: &Document,
    container: BlockId,
    measure: &dyn TextMeasure,
    options: FlowOptions,
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
                for text in wrap(content, style, measure, options.width) {
                    let height = measure.line_height(style);
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
                flow_children(doc, id, measure, options, out)?;
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
