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
fn loose_list_reports_line_and_hint() {
    let err = from_markdown("# Title\n\n- item one\n\n- item two\n").unwrap_err();
    assert_eq!(err.line, 5);
    assert_eq!(err.column, 1);
    assert!(err.message.contains("loose"), "{}", err.message);
    assert!(err.suggestion.is_some());
}

#[test]
fn each_unsupported_construct_is_an_error() {
    for src in [
        "> quote\n",
        "| a | b |\n",
        "    indented\n",
        ":::  grid\n",
        "<div>\n",
        "[^1]: note\n",
        "para\n---\n",
        "para\n![a](b.png)\n",
        "- item\n  continued\n",
        "- item\n    nested\n",
        "```\ncode\n",
        "1. one\n\n2. two\n",
        "text\n3. not one\n",
        "- \n",
        "![a](b c)\n",
        "![a](b)\nmore text\n",
    ] {
        assert!(from_markdown(src).is_err(), "should reject: {src:?}");
    }
}

#[test]
fn reads_bulleted_and_numbered_lists() {
    let doc = from_markdown("- one\n- two\n\n3) three\n4) four\n").unwrap();
    assert_eq!(doc, {
        let mut d = Document::new(
            BlockId(0),
            Block::Container {
                layout_mode: LayoutMode::Flow,
                children: alloc_ids(&[3, 6]),
            },
        );
        d.insert(BlockId(1), para("one"));
        d.insert(BlockId(2), para("two"));
        d.insert(
            BlockId(3),
            Block::List {
                ordered: false,
                start: 1,
                items: alloc_ids(&[1, 2]),
            },
        );
        d.insert(BlockId(4), para("three"));
        d.insert(BlockId(5), para("four"));
        d.insert(
            BlockId(6),
            Block::List {
                ordered: true,
                start: 3,
                items: alloc_ids(&[4, 5]),
            },
        );
        d
    });
}

fn alloc_ids(raw: &[u64]) -> Vec<BlockId> {
    raw.iter().copied().map(BlockId).collect()
}

#[test]
fn reads_code_rules_and_images() {
    let src = "```rust\nfn main() {}\n\n    indented\n```\n\n***\n\n![Logo](images/logo.png)\n";
    let doc = from_markdown(src).unwrap();
    let kids: Vec<&Block> = match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => {
            children.iter().filter_map(|c| doc.get(*c)).collect()
        }
        _ => panic!("root is not a container"),
    };
    assert_eq!(kids.len(), 3);
    assert_eq!(
        kids[0],
        &Block::Code {
            language: Some("rust".into()),
            code: "fn main() {}\n\n    indented".into(),
        }
    );
    assert_eq!(kids[1], &Block::ThematicBreak);
    assert_eq!(
        kids[2],
        &Block::Media {
            source: tpt_codex_core::MediaSource::Uri {
                uri: "images/logo.png".into()
            },
            alt_text: Some("Logo".into()),
        }
    );
}

#[test]
fn fence_closes_only_with_the_same_character_and_length() {
    let doc = from_markdown("````\n```\n~~~\n````\n").unwrap();
    let kids = match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children.clone(),
        _ => panic!(),
    };
    assert_eq!(kids.len(), 1);
    assert_eq!(
        doc.get(kids[0]),
        Some(&Block::Code {
            language: None,
            code: "```\n~~~".into(),
        })
    );
}

#[test]
fn reads_a_paragraph_that_a_bullet_interrupts() {
    let doc = from_markdown("Intro\n- point\n").unwrap();
    assert_eq!(doc.blocks.len(), 4, "root plus three blocks");
    assert_eq!(doc.get(BlockId(1)), Some(&para("Intro")));
    assert!(matches!(
        doc.get(BlockId(3)),
        Some(Block::List { ordered: false, .. })
    ));
}

#[test]
fn writes_lists_code_rules_and_images_and_reads_them_back() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: alloc_ids(&[1, 2, 3, 4, 5]),
        },
    );
    doc.insert(BlockId(1), heading(2, "Steps"));
    doc.insert(BlockId(2), para("first"));
    doc.insert(
        BlockId(3),
        Block::Code {
            language: None,
            code: "a ``` b\n".into(),
        },
    );
    doc.insert(BlockId(4), Block::ThematicBreak);
    doc.insert(
        BlockId(5),
        Block::Media {
            source: tpt_codex_core::MediaSource::Uri {
                uri: "a.png".into(),
            },
            alt_text: None,
        },
    );
    let md = to_markdown(&doc).unwrap();
    assert_eq!(
        md,
        "## Steps\n\nfirst\n\n````\na ``` b\n\n````\n\n***\n\n![](a.png)\n"
    );
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn list_numbers_are_written_from_the_start_value() {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: alloc_ids(&[3]),
        },
    );
    doc.insert(BlockId(1), para("a"));
    doc.insert(BlockId(2), para("b"));
    doc.insert(
        BlockId(3),
        Block::List {
            ordered: true,
            start: 7,
            items: alloc_ids(&[1, 2]),
        },
    );
    let md = to_markdown(&doc).unwrap();
    assert_eq!(md, "7. a\n8. b\n");
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn writer_refuses_lists_it_cannot_write_back_exactly() {
    let list = |ordered: bool, start: u32, items: &[&str]| {
        let n = items.len() as u64;
        let mut doc = Document::new(
            BlockId(0),
            Block::Container {
                layout_mode: LayoutMode::Flow,
                children: alloc_ids(&[n + 1]),
            },
        );
        for (i, t) in items.iter().enumerate() {
            doc.insert(BlockId(i as u64 + 1), para(t));
        }
        doc.insert(
            BlockId(n + 1),
            Block::List {
                ordered,
                start,
                items: (1..=n).map(BlockId).collect(),
            },
        );
        doc
    };
    assert!(to_markdown(&list(false, 1, &["a", "b"])).is_ok());
    assert!(
        to_markdown(&list(false, 2, &["a"])).is_err(),
        "bullet start"
    );
    assert!(
        to_markdown(&list(true, 1, &["a---"])).is_ok(),
        "ordered dashes are fine"
    );
    assert!(
        to_markdown(&list(false, 1, &["---"])).is_err(),
        "rule-like item"
    );
    assert!(
        to_markdown(&list(false, 1, &["a\nb"])).is_err(),
        "multi-line item"
    );
    assert!(
        to_markdown(&list(false, 1, &[" a"])).is_err(),
        "padded item"
    );
    assert!(to_markdown(&list(false, 1, &[""])).is_err(), "empty item");
    assert!(to_markdown(&list(true, 1, &[])).is_err(), "empty list");
    assert!(
        to_markdown(&list(true, 999_999_999, &["a", "b"])).is_err(),
        "number overflow"
    );
}

#[test]
fn writer_refuses_adjacent_lists_and_numbering_after_a_paragraph() {
    // Two bulleted lists in a row would read back as one list.
    let mut adjacent = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: alloc_ids(&[3, 4]),
        },
    );
    adjacent.insert(BlockId(1), para("a"));
    adjacent.insert(BlockId(2), para("b"));
    adjacent.insert(
        BlockId(3),
        Block::List {
            ordered: false,
            start: 1,
            items: alloc_ids(&[1]),
        },
    );
    adjacent.insert(
        BlockId(4),
        Block::List {
            ordered: false,
            start: 1,
            items: alloc_ids(&[2]),
        },
    );
    assert!(to_markdown(&adjacent).is_err());

    let mut after_para = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: alloc_ids(&[1, 3]),
        },
    );
    after_para.insert(BlockId(1), para("intro"));
    after_para.insert(BlockId(2), para("x"));
    after_para.insert(
        BlockId(3),
        Block::List {
            ordered: true,
            start: 3,
            items: alloc_ids(&[2]),
        },
    );
    assert!(to_markdown(&after_para).is_err());
}

#[test]
fn writer_refuses_code_and_image_it_cannot_write_back_exactly() {
    let code = |language: Option<&str>, code: &str| {
        let doc = doc_of(vec![Block::Code {
            language: language.map(String::from),
            code: code.into(),
        }]);
        to_markdown(&doc)
    };
    assert!(code(Some("rust"), "x").is_ok());
    assert!(code(Some("a`b"), "x").is_err());
    assert!(code(Some(" rust"), "x").is_err());
    assert!(code(Some("rust\\n"), "x").is_err());
    assert!(code(None, "a\rb").is_err());

    let image = |alt: Option<&str>, uri: &str| {
        to_markdown(&doc_of(vec![Block::Media {
            source: tpt_codex_core::MediaSource::Uri { uri: uri.into() },
            alt_text: alt.map(String::from),
        }]))
    };
    assert!(image(Some("Logo"), "logo.png").is_ok());
    assert!(
        image(Some(""), "logo.png").is_err(),
        "empty alt is not the same as none"
    );
    assert!(image(Some("a [b]"), "logo.png").is_err());
    assert!(image(None, "my logo.png").is_err());
    assert!(image(None, "").is_err());
    assert!(
        to_markdown(&doc_of(vec![Block::Media {
            source: tpt_codex_core::MediaSource::Svg {
                markup: "<svg/>".into()
            },
            alt_text: None,
        }]))
        .is_err()
    );
}

#[test]
fn list_item_text_in_a_paragraph_line_is_refused() {
    assert!(to_markdown(&doc_of(vec![para("intro\n1. x")])).is_err());
    assert!(to_markdown(&doc_of(vec![para("intro\n***")])).is_err());
    assert!(to_markdown(&doc_of(vec![para("intro\n![a](b)")])).is_err());
}

#[test]
fn unclosed_code_and_image_after_text_have_hints() {
    let err = from_markdown("text\n![a](b)\n").unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.suggestion.is_some());
    let err = from_markdown("```\nnever closed\n").unwrap_err();
    assert!(err.message.contains("not closed"), "{}", err.message);
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
    let err = from_markdown("para\n---\n").unwrap_err();
    let text = err.to_string();
    assert!(text.starts_with("line 2, column 1:"), "{text}");
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

/// A top-level block in the random generator. Lists expand to items plus the list block.
enum Node {
    Heading(u8, String),
    Para(String),
    List {
        ordered: bool,
        start: u32,
        items: Vec<String>,
    },
    Code {
        language: Option<String>,
        code: String,
    },
    Break,
    Image {
        alt: Option<String>,
        uri: String,
    },
}

/// Builds the document the reader should produce: items are allocated before their list.
fn build(nodes: Vec<Node>) -> Document {
    let mut next = 1u64;
    let mut blocks: Vec<(BlockId, Block)> = Vec::new();
    let mut root = Vec::new();
    let alloc = |next: &mut u64| {
        let id = BlockId(*next);
        *next += 1;
        id
    };
    for node in nodes {
        match node {
            Node::Heading(level, text) => {
                let id = alloc(&mut next);
                blocks.push((id, heading(level, &text)));
                root.push(id);
            }
            Node::Para(text) => {
                let id = alloc(&mut next);
                blocks.push((id, para(&text)));
                root.push(id);
            }
            Node::List {
                ordered,
                start,
                items,
            } => {
                let mut ids = Vec::new();
                for text in items {
                    let id = alloc(&mut next);
                    blocks.push((id, para(&text)));
                    ids.push(id);
                }
                let id = alloc(&mut next);
                blocks.push((
                    id,
                    Block::List {
                        ordered,
                        start,
                        items: ids,
                    },
                ));
                root.push(id);
            }
            Node::Code { language, code } => {
                let id = alloc(&mut next);
                blocks.push((id, Block::Code { language, code }));
                root.push(id);
            }
            Node::Break => {
                let id = alloc(&mut next);
                blocks.push((id, Block::ThematicBreak));
                root.push(id);
            }
            Node::Image { alt, uri } => {
                let id = alloc(&mut next);
                blocks.push((
                    id,
                    Block::Media {
                        source: tpt_codex_core::MediaSource::Uri { uri },
                        alt_text: alt,
                    },
                ));
                root.push(id);
            }
        }
    }
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: root,
        },
    );
    for (id, block) in blocks {
        doc.insert(id, block);
    }
    doc
}

fn random_node(rng: &mut Rng) -> Node {
    match rng.below(10) {
        0..=2 => Node::Heading(1 + rng.below(6) as u8, random_text(rng)),
        3..=4 => Node::Para(random_text(rng)),
        5 => Node::List {
            ordered: rng.below(2) == 0,
            start: 1 + rng.below(3) as u32,
            items: (0..1 + rng.below(3)).map(|_| random_text(rng)).collect(),
        },
        6 => Node::Code {
            language: (rng.below(2) == 0).then(|| random_text(rng)),
            code: (0..rng.below(4))
                .map(|_| random_text(rng))
                .collect::<Vec<_>>()
                .join("\n"),
        },
        7 => Node::Break,
        _ => Node::Image {
            alt: (rng.below(2) == 0).then(|| random_text(rng)),
            uri: random_text(rng),
        },
    }
}

#[test]
fn randomized_documents_round_trip_or_are_refused() {
    let mut accepted = 0;
    for seed in 0..500u64 {
        let mut rng = Rng(seed);
        let n = rng.below(8);
        let nodes: Vec<Node> = (0..n).map(|_| random_node(&mut rng)).collect();
        let mut doc = build(nodes);
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

fn flow_of(children: Vec<Block>) -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=children.len() as u64).map(BlockId).collect(),
        },
    );
    for (i, block) in children.into_iter().enumerate() {
        doc.insert(BlockId(i as u64 + 1), block);
    }
    doc
}

fn math(source: &str) -> Block {
    Block::Math {
        source: source.into(),
    }
}

fn math_sources(src: &str) -> Vec<String> {
    let doc = from_markdown(src).unwrap();
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!("root is not a container");
    };
    children
        .iter()
        .map(|id| match doc.get(*id) {
            Some(Block::Math { source }) => source.clone(),
            other => panic!("expected math, got {other:?}"),
        })
        .collect()
}

#[test]
fn reads_display_math_in_both_forms() {
    assert_eq!(
        math_sources("$$\nx^2 + y\n$$\n\n$$ a = b $$\n"),
        vec!["x^2 + y".to_string(), "a = b".to_string()]
    );
}

#[test]
fn math_source_is_kept_exactly_including_blank_and_indented_lines() {
    assert_eq!(
        math_sources("$$\n  \\sum_i a_i\n\n$$\n"),
        vec!["  \\sum_i a_i\n".to_string()]
    );
}

#[test]
fn math_interrupts_paragraphs_without_joining_them() {
    let doc = from_markdown("text\n$$\nx\n$$\nmore\n").unwrap();
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!();
    };
    assert_eq!(children.len(), 3);
    assert_eq!(doc.get(children[0]), Some(&para("text")));
    assert_eq!(doc.get(children[1]), Some(&math("x")));
    assert_eq!(doc.get(children[2]), Some(&para("more")));
}

#[test]
fn rejects_malformed_math() {
    for src in [
        "$$\nx\n",      // not closed
        "$$\n$$\n",     // empty
        "$$ $$\n",      // empty single line
        "$$x\n",        // no closing delimiter
        "$$ a $$ b\n",  // text after the closing delimiter
        "$$ a $$ $$\n", // two delimiters on one line
    ] {
        assert!(from_markdown(src).is_err(), "should reject: {src:?}");
    }
}

#[test]
fn math_writes_and_reads_back_identically() {
    let doc = flow_of(vec![
        math("x^2"),
        para("between"),
        math("a\n\n    \\frac{1}{2}\n  "),
    ]);
    let md = to_markdown(&doc).unwrap();
    assert_eq!(
        md,
        "$$\nx^2\n$$\n\nbetween\n\n$$\na\n\n    \\frac{1}{2}\n  \n$$\n"
    );
    let back = from_markdown(&md).unwrap();
    assert_eq!(back.get(BlockId(1)), Some(&math("x^2")));
    assert_eq!(
        back.get(BlockId(3)),
        Some(&math("a\n\n    \\frac{1}{2}\n  "))
    );
}

#[test]
fn writer_refuses_math_that_would_not_read_back() {
    for source in ["", "a\r\nb", "a\n$$\nb", "a\n   $$  \nb"] {
        let doc = flow_of(vec![math(source)]);
        assert!(to_markdown(&doc).is_err(), "should refuse: {source:?}");
    }
    // Four leading spaces do not close a block, so this one is fine.
    let doc = flow_of(vec![math("a\n    $$")]);
    assert!(to_markdown(&doc).is_ok());
}

#[test]
fn writer_refuses_paragraph_text_that_starts_math() {
    let doc = flow_of(vec![para("$$ not math")]);
    assert!(to_markdown(&doc).is_err());
}
