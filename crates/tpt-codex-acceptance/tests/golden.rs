// SPDX-License-Identifier: MIT OR Apache-2.0
//! Golden render tests: a fixed document rendered to the slide SVG and to the paginated view, and
//! compared byte for byte with files in `golden/`. Both are plain text, so they are the same on
//! every target. Set `TPT_UPDATE_GOLDEN=1` to write the files after a deliberate change.
//!
//! The wasm32 build of this file is compiled by CI (`cargo check --target wasm32-unknown-unknown`)
//! and the same functions are called from the browser build; running these tests inside a wasm
//! runner is not set up.

mod common;

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use common::MEASURE;
use tpt_codex_core::{Block, BlockId, Canvas, Document, LayoutMode, Rect, TextStyle};
use tpt_codex_views::{FlowOptions, PageGeometry, layout_flow, paginate, render_slide_svg};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("golden")
        .join(name)
}

/// Compares `actual` with the golden file, or writes it when `TPT_UPDATE_GOLDEN` is set.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {} ({e}); run with TPT_UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        expected.replace("\r\n", "\n"),
        actual.replace("\r\n", "\n"),
        "golden mismatch for {name}"
    );
}

/// A one-slide deck: a title, a bullet-like paragraph and a shape, placed by frames.
fn slide_deck() -> Document {
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
            children: vec![BlockId(11), BlockId(12)],
        },
    );
    doc.canvases.insert(BlockId(10), Canvas::WIDESCREEN);
    doc.insert(
        BlockId(11),
        Block::Text {
            content: "Quarterly <review> & plans".into(),
            style: TextStyle {
                heading_level: Some(1),
                ..TextStyle::default()
            },
        },
    );
    doc.insert(
        BlockId(12),
        Block::Text {
            content: "Revenue grew steadily across every region this year.".into(),
            style: TextStyle::default(),
        },
    );
    doc.frames.insert(
        BlockId(11),
        Rect {
            x: 60.0,
            y: 40.0,
            width: 840.0,
            height: 90.0,
        },
    );
    doc.frames.insert(
        BlockId(12),
        Rect {
            x: 60.0,
            y: 160.0,
            width: 480.0,
            height: 200.0,
        },
    );
    doc
}

/// A flow document long enough to paginate: headings and paragraphs of fixed text.
fn report_document() -> Document {
    let mut children = Vec::new();
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    for i in 1..=12u64 {
        let id = BlockId(i);
        let block = if i % 4 == 1 {
            Block::Text {
                content: format!("Section {}", i / 4 + 1),
                style: TextStyle {
                    heading_level: Some(2),
                    ..TextStyle::default()
                },
            }
        } else {
            Block::Text {
                content: format!(
                    "Paragraph {i}. The text wraps across lines at this width so that the \
                     layout must break it and carry the rest to the next line."
                ),
                style: TextStyle::default(),
            }
        };
        doc.insert(id, block);
        children.push(id);
    }
    if let Some(Block::Container { children: root, .. }) = doc.blocks.get_mut(&BlockId(0)) {
        *root = children;
    }
    doc
}

/// Lays out the report and writes one line per item, grouped by page.
fn paginated_text(doc: &Document) -> String {
    let flow = layout_flow(
        doc,
        &MEASURE,
        FlowOptions {
            width: 300.0,
            block_spacing: 6.0,
            box_height: 60.0,
            row_height: 14.0,
        },
    )
    .unwrap();
    let pages = paginate(
        &flow,
        PageGeometry {
            height: 200.0,
            margin_top: 16.0,
            margin_bottom: 16.0,
            min_lines: 2,
        },
    )
    .unwrap();
    let mut out = String::new();
    for page in &pages {
        let _ = writeln!(out, "page {}", page.number);
        for item in &page.items {
            let _ = writeln!(
                out,
                "  block {} y {:.2} h {:.2} {:?}",
                item.block.0, item.y, item.height, item.kind
            );
        }
    }
    out
}

#[test]
fn the_slide_renders_to_the_golden_svg() {
    let doc = slide_deck();
    let svg = render_slide_svg(&doc, BlockId(10), &MEASURE).unwrap();
    assert_golden("slide.svg", &svg);
}

#[test]
fn the_report_paginates_to_the_golden_layout() {
    let doc = report_document();
    let text = paginated_text(&doc);
    assert!(text.matches("page ").count() >= 2, "the report spans pages");
    assert_golden("paginated.txt", &text);
}
