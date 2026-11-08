// SPDX-License-Identifier: MIT OR Apache-2.0
//! Font-backed text measurement.
//!
//! The fonts are bundled in the binary and are the only fonts used, so that text
//! layout is the same on every platform (system fonts are never looked up).
//! Shaping and line breaking come from `tpt-text`. The result is exposed as
//! [`tpt_codex_views::TextMeasure`] for the Flow view and as
//! [`tpt_layout::text::TextMeasurer`] for the box layout engine.

use std::sync::Arc;

use tpt_codex_core::TextStyle as CoreStyle;
use tpt_codex_views::TextMeasure;
use tpt_layout::px::Px;
use tpt_layout::text::{LineBox, TextMeasurer};
use tpt_text::font::{FontError, FontFace, FontRegistry};
use tpt_text::fx::Fx;
use tpt_text::layout::{LayoutError, ParagraphLayout, layout_paragraph_with};
use tpt_text::style::{FontVariation, TextStyle};
use tpt_typeset::{MathConstants, MathMetrics};

/// Family name used for the bundled font.
pub const FAMILY: &str = "Noto Sans";

/// Noto Sans Regular (variable font), under the SIL Open Font License (see `fonts/OFL.txt`).
/// Its default instance is Regular. Bold is the same font at weight 700.
pub const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../fonts/NotoSans-Regular.ttf");

/// Noto Sans Italic (variable font). Bold italic is this font at weight 700.
pub const NOTO_SANS_ITALIC: &[u8] = include_bytes!("../fonts/NotoSans-Italic.ttf");

/// Body text size, in points. Headings scale from this, as in `MonospaceMeasure`.
pub const BODY_POINTS: f32 = 11.0;

/// Errors from loading the bundled font or laying text out with it.
#[derive(Debug)]
pub enum TextError {
    /// The bundled font could not be parsed.
    Font(FontError),
    /// Text could not be laid out with the bundled font.
    Layout(LayoutError),
}

impl core::fmt::Display for TextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Font(e) => write!(f, "bundled font could not be loaded: {e:?}"),
            Self::Layout(e) => write!(f, "text could not be laid out: {e:?}"),
        }
    }
}

impl std::error::Error for TextError {}

/// Text measurement backed by the bundled font.
#[derive(Debug)]
pub struct FontMeasure {
    registry: FontRegistry,
    regular: Arc<FontFace>,
    _faces: Vec<Arc<FontFace>>,
    /// Layout constants from the bundled math font's MATH table.
    math: Option<MathConstants>,
}

/// The weight used for bold text. The bundled fonts are variable, so bold is this weight on
/// the `wght` axis, not a separate file.
pub const BOLD_WEIGHT: f32 = 700.0;

impl FontMeasure {
    /// Loads the bundled fonts.
    pub fn bundled() -> Result<Self, TextError> {
        let registry = FontRegistry::new();
        let regular = registry
            .load_from_memory(NOTO_SANS_REGULAR.to_vec(), 0, FAMILY, 400, false)
            .map_err(TextError::Font)?;
        let italic = registry
            .load_from_memory(NOTO_SANS_ITALIC.to_vec(), 0, FAMILY, 400, true)
            .map_err(TextError::Font)?;
        Ok(Self {
            registry,
            regular: Arc::clone(&regular),
            _faces: vec![regular, italic],
            math: tpt_typeset::math_constants(tpt_typeset::STIX_TWO_MATH),
        })
    }

    /// Lays out `text` at `size` points with the given emphasis, breaking lines at `max_width`.
    fn layout(
        &self,
        text: &str,
        size: f32,
        emphasis: Emphasis,
        max_width: Fx,
    ) -> Result<ParagraphLayout, TextError> {
        let mut style = TextStyle::new(FAMILY, Fx::from_f32(size));
        style.italic = emphasis.italic;
        if emphasis.bold {
            style.variations = vec![FontVariation {
                tag: *b"wght",
                value: Fx::from_f32(BOLD_WEIGHT),
            }];
        }
        layout_paragraph_with(
            &self.registry,
            text,
            &style,
            &[],
            max_width,
            &Default::default(),
        )
        .map_err(TextError::Layout)
    }

    /// Size of a view style, in points. The heading scale matches `MonospaceMeasure`.
    fn points(style: &CoreStyle) -> f32 {
        let scale = match style.heading_level {
            Some(1) => 2.0,
            Some(2) => 1.6,
            Some(3) => 1.3,
            Some(_) => 1.1,
            None => 1.0,
        };
        BODY_POINTS * scale
    }
}

/// The emphasis of a piece of text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Emphasis {
    bold: bool,
    italic: bool,
}

impl Emphasis {
    fn of(style: &CoreStyle) -> Self {
        Self {
            bold: style.bold,
            italic: style.italic,
        }
    }
}

/// Advance width of one line, in points.
fn line_width(line: &tpt_text::layout::LayoutLine) -> f32 {
    line.runs
        .iter()
        .flat_map(|run| run.glyphs.iter())
        .map(|g| g.x_advance.to_f32())
        .sum()
}

/// A limit so large that text is never wrapped.
fn unbounded() -> Fx {
    Fx::from_int(1_000_000)
}

impl TextMeasure for FontMeasure {
    fn width(&self, text: &str, style: &CoreStyle) -> f64 {
        let size = Self::points(style);
        match self.layout(text, size, Emphasis::of(style), unbounded()) {
            Ok(layout) => f64::from(layout.lines.iter().map(line_width).fold(0.0, f32::max)),
            // The font is checked when it is loaded, so this is not expected in practice.
            Err(_) => text.chars().count() as f64 * f64::from(size) * 0.5,
        }
    }

    fn line_height(&self, style: &CoreStyle) -> f64 {
        let size = Self::points(style);
        match self.layout("Ag", size, Emphasis::of(style), unbounded()) {
            Ok(layout) => f64::from(layout.total_height.to_f32()),
            Err(_) => f64::from(size) * 1.2,
        }
    }
}

impl MathMetrics for FontMeasure {
    fn advance(&self, ch: char, size: f64) -> f64 {
        let mut buf = [0u8; 4];
        let text: &str = ch.encode_utf8(&mut buf);
        match self.layout(text, size as f32, Emphasis::default(), unbounded()) {
            Ok(layout) => f64::from(layout.lines.iter().map(line_width).fold(0.0, f32::max)),
            Err(_) => size * 0.5,
        }
    }

    fn ascent(&self, size: f64) -> f64 {
        let upm = f64::from(self.regular.units_per_em().max(1));
        f64::from(self.regular.ascender()) / upm * size
    }

    fn descent(&self, size: f64) -> f64 {
        let upm = f64::from(self.regular.units_per_em().max(1));
        f64::from(self.regular.descender()).abs() / upm * size
    }

    fn math_constants(&self) -> Option<MathConstants> {
        self.math
    }
}

impl TextMeasurer for FontMeasure {
    fn line_boxes(&self, text: &str, max_width: Px) -> Vec<LineBox> {
        let width = Fx::from_f32(max_width.to_f32().max(1.0));
        let Ok(layout) = self.layout(text, BODY_POINTS, Emphasis::default(), width) else {
            return Vec::new();
        };
        layout
            .lines
            .iter()
            .map(|line| LineBox {
                width: Px::from_f32(line_width(line)),
                height: Px::from_f32(line.height.to_f32()),
                baseline: Px::from_f32((line.baseline_y - line.top).to_f32()),
                text: line.text_range.clone(),
            })
            .collect()
    }
}

/// One glyph of a shaped run: its id in the face, and its origin in points relative to the start
/// of the line, on the baseline, y down.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphPlacement {
    /// Glyph id in the face.
    pub glyph: u32,
    /// Horizontal origin, in points from the start of the line.
    pub x: f32,
    /// Vertical origin, in points from the baseline. Positive is down.
    pub y: f32,
    /// Horizontal advance of the glyph, in points.
    pub advance: f32,
    /// The characters this glyph stands for. A cluster of characters (a ligature, for example) is
    /// one or more glyphs, and only its first glyph carries the text; the others are empty.
    pub text: String,
}

/// Glyphs from one face at one size.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    /// Index into [`FontMeasure::faces`]: 0 is the regular face, 1 the italic face.
    pub face: usize,
    /// Size in points.
    pub size: f32,
    /// Glyphs in visual order.
    pub glyphs: Vec<GlyphPlacement>,
}

/// One laid-out line of text.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphLine {
    /// Top of the line, in points from the top of the text block.
    pub top: f32,
    /// Baseline, in points from the top of the text block.
    pub baseline: f32,
    /// Runs in visual order.
    pub runs: Vec<GlyphRun>,
}

impl FontMeasure {
    /// The faces that [`GlyphRun::face`] indexes: regular first, then italic.
    ///
    /// Bold text is shaped with the `wght` axis set, so its advances match bold, but the outlines
    /// come from the regular face at its default weight. A caller that draws these outlines
    /// draws bold text at regular weight.
    pub fn faces(&self) -> Vec<Arc<FontFace>> {
        self._faces.clone()
    }

    /// Shapes `text` in `style` into lines, breaking at `max_width` points, or never breaking
    /// when `max_width` is `None`. Line breaks and advances are the same as [`TextMeasure`] and
    /// [`TextMeasurer`] use.
    pub fn shape_lines(
        &self,
        text: &str,
        style: &CoreStyle,
        max_width: Option<f32>,
    ) -> Result<Vec<GlyphLine>, TextError> {
        let size = Self::points(style);
        let emphasis = Emphasis::of(style);
        let width = max_width.map_or_else(unbounded, |w| Fx::from_f32(w.max(1.0)));
        let layout = self.layout(text, size, emphasis, width)?;
        // `layout` picks the italic face when italic is requested, and the regular face otherwise.
        let face = usize::from(emphasis.italic);
        // A cluster runs from its start byte to the next cluster start in the text, or the end.
        let mut starts: Vec<usize> = layout
            .lines
            .iter()
            .flat_map(|l| l.runs.iter())
            .flat_map(|r| r.glyphs.iter())
            .map(|g| g.cluster_index)
            .chain(core::iter::once(text.len()))
            .collect();
        starts.sort_unstable();
        starts.dedup();
        let cluster_text = |start: usize| -> String {
            if start >= text.len() || !text.is_char_boundary(start) {
                return String::new();
            }
            let next = starts.get(starts.partition_point(|&b| b <= start)).copied();
            text.get(start..next.unwrap_or(text.len()))
                .unwrap_or_default()
                .to_string()
        };
        let mut seen_clusters = std::collections::BTreeSet::new();
        let mut lines = Vec::with_capacity(layout.lines.len());
        for line in &layout.lines {
            let mut runs = Vec::with_capacity(line.runs.len());
            // Advances carry across runs, so the glyphs of a line run from left to right.
            let mut cursor = 0.0_f32;
            for run in &line.runs {
                let mut glyphs = Vec::with_capacity(run.glyphs.len());
                for g in &run.glyphs {
                    let first_of_cluster = seen_clusters.insert(g.cluster_index);
                    glyphs.push(GlyphPlacement {
                        glyph: g.glyph_id,
                        x: line.x_offset.to_f32() + cursor + g.x_offset.to_f32(),
                        y: -g.y_offset.to_f32(),
                        advance: g.x_advance.to_f32(),
                        text: if first_of_cluster {
                            cluster_text(g.cluster_index)
                        } else {
                            String::new()
                        },
                    });
                    cursor += g.x_advance.to_f32();
                }
                runs.push(GlyphRun { face, size, glyphs });
            }
            lines.push(GlyphLine {
                top: line.top.to_f32(),
                baseline: line.baseline_y.to_f32(),
                runs,
            });
        }
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_font_is_the_expected_noto_sans_file() {
        // Noto Sans Regular is 2,049,096 bytes. A changed file fails here.
        assert_eq!(NOTO_SANS_REGULAR.len(), 2_049_096);
        assert!(FontMeasure::bundled().is_ok());
    }

    #[test]
    fn wider_text_measures_wider() {
        let m = FontMeasure::bundled().unwrap();
        let style = CoreStyle::default();
        let short = m.width("Hi", &style);
        let long = m.width("Hello, world, this is wider", &style);
        assert!(short > 0.0);
        assert!(long > short);
    }

    #[test]
    fn headings_are_larger_than_body_text() {
        let m = FontMeasure::bundled().unwrap();
        let body = CoreStyle::default();
        let heading = CoreStyle {
            heading_level: Some(1),
            ..CoreStyle::default()
        };
        assert!(m.line_height(&heading) > m.line_height(&body));
        assert!(m.width("Title", &heading) > m.width("Title", &body));
    }

    #[test]
    fn long_text_wraps_within_the_width() {
        let m = FontMeasure::bundled().unwrap();
        let text = "the quick brown fox jumps over the lazy dog ".repeat(8);
        let boxes = m.line_boxes(&text, Px::from_f32(200.0));
        assert!(
            boxes.len() > 1,
            "expected wrapping, got {} line(s)",
            boxes.len()
        );
        // One unit of rounding is allowed: glyph advances are rounded to fixed point.
        for line in &boxes {
            assert!(line.width.to_f32() <= 201.0, "{line:?}");
        }
    }
}

#[cfg(test)]
mod emphasis_tests {
    use super::*;

    #[test]
    fn bold_is_wider_than_regular_and_italic_differs() {
        let m = FontMeasure::bundled().unwrap();
        let plain = CoreStyle::default();
        let bold = CoreStyle {
            bold: true,
            ..CoreStyle::default()
        };
        let italic = CoreStyle {
            italic: true,
            ..CoreStyle::default()
        };
        let text = "Emphasis matters here";
        assert!(m.width(text, &bold) > m.width(text, &plain));
        assert_ne!(m.width(text, &italic), m.width(text, &plain));
        assert!(m.width(text, &bold) > 0.0);
    }

    #[test]
    fn the_bundled_italic_font_is_the_expected_file() {
        assert_eq!(NOTO_SANS_ITALIC.len(), 2_322_640);
    }
}

#[cfg(test)]
mod math_tests {
    use super::*;
    use tpt_typeset::{MathBox, MathItem, layout_math, parse_math};

    #[test]
    fn wide_glyphs_advance_further_than_narrow_ones() {
        let m = FontMeasure::bundled().unwrap();
        assert!(m.advance('W', 10.0) > m.advance('i', 10.0));
        assert!(m.advance('a', 20.0) > m.advance('a', 10.0));
    }

    #[test]
    fn a_font_backed_formula_lays_out_with_real_metrics() {
        let m = FontMeasure::bundled().unwrap();
        let formula = parse_math(r"\frac{a^2}{b}").unwrap();
        let b: MathBox = layout_math(&formula, &m, 11.0);
        assert!(b.width > 0.0 && b.ascent > 0.0 && b.descent > 0.0);
        assert!(b.items.iter().any(|i| matches!(i, MathItem::Rule { .. })));
    }
}
