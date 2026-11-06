// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Query};
use tpt_typeset::entity_type;
use tpt_typeset::query_builder::{Condition, QuerySpec};

/// Root 0 -> [1, 2]; 1 is a query block; 2 is a paragraph.
fn document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Query(Query {
            source: r#"{"source":1,"filters":[]}"#.into(),
        }),
    );
    doc.insert(
        BlockId(2),
        Block::Text {
            content: "Para".into(),
            style: Default::default(),
        },
    );
    doc
}

fn source(editor: &Editor) -> String {
    match editor.document().get(BlockId(1)).unwrap() {
        Block::Query(q) => q.source.clone(),
        other => panic!("not a query: {other:?}"),
    }
}

#[test]
fn set_query_stores_the_built_source_as_one_undoable_step() {
    let mut editor = Editor::new(document());
    let spec = QuerySpec::new(entity_type::PARAGRAPH).with(Condition::Equals {
        field: "kind".into(),
        value: "paragraph".into(),
    });
    editor.update(Msg::SetQuery {
        block: BlockId(1),
        spec,
    });
    assert!(source(&editor).contains(r#""Eq":{"field":"kind","value":"paragraph"}"#));
    assert_eq!(editor.status(), None);

    editor.update(Msg::Undo);
    assert_eq!(source(&editor), r#"{"source":1,"filters":[]}"#);
}

#[test]
fn set_query_rejects_a_field_that_cannot_be_filtered_on() {
    let mut editor = Editor::new(document());
    let spec = QuerySpec::new(entity_type::PARAGRAPH).with(Condition::Equals {
        field: "position".into(),
        value: "1".into(),
    });
    editor.update(Msg::SetQuery {
        block: BlockId(1),
        spec,
    });
    assert!(
        editor
            .status()
            .is_some_and(|s| s.contains("cannot be filtered on"))
    );
    assert_eq!(source(&editor), r#"{"source":1,"filters":[]}"#);
}

#[test]
fn set_query_rejects_a_block_that_is_not_a_query() {
    let mut editor = Editor::new(document());
    editor.update(Msg::SetQuery {
        block: BlockId(2),
        spec: QuerySpec::new(1),
    });
    assert!(editor.status().is_some_and(|s| s.contains("not a query")));
}
