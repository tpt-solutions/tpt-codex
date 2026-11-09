// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Query, TextStyle};
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

fn sample() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3), BlockId(6)],
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
    doc
}

fn rows(query: &str) -> String {
    let src = document_entities(&sample());
    run_query(
        &Query {
            source: query.into(),
        },
        &src,
    )
    .unwrap()
}

#[test]
fn headings_can_be_filtered_by_level() {
    let q = format!(
        r#"{{"source":{},"filters":[{{"Eq":{{"field":"level","value":2}}}}]}}"#,
        entity_type::HEADING
    );
    assert_eq!(rows(&q), r#"[{"id":6}]"#);
}

#[test]
fn list_items_can_be_found_by_their_list() {
    let q = format!(
        r#"{{"source":{},"filters":[{{"Eq":{{"field":"list","value":3}}}}]}}"#,
        entity_type::LIST_ITEM
    );
    assert_eq!(rows(&q), r#"[{"id":4},{"id":5}]"#);
}

#[test]
fn paragraphs_are_found_by_their_entity_type() {
    // Only block 2 is a paragraph. Headings, list items and the list have their own types.
    let q = format!(r#"{{"source":{},"filters":[]}}"#, entity_type::PARAGRAPH);
    assert_eq!(rows(&q), r#"[{"id":2}]"#);
}

#[test]
fn a_bad_query_source_is_reported_as_a_parse_error() {
    let src = document_entities(&sample());
    assert!(run_query(&Query { source: "{".into() }, &src).is_err());
}

#[test]
fn a_query_result_is_bound_as_its_row_count() {
    use tpt_typeset::{query_bindings, query_name};

    let mut doc = sample();
    // Block 7 runs the list-item query from above: two rows, so "2 rows".
    let q = format!(
        r#"{{"source":{},"filters":[{{"Eq":{{"field":"list","value":3}}}}]}}"#,
        entity_type::LIST_ITEM
    );
    doc.insert(BlockId(7), Block::Query(Query { source: q }));
    // Block 8 has one row: "1 row".
    let one = format!(
        r#"{{"source":{},"filters":[{{"Eq":{{"field":"level","value":2}}}}]}}"#,
        entity_type::HEADING
    );
    doc.insert(BlockId(8), Block::Query(Query { source: one }));
    // Block 9 is not valid JSON, so it is not bound.
    doc.insert(BlockId(9), Block::Query(Query { source: "{".into() }));

    assert_eq!(query_name(BlockId(7)), "q7");
    let bindings = query_bindings(&doc);
    assert_eq!(bindings.get("q7"), Some("2 rows"));
    assert_eq!(bindings.get("q8"), Some("1 row"));
    assert_eq!(bindings.get("q9"), None);
}
