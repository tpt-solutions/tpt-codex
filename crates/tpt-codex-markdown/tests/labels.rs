// SPDX-License-Identifier: MIT OR Apache-2.0
//! Equation and table labels: exact round trips, reader errors with lines, and writer refusals.

use tpt_codex_core::{Block, BlockId, SheetId};
use tpt_codex_markdown::{from_markdown, to_markdown};

const EQUATION_BLOCK: &str = "$$\na^2 + b^2\n$$ {#eq:pythagoras}";
const EQUATION_LINE: &str = "$$ x^2 $$ {#eq:square}";
const LABELLED_GRID: &str = "::: grid { id=\"1\" label=\"tab:results\" }\n| Item | Qty |\n| --- | --- |\n| Bolts | 4 |\n:::";

fn read_err(src: &str) -> (usize, String) {
    let err = from_markdown(src).expect_err("should be an error");
    (err.line, err.message)
}

#[test]
fn a_multi_line_equation_label_round_trips_exactly() {
    let doc = from_markdown(EQUATION_BLOCK).unwrap();
    assert_eq!(
        doc.equation_labels.get(&BlockId(1)).map(String::as_str),
        Some("eq:pythagoras")
    );
    assert_eq!(
        to_markdown(&doc).unwrap(),
        format!(
            "{EQUATION_BLOCK}
"
        )
    );
}

#[test]
fn a_single_line_equation_label_round_trips_exactly() {
    let doc = from_markdown(EQUATION_LINE).unwrap();
    assert_eq!(
        doc.equation_labels.get(&BlockId(1)).map(String::as_str),
        Some("eq:square")
    );
    let written = to_markdown(&doc).unwrap();
    // The writer always uses the multi-line form; it must read back the same block.
    let again = from_markdown(&written).unwrap();
    assert_eq!(again, doc);
}

#[test]
fn an_unlabelled_equation_is_unchanged() {
    let src = "$$\na^2\n$$";
    let doc = from_markdown(src).unwrap();
    assert!(doc.equation_labels.is_empty());
    assert_eq!(
        to_markdown(&doc).unwrap(),
        format!(
            "{src}
"
        )
    );
}

#[test]
fn a_labelled_grid_round_trips_exactly() {
    let doc = from_markdown(LABELLED_GRID).unwrap();
    assert_eq!(
        doc.table_labels.get(&SheetId(1)).map(String::as_str),
        Some("tab:results")
    );
    assert_eq!(
        to_markdown(&doc).unwrap(),
        format!(
            "{LABELLED_GRID}
"
        )
    );
}

#[test]
fn an_equation_label_must_start_with_eq() {
    let (line, message) = read_err("$$\nx\n$$ {#fig:a}");
    assert_eq!(line, 3);
    assert!(message.contains("eq:"), "{message}");
}

#[test]
fn an_equation_attribute_must_be_braced_and_a_single_label() {
    let (line, _) = read_err("$$ x $$ #eq:a");
    assert_eq!(line, 1);
    let (_, message) = read_err("$$ x $$ {#eq:a #eq:b}");
    assert!(message.contains("only one label"), "{message}");
}

#[test]
fn a_table_label_must_start_with_tab() {
    let (line, message) =
        read_err("::: grid { id=\"1\" label=\"fig:a\" }\n| A |\n| --- |\n| 1 |\n:::");
    assert_eq!(line, 1);
    assert!(message.contains("tab:"), "{message}");
}

#[test]
fn a_table_label_on_a_grid_with_an_unknown_attribute_is_rejected() {
    let (_, message) = read_err("::: grid { id=\"1\" name=\"x\" }\n| A |\n| --- |\n| 1 |\n:::");
    assert!(message.contains("id, label and names"), "{message}");
}

#[test]
fn the_writer_refuses_an_equation_label_on_a_block_that_is_not_math() {
    let mut doc = from_markdown("Some text.").unwrap();
    doc.equation_labels.insert(BlockId(1), "eq:text".into());
    let err = to_markdown(&doc).expect_err("should refuse");
    assert!(err.reason.contains("display math"), "{}", err.reason);
}

#[test]
fn the_writer_refuses_a_table_label_without_the_tab_prefix() {
    let mut doc = from_markdown(LABELLED_GRID).unwrap();
    doc.table_labels.insert(SheetId(1), "fig:results".into());
    let err = to_markdown(&doc).expect_err("should refuse");
    assert!(err.reason.contains("tab:"), "{}", err.reason);
}

#[test]
fn a_block_kind_stays_unchanged_by_labels() {
    let doc = from_markdown(EQUATION_BLOCK).unwrap();
    assert!(matches!(doc.get(BlockId(1)), Some(Block::Math { .. })));
}
