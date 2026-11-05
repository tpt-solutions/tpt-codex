// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Document, LayoutMode, MediaSource, Metadata, Rect, SheetId, TextStyle,
};
use tpt_codex_interop::{Archive, Fidelity, export_docx, import_docx};

fn text(content: &str, level: Option<u8>, bold: bool, italic: bool) -> Block {
    Block::Text {
        content: content.into(),
        style: TextStyle {
            bold,
            italic,
            heading_level: level,
            runs: Vec::new(),
        },
    }
}

/// Builds a flow document from `children`, numbering the blocks 1, 2, ...
fn doc(children: Vec<Block>) -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=children.len() as u64).map(BlockId).collect(),
        },
    );
    for (i, b) in children.into_iter().enumerate() {
        d.insert(BlockId(i as u64 + 1), b);
    }
    d
}

fn texts(d: &Document) -> Vec<(String, Option<u8>, bool, bool)> {
    let Some(Block::Container { children, .. }) = d.get(d.root) else {
        panic!("root");
    };
    children
        .iter()
        .map(|id| match d.get(*id) {
            Some(Block::Text { content, style }) => (
                content.clone(),
                style.heading_level,
                style.bold,
                style.italic,
            ),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn text_styles_breaks_tabs_and_special_characters_round_trip() {
    let mut d = doc(vec![
        text("Title here", Some(1), false, false),
        text("Sub & <heading>", Some(3), false, false),
        text("bold line", None, true, false),
        text("italic line", None, false, true),
        text("two\nlines\twith a tab", None, false, false),
        text("  padded  ", None, false, false),
        text("", None, false, false),
    ]);
    d.metadata = Metadata {
        title: Some("Notes \"draft\" & more".into()),
        author: Some("A. Author".into()),
        ..Metadata::default()
    };
    let export = export_docx(&d).unwrap();
    assert!(export.report.is_lossless(), "{}", export.report);

    let back = import_docx(&export.bytes).unwrap();
    assert_eq!(back.document.metadata, d.metadata);
    let got = texts(&back.document);
    assert_eq!(got[0], ("Title here".into(), Some(1), false, false));
    assert_eq!(got[1], ("Sub & <heading>".into(), Some(3), false, false));
    assert_eq!(got[2], ("bold line".into(), None, true, false));
    assert_eq!(got[3], ("italic line".into(), None, false, true));
    assert_eq!(got[4].0, "two\nlines\twith a tab");
    // The importer trims paragraph text, so padding does not survive; the rest does.
    assert_eq!(got[5].0, "padded");
    assert_eq!(got.len(), 6, "the empty paragraph is not imported");
}

#[test]
fn lists_code_math_and_rules_are_written_with_a_report() {
    let d = doc(vec![
        Block::List {
            ordered: true,
            start: 1,
            items: vec![BlockId(100), BlockId(101)],
        },
        Block::Code {
            language: Some("rust".into()),
            code: "fn f() {}".into(),
        },
        Block::Math {
            source: r"\frac{a}{b}".into(),
        },
        Block::ThematicBreak,
        text("after", None, false, false),
    ]);
    let mut d = d;
    d.insert(BlockId(100), text("first", None, false, false));
    d.insert(BlockId(101), text("second", None, false, false));

    let export = export_docx(&d).unwrap();
    let items = export.report.items();
    for feature in ["lists", "code", "math", "rules"] {
        assert!(
            items
                .iter()
                .any(|i| i.feature == feature && i.outcome == Fidelity::Approximated),
            "missing {feature} in {}",
            export.report
        );
    }
    let back = import_docx(&export.bytes).unwrap();
    let got: Vec<String> = texts(&back.document).into_iter().map(|t| t.0).collect();
    // The rule is written as an empty paragraph, which the importer skips. The report records that.
    assert_eq!(
        got,
        ["first", "second", "fn f() {}", r"\frac{a}{b}", "after"]
    );
}

#[test]
fn images_grids_and_queries_are_reported_as_dropped() {
    let d = doc(vec![
        Block::Media {
            source: MediaSource::Uri {
                uri: "a.png".into(),
            },
            alt_text: None,
        },
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
        },
        Block::Query(tpt_codex_core::Query {
            source: "{}".into(),
        }),
        text("kept", None, false, false),
    ]);
    let export = export_docx(&d).unwrap();
    let dropped: Vec<&str> = export
        .report
        .items()
        .iter()
        .filter(|i| i.outcome == Fidelity::Dropped)
        .map(|i| i.feature.as_str())
        .collect();
    assert!(dropped.contains(&"images"));
    assert!(dropped.contains(&"live blocks"));
    let back = import_docx(&export.bytes).unwrap();
    assert_eq!(texts(&back.document).len(), 1);
}

#[test]
fn the_same_document_gives_the_same_bytes() {
    let d = doc(vec![
        text("a", Some(1), true, false),
        text("b", None, false, false),
    ]);
    assert_eq!(
        export_docx(&d).unwrap().bytes,
        export_docx(&d).unwrap().bytes
    );
}

#[test]
fn control_characters_in_metadata_are_refused() {
    let mut d = doc(vec![text("a", None, false, false)]);
    d.metadata.title = Some("bad \u{1} title".into());
    assert!(export_docx(&d).is_err());
}

#[test]
fn the_package_has_the_parts_word_needs() {
    let d = doc(vec![text("a", Some(2), false, false)]);
    let export = export_docx(&d).unwrap();
    let archive = Archive::open(&export.bytes).unwrap();
    let names: Vec<&str> = archive.names().collect();
    for part in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/styles.xml",
        "docProps/core.xml",
    ] {
        assert!(names.contains(&part), "missing {part}");
    }
    let styles = String::from_utf8(archive.read("word/styles.xml").unwrap()).unwrap();
    assert!(styles.contains(r#"w:styleId="Heading2""#));
}
