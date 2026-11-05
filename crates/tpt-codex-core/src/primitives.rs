// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;
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

impl Rect {
    /// Whether every edge is finite and the size is non-negative.
    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height >= 0.0
    }
}

/// The size of a canvas (a slide) in points. Children of an absolute container are placed on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    /// Width (positive).
    pub width: f64,
    /// Height (positive).
    pub height: f64,
}

impl Canvas {
    /// A 16:9 canvas, 960 points wide.
    pub const WIDESCREEN: Canvas = Canvas {
        width: 960.0,
        height: 540.0,
    };

    /// Whether both sides are finite and positive.
    pub fn is_valid(&self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }

    /// Width divided by height. A canvas keeps its aspect ratio for as long as it is not resized.
    pub fn aspect(&self) -> f64 {
        self.width / self.height
    }
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
    /// Inline marks on parts of the text: emphasis and links. A character takes the marks
    /// of every run that covers it, so runs may overlap. Empty for plain text, and then not
    /// written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<InlineRun>,
}

/// Inline marks on a range of characters of a `Block::Text`.
///
/// `start` and `end` count `char`s, as for editing. The range is half-open and never empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineRun {
    /// First character covered.
    pub start: usize,
    /// One past the last character covered.
    pub end: usize,
    /// Bold weight for these characters.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    /// Italic style for these characters.
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Link target, or `None` for text that is not a link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Inline math: the TeX source of the formula, which is also the run's text. `None` for
    /// text that is not math. The source has no `$` and no line breaks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
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
