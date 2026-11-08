// SPDX-License-Identifier: MIT OR Apache-2.0
//! The tagged structure of an exported PDF: its catalog entries, its structure elements, the
//! alternative text of figures, the marked content in the page, and the untagged slide.

use tpt_codex_core::{
    Block, BlockId, Canvas, Document, LayoutMode, Rect, Sheet, SheetId, TextStyle,
};
use tpt_codex_markdown::from_markdown;
use tpt_codex_pdf::{Margins, export_document, export_document_with_chrome};
use tpt_codex_text::FontMeasure;
use tpt_codex_views::{PageChrome, PaperSize};

fn export(markdown: &str) -> Vec<u8> {
    let doc = from_markdown(markdown).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    export_document(&doc, PaperSize::A4, Margins::all(56.0), &measure).unwrap()
}

fn text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

/// The part of the file the layer added: everything after the first end marker.
fn layer(pdf: &[u8]) -> String {
    let marker = b"%%EOF\n";
    let at = pdf.windows(marker.len()).position(|w| w == marker).unwrap();
    text(&pdf[at + marker.len()..])
}

#[test]
fn a_tagged_file_says_it_is_tagged_and_gives_its_language_and_title() {
    let pdf = export("---\ntitle: \"Field notes\"\n---\n\n# Heading\n\nA paragraph.\n");
    let layer = layer(&pdf);
    assert!(layer.contains("/MarkInfo << /Marked true >>"), "{layer}");
    assert!(layer.contains("/Lang (en)"), "{layer}");
    assert!(layer.contains("/StructTreeRoot"), "{layer}");
    assert!(layer.contains("/DisplayDocTitle true"), "{layer}");
    // The title is UTF-16 in hexadecimal: "Field notes" starts with F (0046).
    assert!(layer.contains("/Title <FEFF0046"), "{layer}");
}

#[test]
fn headings_and_paragraphs_are_structure_elements() {
    let pdf = export("# Heading\n\nFirst paragraph.\n\nSecond paragraph.\n");
    let layer = layer(&pdf);
    assert!(layer.contains("/S /H1"), "{layer}");
    assert!(layer.contains("/S /P"), "{layer}");
    assert!(layer.contains("/S /Document"), "{layer}");
    // Each text run is marked with an identifier, and the parent tree maps each to its element.
    assert!(layer.contains("/ParentTree"), "{layer}");
    assert!(layer.contains("/Nums ["), "{layer}");
}

#[test]
fn a_list_is_an_l_with_an_item_and_body_for_each_item() {
    let pdf = export("- one\n- two\n");
    let layer = layer(&pdf);
    assert!(layer.contains("/S /L "), "{layer}");
    assert_eq!(layer.matches("/S /LI ").count(), 2, "{layer}");
    assert_eq!(layer.matches("/S /LBody ").count(), 2, "{layer}");
}

#[test]
fn every_marked_sequence_is_closed() {
    let pdf = export("# Heading\n\nA paragraph.\n\n- one\n- two\n");
    let layer = layer(&pdf);
    let opened = layer.matches(" BDC").count();
    let closed = layer.matches("EMC").count();
    assert!(opened > 0, "marked content is written");
    assert_eq!(opened, closed, "every BDC has an EMC");
}

#[test]
fn a_figure_has_its_alternative_text() {
    let pdf = export("# Charts\n\n![A chart of sales](chart.png)\n");
    let layer = layer(&pdf);
    assert!(layer.contains("/S /Figure"), "{layer}");
    // "A chart" in UTF-16 starts with 0041 0020 0063.
    assert!(layer.contains("/Alt <FEFF00410020"), "{layer}");
    assert!(
        layer.contains("/Figure <</MCID"),
        "a figure is marked where it is drawn"
    );
}

#[test]
fn headers_and_footers_are_artifacts_and_not_structure() {
    let doc = from_markdown("# Heading\n\nBody.\n").unwrap();
    let measure = FontMeasure::bundled().unwrap();
    let chrome = PageChrome {
        header: Some("Running head".into()),
        footer: Some("Page {page} of {pages}".into()),
    };
    let pdf =
        export_document_with_chrome(&doc, PaperSize::A4, Margins::all(56.0), &measure, &chrome)
            .unwrap();
    let layer = layer(&pdf);
    assert!(layer.matches("/Artifact BMC").count() >= 2, "{layer}");
}

fn slide_document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(10)],
        },
    );
    doc.insert(
        BlockId(10),
        Block::Container {
            layout_mode: LayoutMode::Absolute,
            children: vec![BlockId(11), BlockId(12), BlockId(13)],
        },
    );
    doc.canvases.insert(BlockId(10), Canvas::WIDESCREEN);
    doc.insert(
        BlockId(11),
        Block::Text {
            content: "Slide title".into(),
            style: TextStyle {
                heading_level: Some(1),
                ..TextStyle::default()
            },
        },
    );
    doc.insert(
        BlockId(12),
        Block::Text {
            content: "A line of body text on the slide.".into(),
            style: TextStyle::default(),
        },
    );
    doc.insert(
        BlockId(13),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.sheets.insert(SheetId(1), Sheet::with_size(1, 1));
    let frame = |x, y, width, height| Rect {
        x,
        y,
        width,
        height,
    };
    doc.frames
        .insert(BlockId(11), frame(60.0, 40.0, 840.0, 80.0));
    doc.frames
        .insert(BlockId(12), frame(60.0, 160.0, 500.0, 200.0));
    doc.frames
        .insert(BlockId(13), frame(600.0, 160.0, 300.0, 200.0));
    doc
}

#[test]
fn a_slide_is_not_tagged() {
    let doc = slide_document();
    let pdf = tpt_codex_pdf::export_slide(
        &doc,
        tpt_codex_core::BlockId(10),
        &FontMeasure::bundled().unwrap(),
    )
    .unwrap();
    let text = text(&pdf);
    assert!(
        !text.contains("/StructTreeRoot"),
        "a slide has no structure"
    );
    assert!(!text.contains("/MarkInfo"), "a slide is not marked");
    // Its text is still drawn as text, so it can be searched.
    assert!(
        text.contains("TPTText0"),
        "the slide's text layer is present"
    );
}

#[test]
fn a_grid_is_a_table_with_rows_and_header_and_data_cells() {
    let pdf =
        export("::: grid { id=\"1\" }\n| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n:::\n");
    let layer = layer(&pdf);
    assert_eq!(layer.matches("/S /Table ").count(), 1, "{layer}");
    assert_eq!(
        layer.matches("/S /TR ").count(),
        3,
        "one row for the header and one for each data row: {layer}"
    );
    // The header row's cells are headers; the others are data cells.
    assert_eq!(layer.matches("/S /TH ").count(), 2, "{layer}");
    assert_eq!(layer.matches("/S /TD ").count(), 4, "{layer}");
}
