// SPDX-License-Identifier: MIT OR Apache-2.0
//! A document saved as PDF opens again with its text, through the same path as any other file.

use tpt_codex_core::Block;
use tpt_codex_files::{open, save};
use tpt_codex_markdown::from_markdown;

#[test]
fn a_saved_pdf_opens_with_its_headings_and_paragraphs() {
    let doc = from_markdown("# Title\n\nA paragraph of words.\n\nAnother one.\n").unwrap();
    let pdf = save(&doc, "doc.pdf").unwrap().bytes;
    let opened = open("doc.pdf", &pdf).unwrap();
    let Some(Block::Container { children, .. }) = opened.document.get(opened.document.root) else {
        panic!("the root is not a container");
    };
    let text: Vec<(Option<u8>, String)> = children
        .iter()
        .map(|id| match opened.document.get(*id) {
            Some(Block::Text { content, style }) => (style.heading_level, content.clone()),
            other => panic!("expected text, got {other:?}"),
        })
        .collect();
    assert_eq!(
        text,
        vec![
            (Some(1), "Title".to_string()),
            (None, "A paragraph of words. Another one.".to_string()),
        ]
    );
    assert!(opened.report.is_some());
}
