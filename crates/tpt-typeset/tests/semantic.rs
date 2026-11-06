//! The semantic tree: sections nest by heading level, bodies stop at the next heading of the same
//! or a higher level, and footnotes and citations are listed where they are referenced.

use tpt_codex_core::{Block, BlockId, Document};
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{Outline, Section, semantic_tree};

const SAMPLE: &str = "Preface with a note [^n] and a cite [@smith2020].

# Introduction

Body one.

## Background

Body two with a note [^n] and [Smith](@smith2020).

# Method

Body three with a note [^m].

[^n]: first note

[^m]: second note
";

/// The text of each text block in `ids`.
fn texts(doc: &Document, ids: &[BlockId]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| match doc.get(*id) {
            Some(Block::Text { content, .. }) => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn sections_nest_by_heading_level_and_carry_their_numbers() {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);

    assert_eq!(tree.sections.len(), 2);
    let intro = &tree.sections[0];
    assert_eq!(intro.title, "Introduction");
    assert_eq!(intro.number, vec![1]);
    assert_eq!(intro.children.len(), 1);
    let background = &intro.children[0];
    assert_eq!(background.title, "Background");
    assert_eq!(background.number, vec![1, 1]);
    assert_eq!(background.level, 2);
    assert_eq!(tree.sections[1].title, "Method");
    assert_eq!(tree.sections[1].number, vec![2]);
    assert!(tree.sections[1].children.is_empty());
}

#[test]
fn bodies_stop_at_the_next_heading_of_the_same_or_higher_level() {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);
    let intro = &tree.sections[0];
    let background = &intro.children[0];

    assert_eq!(texts(&doc, &intro.body), vec!["Body one.".to_string()]);
    let background_text = texts(&doc, &background.body);
    assert_eq!(background_text.len(), 1);
    assert!(background_text[0].starts_with("Body two"));
    // The heading itself is not part of its own body.
    assert!(!intro.body.contains(&intro.heading));
    assert_eq!(
        texts(&doc, &tree.preamble)[0],
        "Preface with a note n and a cite smith2020."
    );
}

#[test]
fn footnotes_and_citations_are_listed_where_they_are_referenced() {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);
    let intro = &tree.sections[0];
    let background = &intro.children[0];

    // Document-wide lists keep reading order: the preamble comes first.
    assert_eq!(tree.footnotes, vec!["n", "m"]);
    assert_eq!(tree.citations, vec!["smith2020"]);

    assert!(intro.footnotes.is_empty());
    assert_eq!(background.footnotes, vec!["n"]);
    assert_eq!(background.citations, vec!["smith2020"]);
    assert_eq!(tree.sections[1].footnotes, vec!["m"]);

    // A section's reachable lists include the sections below it.
    assert_eq!(intro.reachable_footnotes(), vec!["n"]);
    assert_eq!(intro.reachable_citations(), vec!["smith2020"]);
    assert_eq!(tree.sections[1].reachable_citations(), Vec::<String>::new());
}

#[test]
fn the_tree_toc_matches_the_outline_toc() {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);
    let outline = Outline::of(&doc);
    for depth in 1..=3 {
        assert_eq!(tree.toc(depth), outline.toc(depth), "depth {depth}");
    }
    assert_eq!(tree.toc(2).len(), 3);
}

#[test]
fn a_section_can_be_found_by_anchor_at_any_depth() {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);
    let found: &Section = tree.find("background").expect("nested section");
    assert_eq!(found.level, 2);
    assert!(tree.find("nope").is_none());
}

#[test]
fn an_empty_document_has_no_sections() {
    let doc = from_markdown("Just a paragraph.\n").unwrap();
    let tree = semantic_tree(&doc);
    assert!(tree.sections.is_empty());
    assert_eq!(tree.preamble.len(), 1);
    assert!(tree.toc(6).is_empty());
}
