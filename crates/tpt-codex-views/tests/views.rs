// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Rect, SheetId, TextStyle};
use tpt_codex_views::{
    FlowKind, FlowLayout, FlowOptions, LayoutError, MonospaceMeasure, PageError, PageGeometry,
    layout_flow, paginate,
};

const MEASURE: MonospaceMeasure = MonospaceMeasure {
    char_width: 1.0,
    line_height: 10.0,
};

fn opts(width: f64) -> FlowOptions {
    FlowOptions {
        width,
        block_spacing: 0.0,
        box_height: 50.0,
        row_height: 20.0,
        header_rows: 0,
    }
}

fn text(s: &str, style: TextStyle) -> Block {
    Block::Text {
        content: s.into(),
        style,
    }
}

fn plain(s: &str) -> Block {
    text(s, TextStyle::default())
}

/// Builds a document whose root (id 0) is a flow container over `children` (ids 1..).
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

fn lines_of(layout: &FlowLayout) -> Vec<String> {
    layout
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            FlowKind::Line { text, .. } => Some(text.clone()),
            FlowKind::Box | FlowKind::GridRow { .. } => None,
        })
        .collect()
}

#[test]
fn wraps_words_greedily() {
    let doc = doc_of(vec![plain("a bb ccc dddd")]);
    let layout = layout_flow(&doc, &MEASURE, opts(4.0)).unwrap();
    assert_eq!(lines_of(&layout), vec!["a bb", "ccc", "dddd"]);
    assert_eq!(layout.height, 30.0);
    assert_eq!(layout.items[1].y, 10.0);
}

#[test]
fn splits_words_wider_than_the_column() {
    let doc = doc_of(vec![plain("abcdefg")]);
    let layout = layout_flow(&doc, &MEASURE, opts(3.0)).unwrap();
    assert_eq!(lines_of(&layout), vec!["abc", "def", "g"]);
}

#[test]
fn keeps_hard_line_breaks() {
    let doc = doc_of(vec![plain("one\ntwo words")]);
    let layout = layout_flow(&doc, &MEASURE, opts(100.0)).unwrap();
    assert_eq!(lines_of(&layout), vec!["one", "two words"]);
}

#[test]
fn headings_are_larger_than_body_text() {
    let h1 = TextStyle {
        bold: false,
        italic: false,
        heading_level: Some(1),
        runs: Vec::new(),
    };
    let doc = doc_of(vec![text("Title", h1), plain("body")]);
    let layout = layout_flow(&doc, &MEASURE, opts(100.0)).unwrap();
    assert_eq!(layout.items[0].height, 20.0);
    assert_eq!(layout.items[1].height, 10.0);
    assert_eq!(layout.items[1].y, 20.0);
}

#[test]
fn non_text_blocks_become_fixed_boxes() {
    let doc = doc_of(vec![
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
        },
        plain("after"),
    ]);
    let layout = layout_flow(&doc, &MEASURE, opts(100.0)).unwrap();
    assert_eq!(layout.items[0].kind, FlowKind::Box);
    assert_eq!(layout.items[0].height, 50.0);
    assert_eq!(layout.items[1].y, 50.0);
}

#[test]
fn nested_flow_containers_are_flattened() {
    let mut doc = doc_of(vec![Block::Container {
        layout_mode: LayoutMode::Flow,
        children: vec![BlockId(3)],
    }]);
    doc.insert(BlockId(3), plain("inner"));
    let layout = layout_flow(&doc, &MEASURE, opts(100.0)).unwrap();
    assert_eq!(lines_of(&layout), vec!["inner"]);
}

#[test]
fn layout_errors() {
    let doc = doc_of(vec![plain("x")]);
    assert_eq!(
        layout_flow(&doc, &MEASURE, opts(0.0)),
        Err(LayoutError::InvalidWidth(0.0))
    );
    assert!(matches!(
        layout_flow(&doc, &MEASURE, opts(f64::NAN)),
        Err(LayoutError::InvalidWidth(_))
    ));
    let loose = Document::new(BlockId(0), plain("loose"));
    assert_eq!(
        layout_flow(&loose, &MEASURE, opts(10.0)),
        Err(LayoutError::RootNotFlow)
    );
}

// ---- pagination ----

fn para_lines(n: usize) -> Document {
    // A paragraph that wraps to exactly `n` lines at width 10 (each word is 10 chars).
    let words: Vec<String> = (0..n).map(|i| format!("{:09}x", i)).collect();
    doc_of(vec![plain(&words.join("\n"))])
}

fn geometry(lines_per_page: usize) -> PageGeometry {
    PageGeometry {
        height: lines_per_page as f64 * 10.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 2,
    }
}

fn page_line_counts(pages: &[tpt_codex_views::Page]) -> Vec<usize> {
    pages
        .iter()
        .map(|p| {
            p.items
                .iter()
                .filter(|i| matches!(i.kind, FlowKind::Line { .. }))
                .count()
        })
        .collect()
}

#[test]
fn rejects_bad_geometry() {
    let flow = layout_flow(&para_lines(1), &MEASURE, opts(10.0)).unwrap();
    let mut g = geometry(4);
    g.margin_top = 100.0;
    assert_eq!(paginate(&flow, g), Err(PageError::InvalidGeometry));
    let mut g = geometry(4);
    g.min_lines = 0;
    assert_eq!(paginate(&flow, g), Err(PageError::InvalidMinLines));
}

#[test]
fn splits_long_paragraph_across_pages() {
    let flow = layout_flow(&para_lines(10), &MEASURE, opts(10.0)).unwrap();
    let pages = paginate(&flow, geometry(4)).unwrap();
    assert_eq!(page_line_counts(&pages), vec![4, 4, 2]);
    assert_eq!(pages[2].number, 3);
}

#[test]
fn widow_control_moves_lines_to_keep_two_on_next_page() {
    // 5 lines, 4 fit: a plain split would leave a single widow line on page 2.
    let flow = layout_flow(&para_lines(5), &MEASURE, opts(10.0)).unwrap();
    let pages = paginate(&flow, geometry(4)).unwrap();
    assert_eq!(page_line_counts(&pages), vec![3, 2]);
}

#[test]
fn orphan_control_pushes_paragraph_when_only_one_line_fits() {
    let doc = doc_of(vec![
        plain("aaaaaaaaaa\nbbbbbbbbbb\ncccccccccc"),
        plain("dddddddddd\neeeeeeeeee\nffffffffff\ngggggggggg\nhhhhhhhhhh\niiiiiiiiii"),
    ]);
    let flow = layout_flow(&doc, &MEASURE, opts(10.0)).unwrap();
    // Page holds 4 lines. Paragraph A is 3 lines, leaving 1 slot: only one line of B would fit,
    // which would be an orphan, so B starts on page 2.
    let pages = paginate(&flow, geometry(4)).unwrap();
    let first_page_blocks: Vec<u64> = pages[0].items.iter().map(|i| i.block.0).collect();
    assert_eq!(first_page_blocks, vec![1, 1, 1]);
    assert_eq!(pages[1].items[0].block, BlockId(2));
}

#[test]
fn pagination_keeps_order_and_content() {
    let doc = doc_of(vec![
        plain("one two three"),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
        plain("four"),
    ]);
    let flow = layout_flow(&doc, &MEASURE, opts(5.0)).unwrap();
    let pages = paginate(&flow, geometry(2)).unwrap();
    let flat: Vec<_> = pages
        .iter()
        .flat_map(|p| p.items.iter().map(|i| (i.block, i.kind.clone())))
        .collect();
    let expected: Vec<_> = flow
        .items
        .iter()
        .map(|i| (i.block, i.kind.clone()))
        .collect();
    assert_eq!(flat, expected);
}

#[test]
fn oversized_box_gets_its_own_page() {
    let doc = doc_of(vec![Block::Grid {
        sheet_id: SheetId(1),
        view_bounds: None,
    }]);
    let flow = layout_flow(&doc, &MEASURE, opts(10.0)).unwrap();
    let pages = paginate(&flow, geometry(2)).unwrap(); // 20pt page, 50pt box
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].items.len(), 1);
}

#[test]
fn empty_document_gives_one_empty_page() {
    let flow = layout_flow(&doc_of(vec![]), &MEASURE, opts(10.0)).unwrap();
    let pages = paginate(&flow, geometry(4)).unwrap();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].items.is_empty());
}

#[test]
fn layout_is_deterministic() {
    let doc = para_lines(7);
    let a = layout_flow(&doc, &MEASURE, opts(10.0)).unwrap();
    let b = layout_flow(&doc, &MEASURE, opts(10.0)).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        paginate(&a, geometry(3)).unwrap(),
        paginate(&b, geometry(3)).unwrap()
    );
}

// Randomized invariants over generated documents.

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

#[test]
fn random_pagination_respects_rules() {
    for seed in 0..300u64 {
        let mut rng = Rng(seed);
        let n_blocks = 1 + rng.below(6);
        let children: Vec<Block> = (0..n_blocks)
            .map(|_| {
                if rng.below(5) == 0 {
                    Block::Grid {
                        sheet_id: SheetId(1),
                        view_bounds: None,
                    }
                } else {
                    let lines = 1 + rng.below(9);
                    let words: Vec<String> = (0..lines).map(|i| format!("{:09}x", i)).collect();
                    plain(&words.join("\n"))
                }
            })
            .collect();
        let doc = doc_of(children);
        let flow = layout_flow(&doc, &MEASURE, opts(10.0)).unwrap();

        let lines_per_page = 4 + rng.below(6); // at least 4 lines, so middle fragments are big enough
        let g = PageGeometry {
            height: lines_per_page as f64 * 10.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            min_lines: 2,
        };
        let pages = paginate(&flow, g).unwrap();

        // 1. Nothing lost or reordered.
        let flat: Vec<_> = pages
            .iter()
            .flat_map(|p| p.items.iter().map(|i| (i.block, i.kind.clone())))
            .collect();
        let expected: Vec<_> = flow
            .items
            .iter()
            .map(|i| (i.block, i.kind.clone()))
            .collect();
        assert_eq!(flat, expected, "seed {seed}");

        // 2. A page with several items never overflows. A single oversized box may stand alone.
        for page in &pages {
            if page.items.len() > 1 {
                let used = page
                    .items
                    .iter()
                    .map(|i| i.y + i.height)
                    .fold(0.0, f64::max);
                assert!(
                    used <= g.height + 1e-9,
                    "seed {seed}: page {} overflows",
                    page.number
                );
            }
        }

        // 3. A paragraph split across pages keeps at least min_lines on each side,
        //    unless the paragraph is too short to split that way.
        for block in doc.blocks.keys() {
            let fragments: Vec<usize> = pages
                .iter()
                .map(|p| {
                    p.items
                        .iter()
                        .filter(|i| i.block == *block && matches!(i.kind, FlowKind::Line { .. }))
                        .count()
                })
                .filter(|&c| c > 0)
                .collect();
            let total: usize = fragments.iter().sum();
            if fragments.len() > 1 && total >= 4 {
                for &f in &fragments {
                    assert!(
                        f >= 2,
                        "seed {seed}: block {block:?} fragments {fragments:?}"
                    );
                }
            }
        }
    }
}
