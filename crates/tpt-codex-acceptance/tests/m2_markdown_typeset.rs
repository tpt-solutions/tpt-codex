// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 2 (todo.md): "A `.cdx` file with Markdown, math, and cross-references exports to a
//! beautifully typeset PDF."
//!
//! The PDF export is blocked: `tpt-canvas-pdf` writes glyph runs as placeholder comments. The
//! headless part is tested here: the file is read, its references and math are checked, it is
//! written back exactly, and it is laid out and paginated. The PDF test is ignored until the
//! backend draws text.

use tpt_codex_markdown::{from_markdown, to_markdown};
use tpt_codex_views::{FlowOptions, PageGeometry, layout_flow, paginate};
use tpt_typeset::{Library, bibliography, check_math_blocks, check_references};

mod common;
use common::MEASURE;

const CDX: &str = "---\ntitle: \"Notes\"\nclass: report\n---\n\n\
# Introduction\n\nSee [the method](#method) and the result [@smith2020].\n\n\
## Method\n\n$$\nE = mc^2\n$$\n\nThe equation is in [the intro](#introduction).\n\n\
A claim with a note[^m].\n\n[^m]: The note explains the claim.\n";

const LIBRARY: &str = r#"[{"id":"smith2020","title":"Notes","author":[{"family":"Smith"}],"issued":{"date-parts":[[2020]]}}]"#;

#[test]
fn a_cdx_file_with_markdown_math_and_cross_references_is_read_checked_and_laid_out() {
    let doc = from_markdown(CDX).unwrap();
    doc.validate().unwrap();

    // Cross-references and math check clean.
    assert!(
        check_references(&doc).iter().all(|r| r.number.is_some()),
        "every #anchor resolves to a heading"
    );
    assert!(check_math_blocks(&doc).is_empty(), "the math parses");

    // The citation resolves against the library, and the bibliography lists it.
    let library = Library::from_csl_json(LIBRARY).unwrap();
    let entries = bibliography(&library, &["smith2020"]);
    assert_eq!(entries.len(), 1, "{entries:?}");

    // It is written back exactly, and read again to the same document.
    let markdown = to_markdown(&doc).unwrap();
    assert_eq!(from_markdown(&markdown).unwrap(), doc);

    // It lays out and paginates headlessly.
    let flow = layout_flow(
        &doc,
        &MEASURE,
        FlowOptions {
            width: 400.0,
            block_spacing: 6.0,
            box_height: 60.0,
            row_height: 18.0,
        },
    )
    .unwrap();
    let pages = paginate(
        &flow,
        PageGeometry {
            height: 300.0,
            margin_top: 20.0,
            margin_bottom: 20.0,
            min_lines: 2,
        },
    )
    .unwrap();
    assert!(!pages.is_empty(), "the document paginates");
}

#[test]
#[ignore = "PDF export is blocked: tpt-canvas-pdf writes glyph runs as placeholder comments"]
fn the_cdx_file_exports_to_a_typeset_pdf() {
    unreachable!("enable when tpt-canvas-pdf draws glyph runs");
}
