// SPDX-License-Identifier: MIT OR Apache-2.0
//! Excel-style ranges (`B2:B9`) in formulas.

use tpt_codex_core::{CellInput, Sheet};
use tpt_codex_grid::{CellResult, evaluate_sheet, translate_ranges};

#[test]
fn ranges_are_rewritten_to_the_evaluators_syntax() {
    for (given, expected) in [
        ("=SUM(B2:B9)", "=SUM(RANGE(B2, B9))"),
        ("=sum(b2:c3)", "=sum(RANGE(B2, C3))"),
        ("=SUM($B$2:$B$9)", "=SUM(RANGE(B2, B9))"),
        (
            "=SUM(A1:A2) + SUM(B1:B2)",
            "=SUM(RANGE(A1, A2)) + SUM(RANGE(B1, B2))",
        ),
        ("=A1+B2", "=A1+B2"),
        ("=SUM(A1, A2)", "=SUM(A1, A2)"),
        ("=\"A1:B2 stays\"", "=\"A1:B2 stays\""),
        ("=IF(A1, \"x:y\", B1:B2)", "=IF(A1, \"x:y\", RANGE(B1, B2))"),
        ("=XFD1:XFD2", "=RANGE(XFD1, XFD2)"),
        ("=ABCD1:ABCD2", "=ABCD1:ABCD2"),
        ("=FOO(1):BAR2", "=FOO(1):BAR2"),
        ("=A1:", "=A1:"),
        ("", ""),
    ] {
        assert_eq!(translate_ranges(given), expected, "{given}");
    }
}

#[test]
fn sum_over_a_range_evaluates() {
    let mut sheet = Sheet::default();
    for (address, value) in [("B2", 1200.0), ("B3", 3400.0)] {
        sheet
            .cells
            .insert(address.into(), CellInput::Number { value });
    }
    for (address, source) in [
        ("B4", "=SUM(B2:B3)"),
        ("B5", "=SUM($B$2:$B$3)"),
        ("B6", "=AVERAGE(B2:B3)"),
    ] {
        sheet.cells.insert(
            address.into(),
            CellInput::Formula {
                source: source.into(),
            },
        );
    }
    let results = evaluate_sheet(&sheet);
    assert_eq!(results["B4"], CellResult::Number(4600.0));
    assert_eq!(results["B5"], CellResult::Number(4600.0));
    assert_eq!(results["B6"], CellResult::Number(2300.0));
}
