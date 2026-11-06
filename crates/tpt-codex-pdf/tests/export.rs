// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF export: page structure, determinism, and golden files for one page and one slide.
//!
//! The golden files are PDF bytes. Regenerate them with `TPT_UPDATE_GOLDEN=1` and review the
//! diff. Text is drawn as outlines, so the golden files are checked by bytes, not by text.

use std::fs;
use std::path::PathBuf;

use tpt_codex_core::{
    Block, BlockId, Canvas, Document, LayoutMode, Rect, Sheet, SheetId, TextStyle,
};
use tpt_codex_markdown::from_markdown;
use tpt_codex_pdf::{
    ExportError, Margins, export_document, export_document_with_chrome, export_slide,
};
use tpt_codex_text::FontMeasure;
use tpt_codex_views::{PageChrome, PaperSize, layout_canvas};

fn measure() -> FontMeasure {
    FontMeasure::bundled().expect("bundled font loads")
}

fn margins() -> Margins {
    Margins::all(56.0)
}

/// Number of page objects in a PDF written by `tpt-canvas-pdf`.
fn page_count(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(pdf).matches("/Type /Page ").count()
}

fn golden(name: &str, actual: &[u8]) {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "golden", name]
        .iter()
        .collect();
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().unwrap()).expect("create golden dir");
        fs::write(&path, actual).expect("write golden file");
        return;
    }
    let expected = fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); run with TPT_UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    assert!(
        expected == actual,
        "golden mismatch for {} ({} bytes expected, {} actual)",
        path.display(),
        expected.len(),
        actual.len()
    );
}

fn document(markdown: &str) -> Document {
    from_markdown(markdown).expect("markdown parses")
}

#[test]
fn a_short_document_is_one_pdf_page() {
    let doc = document("# Title\n\nA short paragraph of body text.\n");
    let pdf = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    assert!(pdf.starts_with(b"%PDF-"), "a PDF header");
    assert!(
        pdf.ends_with(b"%%EOF\n") || pdf.ends_with(b"%%EOF"),
        "an end marker"
    );
    assert_eq!(page_count(&pdf), 1);
}

#[test]
fn a_long_document_is_split_into_pages() {
    let body = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(4);
    let markdown: String = (0..40)
        .map(|i| format!("Paragraph {i}. {body}\n\n"))
        .collect();
    let doc = document(&markdown);
    let pdf = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    assert!(page_count(&pdf) > 1, "{} pages", page_count(&pdf));
}

#[test]
fn the_same_document_gives_the_same_bytes() {
    let doc = document("# Title\n\n*Emphasis* and **strong** text.\n\n- one\n- two\n");
    let first = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    let second = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn bold_text_is_drawn_differently_from_regular() {
    let regular = document("A line of text.\n");
    let mut bold = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    bold.insert(
        BlockId(1),
        Block::Text {
            content: "A line of text.".into(),
            style: TextStyle {
                bold: true,
                ..TextStyle::default()
            },
        },
    );
    let regular = export_document(&regular, PaperSize::A4, margins(), &measure()).unwrap();
    let bold = export_document(&bold, PaperSize::A4, margins(), &measure()).unwrap();
    assert_ne!(regular, bold, "bold text should be thickened");
}

#[test]
fn headers_and_footers_are_drawn_on_every_page() {
    let markdown: String = (0..200)
        .map(|i| format!("Paragraph {i} with a little text.\n\n"))
        .collect();
    let doc = document(&markdown);
    let chrome = PageChrome {
        header: Some("Quarterly report".into()),
        footer: Some("Page {page} of {pages}".into()),
    };
    let plain = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    let with =
        export_document_with_chrome(&doc, PaperSize::A4, margins(), &measure(), &chrome).unwrap();
    assert!(page_count(&plain) > 1, "{} pages", page_count(&plain));
    assert_eq!(page_count(&with), page_count(&plain));
    assert_ne!(plain, with, "the header and footer should be drawn");
    let again =
        export_document_with_chrome(&doc, PaperSize::A4, margins(), &measure(), &chrome).unwrap();
    assert_eq!(with, again);
}

#[test]
fn an_empty_chrome_draws_the_same_as_no_chrome() {
    let doc = document("# Title\n\nBody.\n");
    let plain = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    let empty = export_document_with_chrome(
        &doc,
        PaperSize::A4,
        margins(),
        &measure(),
        &PageChrome::default(),
    )
    .unwrap();
    assert_eq!(plain, empty);
}

#[test]
fn a_footnote_is_drawn_at_the_foot_of_its_page() {
    let with_note = document(
        "A claim[^a].

[^a]: The source for the claim.
",
    );
    let without = document(
        "A claim.
",
    );
    let with_note = export_document(&with_note, PaperSize::A4, margins(), &measure()).unwrap();
    let without = export_document(&without, PaperSize::A4, margins(), &measure()).unwrap();
    assert_ne!(with_note, without, "the note should be drawn");
    assert_eq!(page_count(&with_note), 1, "a note does not add a page");
}

#[test]
fn lists_export_as_text_and_differ_from_plain_paragraphs() {
    let listed = document("- first item\n- second item\n");
    let plain = document("first item\n\nsecond item\n");
    let listed = export_document(&listed, PaperSize::A4, margins(), &measure()).unwrap();
    let plain = export_document(&plain, PaperSize::A4, margins(), &measure()).unwrap();
    assert_ne!(listed, plain, "the bullets should be drawn");
}

#[test]
fn grid_values_change_the_output() {
    let mut empty = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    let mut filled = empty.clone();
    let grid = |value: &str| {
        let mut sheet = Sheet::with_size(2, 2);
        if !value.is_empty() {
            sheet.cells.insert(
                "A1".into(),
                tpt_codex_core::CellInput::parse(value).unwrap().unwrap(),
            );
        }
        sheet
    };
    empty.sheets.insert(SheetId(1), grid(""));
    filled.sheets.insert(SheetId(1), grid("42"));
    for doc in [&mut empty, &mut filled] {
        doc.insert(
            BlockId(1),
            Block::Grid {
                sheet_id: SheetId(1),
                view_bounds: None,
            },
        );
    }
    let a = export_document(&empty, PaperSize::A4, margins(), &measure()).unwrap();
    let b = export_document(&filled, PaperSize::A4, margins(), &measure()).unwrap();
    assert_ne!(a, b);
}

#[test]
fn a_page_golden_file_matches() {
    let doc = document("# Title\n\nA short paragraph of body text.\n\n- An item\n");
    let pdf = export_document(&doc, PaperSize::A4, margins(), &measure()).unwrap();
    golden("page.pdf", &pdf);
}

/// A 16:9 slide with a title and a body text box, and a placeholder box for an image.
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
fn a_slide_is_one_page_the_size_of_the_slide() {
    let doc = slide_document();
    let layout = layout_canvas(&doc, BlockId(10)).unwrap();
    let pdf = export_slide(&doc, BlockId(10), &measure()).unwrap();
    assert_eq!(page_count(&pdf), 1);
    let media_box = format!("/MediaBox [0 0 {} {}]", layout.width, layout.height);
    assert!(
        String::from_utf8_lossy(&pdf).contains(&media_box),
        "expected {media_box}"
    );
}

#[test]
fn the_same_slide_gives_the_same_bytes() {
    let doc = slide_document();
    let first = export_slide(&doc, BlockId(10), &measure()).unwrap();
    let second = export_slide(&doc, BlockId(10), &measure()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_slide_golden_file_matches() {
    let pdf = export_slide(&slide_document(), BlockId(10), &measure()).unwrap();
    golden("slide.pdf", &pdf);
}

#[test]
fn a_flow_block_is_not_a_slide() {
    let doc = document("# Title\n");
    let error = export_slide(&doc, BlockId(0), &measure()).unwrap_err();
    assert!(matches!(error, ExportError::Canvas(_)), "{error}");
}

const LIVE: &str = "# Budget

::: grid { id=\"1\" }
| Item | Cost |
| --- | --- |
| Design | 1200 |
| Build | 3400 |
| Total | =SUM(B2:B3) |
:::

The total is [[sheet1.B4]].
";

/// `[[sheet1.B4]]` is drawn as that cell's value: the file is the same as for a document that
/// has the number typed in.
#[test]
fn a_binding_is_drawn_as_its_computed_value() {
    let live = from_markdown(LIVE).unwrap();
    let typed = from_markdown(&LIVE.replace("[[sheet1.B4]]", "4600")).unwrap();
    let other = from_markdown(&LIVE.replace("[[sheet1.B4]]", "4601")).unwrap();
    let export = |doc: &Document| {
        export_document(doc, PaperSize::A4, margins(), &measure()).expect("exports")
    };
    assert_eq!(export(&live), export(&typed));
    assert_ne!(export(&live), export(&other), "the number is really drawn");
}
