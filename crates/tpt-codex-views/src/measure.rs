// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::TextStyle;

/// Measures text for layout. Hosts supply the real implementation (`tpt-text` later).
pub trait TextMeasure {
    /// Width of `text` when set in `style`, in points.
    fn width(&self, text: &str, style: &TextStyle) -> f64;

    /// Height of one line of text in `style`, in points.
    fn line_height(&self, style: &TextStyle) -> f64;
}

/// A fixed-pitch measure: every character has the same width. Headless and deterministic.
///
/// Headings are scaled by `heading_scale` per level, so a level-1 heading is larger than body text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonospaceMeasure {
    /// Width of one character at body size.
    pub char_width: f64,
    /// Line height at body size.
    pub line_height: f64,
}

impl MonospaceMeasure {
    fn scale(style: &TextStyle) -> f64 {
        match style.heading_level {
            Some(1) => 2.0,
            Some(2) => 1.6,
            Some(3) => 1.3,
            Some(_) => 1.1,
            None => 1.0,
        }
    }
}

impl TextMeasure for MonospaceMeasure {
    fn width(&self, text: &str, style: &TextStyle) -> f64 {
        let chars = text.chars().count() as f64;
        chars * self.char_width * Self::scale(style)
    }

    fn line_height(&self, style: &TextStyle) -> f64 {
        self.line_height * Self::scale(style)
    }
}
