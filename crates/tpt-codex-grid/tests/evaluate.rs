// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{CellInput, Sheet};
use tpt_codex_grid::{CellResult, evaluate_sheet};

fn sheet(cells: &[(&str, CellInput)]) -> Sheet {
    Sheet {
        cells: cells
            .iter()
            .map(|(a, c)| (a.to_string(), c.clone()))
            .collect(),
    }
}

fn num(v: f64) -> CellInput {
    CellInput::Number { value: v }
}

fn formula(src: &str) -> CellInput {
    CellInput::Formula { source: src.into() }
}

#[test]
fn formulas_follow_their_inputs() {
    let results = evaluate_sheet(&sheet(&[
        ("A1", num(21.0)),
        ("B1", formula("=A1 * 2")),
        ("C1", formula("=B1 + A1")),
    ]));
    assert_eq!(results["A1"], CellResult::Number(21.0));
    assert_eq!(results["B1"], CellResult::Number(42.0));
    assert_eq!(results["C1"], CellResult::Number(63.0));
}

#[test]
fn text_and_booleans_are_passed_through() {
    let results = evaluate_sheet(&sheet(&[
        ("A1", CellInput::Text { value: "hi".into() }),
        ("A2", CellInput::Boolean { value: true }),
    ]));
    assert_eq!(results["A1"], CellResult::Text("hi".into()));
    assert_eq!(results["A2"], CellResult::Boolean(true));
}

#[test]
fn a_bad_formula_is_an_error_in_its_own_cell_only() {
    let results = evaluate_sheet(&sheet(&[
        ("A1", num(1.0)),
        ("B1", formula("=(((")),
        ("C1", formula("=A1 + 1")),
    ]));
    assert!(matches!(results["B1"], CellResult::Error(_)));
    assert_eq!(results["C1"], CellResult::Number(2.0));
}

#[test]
fn cycles_are_errors_and_do_not_stop_other_cells() {
    let results = evaluate_sheet(&sheet(&[
        ("A1", formula("=B1")),
        ("B1", formula("=A1")),
        ("C1", num(3.0)),
    ]));
    assert!(matches!(results["A1"], CellResult::Error(_)));
    assert!(matches!(results["B1"], CellResult::Error(_)));
    assert_eq!(results["C1"], CellResult::Number(3.0));
}

#[test]
fn division_by_zero_is_an_error_value() {
    let results = evaluate_sheet(&sheet(&[("A1", formula("=1/0"))]));
    assert!(matches!(results["A1"], CellResult::Error(_)));
}

#[test]
fn an_empty_sheet_has_no_results() {
    assert!(evaluate_sheet(&Sheet::default()).is_empty());
}

#[test]
fn session_edits_propagate_to_dependents() {
    use tpt_codex_grid::GridSession;
    let base = sheet(&[("A1", num(2.0)), ("B1", formula("=A1 * 2"))]);
    let mut session = GridSession::new(&base);
    assert_eq!(session.results()["B1"], CellResult::Number(4.0));

    session.set("A1", Some(num(5.0))).unwrap();
    assert_eq!(session.results()["B1"], CellResult::Number(10.0));

    session.set("C1", Some(formula("=B1 + 1"))).unwrap();
    assert_eq!(session.results()["C1"], CellResult::Number(11.0));

    // Fixing a formula that did not parse makes it work.
    session.set("D1", Some(formula("=((("))).unwrap();
    assert!(matches!(session.results()["D1"], CellResult::Error(_)));
    session.set("D1", Some(formula("=A1 + 0"))).unwrap();
    assert_eq!(session.results()["D1"], CellResult::Number(5.0));

    // Clearing a cell removes it from the results.
    session.set("D1", None).unwrap();
    assert!(!session.results().contains_key("D1"));
}

#[test]
fn session_rejects_bad_addresses_without_changing_anything() {
    use tpt_codex_grid::{GridError, GridSession};
    let mut session = GridSession::new(&sheet(&[("A1", num(1.0))]));
    assert_eq!(
        session.set("a1", Some(num(9.0))),
        Err(GridError::InvalidAddress("a1".into()))
    );
    assert_eq!(session.results()["A1"], CellResult::Number(1.0));
}

#[test]
fn session_matches_one_shot_evaluation() {
    use tpt_codex_grid::GridSession;
    let s = sheet(&[
        ("A1", num(3.0)),
        ("A2", formula("=A1 * A1")),
        ("A3", formula("=A2 - A1")),
    ]);
    assert_eq!(GridSession::new(&s).results(), evaluate_sheet(&s));
}
