// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_app::Editor;
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, MediaSource};

fn document(source: MediaSource) -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Media {
            source,
            alt_text: Some("a dot".into()),
        },
    );
    doc
}

#[test]
fn an_svg_media_block_is_drawn_as_an_svg_image_with_its_alt_text() {
    let svg = MediaSource::Svg {
        markup: r#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="1"/></svg>"#.into(),
    };
    let editor = Editor::new(document(svg));
    let view = format!("{:?}", editor.view());
    assert!(
        view.contains("data:image/svg+xml,%3Csvg%20xmlns%3D%22"),
        "{view}"
    );
    assert!(view.contains("a dot"), "{view}");
}

#[test]
fn svg_markup_cannot_break_out_of_the_attribute() {
    let svg = MediaSource::Svg {
        markup: r#""><script>alert(1)</script>"#.into(),
    };
    let editor = Editor::new(document(svg));
    let view = format!("{:?}", editor.view());
    assert!(!view.contains("<script>"), "{view}");
    assert!(view.contains("%3Cscript%3E"), "{view}");
}

#[test]
fn a_uri_media_block_is_drawn_as_before() {
    let uri = MediaSource::Uri {
        uri: "images/dot.png".into(),
    };
    let editor = Editor::new(document(uri));
    let view = format!("{:?}", editor.view());
    assert!(view.contains("images/dot.png"), "{view}");
}
