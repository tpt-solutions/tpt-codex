// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Axis-aligned rectangle in document units (points).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width (non-negative).
    pub width: f64,
    /// Height (non-negative).
    pub height: f64,
}

/// Character and paragraph styling for a `Block::Text`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStyle {
    /// Bold weight.
    #[serde(default)]
    pub bold: bool,
    /// Italic style.
    #[serde(default)]
    pub italic: bool,
    /// Heading level 1..=6, or `None` for body text.
    #[serde(default)]
    pub heading_level: Option<u8>,
}

/// How the children of a `Block::Container` are positioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    /// Block/inline flow, as in a document.
    Flow,
    /// Grid placement.
    Grid,
    /// Explicit X/Y positioning, as in a slide.
    Absolute,
}

/// Where the bytes of a `Block::Media` come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    /// A reference to an external image by URI or relative path.
    Uri {
        /// The URI or path.
        uri: String,
    },
    /// An inline SVG document.
    Svg {
        /// The SVG source text.
        markup: String,
    },
}
