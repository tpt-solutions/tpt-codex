// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.docx` tests on a file written by python-docx (`fixtures/office/make_fixtures.py`).

use tpt_codex_core::Block;
use tpt_codex_interop::{ArchiveWriter, Fidelity, import_docx};

const BASIC: &[u8] = include_bytes!("../../../fixtures/office/basic.docx");

/// The text blocks of the imported document, in order, as `(text, heading level, bold)`.
fn paragraphs(doc: &tpt_codex_core::Document) -> Vec<(String, Option<u8>, bool)> {
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!("root is not a container");
    };
    children
        .iter()
        .map(|id| match doc.get(*id) {
            Some(Block::Text { content, style }) => {
                (content.clone(), style.heading_level, style.bold)
            }
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

#[test]
fn paragraphs_headings_and_breaks_come_out_in_order() {
    let import = import_docx(BASIC).unwrap();
    let got = paragraphs(&import.document);
    let texts: Vec<&str> = got.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Introduction",
            "Plain paragraph with two lines.\nSecond line.",
            "bold then plain",
            "Details",
            "first bullet",
            "second bullet",
            "cell A1",
            "cell B1",
            "cell A2",
            "cell B2",
            "Final paragraph & <end>.",
        ]
    );
    assert_eq!(got[0].1, Some(1), "Heading 1");
    assert_eq!(got[3].1, Some(2), "Heading 2");
    assert_eq!(got[1].1, None);
    assert!(
        !got[2].2,
        "a paragraph with mixed bold is not bold as a whole"
    );
    import.document.validate().unwrap();
}

#[test]
fn title_and_author_come_from_the_properties() {
    let import = import_docx(BASIC).unwrap();
    assert_eq!(
        import.document.metadata.title.as_deref(),
        Some("Field Notes")
    );
    assert_eq!(
        import.document.metadata.author.as_deref(),
        Some("A. Author")
    );
}

#[test]
fn the_report_counts_what_was_flattened_or_dropped() {
    let import = import_docx(BASIC).unwrap();
    let items = import.report.items();
    let find = |feature: &str| {
        items
            .iter()
            .find(|i| i.feature == feature)
            .unwrap_or_else(|| panic!("no {feature}"))
    };
    assert_eq!(find("tables").outcome, Fidelity::Approximated);
    assert!(find("tables").note.starts_with("1 table"));
    assert_eq!(find("lists").outcome, Fidelity::Approximated);
    assert!(find("lists").note.starts_with("2 list item"));
    // Mixed bold and italic are kept as runs now, so they are not reported as lost.
    assert!(items.iter().all(|i| i.feature != "inline formatting"));
    assert_eq!(find("images").outcome, Fidelity::Dropped);
    assert!(find("images").note.starts_with("1 image"));
    assert!(!import.report.is_lossless());
}

#[test]
fn a_zip_without_a_body_is_an_error() {
    let mut w = ArchiveWriter::new();
    w.add("docProps/core.xml", b"<x/>".to_vec()).unwrap();
    let bytes = w.finish().unwrap();
    assert!(import_docx(&bytes).is_err());
}

#[test]
fn a_paragraph_with_only_spaces_is_skipped_and_breaks_are_kept() {
    let body = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
        <w:p><w:r><w:t xml:space="preserve">   </w:t></w:r></w:p>
        <w:p><w:r><w:t>a</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>b</w:t></w:r></w:p>
        <w:p><w:pPr><w:pStyle w:val="Heading3"/></w:pPr><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>c</w:t></w:r></w:p>
        </w:body></w:document>"#;
    let mut w = ArchiveWriter::new();
    w.add("word/document.xml", body.as_bytes().to_vec())
        .unwrap();
    let import = import_docx(&w.finish().unwrap()).unwrap();
    let got = paragraphs(&import.document);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, "a\tb");
    assert_eq!(got[1], ("c".to_string(), Some(3), false));
    assert!(import.document.metadata.title.is_none());
}
