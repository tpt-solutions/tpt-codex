// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF export: a paginated document, or one slide, written as a PDF file.
//!
//! Layout comes from `tpt-codex-views`. Text is shaped with `tpt-codex-text` (through
//! `tpt-text`), and drawn with `tpt-canvas` as glyph outlines. `tpt-canvas-pdf` writes the file.
//!
//! Because text is drawn as outlines, the PDF shows the text as laid out, but the text cannot be
//! selected or searched. Bold text is drawn at regular weight (see
//! [`FontMeasure::faces`](tpt_codex_text::FontMeasure::faces)).
//!
//! The output is deterministic: the same document and options give the same bytes.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use tpt_canvas::{Color, Context, GlyphSource, Paint, Path, PathBuilder, ShapedRun, Vec2};
use tpt_canvas_pdf::PdfBackend;
use tpt_codex_core::{Block, BlockId, Document, SheetId, TextStyle};
use tpt_codex_grid::{CellResult, evaluate_sheet};
use tpt_codex_text::{FontMeasure, GlyphLine, TextError};
use tpt_codex_views::{
    CanvasError, FlowItem, FlowKind, FlowOptions, LayoutError, PageError, PageGeometry, PaperSize,
    layout_canvas, layout_flow, paginate,
};
use tpt_text::PathSegment;
use tpt_text::fx::Fx;

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

/// Draws laid-out lines with their baseline at `(x, y)`, the top of the block.
fn draw_lines(ctx: &mut Context<PdfBackend>, lines: &[GlyphLine], x: f32, y: f32) {
    ctx.set_fill_paint(Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 1.0)));
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
            ctx.restore();
        }
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
            FlowKind::Line { text, style } => {
                let lines = self.measure.shape_lines(text, style, None)?;
                draw_lines(ctx, &lines, x, y);
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
            draw_lines(ctx, &lines, cell_x + 2.0, y);
        }
        Ok(())
    }
}

/// Exports a paginated document as a PDF.
///
/// The document is laid out as one column of width `paper.width - margins`, split into pages of
/// `paper.height`, and drawn page by page. Headers and footers are not drawn.
pub fn export_document(
    doc: &Document,
    paper: PaperSize,
    margins: Margins,
    measure: &FontMeasure,
) -> Result<Vec<u8>, ExportError> {
    let column = paper.width - margins.left - margins.right;
    let flow = layout_flow(
        doc,
        measure,
        FlowOptions {
            width: column,
            block_spacing: BLOCK_SPACING,
            box_height: BOX_HEIGHT,
            row_height: ROW_HEIGHT,
        },
    )?;
    let pages = paginate(
        &flow,
        PageGeometry {
            height: paper.height,
            margin_top: margins.top,
            margin_bottom: margins.bottom,
            min_lines: MIN_LINES,
        },
    )?;

    let (width, height) = (paper.width as f32, paper.height as f32);
    let mut backend = PdfBackend::new(width, height);
    let mut page_context = PageContext {
        doc,
        measure,
        margins,
        column,
        results: BTreeMap::new(),
    };
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            backend.new_page(width, height);
        }
        let mut ctx = Context::new(backend);
        ctx.set_glyph_source(Box::new(FaceOutlines {
            faces: measure.faces(),
        }));
        for item in &page.items {
            page_context.draw_flow_item(&mut ctx, item)?;
        }
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
    let mut ctx = Context::new(backend);
    ctx.set_glyph_source(Box::new(FaceOutlines {
        faces: measure.faces(),
    }));
    for item in &layout.items {
        let Some(frame) = item.frame else { continue };
        let (x, y) = (frame.x as f32, frame.y as f32);
        match doc.get(item.block) {
            Some(Block::Text { content, style }) => {
                let lines = measure.shape_lines(content, style, Some(frame.width as f32))?;
                draw_lines(&mut ctx, &lines, x, y);
            }
            _ => draw_box(&mut ctx, x, y, frame.width as f32, frame.height as f32),
        }
    }
    Ok(ctx.into_backend().finish())
}
