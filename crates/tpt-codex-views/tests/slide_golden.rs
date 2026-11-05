// SPDX-License-Identifier: MIT OR Apache-2.0
//! The slide SVG is compared byte for byte with a golden file. Set `TPT_UPDATE_GOLDEN=1` to
//! write the file after a deliberate change.

use std::fs;
use std::path::PathBuf;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, MediaSource, Rect, TextStyle};
use tpt_codex_views::{MonospaceMeasure, render_slide_svg};

const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 6.0,
    line_height: 14.0,
};

fn deck() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(10)],
        },
    );
    d.insert(
        BlockId(10),
        Block::Container {
            layout_mode: LayoutMode::Absolute,
            children: vec![BlockId(11), BlockId(12), BlockId(13), BlockId(14)],
        },
    );
    d.insert(
        BlockId(11),
        Block::Text {
            content: "Quarterly <review> & plans".into(),
            style: TextStyle {
                heading_level: Some(1),
                ..TextStyle::default()
            },
        },
    );
    d.insert(
        BlockId(12),
        Block::Text {
            content: "Revenue grew steadily across every region this year.".into(),
            style: TextStyle::default(),
        },
    );
    d.insert(
        BlockId(13),
        Block::Media {
            source: MediaSource::Svg {
                markup: r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect width="5" height="5"/></svg>"#.into(),
            },
            alt_text: Some("chart".into()),
        },
    );
    d.insert(BlockId(14), Block::ThematicBreak);
    d.frames.insert(
        BlockId(11),
        Rect {
            x: 40.0,
            y: 30.0,
            width: 500.0,
            height: 60.0,
        },
    );
    d.frames.insert(
        BlockId(12),
        Rect {
            x: 40.0,
            y: 110.0,
            width: 300.0,
            height: 120.0,
        },
    );
    d.frames.insert(
        BlockId(13),
        Rect {
            x: 600.0,
            y: 110.0,
            width: 300.0,
            height: 200.0,
        },
    );
    d.frames.insert(
        BlockId(14),
        Rect {
            x: 40.0,
            y: 400.0,
            width: 200.0,
            height: 40.0,
        },
    );
    d
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/slide.svg")
}

#[test]
fn slide_svg_matches_the_golden_file() {
    let svg = render_slide_svg(&deck(), BlockId(10), &MEASURE).unwrap();
    let path = golden_path();
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &svg).unwrap();
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("golden missing; run with TPT_UPDATE_GOLDEN=1"));
    assert_eq!(svg, expected);
}

#[test]
fn embedded_svg_is_an_image_so_its_script_cannot_run() {
    let svg = render_slide_svg(&deck(), BlockId(10), &MEASURE).unwrap();
    assert!(!svg.contains("<script"), "{svg}");
    assert!(svg.contains("data:image/svg+xml;base64,"), "{svg}");
    assert!(
        svg.contains("Quarterly &lt;review&gt; &amp; plans"),
        "{svg}"
    );
}
