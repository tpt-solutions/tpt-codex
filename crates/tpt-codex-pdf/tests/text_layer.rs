// SPDX-License-Identifier: MIT OR Apache-2.0
//! The text layer: the PDF carries its words as invisible text, and the outlines are unchanged.
//!
//! The file is checked by its structure: the layer is an incremental update after the outlines,
//! its text objects use render mode 3 (invisible), and the ToUnicode map gives the characters.
//! Text extraction was checked by hand with `pdftotext` and `pypdf` on the same files.

use tpt_codex_markdown::from_markdown;
use tpt_codex_pdf::{Margins, export_document};
use tpt_codex_text::FontMeasure;
use tpt_codex_views::PaperSize;

fn export(markdown: &str) -> Vec<u8> {
    let doc = from_markdown(markdown).unwrap();
    let measure = FontMeasure::bundled().unwrap();
    export_document(&doc, PaperSize::A4, Margins::all(56.0), &measure).unwrap()
}

fn text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

/// The outlines part: everything up to the first end marker.
fn outlines_part(pdf: &[u8]) -> &[u8] {
    let marker = b"%%EOF\n";
    let at = pdf.windows(marker.len()).position(|w| w == marker).unwrap();
    &pdf[..at + marker.len()]
}

#[test]
fn the_text_layer_is_an_update_after_the_outlines() {
    let pdf = export("# Title\n\nSome words.\n");
    let outlines = outlines_part(&pdf);
    assert!(
        pdf.len() > outlines.len(),
        "the layer adds bytes after the outlines"
    );
    // The update is a new cross-reference section and trailer that point back at the original.
    let tail = text(&pdf[outlines.len()..]);
    assert!(tail.contains("\nxref\n"), "{tail}");
    assert!(tail.contains("/Prev "), "{tail}");
    assert!(pdf.ends_with(b"%%EOF\n"));
}

#[test]
fn the_layer_text_is_invisible_and_uses_an_embedded_cid_font() {
    let pdf = text(&export("Hello world.\n"));
    assert!(pdf.contains(" 3 Tr"), "render mode 3 is invisible");
    assert!(pdf.contains("/Subtype /Type0"));
    assert!(pdf.contains("/Encoding /Identity-H"));
    assert!(pdf.contains("/Subtype /CIDFontType2"));
    assert!(pdf.contains("/FontFile2"), "the font is embedded");
    assert!(pdf.contains("/ToUnicode"));
}

#[test]
fn the_tounicode_map_says_which_characters_each_glyph_stands_for() {
    let pdf = text(&export("Hello.\n"));
    // H is U+0048 and e is U+0065; each appears as the destination of a glyph in the map.
    assert!(pdf.contains("> <0048>"), "H in the map");
    assert!(pdf.contains("> <0065>"), "e in the map");
}

#[test]
fn each_page_is_rewritten_to_use_the_layer_and_keeps_its_outlines() {
    let markdown: String = (0..120)
        .map(|i| format!("Paragraph {i} with some text.\n\n"))
        .collect();
    let pdf = export(&markdown);
    let outlines = text(outlines_part(&pdf));
    let pages = outlines.matches("/Type /Page ").count();
    assert!(pages > 1, "{pages} pages");
    // The rewritten page objects appear in the update, once per page.
    let all = text(&pdf);
    assert_eq!(
        all.matches("/Type /Page ").count(),
        pages * 2,
        "each page once and once again"
    );
    assert_eq!(
        all.matches("/Font << ").count(),
        pages,
        "each page names the fonts it uses"
    );
}

#[test]
fn the_same_document_gives_the_same_bytes_with_the_layer() {
    assert_eq!(export("A line.\n"), export("A line.\n"));
}
