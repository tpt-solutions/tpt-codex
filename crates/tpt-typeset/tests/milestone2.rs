// SPDX-License-Identifier: MIT OR Apache-2.0
//! Milestone 2, up to the page: a `.cdx`-style document with Markdown, math, cross-references,
//! footnotes and citations is read, numbered and checked. The PDF export itself is blocked on
//! `tpt-canvas-pdf` and is not tested here.

use tpt_codex_markdown::from_markdown;
use tpt_typeset::{
    Library, Outline, check_citations, check_footnotes, check_math_blocks, check_references,
    footnote_numbers, unreferenced_footnotes,
};

const DOC: &str = "---\ntitle: \"Notes\"\nclass: report\n---\n\n\
# Introduction\n\nSee [the method](#method) and the result [@smith2020].\n\n\
A claim with a note[^m].\n\n\
## Method\n\n$$\nE = mc^2\n$$\n\n\
The equation above is inline-free; see [the intro](#introduction) too.\n\n\
[^m]: The note explains the claim.\n";

const LIBRARY: &str = r#"[{"id":"smith2020","title":"Notes","author":[{"family":"Smith"}],"issued":{"date-parts":[[2020]]}}]"#;

#[test]
fn a_cdx_style_document_is_read_numbered_and_checked() {
    let doc = from_markdown(DOC).unwrap();
    doc.validate().unwrap();
    assert_eq!(doc.metadata.title.as_deref(), Some("Notes"));

    // Outline and table of contents: two sections, Method numbered 1.1 under Introduction.
    let outline = Outline::of(&doc);
    let toc = outline.toc(3);
    assert_eq!(toc.len(), 2, "{toc:?}");
    assert_eq!(outline.resolve("method").unwrap().number, vec![1, 1]);

    // Math parses, and the cross-references resolve.
    assert!(check_math_blocks(&doc).is_empty());
    let refs = check_references(&doc);
    assert_eq!(refs.len(), 2);
    assert!(refs.iter().all(|r| r.number.is_some()), "{refs:?}");

    // Footnote numbered 1, referenced once, defined.
    assert_eq!(footnote_numbers(&doc).get("m"), Some(&1));
    let notes = check_footnotes(&doc);
    assert_eq!(notes.len(), 1);
    assert!(notes[0].definition.is_some());
    assert!(unreferenced_footnotes(&doc).is_empty());

    // Citation resolves against the library.
    let library = Library::from_csl_json(LIBRARY).unwrap();
    let cites = check_citations(&doc, &library);
    assert_eq!(cites.len(), 1);
    assert_eq!(cites[0].text.as_deref(), Some("Smith 2020"));
}

#[test]
fn the_document_writes_back_to_the_same_markdown() {
    let doc = from_markdown(DOC).unwrap();
    let written = tpt_codex_markdown::to_markdown(&doc).unwrap();
    assert_eq!(from_markdown(&written).unwrap(), doc);
}
