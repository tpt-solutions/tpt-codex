// SPDX-License-Identifier: MIT OR Apache-2.0
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

#[test]
fn resolving_replaces_each_bound_name_and_moves_the_other_runs() {
    use tpt_codex_core::{Block, BlockId};
    use tpt_typeset::resolve_bindings;

    let doc = from_markdown("A **bold** [[x]] then *tail* and [[y]] end.\n").unwrap();
    let mut bindings = Bindings::new();
    bindings.set("x", "12345");
    bindings.set("y", "7");
    let resolved = resolve_bindings(&doc, &bindings).expect("two bindings have values");

    let Some(Block::Text { content, style }) = resolved.get(BlockId(1)) else {
        panic!("not text");
    };
    assert_eq!(content, "A bold 12345 then tail and 7 end.");
    let spans: Vec<(String, bool, bool)> = style
        .runs
        .iter()
        .map(|r| {
            let text: String = content
                .chars()
                .skip(r.start)
                .take(r.end - r.start)
                .collect();
            (text, r.bold, r.italic)
        })
        .collect();
    assert_eq!(
        spans,
        [
            ("bold".to_string(), true, false),
            ("tail".to_string(), false, true)
        ],
        "the binding runs are gone and the emphasis still covers its words"
    );
    // The original is untouched, and other blocks keep their ids.
    assert!(
        matches!(doc.get(BlockId(1)), Some(Block::Text { content, .. }) if content.contains("[[")
        || content.contains("x"))
    );
    assert_eq!(resolved.root, doc.root);
}

#[test]
fn resolving_with_nothing_to_replace_returns_none() {
    use tpt_typeset::resolve_bindings;
    let doc = from_markdown("No bindings here.\n\nMissing [[nope]].\n").unwrap();
    assert!(resolve_bindings(&doc, &Bindings::new()).is_none());
    let mut other = Bindings::new();
    other.set("different", "1");
    assert!(resolve_bindings(&doc, &other).is_none());
}

#[test]
fn an_unbound_name_keeps_its_text_when_others_are_resolved() {
    use tpt_codex_core::{Block, BlockId};
    use tpt_typeset::resolve_bindings;
    let doc = from_markdown("[[a]] and [[b]]\n").unwrap();
    let mut bindings = Bindings::new();
    bindings.set("b", "two");
    let resolved = resolve_bindings(&doc, &bindings).unwrap();
    let Some(Block::Text { content, style }) = resolved.get(BlockId(1)) else {
        panic!();
    };
    assert_eq!(content, "a and two");
    assert_eq!(style.runs.len(), 1, "[[a]] is still a binding run");
    assert_eq!((style.runs[0].start, style.runs[0].end), (0, 1));
}
