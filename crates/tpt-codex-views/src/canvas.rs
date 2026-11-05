// SPDX-License-Identifier: MIT OR Apache-2.0
//! Canvas projection: places the children of an absolute container (a slide) at their frames.

use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Canvas, Document, LayoutMode, Rect};

/// One child of a slide, with its frame. A child without a frame has `frame: None` and is not
/// drawn at any position.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasItem {
    /// The child block.
    pub block: BlockId,
    /// Where it is on the slide, in points. `None` if no frame has been set.
    pub frame: Option<Rect>,
}

/// A slide laid out for drawing. Items are in z-order: later items are drawn over earlier ones.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLayout {
    /// Slide width in points.
    pub width: f64,
    /// Slide height in points.
    pub height: f64,
    /// The children of the slide, in z-order.
    pub items: Vec<CanvasItem>,
}

/// Why a slide could not be laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasError {
    /// The block does not exist or is not an absolute container.
    NotASlide(BlockId),
}

impl core::fmt::Display for CanvasError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotASlide(id) => write!(f, "block {} is not an absolute container", id.0),
        }
    }
}

impl core::error::Error for CanvasError {}

/// Lays out slide `slide`. A slide without a canvas size is 16:9 ([`Canvas::WIDESCREEN`]).
pub fn layout_canvas(doc: &Document, slide: BlockId) -> Result<CanvasLayout, CanvasError> {
    let children = match doc.get(slide) {
        Some(Block::Container {
            layout_mode: LayoutMode::Absolute,
            children,
        }) => children,
        _ => return Err(CanvasError::NotASlide(slide)),
    };
    let size = doc
        .canvases
        .get(&slide)
        .copied()
        .unwrap_or(Canvas::WIDESCREEN);
    Ok(CanvasLayout {
        width: size.width,
        height: size.height,
        items: children
            .iter()
            .map(|&block| CanvasItem {
                block,
                frame: doc.frames.get(&block).copied(),
            })
            .collect(),
    })
}

/// The slides of a document: the absolute containers directly under the root, in order.
pub fn slide_ids(doc: &Document) -> Vec<BlockId> {
    match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    doc.get(*id),
                    Some(Block::Container {
                        layout_mode: LayoutMode::Absolute,
                        ..
                    })
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}
