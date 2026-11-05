// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.pptx` tests, on a presentation written by python-pptx (`fixtures/office/make_fixtures.py`).

use tpt_codex_core::Block;
use tpt_codex_interop::{ArchiveWriter, Fidelity, import_pptx};

const BASIC: &[u8] = include_bytes!("../../../fixtures/office/basic.pptx");

fn texts(doc: &tpt_codex_core::Document) -> Vec<(String, Option<u8>)> {
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!("root");
    };
    children
        .iter()
        .map(|id| match doc.get(*id) {
            Some(Block::Text { content, style }) => (content.clone(), style.heading_level),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn each_slide_is_a_heading_then_its_text() {
    let import = import_pptx(BASIC).unwrap();
    assert_eq!(
        texts(&import.document),
        [
            ("Agenda".to_string(), Some(2)),
            ("First point".to_string(), None),
            ("Second point".to_string(), None),
            ("Figures".to_string(), Some(2)),
            ("free text box & <ok>".to_string(), None),
        ]
    );
    import.document.validate().unwrap();
}

#[test]
fn the_report_says_what_was_left_out() {
    let import = import_pptx(BASIC).unwrap();
    let items = import.report.items();
    assert!(
        items
            .iter()
            .any(|i| i.feature == "slides" && i.outcome == Fidelity::Approximated)
    );
    let notes = items
        .iter()
        .find(|i| i.feature == "speaker notes")
        .expect("notes item");
    assert_eq!(notes.outcome, Fidelity::Dropped);
    assert!(notes.note.starts_with("1 notes page"));
    assert!(!items.iter().any(|i| i.feature == "pictures"));
}

/// A minimal package with one slide, which has no title placeholder, a picture and a table.
fn untitled_slide() -> Vec<u8> {
    let mut w = ArchiveWriter::new();
    w.add(
        "ppt/presentation.xml",
        br#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst></p:presentation>"#
            .to_vec(),
    )
    .unwrap();
    w.add(
        "ppt/_rels/presentation.xml.rels",
        br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="x" Target="slides/slide1.xml"/></Relationships>"#
            .to_vec(),
    )
    .unwrap();
    w.add(
        "ppt/slides/slide1.xml",
        br#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree>
            <p:pic><p:blipFill/></p:pic>
            <p:sp><p:txBody><a:p><a:r><a:rPr b="1"/><a:t>bold body</a:t></a:r></a:p></p:txBody></p:sp>
            <p:graphicFrame><a:tbl><a:tr><a:tc><a:txBody><a:p><a:r><a:t>cell</a:t></a:r></a:p></a:txBody></a:tc></a:tr></a:tbl></p:graphicFrame>
            </p:spTree></p:cSld></p:sld>"#
            .to_vec(),
    )
    .unwrap();
    w.finish().unwrap()
}

#[test]
fn a_slide_without_a_title_is_numbered_and_its_extras_are_reported() {
    let import = import_pptx(&untitled_slide()).unwrap();
    assert_eq!(
        texts(&import.document),
        [
            ("Slide 1".to_string(), Some(2)),
            ("bold body".to_string(), None),
            ("cell".to_string(), None),
        ]
    );
    let items = import.report.items();
    assert!(
        items
            .iter()
            .any(|i| i.feature == "pictures" && i.outcome == Fidelity::Dropped)
    );
    assert!(
        items
            .iter()
            .any(|i| i.feature == "tables" && i.outcome == Fidelity::Approximated)
    );
    assert!(items.iter().any(|i| i.feature == "text formatting"));
}

#[test]
fn a_file_without_a_presentation_is_an_error() {
    let mut w = ArchiveWriter::new();
    w.add("docProps/core.xml", b"<x/>".to_vec()).unwrap();
    assert!(import_pptx(&w.finish().unwrap()).is_err());
}
