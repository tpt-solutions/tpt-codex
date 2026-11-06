// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{BlockId, SheetId};
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{LabelTarget, check_label_references, check_references, label_entries};

const SRC: &str = "# Intro\n\n\
    $$\na^2\n$$ {#eq:first}\n\n\
    See [the second](#eq:second), [the results](#tab:results) and [nothing](#eq:missing).\n\n\
    $$ b^2 $$ {#eq:second}\n\n\
    ::: grid { id=\"1\" label=\"tab:results\" }\n| A |\n| --- |\n| 1 |\n:::\n\n\
    ::: grid { id=\"2\" }\n| B |\n| --- |\n| 2 |\n:::\n\n\
    ::: grid { id=\"3\" label=\"tab:other\" }\n| C |\n| --- |\n| 3 |\n:::\n";

#[test]
fn equations_and_tables_are_numbered_separately_in_reading_order() {
    let doc = from_markdown(SRC).unwrap();
    let entries = label_entries(&doc);
    let labels: Vec<(&str, u32)> = entries
        .iter()
        .map(|e| (e.label.as_str(), e.number))
        .collect();
    assert_eq!(
        labels,
        [
            ("eq:first", 1),
            ("eq:second", 2),
            ("tab:results", 1),
            ("tab:other", 2),
        ]
    );
    assert_eq!(entries[0].target, LabelTarget::Equation(BlockId(2)));
    assert_eq!(entries[2].target, LabelTarget::Table(SheetId(1)));
}

#[test]
fn unlabelled_grids_are_not_numbered() {
    let doc = from_markdown(SRC).unwrap();
    // The grid with id 2 has no label, so it is not among the entries.
    assert!(
        label_entries(&doc)
            .iter()
            .all(|e| e.target != LabelTarget::Table(SheetId(2)))
    );
}

#[test]
fn label_references_resolve_to_numbers_and_missing_ones_are_reported() {
    let doc = from_markdown(SRC).unwrap();
    let refs = check_label_references(&doc);
    let found: Vec<(&str, Option<u32>)> =
        refs.iter().map(|r| (r.label.as_str(), r.number)).collect();
    assert_eq!(
        found,
        [
            ("eq:second", Some(2)),
            ("tab:results", Some(1)),
            ("eq:missing", None),
        ]
    );
}

#[test]
fn heading_references_do_not_report_equation_or_table_links() {
    let doc = from_markdown(SRC).unwrap();
    assert!(check_references(&doc).is_empty());
}
