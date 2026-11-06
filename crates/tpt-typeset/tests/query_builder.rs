// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Query, TextStyle};
use tpt_typeset::query_builder::{Condition, QuerySpec, build_source};
use tpt_typeset::{document_entities, entity_type, run_query};

fn text(content: &str, level: Option<u8>) -> Block {
    Block::Text {
        content: content.into(),
        style: TextStyle {
            bold: false,
            italic: false,
            heading_level: level,
            runs: Vec::new(),
        },
    }
}

/// Headings "Intro" and "Details", paragraphs "Para" and "Notes", a list of "apples" and
/// "pears", and a code block in Rust.
fn sample() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![
                BlockId(1),
                BlockId(2),
                BlockId(3),
                BlockId(6),
                BlockId(7),
                BlockId(8),
            ],
        },
    );
    doc.insert(BlockId(1), text("Intro", Some(1)));
    doc.insert(BlockId(2), text("Para", None));
    doc.insert(
        BlockId(3),
        Block::List {
            ordered: false,
            start: 1,
            items: vec![BlockId(4), BlockId(5)],
        },
    );
    doc.insert(BlockId(4), text("apples", None));
    doc.insert(BlockId(5), text("pears", None));
    doc.insert(BlockId(6), text("Details", Some(2)));
    doc.insert(BlockId(7), text("Notes", None));
    doc.insert(
        BlockId(8),
        Block::Code {
            language: Some("rust".into()),
            code: "fn main() {}".into(),
        },
    );
    doc
}

/// Runs the spec's source over the sample and returns the text of each matching block, in the
/// order the engine returns them. Results are entity ids, and an entity id is its block id.
fn contents(spec: &QuerySpec) -> Vec<String> {
    let source = build_source(spec);
    let json = run_query(&Query { source }, &document_entities(&sample())).unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
    let doc = sample();
    rows.iter()
        .map(|row| {
            let id = row["id"].as_u64().expect("each row carries an id");
            match doc.get(BlockId(id)) {
                Some(Block::Text { content, .. }) => content.clone(),
                Some(Block::Code { code, .. }) => code.clone(),
                other => panic!("row {id} is not text: {other:?}"),
            }
        })
        .collect()
}

#[test]
fn equality_selects_the_matching_kind() {
    let spec = QuerySpec::new(entity_type::HEADING).with(Condition::Equals {
        field: "kind".into(),
        value: "heading".into(),
    });
    assert_eq!(contents(&spec), vec!["Intro", "Details"]);
}

#[test]
fn not_equals_excludes_the_value_and_keeps_the_rest() {
    let spec = QuerySpec::new(entity_type::PARAGRAPH).with(Condition::NotEquals {
        field: "content".into(),
        value: "Para".into(),
    });
    assert_eq!(contents(&spec), vec!["Notes"]);
}

#[test]
fn one_of_matches_any_listed_value() {
    let spec = QuerySpec::new(entity_type::LIST_ITEM).with(Condition::OneOf {
        field: "content".into(),
        values: vec!["pears".into(), "plums".into()],
    });
    assert_eq!(contents(&spec), vec!["pears"]);
}

#[test]
fn conditions_are_combined_with_and() {
    let spec = QuerySpec::new(entity_type::HEADING)
        .with(Condition::Equals {
            field: "kind".into(),
            value: "heading".into(),
        })
        .with(Condition::NotEquals {
            field: "content".into(),
            value: "Intro".into(),
        });
    assert_eq!(contents(&spec), vec!["Details"]);
}

#[test]
fn sort_and_limit_pick_the_first_rows_in_order() {
    let spec = QuerySpec::new(entity_type::LIST_ITEM)
        .with(Condition::Equals {
            field: "kind".into(),
            value: "list_item".into(),
        })
        .sorted_by("content", true)
        .limited_to(1);
    assert_eq!(contents(&spec), vec!["pears"]);
}

#[test]
fn an_unknown_field_matches_nothing() {
    let spec = QuerySpec::new(entity_type::PARAGRAPH).with(Condition::Equals {
        field: "no_such_field".into(),
        value: "x".into(),
    });
    assert!(contents(&spec).is_empty());
}

#[test]
fn the_source_names_the_entity_type() {
    assert!(build_source(&QuerySpec::new(3)).contains(r#""source":3"#));
}
