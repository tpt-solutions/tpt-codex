// SPDX-License-Identifier: MIT OR Apache-2.0
//! Flow projection: lays the document out as one continuous column.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, SheetId, TextStyle};

use crate::measure::TextMeasure;

/// Indent of list item text from the left edge of the column, in points. The marker sits in
/// this space.
pub const LIST_INDENT: f64 = 18.0;

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
    /// One line of wrapped text. Text in a list item is indented by `indent`, and the first line
    /// of an item carries its marker (a bullet, or a number such as `2.`).
    Line {
        /// The text of this line.
        text: String,
        /// The style it is set in.
        style: TextStyle,
        /// The list marker, on the first line of a list item. `None` for other text.
        marker: Option<String>,
        /// Space before the text, in points, from the left edge of the column. `0.0` for other
        /// text.
        indent: f64,
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
    /// Whether lines are broken at hyphenation points. Set by the layout that uses the cache.
    hyphenate: bool,
}

#[derive(Debug, Clone)]
struct CachedBlock {
    content: String,
    style: TextStyle,
    width: f64,
    hyphenate: bool,
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

/// Lays out `doc` as a single flow column, breaking long words at hyphenation points (see the
/// `hyphenate` module). The result is the same as [`layout_flow`] except where a word is broken.
pub fn layout_flow_hyphenated(
    doc: &Document,
    measure: &dyn TextMeasure,
    options: FlowOptions,
) -> Result<FlowLayout, LayoutError> {
    let mut cache = FlowCache {
        hyphenate: true,
        ..FlowCache::default()
    };
    layout_flow_cached(doc, measure, options, &mut cache)
}

/// Lays out `doc` as a single flow column, reusing `cache` for blocks that have not changed. The
/// cache's hyphenation setting applies, and is `false` in a new cache.
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
    let mut pending = 0.0;
    flow_children(
        doc,
        doc.root,
        measure,
        options,
        cache,
        &mut out,
        &mut pending,
    )?;
    // The last block's bottom margin is kept, as trailing space in the column.
    out.height += pending;
    cache.blocks.retain(|id, _| doc.blocks.contains_key(id));
    Ok(out)
}

/// The margins above and below a text block, in points. A heading gets more space above it
/// than body text, and the space shrinks with the level, so headings read as a hierarchy.
fn text_margins(style: &TextStyle, block_spacing: f64) -> (f64, f64) {
    match style.heading_level {
        Some(level) => {
            let above = if level <= 2 { 2.0 } else { 1.5 };
            (block_spacing * above, block_spacing)
        }
        None => (0.0, block_spacing),
    }
}

/// Starts a block at the current height, after the gap its top margin and the previous block's
/// bottom margin make. Adjacent margins collapse: the gap is the larger of the two, not their
/// sum. Nothing is gapped before the first item. Returns the block's top edge.
fn start_block(out: &mut FlowLayout, pending: &mut f64, top: f64) -> f64 {
    let gap = if out.items.is_empty() {
        0.0
    } else {
        pending.max(top)
    };
    *pending = 0.0;
    out.height += gap;
    out.height
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
    let hyphenate = cache.hyphenate;
    if let Some(hit) = cache.blocks.get(&id) {
        if hit.content == content
            && hit.style == *style
            && hit.width == width
            && hit.hyphenate == hyphenate
        {
            return (hit.lines.clone(), hit.line_height);
        }
    }
    let lines = wrap(content, style, measure, width, hyphenate);
    let line_height = measure.line_height(style);
    cache.blocks.insert(
        id,
        CachedBlock {
            content: String::from(content),
            style: style.clone(),
            width,
            hyphenate,
            lines: lines.clone(),
            line_height,
        },
    );
    (lines, line_height)
}

/// Lays out the children of `container`. `pending` is the bottom margin of the last block placed,
/// which the next block collapses with its top margin.
fn flow_children(
    doc: &Document,
    container: BlockId,
    measure: &dyn TextMeasure,
    options: FlowOptions,
    cache: &mut FlowCache,
    out: &mut FlowLayout,
    pending: &mut f64,
) -> Result<(), LayoutError> {
    let children = match doc.get(container) {
        Some(Block::Container { children, .. }) => children,
        _ => return Ok(()),
    };
    for &id in children {
        let block = doc.get(id).ok_or(LayoutError::MissingBlock(id))?;
        match block {
            Block::Text { content, style } => {
                let (top, bottom) = text_margins(style, options.block_spacing);
                let (lines, height) =
                    wrapped_text(id, content, style, measure, options.width, cache);
                start_block(out, pending, top);
                for text in lines {
                    out.items.push(FlowItem {
                        block: id,
                        y: out.height,
                        height,
                        kind: FlowKind::Line {
                            text,
                            style: style.clone(),
                            marker: None,
                            indent: 0.0,
                        },
                    });
                    out.height += height;
                }
                *pending = bottom;
            }
            Block::List {
                ordered,
                start,
                items,
            } => {
                // The items of a list stack with no gap between them. Each item's text is wrapped
                // to the width left after the marker indent, and its first line carries the marker.
                start_block(out, pending, 0.0);
                let width = (options.width - LIST_INDENT).max(1.0);
                for (index, &item) in items.iter().enumerate() {
                    let Some(Block::Text { content, style }) = doc.get(item) else {
                        continue;
                    };
                    let (lines, height) = wrapped_text(item, content, style, measure, width, cache);
                    let marker = if *ordered {
                        alloc::format!("{}.", start.saturating_add(index as u32))
                    } else {
                        String::from("\u{2022}")
                    };
                    for (line, text) in lines.into_iter().enumerate() {
                        out.items.push(FlowItem {
                            block: item,
                            y: out.height,
                            height,
                            kind: FlowKind::Line {
                                text,
                                style: style.clone(),
                                marker: (line == 0).then(|| marker.clone()),
                                indent: LIST_INDENT,
                            },
                        });
                        out.height += height;
                    }
                }
                *pending = options.block_spacing;
            }
            Block::Container {
                layout_mode: LayoutMode::Flow,
                ..
            } => {
                flow_children(doc, id, measure, options, cache, out, pending)?;
            }
            Block::Grid {
                sheet_id,
                view_bounds,
            } if doc.sheets.contains_key(sheet_id) => {
                let sheet = &doc.sheets[sheet_id];
                let (rows, _) = sheet.extent();
                let header_rows = sheet.header_rows;
                let shown = grid_rows_shown(rows, header_rows, *view_bounds, options.row_height);
                start_block(out, pending, 0.0);
                for row in shown {
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
                *pending = options.block_spacing;
            }
            _ => {
                let y = start_block(out, pending, 0.0);
                out.items.push(FlowItem {
                    block: id,
                    y,
                    height: options.box_height,
                    kind: FlowKind::Box,
                });
                out.height += options.box_height;
                *pending = options.block_spacing;
            }
        }
    }
    Ok(())
}

/// The rows of a grid that the flow shows, in order (1-based).
///
/// A grid with no view bounds shows every row. With bounds, it is a window onto the sheet: the
/// rows whose height band meets the window's vertical extent, `[y, y + height)` in points from the
/// top of the sheet. The header rows are always shown, first. Columns are not laid out in the flow,
/// so the window is vertical only.
pub fn grid_rows_shown(
    rows: u32,
    header_rows: u32,
    view_bounds: Option<tpt_codex_core::Rect>,
    row_height: f64,
) -> Vec<u32> {
    let Some(window) = view_bounds else {
        return (1..=rows).collect();
    };
    let mut shown: alloc::collections::BTreeSet<u32> = (1..=header_rows.min(rows)).collect();
    if row_height > 0.0 && window.height > 0.0 {
        // Casts truncate towards zero, which is floor for the non-negative values here.
        let top = (window.y.max(0.0) / row_height) as u32; // zero-based first row
        let end = ((window.y + window.height).max(0.0)) / row_height;
        let whole = end as u32;
        let bottom = if f64::from(whole) < end {
            whole + 1
        } else {
            whole
        };
        for zero_based in top..bottom.min(rows) {
            shown.insert(zero_based + 1);
        }
    }
    shown.into_iter().collect()
}

/// Greedy word wrap. Hard line breaks in `content` are kept. Words wider than the
/// column are split between characters.
pub(crate) fn wrap(
    content: &str,
    style: &TextStyle,
    measure: &dyn TextMeasure,
    width: f64,
    hyphenate: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    for hard in content.split('\n') {
        let mut current = String::new();
        for word in hard.split_whitespace() {
            let mut rest = word;
            loop {
                let candidate = if current.is_empty() {
                    String::from(rest)
                } else {
                    let mut c = current.clone();
                    c.push(' ');
                    c.push_str(rest);
                    c
                };
                if measure.width(&candidate, style) <= width {
                    current = candidate;
                    break;
                }
                if hyphenate && !current.is_empty() {
                    // Put the head of the word on this line, with a hyphen, and go on with the tail.
                    if let Some(cut) = fitting_cut(&current, rest, style, measure, width) {
                        lines.push(alloc::format!("{current} {}-", &rest[..cut]));
                        current.clear();
                        rest = &rest[cut..];
                        continue;
                    }
                }
                if !current.is_empty() {
                    lines.push(core::mem::take(&mut current));
                    continue;
                }
                if hyphenate {
                    if let Some(cut) = fitting_cut("", rest, style, measure, width) {
                        lines.push(alloc::format!("{}-", &rest[..cut]));
                        rest = &rest[cut..];
                        continue;
                    }
                }
                // The word alone is too wide: split it by characters.
                let mut chunk = String::new();
                for ch in rest.chars() {
                    chunk.push(ch);
                    if chunk.chars().count() > 1 && measure.width(&chunk, style) > width {
                        chunk.pop();
                        lines.push(core::mem::take(&mut chunk));
                        chunk.push(ch);
                    }
                }
                current = chunk;
                break;
            }
        }
        lines.push(current);
    }
    lines
}

/// The longest hyphenation point of `word` at which `head`, a space, the start of the word and a
/// hyphen fit in `width`. `head` is empty when the word starts its line.
fn fitting_cut(
    head: &str,
    word: &str,
    style: &TextStyle,
    measure: &dyn TextMeasure,
    width: f64,
) -> Option<usize> {
    crate::hyphenate::break_points(word)
        .into_iter()
        .rev()
        .find(|&cut| {
            let prefix = &word[..cut];
            let text = if head.is_empty() {
                alloc::format!("{prefix}-")
            } else {
                alloc::format!("{head} {prefix}-")
            };
            measure.width(&text, style) <= width
        })
}
