// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::Query;
use tpt_query::{Entity, EntityId, EntityTypeId, MemorySource};
use tpt_typeset::{QueryRunError, run_query};

fn source() -> MemorySource {
    let mut src = MemorySource::new();
    src.insert(Entity::new(EntityId(1), EntityTypeId(1)).with_field("material", "Steel"));
    src.insert(Entity::new(EntityId(2), EntityTypeId(1)).with_field("material", "Titanium"));
    src
}

#[test]
fn json_query_runs_against_the_host_source() {
    let block = Query {
        source: r#"{"source":1,"filters":[{"Eq":{"field":"material","value":"Steel"}}]}"#.into(),
    };
    let rows = run_query(&block, &source()).unwrap();
    assert_eq!(rows, r#"[{"id":1}]"#);
}

#[test]
fn invalid_json_is_a_parse_error_with_a_message() {
    let block = Query {
        source: "not a query".into(),
    };
    match run_query(&block, &source()) {
        Err(QueryRunError::Parse(message)) => assert!(!message.is_empty()),
        other => panic!("expected parse error, got {other:?}"),
    }
}
