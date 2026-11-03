// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Metadata, TextStyle};
use tpt_codex_markdown::{from_markdown, to_markdown};

fn para(s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle::default(),
    }
}

fn heading(level: u8, s: &str) -> Block {
    Block::Text {
        content: s.into(),
        style: TextStyle {
            bold: false,
            italic: false,
            heading_level: Some(level),
        },
    }
}

/// Builds a document in the shape `from_markdown` produces: root 0, children 1..=n.
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

#[test]
fn reads_headings_and_paragraphs() {
    let src = "# Introduction\n\nThis is a paragraph\nthat spans two lines.\n\n## Live Data\n\nMore text.\n";
    let doc = from_markdown(src).unwrap();
    assert_eq!(doc.validate(), Ok(()));
    assert_eq!(
        doc,
        doc_of(vec![
            heading(1, "Introduction"),
            para("This is a paragraph\nthat spans two lines."),
            heading(2, "Live Data"),
            para("More text."),
        ])
    );
}

#[test]
fn reads_front_matter_with_quoted_values() {
    let src = "---\ntitle: \"A \\\"Modern\\\" Approach\"\nauthor: Phillip @ TPT Solutions\ndate: 2026-11-04\nclass: article\n---\n\n# Intro\n";
    let doc = from_markdown(src).unwrap();
    assert_eq!(
        doc.metadata,
        Metadata {
            title: Some("A \"Modern\" Approach".into()),
            author: Some("Phillip @ TPT Solutions".into()),
            date: Some("2026-11-04".into()),
            class: Some("article".into()),
        }
    );
    assert_eq!(doc.blocks.len(), 2);
}

#[test]
fn empty_input_gives_empty_document() {
    let doc = from_markdown("").unwrap();
    assert_eq!(doc, doc_of(vec![]));
}

#[test]
fn hash_without_space_is_a_paragraph() {
    let doc = from_markdown("#hashtag\n").unwrap();
    assert_eq!(doc, doc_of(vec![para("#hashtag")]));
}

#[test]
fn seven_hashes_is_a_paragraph() {
    let doc = from_markdown("####### too deep\n").unwrap();
    assert_eq!(doc, doc_of(vec![para("####### too deep")]));
}

#[test]
fn unsupported_list_reports_line_and_hint() {
    let err = from_markdown("# Title\n\n- item one\n").unwrap_err();
    assert_eq!(err.line, 3);
    assert_eq!(err.column, 1);
    assert!(err.message.contains("bullet list"), "{}", err.message);
    assert!(err.suggestion.is_some());
}

#[test]
fn each_unsupported_construct_is_an_error() {
    for src in [
        "```\ncode\n```\n",
        "> quote\n",
        "1. first\n",
        "| a | b |\n",
        "    indented\n",
        "para\n\n---\n",
        "***\n",
        "$$\nx\n$$\n",
        ":::  grid\n",
    ] {
        assert!(from_markdown(src).is_err(), "should reject: {src:?}");
    }
}

#[test]
fn unknown_front_matter_key_is_an_error() {
    let err = from_markdown("---\nversion: 2\n---\n").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.message.contains("version"));
}

#[test]
fn unclosed_front_matter_is_an_error() {
    let err = from_markdown("---\ntitle: \"x\"\n").unwrap_err();
    assert!(err.message.contains("not closed"));
}

#[test]
fn bad_escape_in_quoted_value_is_an_error() {
    let err = from_markdown("---\ntitle: \"bad \\n escape\"\n---\n").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.message.contains("escape"));
}

#[test]
fn empty_heading_is_an_error() {
    let err = from_markdown("#\n").unwrap_err();
    assert_eq!(err.line, 1);
}

#[test]
fn error_display_includes_position() {
    let err = from_markdown("- x\n").unwrap_err();
    let text = err.to_string();
    assert!(text.starts_with("line 1, column 1:"), "{text}");
}

#[test]
fn writes_front_matter_and_body() {
    let mut doc = doc_of(vec![heading(1, "Intro"), para("Hello.")]);
    doc.metadata.title = Some("Say \"hi\"".into());
    let md = to_markdown(&doc).unwrap();
    assert_eq!(
        md,
        "---\ntitle: \"Say \\\"hi\\\"\"\n---\n\n# Intro\n\nHello.\n"
    );
}

#[test]
fn round_trip_of_supported_subset_is_exact() {
    let doc = doc_of(vec![
        heading(1, "Introduction"),
        para("Line one\n  indented continuation\n#not-a-heading"),
        heading(6, "Deep #heading #"),
        para("Last"),
    ]);
    let md = to_markdown(&doc).unwrap();
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn writer_refuses_bold_and_italic_rather_than_dropping_them() {
    let doc = doc_of(vec![Block::Text {
        content: "x".into(),
        style: TextStyle {
            bold: true,
            italic: false,
            heading_level: None,
        },
    }]);
    let err = to_markdown(&doc).unwrap_err();
    assert_eq!(err.index, Some(0));
    assert!(err.reason.contains("bold"));
}

#[test]
fn writer_refuses_blocks_outside_the_subset() {
    let doc = doc_of(vec![Block::Query(tpt_codex_core::Query {
        source: "SELECT 1".into(),
    })]);
    assert!(to_markdown(&doc).is_err());
}

#[test]
fn writer_refuses_text_that_would_read_back_differently() {
    for content in [
        "- item",
        "# heading",
        "a\n\nb",
        "a\n---",
        " padded ",
        "",
        "a\n    code",
    ] {
        let doc = doc_of(vec![para(content)]);
        assert!(to_markdown(&doc).is_err(), "should refuse {content:?}");
    }
}

#[test]
fn writer_refuses_multiline_heading() {
    let doc = doc_of(vec![heading(1, "two\nlines")]);
    assert!(to_markdown(&doc).is_err());
}

#[test]
fn writer_refuses_non_flow_root() {
    let doc = Document::new(BlockId(0), para("loose"));
    assert!(to_markdown(&doc).is_err());
}

#[test]
fn writer_refuses_front_matter_with_line_break() {
    let mut doc = doc_of(vec![]);
    doc.metadata.author = Some("a\nb".into());
    assert!(to_markdown(&doc).is_err());
}

// Randomized round trip: any document the writer accepts must read back equal.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const ALPHABET: &[char] = &[
    'a', 'b', ' ', '#', '-', '*', '>', '|', '`', '\\', '"', '1', '.', '\t', 'é',
];

fn random_text(rng: &mut Rng) -> String {
    let len = 1 + rng.below(12);
    (0..len)
        .map(|_| ALPHABET[rng.below(ALPHABET.len())])
        .collect()
}

#[test]
fn randomized_documents_round_trip_or_are_refused() {
    let mut accepted = 0;
    for seed in 0..500u64 {
        let mut rng = Rng(seed);
        let n = rng.below(8);
        let children: Vec<Block> = (0..n)
            .map(|_| {
                let text = random_text(&mut rng);
                if rng.below(3) == 0 {
                    heading(1 + rng.below(6) as u8, &text)
                } else {
                    para(&text)
                }
            })
            .collect();
        let mut doc = doc_of(children);
        if rng.below(2) == 0 {
            doc.metadata.title = Some(random_text(&mut rng));
        }
        if let Ok(md) = to_markdown(&doc) {
            accepted += 1;
            let back = from_markdown(&md)
                .unwrap_or_else(|e| panic!("seed {seed}: writer output failed to read: {e}\n{md}"));
            assert_eq!(
                back, doc,
                "seed {seed}: round trip changed the document\n{md}"
            );
        }
    }
    assert!(
        accepted > 100,
        "too few documents were accepted: {accepted}"
    );
}

// Not a fuzzer: random inputs must never panic. Malformed input returns an error.
#[test]
fn reader_never_panics_on_random_input() {
    const CHARS: &[char] = &[
        'a',
        ' ',
        '\n',
        '\r',
        '\t',
        '#',
        '-',
        '*',
        '>',
        '|',
        '`',
        '~',
        '"',
        '\\',
        ':',
        '$',
        '[',
        '!',
        '1',
        '.',
        ')',
        '=',
        '_',
        'é',
        '\u{1F600}',
        '\0',
    ];
    for seed in 0..2000u64 {
        let mut rng = Rng(seed ^ 0xABCD);
        let len = rng.below(120);
        let src: String = (0..len).map(|_| CHARS[rng.below(CHARS.len())]).collect();
        if let Ok(doc) = from_markdown(&src) {
            assert_eq!(
                doc.validate(),
                Ok(()),
                "seed {seed}: accepted an invalid document"
            );
        }
    }
}
