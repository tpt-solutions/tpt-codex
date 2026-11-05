// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{
    MonospaceMeasure, PageChrome, PageGeometry, PaperSize, layout_flow, paginate, render_chrome,
    render_template,
};

#[test]
fn template_fills_page_and_pages() {
    assert_eq!(
        render_template("Page {page} of {pages}", 2, 5),
        "Page 2 of 5"
    );
    assert_eq!(render_template("{unknown} {page}", 1, 1), "{unknown} 1");
    assert_eq!(render_template("no fields", 1, 1), "no fields");
    assert_eq!(render_template("trailing {", 1, 1), "trailing {");
    assert_eq!(render_template("{{page}}", 3, 3), "{3}");
}

#[test]
fn chrome_is_rendered_for_every_page() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: (0..6)
                .map(|i| format!("{i:09}x"))
                .collect::<Vec<_>>()
                .join("\n"),
            style: TextStyle::default(),
        },
    );
    let flow = layout_flow(
        &doc,
        &MonospaceMeasure {
            char_width: 1.0,
            line_height: 10.0,
        },
        tpt_codex_views::FlowOptions {
            width: 10.0,
            block_spacing: 0.0,
            box_height: 0.0,
            row_height: 20.0,
        },
    )
    .unwrap();
    let pages = paginate(
        &flow,
        PageGeometry {
            height: 30.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            min_lines: 2,
        },
    )
    .unwrap();
    assert_eq!(pages.len(), 2);
    let chrome = PageChrome {
        header: Some("Notes".into()),
        footer: Some("{page} / {pages}".into()),
    };
    let rendered = render_chrome(&pages, &chrome);
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].footer.as_deref(), Some("1 / 2"));
    assert_eq!(rendered[1].page, 2);
    assert_eq!(rendered[1].header.as_deref(), Some("Notes"));
    let none = render_chrome(&pages, &PageChrome::default());
    assert!(
        none.iter()
            .all(|r| r.header.is_none() && r.footer.is_none())
    );
}

#[test]
fn paper_sizes_and_column_width() {
    assert_eq!(PaperSize::LETTER.width, 612.0);
    assert!((PaperSize::A4.height - 841.89).abs() < 1e-9);
    assert_eq!(PaperSize::LETTER.column_width(72.0, 72.0), 468.0);
    assert_eq!(PaperSize::LETTER.column_width(400.0, 400.0), 0.0);
}
