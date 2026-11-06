// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF export: a paginated document, or one slide, written as a PDF file.
//!
//! Layout comes from `tpt-codex-views`. Text is shaped with `tpt-codex-text` (through
//! `tpt-text`), and drawn with `tpt-canvas` as glyph outlines. `tpt-canvas-pdf` writes the file.
//!
//! Because text is drawn as outlines, the PDF shows the text as laid out, but the text cannot be
//! selected or searched. Bold text is drawn with the regular outlines, stroked with a thin line to
//! thicken them. The bundled font has no bold outlines of its own, so this is an approximation of
//! the bold weight that the layout measured.
//!
//! The output is deterministic: the same document and options give the same bytes.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use tpt_canvas::{
    Affine2, Color, Context, GlyphSource, LineJoin, Paint, Path, PathBuilder, ShapedRun,
    StrokeStyle, Vec2,
};
use tpt_canvas_pdf::PdfBackend;
use tpt_codex_core::{Block, BlockId, Document, SheetId, TextStyle};
use tpt_codex_grid::{CellResult, evaluate_sheet};
use tpt_codex_text::{FontMeasure, GlyphLine, GlyphRun, TextError};
use tpt_codex_views::{
    CanvasError, FlowItem, FlowKind, FlowOptions, LayoutError, PageChrome, PageError, PageGeometry,
    PaperSize, RenderedChrome, TextMeasure, layout_canvas, layout_flow_hyphenated,
    paginate_with_notes, render_chrome,
};
use tpt_text::PathSegment;
use tpt_text::fx::Fx;
use tpt_typeset::footnote_notes;

/// Margins, in points, from the edges of the paper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    /// Space above the content.
    pub top: f64,
    /// Space below the content.
    pub bottom: f64,
    /// Space left of the content.
    pub left: f64,
    /// Space right of the content.
    pub right: f64,
}

impl Margins {
    /// One margin on every side.
    pub const fn all(points: f64) -> Self {
        Self {
            top: points,
            bottom: points,
            left: points,
            right: points,
        }
    }
}

/// Space between blocks of a flow, in points.
const BLOCK_SPACING: f64 = 8.0;
/// Height of the placeholder for a non-text block, in points.
const BOX_HEIGHT: f64 = 60.0;
/// Height of one grid row, in points.
const ROW_HEIGHT: f64 = 16.0;
/// Minimum lines of a split paragraph kept together at a page break.
const MIN_LINES: usize = 2;
/// Fill of placeholder boxes.
const BOX_FILL: Color = Color::rgba(0.9, 0.9, 0.9, 1.0);
/// Colour of text and of the stroke that thickens bold text.
const INK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);
/// Width of the stroke that thickens bold text, as a fraction of the font size.
const BOLD_STROKE_EM: f32 = 0.03;
/// Height of a header or footer line, in points. Used to centre it in its margin.
const CHROME_LINE: f64 = 14.0;

/// Why an export failed.
#[derive(Debug)]
pub enum ExportError {
    /// The flow layout could not be produced.
    Layout(LayoutError),
    /// The page geometry was refused.
    Page(PageError),
    /// The slide could not be laid out.
    Canvas(CanvasError),
    /// Text could not be shaped with the bundled font.
    Text(TextError),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(e) => write!(f, "layout failed: {e}"),
            Self::Page(e) => write!(f, "pagination failed: {e}"),
            Self::Canvas(e) => write!(f, "slide layout failed: {e}"),
            Self::Text(e) => write!(f, "text failed: {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<LayoutError> for ExportError {
    fn from(e: LayoutError) -> Self {
        Self::Layout(e)
    }
}

impl From<PageError> for ExportError {
    fn from(e: PageError) -> Self {
        Self::Page(e)
    }
}

impl From<CanvasError> for ExportError {
    fn from(e: CanvasError) -> Self {
        Self::Canvas(e)
    }
}

impl From<TextError> for ExportError {
    fn from(e: TextError) -> Self {
        Self::Text(e)
    }
}

/// Glyph outlines for [`Context::draw_glyphs`], from the bundled faces.
///
/// `tpt-text` returns outlines in layout units with y down. `tpt-canvas` wants font units with
/// y up, and scales them by `font_size / units_per_em` itself. So this asks for the outline at a
/// size equal to units-per-em, which makes layout units equal font units, and then flips y.
#[derive(Clone)]
struct FaceOutlines {
    faces: Vec<Arc<tpt_text::font::FontFace>>,
}

impl GlyphSource for FaceOutlines {
    fn units_per_em(&self, font_id: u32) -> Option<f32> {
        let face = self.faces.get(font_id as usize)?;
        Some(f32::from(face.units_per_em()))
    }

    fn outline(&self, font_id: u32, glyph: u32) -> Option<Path> {
        let face = self.faces.get(font_id as usize)?;
        let upem = Fx::from_int(i32::from(face.units_per_em()));
        let segments = face.glyph_outline(glyph, upem);
        if segments.is_empty() {
            // Glyphs without an outline, such as spaces, are skipped.
            return None;
        }
        let mut path = PathBuilder::new();
        for segment in segments {
            match segment {
                PathSegment::MoveTo(x, y) => path.move_to(x.to_f32(), -y.to_f32()),
                PathSegment::LineTo(x, y) => path.line_to(x.to_f32(), -y.to_f32()),
                PathSegment::QuadTo(x1, y1, x, y) => {
                    path.quad_to(x1.to_f32(), -y1.to_f32(), x.to_f32(), -y.to_f32());
                }
                PathSegment::CubicTo(x1, y1, x2, y2, x, y) => path.cubic_to(
                    x1.to_f32(),
                    -y1.to_f32(),
                    x2.to_f32(),
                    -y2.to_f32(),
                    x.to_f32(),
                    -y.to_f32(),
                ),
                PathSegment::Close => path.close(),
            }
        }
        Some(path.build())
    }
}

/// Draws laid-out lines with their baseline at `(x, y)`, the top of the block. Bold lines are
/// thickened by stroking each glyph's outline with the ink colour.
fn draw_lines(
    ctx: &mut Context<PdfBackend>,
    outlines: &FaceOutlines,
    lines: &[GlyphLine],
    x: f32,
    y: f32,
    bold: bool,
) {
    ctx.set_fill_paint(Paint::Solid(INK));
    for line in lines {
        for run in &line.runs {
            let mut shaped = ShapedRun::new(run.face as u32, run.size);
            for glyph in &run.glyphs {
                shaped.push(
                    glyph.glyph,
                    Vec2 {
                        x: glyph.x,
                        y: glyph.y,
                    },
                );
            }
            ctx.save();
            ctx.translate(x, y + line.baseline);
            ctx.draw_glyphs(&shaped);
            if bold {
                stroke_bold(ctx, outlines, run);
            }
            ctx.restore();
        }
    }
}

/// Strokes the outlines of a run's glyphs, in the same place [`Context::draw_glyphs`] fills them.
/// The stroke adds weight to the regular glyphs, which approximates bold.
fn stroke_bold(ctx: &mut Context<PdfBackend>, outlines: &FaceOutlines, run: &GlyphRun) {
    let font = run.face as u32;
    let Some(upem) = outlines.units_per_em(font).filter(|u| *u > 0.0) else {
        return;
    };
    let scale = run.size / upem;
    ctx.set_stroke_paint(Paint::Solid(INK));
    ctx.set_stroke_style(StrokeStyle {
        width: run.size * BOLD_STROKE_EM,
        join: LineJoin::Round,
        ..StrokeStyle::default()
    });
    for glyph in &run.glyphs {
        let Some(outline) = outlines.outline(font, glyph.glyph) else {
            continue;
        };
        let m = Affine2::translate(glyph.x, glyph.y) * Affine2::scale(scale, -scale);
        ctx.stroke_path(&outline.transform(&m));
    }
}

/// Fills a placeholder box at `(x, y)`.
fn draw_box(ctx: &mut Context<PdfBackend>, x: f32, y: f32, width: f32, height: f32) {
    ctx.set_fill_paint(Paint::Solid(BOX_FILL));
    ctx.fill_rect(x, y, width, height);
}

/// The text a grid cell shows: its computed value, not its formula.
fn cell_text(result: Option<&CellResult>) -> String {
    match result {
        None | Some(CellResult::Empty) => String::new(),
        Some(CellResult::Number(n)) => format!("{n}"),
        Some(CellResult::Text(s)) => s.clone(),
        Some(CellResult::Boolean(b)) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        Some(CellResult::Error(message)) => format!("#{message}"),
    }
}

/// The A1-style address of a cell, from a 0-based column and a 1-based row. This matches
/// `tpt_codex_core`'s addresses (A, B, ..., Z, AA, ...).
fn cell_address(col: u32, row: u32) -> String {
    let mut letters = Vec::new();
    let mut c = col + 1;
    while c > 0 {
        letters.push(b'A' + ((c - 1) % 26) as u8);
        c = (c - 1) / 26;
    }
    letters.reverse();
    let mut address = String::from_utf8(letters).unwrap_or_default();
    address.push_str(&row.to_string());
    address
}

/// Everything a page needs to draw its items.
struct PageContext<'a> {
    doc: &'a Document,
    measure: &'a FontMeasure,
    outlines: &'a FaceOutlines,
    margins: Margins,
    column: f64,
    /// Computed cell values per sheet, filled on first use.
    results: BTreeMap<SheetId, BTreeMap<String, CellResult>>,
}

impl PageContext<'_> {
    fn draw_flow_item(
        &mut self,
        ctx: &mut Context<PdfBackend>,
        item: &FlowItem,
    ) -> Result<(), ExportError> {
        let x = self.margins.left as f32;
        let y = (self.margins.top + item.y) as f32;
        match &item.kind {
            FlowKind::Line {
                text,
                style,
                marker,
                indent,
            } => {
                let lines = self.measure.shape_lines(text, style, None)?;
                draw_lines(
                    ctx,
                    self.outlines,
                    &lines,
                    x + *indent as f32,
                    y,
                    style.bold,
                );
                // The marker shares the first line's baseline, since it is set in the same style.
                if let Some(marker) = marker {
                    let marks = self.measure.shape_lines(marker, style, None)?;
                    draw_lines(ctx, self.outlines, &marks, x, y, style.bold);
                }
            }
            FlowKind::Box => draw_box(ctx, x, y, self.column as f32, item.height as f32),
            FlowKind::GridRow { sheet, row, header } => {
                self.draw_grid_row(ctx, *sheet, *row, *header, x, y)?;
            }
        }
        Ok(())
    }

    fn draw_grid_row(
        &mut self,
        ctx: &mut Context<PdfBackend>,
        sheet_id: SheetId,
        row: u32,
        header: bool,
        x: f32,
        y: f32,
    ) -> Result<(), ExportError> {
        let Some(sheet) = self.doc.sheets.get(&sheet_id) else {
            // A grid whose sheet is missing is drawn as a placeholder by the flow.
            return Ok(());
        };
        let results = self
            .results
            .entry(sheet_id)
            .or_insert_with(|| evaluate_sheet(sheet));
        let cell_width = self.column / f64::from(sheet.cols.max(1));
        let style = TextStyle {
            bold: header,
            ..TextStyle::default()
        };
        for col in 0..sheet.cols {
            let address = cell_address(col, row);
            let text = cell_text(results.get(&address));
            if text.is_empty() {
                continue;
            }
            let cell_x = x + (cell_width * f64::from(col)) as f32;
            let lines = self.measure.shape_lines(
                &text,
                &style,
                Some((cell_width - 4.0).max(1.0) as f32),
            )?;
            draw_lines(ctx, self.outlines, &lines, cell_x + 2.0, y, style.bold);
        }
        Ok(())
    }
}

/// Draws one line of header or footer text at `x`, its block top at `y`.
fn draw_chrome_text(
    ctx: &mut Context<PdfBackend>,
    outlines: &FaceOutlines,
    measure: &FontMeasure,
    text: &str,
    x: f32,
    y: f32,
) -> Result<(), ExportError> {
    let lines = measure.shape_lines(text, &TextStyle::default(), None)?;
    draw_lines(ctx, outlines, &lines, x, y, false);
    Ok(())
}

/// Draws the header and footer of one page. The header sits at the left of the top margin, and
/// the footer is centred in the bottom margin. Each is centred vertically in its margin.
fn draw_page_chrome(
    ctx: &mut Context<PdfBackend>,
    outlines: &FaceOutlines,
    measure: &FontMeasure,
    rendered: &RenderedChrome,
    paper: PaperSize,
    margins: Margins,
) -> Result<(), ExportError> {
    if let Some(header) = &rendered.header {
        let top = ((margins.top - CHROME_LINE).max(0.0) / 2.0) as f32;
        draw_chrome_text(ctx, outlines, measure, header, margins.left as f32, top)?;
    }
    if let Some(footer) = &rendered.footer {
        let width = measure.width(footer, &TextStyle::default()) as f32;
        let x = ((paper.width as f32) - width) / 2.0;
        let top =
            (paper.height - margins.bottom + (margins.bottom - CHROME_LINE).max(0.0) / 2.0) as f32;
        draw_chrome_text(ctx, outlines, measure, footer, x, top)?;
    }
    Ok(())
}

/// Exports a paginated document as a PDF.
///
/// The document is laid out as one column of width `paper.width - margins`, split into pages of
/// `paper.height`, and drawn page by page. No headers or footers are drawn; use
/// [`export_document_with_chrome`] for those.
pub fn export_document(
    doc: &Document,
    paper: PaperSize,
    margins: Margins,
    measure: &FontMeasure,
) -> Result<Vec<u8>, ExportError> {
    export_document_with_chrome(doc, paper, margins, measure, &PageChrome::default())
}

/// Exports a paginated document as a PDF, with a header and footer on every page.
///
/// The header and footer templates are those of [`PageChrome`]: `{page}` is the page number and
/// `{pages}` the total. They are drawn in the top and bottom margins, so they never overlap the
/// content. The content is laid out exactly as in [`export_document`].
pub fn export_document_with_chrome(
    doc: &Document,
    paper: PaperSize,
    margins: Margins,
    measure: &FontMeasure,
    chrome: &PageChrome,
) -> Result<Vec<u8>, ExportError> {
    let column = paper.width - margins.left - margins.right;
    let flow = layout_flow_hyphenated(
        doc,
        measure,
        FlowOptions {
            width: column,
            block_spacing: BLOCK_SPACING,
            box_height: BOX_HEIGHT,
            row_height: ROW_HEIGHT,
        },
    )?;
    let notes = footnote_notes(doc, measure, column);
    let pages = paginate_with_notes(
        &flow,
        PageGeometry {
            height: paper.height,
            margin_top: margins.top,
            margin_bottom: margins.bottom,
            min_lines: MIN_LINES,
        },
        &notes,
    )?;
    let rendered = render_chrome(&pages, chrome);

    let outlines = FaceOutlines {
        faces: measure.faces(),
    };
    let (width, height) = (paper.width as f32, paper.height as f32);
    let mut backend = PdfBackend::new(width, height);
    let mut page_context = PageContext {
        doc,
        measure,
        outlines: &outlines,
        margins,
        column,
        results: BTreeMap::new(),
    };
    for (index, (page, chrome_page)) in pages.iter().zip(&rendered).enumerate() {
        if index > 0 {
            backend.new_page(width, height);
        }
        let mut ctx = Context::new(backend);
        ctx.set_glyph_source(Box::new(outlines.clone()));
        for item in page.items.iter().chain(&page.notes) {
            page_context.draw_flow_item(&mut ctx, item)?;
        }
        draw_page_chrome(&mut ctx, &outlines, measure, chrome_page, paper, margins)?;
        backend = ctx.into_backend();
    }
    Ok(backend.finish())
}

/// Exports one slide as a PDF, one page the size of the slide.
///
/// Items are drawn in z-order. Text is wrapped at the width of its frame by `tpt-text`, so the
/// line breaks can differ from the SVG export, which uses the Flow view's wrapping. Other blocks
/// are drawn as placeholder boxes, and an item without a frame is not drawn.
pub fn export_slide(
    doc: &Document,
    slide: BlockId,
    measure: &FontMeasure,
) -> Result<Vec<u8>, ExportError> {
    let layout = layout_canvas(doc, slide)?;
    let backend = PdfBackend::new(layout.width as f32, layout.height as f32);
    let outlines = FaceOutlines {
        faces: measure.faces(),
    };
    let mut ctx = Context::new(backend);
    ctx.set_glyph_source(Box::new(outlines.clone()));
    for item in &layout.items {
        let Some(frame) = item.frame else { continue };
        let (x, y) = (frame.x as f32, frame.y as f32);
        match doc.get(item.block) {
            Some(Block::Text { content, style }) => {
                let lines = measure.shape_lines(content, style, Some(frame.width as f32))?;
                draw_lines(&mut ctx, &outlines, &lines, x, y, style.bold);
            }
            _ => draw_box(&mut ctx, x, y, frame.width as f32, frame.height as f32),
        }
    }
    Ok(ctx.into_backend().finish())
}
