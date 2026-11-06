//! Data binding: `[[name]]` in a text block resolves to the value the caller supplies, and a
//! binding that is not supplied keeps its name.

use tpt_codex_core::SheetId;
use tpt_codex_markdown::{from_markdown, to_markdown};
use tpt_typeset::{Bindings, bound_text, check_bindings, grid_cell_name};

const SAMPLE: &str = "Total [[sheet1.B2]] over [[q1]].

Plain paragraph.

Missing [[nope]] here.
";

#[test]
fn each_binding_is_found_with_its_value_or_none() {
    let doc = from_markdown(SAMPLE).unwrap();
    let mut bindings = Bindings::new();
    assert_eq!(bindings.set("sheet1.B2", "42"), None);
    bindings.set("q1", "3 rows");
    assert_eq!(bindings.len(), 2);

    let checks = check_bindings(&doc, &bindings);
    let mut names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["nope", "q1", "sheet1.B2"]);

    let nope = checks.iter().find(|c| c.name == "nope").unwrap();
    assert_eq!(nope.value, None);
    let q1 = checks.iter().find(|c| c.name == "q1").unwrap();
    assert_eq!(q1.value.as_deref(), Some("3 rows"));
}

#[test]
fn bound_text_replaces_bound_names_and_keeps_unbound_ones() {
    let doc = from_markdown(SAMPLE).unwrap();
    let mut bindings = Bindings::new();
    bindings.set("sheet1.B2", "42");
    bindings.set("q1", "3 rows");

    let checks = check_bindings(&doc, &bindings);
    let total = checks.iter().find(|c| c.name == "q1").unwrap().block;
    let missing = checks.iter().find(|c| c.name == "nope").unwrap().block;

    assert_eq!(
        bound_text(&doc, total, &bindings).as_deref(),
        Some("Total 42 over 3 rows.")
    );
    assert_eq!(
        bound_text(&doc, missing, &bindings).as_deref(),
        Some("Missing nope here.")
    );
}

#[test]
fn bound_text_is_none_for_a_block_that_is_not_text() {
    let doc = from_markdown(SAMPLE).unwrap();
    assert_eq!(bound_text(&doc, doc.root, &Bindings::new()), None);
}

#[test]
fn a_grid_cell_is_bound_under_its_sheet_and_address() {
    assert_eq!(grid_cell_name(SheetId(1), "B2"), "sheet1.B2");
    let doc = from_markdown(SAMPLE).unwrap();
    let mut bindings = Bindings::new();
    bindings.set(&grid_cell_name(SheetId(1), "B2"), "42");
    let checks = check_bindings(&doc, &bindings);
    let cell = checks.iter().find(|c| c.name == "sheet1.B2").unwrap();
    assert_eq!(cell.value.as_deref(), Some("42"));
}

#[test]
fn bindings_write_back_as_double_brackets() {
    let doc = from_markdown(SAMPLE).unwrap();
    let written = to_markdown(&doc).unwrap();
    assert!(
        written.contains("Total [[sheet1.B2]] over [[q1]]."),
        "{written}"
    );
    let again = from_markdown(&written).unwrap();
    assert_eq!(to_markdown(&again).unwrap(), written);
}
