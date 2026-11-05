// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_markdown::from_markdown;
use tpt_typeset::check_references;

#[test]
fn references_resolve_to_section_numbers_and_missing_ones_are_reported() {
    let md =
        "# Intro\n\nSee [the details](#details) and [nothing](#missing).\n\n## Details\n\nBody.\n";
    let doc = from_markdown(md).unwrap();
    let refs = check_references(&doc);
    assert_eq!(refs.len(), 2, "{refs:?}");
    let found = refs.iter().find(|r| r.target == "details").unwrap();
    assert_eq!(found.number.as_deref(), Some("1.1"));
    let missing = refs.iter().find(|r| r.target == "missing").unwrap();
    assert_eq!(missing.number, None);
}

#[test]
fn ordinary_links_are_not_references() {
    let md = "A [site](https://example.test) only.\n";
    let doc = from_markdown(md).unwrap();
    assert!(check_references(&doc).is_empty());
}

#[test]
fn a_document_without_links_has_no_references() {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    d.insert(
        BlockId(1),
        Block::Text {
            content: "plain".into(),
            style: TextStyle::default(),
        },
    );
    assert!(check_references(&d).is_empty());
}
