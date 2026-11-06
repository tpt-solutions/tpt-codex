// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_appfront_html::render;
use tpt_codex_app::query_form::DraftChange;
use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Query};
use tpt_typeset::entity_type;
use tpt_typeset::query_builder::{Condition, QuerySpec, build_source};

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

/// Sends one change per control for block 1: the paragraph source, the field, the test, the value,
/// the sort, the direction and the limit.
fn fill(editor: &mut Editor, field: &str, value: &str, sort: &str, descending: bool, limit: &str) {
    let changes = [
        DraftChange::Source(entity_type::PARAGRAPH.to_string()),
        DraftChange::Field(field.into()),
        DraftChange::Test("equals".into()),
        DraftChange::Value(value.into()),
        DraftChange::Sort(sort.into()),
        DraftChange::Direction(if descending { "desc" } else { "asc" }.into()),
        DraftChange::Limit(limit.into()),
    ];
    for change in changes {
        editor.update(Msg::SetQueryDraft {
            block: BlockId(1),
            change,
        });
    }
}

#[test]
fn the_view_shows_every_form_control_with_its_label() {
    let editor = Editor::new(document());
    let view = render(&editor.view());
    // Source type, field, test, value, sort field, sort direction and limit.
    assert_eq!(view.matches("query-form-control").count(), 7, "{view}");
    assert!(
        view.contains("not equals"),
        "the test select offers not-equals"
    );
    assert!(view.contains("no sort"), "the sort select offers no sort");
    assert!(view.contains("Apply query"));
}

#[test]
fn changing_the_form_changes_the_draft_but_not_the_document_or_undo_history() {
    let mut editor = Editor::new(document());
    let before = source(&editor);
    fill(&mut editor, "content", "Hello", "", false, "");
    assert_eq!(source(&editor), before, "the query is not applied yet");
    assert_eq!(editor.status(), None);
    // The form shows the typed value.
    assert!(render(&editor.view()).contains("Hello"));
    // Nothing to undo: a draft is not a document change.
    editor.update(Msg::Undo);
    assert_eq!(source(&editor), before);
}

#[test]
fn apply_stores_the_query_the_form_describes() {
    let mut editor = Editor::new(document());
    fill(&mut editor, "kind", "paragraph", "content", true, "3");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert_eq!(editor.status(), None);

    let expected = QuerySpec::new(entity_type::PARAGRAPH)
        .with(Condition::Equals {
            field: "kind".into(),
            value: "paragraph".into(),
        })
        .sorted_by("content", true)
        .limited_to(3);
    assert_eq!(source(&editor), build_source(&expected));
}

#[test]
fn apply_is_one_undoable_step() {
    let mut editor = Editor::new(document());
    let before = source(&editor);
    fill(&mut editor, "kind", "paragraph", "", false, "");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert_ne!(source(&editor), before);
    editor.update(Msg::Undo);
    assert_eq!(source(&editor), before);
}

#[test]
fn a_limit_that_is_not_a_number_is_reported_and_the_query_is_unchanged() {
    let mut editor = Editor::new(document());
    let before = source(&editor);
    fill(&mut editor, "kind", "paragraph", "", false, "lots");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert!(editor.status().unwrap().contains("whole number"));
    assert_eq!(source(&editor), before);
}

#[test]
fn a_field_outside_the_form_is_reported_by_the_editor() {
    let mut editor = Editor::new(document());
    let before = source(&editor);
    fill(&mut editor, "secret", "x", "", false, "");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert!(editor.status().unwrap().contains("cannot be filtered on"));
    assert_eq!(source(&editor), before);
}

/// Sends the test and value controls for block 1.
fn fill_test(editor: &mut Editor, test: &str, value: &str) {
    for change in [
        DraftChange::Field("kind".into()),
        DraftChange::Test(test.into()),
        DraftChange::Value(value.into()),
    ] {
        editor.update(Msg::SetQueryDraft {
            block: BlockId(1),
            change,
        });
    }
}

#[test]
fn one_of_stores_an_in_condition_from_the_comma_separated_values() {
    let mut editor = Editor::new(document());
    fill_test(&mut editor, "one-of", "paragraph, heading");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert_eq!(editor.status(), None);

    let expected = QuerySpec::new(entity_type::PARAGRAPH).with(Condition::OneOf {
        field: "kind".into(),
        values: vec!["paragraph".into(), "heading".into()],
    });
    assert_eq!(source(&editor), build_source(&expected));
}

#[test]
fn one_of_with_no_values_is_reported_and_the_query_is_unchanged() {
    let mut editor = Editor::new(document());
    let before = source(&editor);
    fill_test(&mut editor, "one-of", " , ");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    assert!(editor.status().unwrap().contains("at least one value"));
    assert_eq!(source(&editor), before);
}

#[test]
fn one_of_shows_the_matching_rows_in_the_view() {
    let mut editor = Editor::new(document());
    fill_test(&mut editor, "one-of", "paragraph, heading");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    // Result rows carry the `query-row` class; the document's own paragraph does not.
    assert!(
        render(&editor.view()).contains("query-row"),
        "a paragraph is in the list"
    );

    fill_test(&mut editor, "one-of", "heading, code");
    editor.update(Msg::ApplyQueryDraft { block: BlockId(1) });
    let page = render(&editor.view());
    assert!(!page.contains("query-row"), "no paragraph is in the list");
    assert!(page.contains("no matching blocks"), "{page}");
}

#[test]
fn a_cached_view_after_a_draft_change_matches_a_full_render() {
    let mut editor = Editor::new(document());
    fill(&mut editor, "content", "Hello", "kind", true, "2");
    let cached = format!("{:?}", editor.view());
    editor.invalidate_view();
    assert_eq!(format!("{:?}", editor.view()), cached);
}
