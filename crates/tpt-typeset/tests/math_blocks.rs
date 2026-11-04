// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_typeset::{check_math_blocks, parse_math};

fn doc_with(blocks: &[(u64, Block)]) -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: blocks.iter().map(|(id, _)| BlockId(*id)).collect(),
        },
    );
    for (id, block) in blocks {
        doc.insert(BlockId(*id), block.clone());
    }
    doc
}

fn math(source: &str) -> Block {
    Block::Math {
        source: source.into(),
    }
}

/// A source that the parser rejects, taken from the parser's own error tests.
fn bad_source() -> String {
    let src = "x^";
    assert!(parse_math(src).is_err(), "test input must be rejected");
    src.to_string()
}

#[test]
fn valid_math_blocks_produce_no_errors() {
    let doc = doc_with(&[(1, math(r"\frac{a}{b}")), (2, math("x_1 + y"))]);
    assert!(check_math_blocks(&doc).is_empty());
}

#[test]
fn failing_math_blocks_are_reported_with_their_id_and_position() {
    let bad = bad_source();
    let doc = doc_with(&[
        (1, math("x + 1")),
        (
            2,
            Block::Text {
                content: bad.clone(),
                style: TextStyle::default(),
            },
        ),
        (3, math(&bad)),
    ]);
    let errors = check_math_blocks(&doc);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].block, BlockId(3));
    assert_eq!(errors[0].error, parse_math(&bad).unwrap_err());
}

#[test]
fn unreferenced_math_blocks_are_checked_too() {
    let bad = bad_source();
    let mut doc = doc_with(&[(1, math("x"))]);
    doc.insert(BlockId(9), math(&bad));
    let errors = check_math_blocks(&doc);
    assert_eq!(
        errors.iter().map(|e| e.block).collect::<Vec<_>>(),
        vec![BlockId(9)]
    );
}
