// SPDX-License-Identifier: MIT OR Apache-2.0
//! Structured errors: bibliography, document class and query errors carry a position or a hint.

use tpt_typeset::{DocumentClass, Library};

#[test]
fn a_json_syntax_error_has_a_position_and_a_hint() {
    let src = "[\n  {\"id\": \"a\"},\n  {\"id\": ,}\n]";
    let error = Library::from_csl_json(src).unwrap_err();
    assert!(error.line >= 1 && error.column >= 1);
    assert!(!error.message.is_empty());
    assert!(!error.hint.is_empty());
    assert!(error.to_string().contains("line 3"), "{error}");
}

#[test]
fn a_top_level_value_that_is_not_an_array_is_at_the_start() {
    let error = Library::from_csl_json("{\"id\": \"a\"}").unwrap_err();
    assert_eq!((error.line, error.column), (1, 1));
    assert_eq!(error.message, "expected a JSON array of entries");
}

#[test]
fn an_entry_without_an_id_points_at_that_entry() {
    let src = "[\n  {\"id\": \"a\"},\n  {\"title\": \"no id\"}\n]";
    let error = Library::from_csl_json(src).unwrap_err();
    assert_eq!((error.line, error.column), (3, 3));
    assert_eq!(error.message, "an entry has no id");
    assert!(error.hint.contains("id"));
}

#[test]
fn a_duplicate_id_points_at_the_second_entry() {
    let src = "[\n  {\"id\": \"a\"},\n  {\"id\": \"a\"}\n]";
    let error = Library::from_csl_json(src).unwrap_err();
    assert_eq!((error.line, error.column), (3, 3));
    assert_eq!(error.message, "two entries share an id");
}

#[test]
fn braces_inside_strings_do_not_move_the_entry_position() {
    let src = "[{\"id\": \"a {x} [y]\", \"title\": \"[\"}, {\"title\": \"t\"}]";
    let error = Library::from_csl_json(src).unwrap_err();
    let expected_column = src.find("{\"title\": \"t\"").unwrap() + 1;
    assert_eq!((error.line, error.column), (1, expected_column));
    assert_eq!(error.message, "an entry has no id");
}

#[test]
fn an_unknown_class_names_the_value_and_gives_a_hint() {
    let error = DocumentClass::parse("novel").unwrap_err();
    assert_eq!(error.name, "novel");
    assert!(error.hint.contains("article"));
    assert!(error.to_string().contains("novel"));
}
