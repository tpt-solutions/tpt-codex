// SPDX-License-Identifier: MIT OR Apache-2.0
//! Equation and table labels: prefixes, validation, JSON, and merging.

use tpt_codex_core::{
    Block, BlockId, Document, LayoutMode, Policy, Sheet, SheetId, TextStyle, ValidationError,
    is_equation_label, is_table_label, three_way_merge,
};

fn container(children: Vec<BlockId>) -> Block {
    Block::Container {
        layout_mode: LayoutMode::Flow,
        children,
    }
}

fn math(source: &str) -> Block {
    Block::Math {
        source: source.into(),
    }
}

/// Root holding a paragraph (1), two equations (2, 3) and a grid (4) of sheet 1.
fn document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        container(vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)]),
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "text".into(),
            style: TextStyle::default(),
        },
    );
    doc.insert(BlockId(2), math("a^2"));
    doc.insert(BlockId(3), math("b^2"));
    doc.insert(
        BlockId(4),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.insert_sheet(SheetId(1), Sheet::with_size(2, 2));
    doc
}

#[test]
fn labels_need_their_prefix_and_a_name() {
    assert!(is_equation_label("eq:euler"));
    assert!(is_table_label("tab:results-2026"));
    assert!(!is_equation_label("fig:euler"));
    assert!(!is_equation_label("eq:"));
    assert!(!is_equation_label("eq:bad label"));
    assert!(!is_table_label("eq:results"));
    assert!(!is_table_label("tab:"));
}

#[test]
fn a_valid_document_with_labels_passes_validation() {
    let mut doc = document();
    doc.equation_labels.insert(BlockId(2), "eq:a".into());
    doc.equation_labels.insert(BlockId(3), "eq:b".into());
    doc.table_labels.insert(SheetId(1), "tab:first".into());
    doc.validate().unwrap();
}

#[test]
fn an_equation_label_must_name_a_math_block() {
    let mut doc = document();
    doc.equation_labels.insert(BlockId(1), "eq:text".into());
    assert_eq!(
        doc.validate(),
        Err(ValidationError::EquationLabelOfNonMath(BlockId(1)))
    );
}

#[test]
fn equation_labels_need_the_right_prefix_and_no_repeats() {
    let mut doc = document();
    doc.equation_labels.insert(BlockId(2), "fig:a".into());
    assert_eq!(
        doc.validate(),
        Err(ValidationError::InvalidEquationLabel(BlockId(2)))
    );

    let mut doc = document();
    doc.equation_labels.insert(BlockId(2), "eq:same".into());
    doc.equation_labels.insert(BlockId(3), "eq:same".into());
    assert_eq!(
        doc.validate(),
        Err(ValidationError::InvalidEquationLabel(BlockId(3)))
    );
}

#[test]
fn a_table_label_needs_an_existing_sheet_and_the_right_prefix() {
    let mut doc = document();
    doc.table_labels.insert(SheetId(9), "tab:gone".into());
    assert_eq!(
        doc.validate(),
        Err(ValidationError::TableLabelOfMissingSheet(SheetId(9)))
    );

    let mut doc = document();
    doc.table_labels.insert(SheetId(1), "eq:table".into());
    assert_eq!(
        doc.validate(),
        Err(ValidationError::InvalidTableLabel(SheetId(1)))
    );
}

#[test]
fn documents_without_labels_keep_their_json() {
    let doc = document();
    let json = serde_json::to_string(&doc).unwrap();
    assert!(!json.contains("equation_labels"), "{json}");
    assert!(!json.contains("table_labels"), "{json}");
}

#[test]
fn labels_survive_a_json_round_trip() {
    let mut doc = document();
    doc.equation_labels.insert(BlockId(2), "eq:a".into());
    doc.table_labels.insert(SheetId(1), "tab:first".into());
    let json = serde_json::to_string(&doc).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(back, doc);
    assert_eq!(
        back.equation_labels.get(&BlockId(2)).map(String::as_str),
        Some("eq:a")
    );
}

#[test]
fn a_label_added_on_one_side_survives_the_merge() {
    let base = document();
    let mut ours = base.clone();
    ours.equation_labels.insert(BlockId(2), "eq:a".into());
    let mut theirs = base.clone();
    theirs.table_labels.insert(SheetId(1), "tab:first".into());

    let merged = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert!(merged.conflicts.is_empty());
    assert_eq!(
        merged
            .document
            .equation_labels
            .get(&BlockId(2))
            .map(String::as_str),
        Some("eq:a")
    );
    assert_eq!(
        merged
            .document
            .table_labels
            .get(&SheetId(1))
            .map(String::as_str),
        Some("tab:first")
    );
    merged.document.validate().unwrap();
}
