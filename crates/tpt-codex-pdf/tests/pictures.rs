// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pictures in an exported PDF: drawn when their bytes can be found and decoded, placeholders
//! otherwise, and always tagged as figures with their alternative text.

use tpt_codex_markdown::from_markdown;
use tpt_codex_pdf::{Margins, export_document, export_document_with_images, export_with_images};
use tpt_codex_text::FontMeasure;
use tpt_codex_views::{PageChrome, PaperSize};

const JPEG: &[u8] = include_bytes!("../../../fixtures/pdf/sample.jpg");

const MARKDOWN: &str = "# Pictures\n\nBefore.\n\n![A gradient](gradient.jpg)\n\nAfter.\n";

fn text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

fn export_with(images: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<u8> {
    let doc = from_markdown(MARKDOWN).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    export_document_with_images(
        &doc,
        PaperSize::A4,
        Margins::all(56.0),
        &measure,
        &PageChrome::default(),
        images,
    )
    .unwrap()
}

#[test]
fn a_picture_whose_bytes_are_found_is_drawn_as_an_image() {
    let pdf = export_with(&|uri| (uri == "gradient.jpg").then(|| JPEG.to_vec()));
    assert!(
        text(&pdf).contains("/Subtype /Image"),
        "the picture is embedded"
    );
}

#[test]
fn without_a_source_a_picture_is_a_placeholder() {
    let doc = from_markdown(MARKDOWN).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    let pdf = export_document(&doc, PaperSize::A4, Margins::all(56.0), &measure).unwrap();
    assert!(
        !text(&pdf).contains("/Subtype /Image"),
        "no picture without its bytes"
    );
}

#[test]
fn a_picture_that_cannot_be_found_or_decoded_stays_a_placeholder() {
    let missing = export_with(&|_| None);
    assert!(!text(&missing).contains("/Subtype /Image"));
    let not_an_image = export_with(&|_| Some(b"not a picture".to_vec()));
    assert!(!text(&not_an_image).contains("/Subtype /Image"));
}

#[test]
fn a_picture_is_tagged_as_a_figure_whether_or_not_it_is_drawn() {
    for pdf in [
        export_with(&|uri| (uri == "gradient.jpg").then(|| JPEG.to_vec())),
        export_with(&|_| None),
    ] {
        let layer = text(&pdf);
        assert!(layer.contains("/S /Figure"), "a figure element");
        assert!(layer.contains("/Alt <FEFF"), "with its alternative text");
    }
}

/// The glyph codes of every text object in the text layer, in order.
fn glyph_codes(pdf: &[u8]) -> Vec<String> {
    let layer = text(pdf);
    let marker = "%%EOF\n";
    let after = &layer[layer.find(marker).map_or(0, |at| at + marker.len())..];
    after
        .split(" Tj ET")
        .filter_map(|part| part.rfind('<').map(|at| part[at..].to_string()))
        .filter(|code| {
            code.len() == 6 && code.bytes().skip(1).take(4).all(|b| b.is_ascii_hexdigit())
        })
        .collect()
}

#[test]
fn a_picture_changes_only_the_drawing_not_the_text_layer() {
    let doc = from_markdown(MARKDOWN).unwrap();
    let pictured =
        export_with_images(&doc, &|uri| (uri == "gradient.jpg").then(|| JPEG.to_vec())).unwrap();
    let placeholder = export_with_images(&doc, &|_| None).unwrap();
    let codes = glyph_codes(&pictured);
    assert!(!codes.is_empty(), "the text layer has glyphs");
    assert_eq!(
        codes,
        glyph_codes(&placeholder),
        "the same words are in the layer"
    );
}

#[test]
fn a_picture_wider_than_the_column_is_drawn_no_wider_than_it() {
    // The column of an A4 page with 56 point margins is 595.28 - 112 points wide.
    let wide = include_bytes!("../../../fixtures/pdf/wide.png").to_vec();
    let doc = from_markdown("![Wide](wide.png)\n").unwrap();
    let pdf = export_document_with_images(
        &doc,
        PaperSize::A4,
        Margins::all(56.0),
        &FontMeasure::bundled().unwrap(),
        &PageChrome::default(),
        &|uri| (uri == "wide.png").then(|| wide.clone()),
    )
    .unwrap();
    let content = text(&pdf);
    // The picture is painted with a transform "w 0 0 h x y cm"; its width is the first operand.
    let line = content
        .lines()
        .find(|l| l.contains(" cm") && l.contains("Do"))
        .unwrap_or_else(|| panic!("no picture is painted"));
    let width: f64 = line
        .split_whitespace()
        .nth(1)
        .and_then(|w| w.parse().ok())
        .unwrap_or_else(|| panic!("no width in {line:?}"));
    assert!(
        width > 0.0 && width <= 483.3,
        "drawn {width} points wide: {line:?}"
    );
}
