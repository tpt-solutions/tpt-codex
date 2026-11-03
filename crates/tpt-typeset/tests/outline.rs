// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, SheetId, TextStyle};
use tpt_typeset::{DocumentClass, Outline, number_string, slugify};

fn heading(level: u8, title: &str) -> Block {
    Block::Text {
        content: title.into(),
        style: TextStyle {
            bold: false,
            italic: false,
            heading_level: Some(level),
        },
    }
}

fn para(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

fn doc_of(children: Vec<Block>) -> Document {
    let n = children.len() as u64;
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=n).map(BlockId).collect(),
        },
    );
    for (i, b) in children.into_iter().enumerate() {
        doc.insert(BlockId(i as u64 + 1), b);
    }
    doc
}

fn numbers(o: &Outline) -> Vec<String> {
    o.entries.iter().map(|e| number_string(&e.number)).collect()
}

#[test]
fn numbers_sections_hierarchically() {
    let doc = doc_of(vec![
        heading(1, "A"),
        heading(2, "A1"),
        heading(2, "A2"),
        heading(3, "A2a"),
        heading(1, "B"),
        heading(2, "B1"),
    ]);
    let o = Outline::of(&doc);
    assert_eq!(numbers(&o), vec!["1", "1.1", "1.2", "1.2.1", "2", "2.1"]);
}

#[test]
fn skipped_levels_are_numbered_one() {
    let doc = doc_of(vec![heading(2, "Orphan"), heading(3, "Deep")]);
    let o = Outline::of(&doc);
    assert_eq!(numbers(&o), vec!["1.1", "1.1.1"]);
}

#[test]
fn anchors_are_slugs_and_unique() {
    let doc = doc_of(vec![
        heading(1, "Introduction"),
        heading(2, "Introduction"),
        heading(1, "Hello, World! 2026"),
    ]);
    let o = Outline::of(&doc);
    let anchors: Vec<&str> = o.entries.iter().map(|e| e.anchor.as_str()).collect();
    assert_eq!(
        anchors,
        vec!["introduction", "introduction-2", "hello-world-2026"]
    );
    assert_eq!(o.resolve("introduction-2").unwrap().title, "Introduction");
    assert!(o.resolve("missing").is_none());
}

#[test]
fn slugify_edge_cases() {
    assert_eq!(slugify("  Leading & trailing  "), "leading-trailing");
    assert_eq!(slugify("!!!"), "section");
    assert_eq!(slugify("Ünïcode title"), "n-code-title");
}

#[test]
fn toc_respects_depth_and_indents() {
    let doc = doc_of(vec![heading(1, "A"), heading(2, "A1"), heading(3, "A1x")]);
    let o = Outline::of(&doc);
    let toc = o.toc(2);
    assert_eq!(toc.len(), 2);
    assert_eq!(toc[0].depth, 0);
    assert_eq!(toc[1].depth, 1);
    assert_eq!(toc[1].number, "1.1");
    assert_eq!(o.toc(3).len(), 3);
}

#[test]
fn headings_inside_flow_containers_are_included_once() {
    let mut doc = doc_of(vec![
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(3)],
        },
        // Shared: referenced by the root and by the container above.
        heading(1, "Shared"),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    ]);
    // Make the root also reference block 3 (the heading) directly.
    if let Some(Block::Container { children, .. }) = doc.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(3));
    }
    let o = Outline::of(&doc);
    assert_eq!(o.entries.len(), 1);
    assert_eq!(o.entries[0].title, "Shared");
}

#[test]
fn body_text_is_not_a_heading() {
    let doc = doc_of(vec![para("not a heading")]);
    assert!(Outline::of(&doc).entries.is_empty());
}

#[test]
fn document_classes_parse_and_describe_themselves() {
    assert_eq!(DocumentClass::parse("article"), Ok(DocumentClass::Article));
    assert_eq!(DocumentClass::parse("report").unwrap().spec().toc_depth, 3);
    assert_eq!(DocumentClass::Article.spec().toc_depth, 2);
    let err = DocumentClass::parse("Article").unwrap_err();
    assert!(err.to_string().contains("'Article'"));
}

#[test]
fn random_outlines_have_consistent_numbers_and_unique_anchors() {
    let mut state = 12345u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..200 {
        let n = next() % 12;
        let children: Vec<Block> = (0..n)
            .map(|i| heading(1 + (next() % 6) as u8, &format!("T{}", i % 3)))
            .collect();
        let doc = doc_of(children);
        let o = Outline::of(&doc);
        assert_eq!(o.entries.len(), n);
        for e in &o.entries {
            assert_eq!(e.number.len(), e.level as usize);
            assert!(e.number.iter().all(|&x| x >= 1));
        }
        let mut anchors: Vec<&str> = o.entries.iter().map(|e| e.anchor.as_str()).collect();
        let total = anchors.len();
        anchors.sort();
        anchors.dedup();
        assert_eq!(anchors.len(), total, "anchors must be unique");
    }
}
