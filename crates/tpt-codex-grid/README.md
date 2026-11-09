# tpt-codex-grid

Live-grid evaluation for tpt-codex. It computes the values of a sheet's cells, including
spreadsheet formulas written in the LES language of `tpt-lattice`, and it turns grid values into
text for prose, PDF and Word output.

## Status

Pre-release. Version `0.1.0-spec`, `publish = false`. The API is not stable and nothing is
published to crates.io.

## What it does

- **Evaluates a sheet.** `evaluate_sheet` computes every non-empty cell of a `tpt_codex_core::Sheet`
  and returns the results keyed by A1 address.
- **Keeps a sheet live.** `GridSession` holds a `tpt-lattice` evaluator for one sheet. `set` changes
  one cell, and `results` recomputes only the cells that depend on the change.
- **Keeps errors in their cells.** A formula that does not parse, a cycle, a bad reference or a
  division by zero shows as `CellResult::Error` in its own cell. Other cells still compute.
- **Reads Excel range syntax.** `translate_ranges` rewrites `B2:B9` to the evaluator's
  `RANGE(B2, B9)`, so `=SUM(B2:B9)` works. Dollar signs are dropped and references are
  upper-cased. Text inside double quotes is left alone.
- **Puts values in prose.** `grid_bindings` binds each cell's value under `sheet<id>.<address>`
  (for example `sheet1.B4`) and under the cell's name, if it has one. `all_bindings` adds the query
  results from `tpt-typeset`. `resolve_document` returns a copy of a document with each `[[name]]`
  replaced by its value, for output that is not live, such as a PDF or a Word file. It returns
  `None` when the document has no bindings, so nothing is copied in the common case.
- **Writes computed values.** `with_computed_cells` returns a copy where each formula cell holds
  its value as a number, boolean or text. A formula that failed holds its message as text. Word
  tables need this, because they have no formulas.
- **Formats numbers for prose.** `format_number` shows whole numbers without a decimal point, and
  other numbers to at most ten decimals with no trailing zeros, so `0.1 + 0.2` reads as `0.3`.

## What it deliberately does not do

- It does not store cells or sheets. `tpt-codex-core` stores the inputs, and this crate computes the
  results.
- It does not evaluate queries or `[[name]]` in text on its own. That is `tpt-typeset`'s job; this
  crate supplies the grid values to it.
- It does not evaluate formulas across sheets. A session holds the cells of one sheet only.

## Limitations

- **Lists.** A cell that holds a list value is reported as `CellResult::Empty`.
- **Dates.** A date is reported and shown as its serial number.
- **Addresses.** Addresses must be canonical upper-case A1 notation, as in `tpt-codex-core`. A
  lower-case or otherwise non-canonical address is rejected by `GridSession::set` with
  `GridError::InvalidAddress`, and `evaluate_sheet` reports it as an error cell.
- **Functions.** The function set is the one `tpt-lattice-evaluator` implements. A test in
  `tests/bindings.rs` checks the common Excel functions (SUM, AVERAGE, ROUND, IF, VLOOKUP, CONCAT
  and others). Functions beyond that set are errors in their cell.
- **std only.** The evaluator is `std`, so this crate is `std`, and it is not part of the `no_std`
  crates.

## Where it sits

Depends on:

- `tpt-codex-core` (the sheet, cell and document types, re-exported as `tpt_codex_grid::core`)
- `tpt-typeset` (bindings, and the query bindings it merges in)
- `tpt-lattice-core` and `tpt-lattice-evaluator` (pinned by git revision in the workspace manifest)

Used by:

- `tpt-codex-app`, `tpt-codex-cli` and `tpt-codex-files` (editing, checks, and saving)
- `tpt-codex-interop` (Word and spreadsheet import and export)
- `tpt-codex-pdf` (PDF output of bindings and grids)
- `tpt-codex-acceptance` (end-to-end tests)

The dependency pins and the evaluator's status are in
[`../../docs/tpt-dependency-map.md`](../../docs/tpt-dependency-map.md).

## Features and key API

The crate has no Cargo features. It has one example, `examples/bench_eval.rs`.

```rust,ignore
use tpt_codex_core::{CellInput, Sheet};
use tpt_codex_grid::{CellResult, GridSession, evaluate_sheet, translate_ranges};

// One-shot evaluation of a whole sheet.
let results = evaluate_sheet(&sheet);
if let Some(CellResult::Number(total)) = results.get("B10") {
    println!("total: {total}");
}

// A live session: change one cell and recompute only what depends on it.
let mut session = GridSession::new(&sheet);
session.set("B2", Some(CellInput::Number { value: 5.0 }))?;
let results = session.results();

// Excel range syntax is accepted in formulas.
assert_eq!(translate_ranges("=SUM(B2:B9)"), "=SUM(RANGE(B2, B9))");
```

`set` returns `Result<(), GridError>`, and `GridError` has one variant, `InvalidAddress`. A
formula that fails to parse is not an error from `set`. It is stored, and `results` reports it in
that cell.

## no_std and WASM

Not applicable. The crate is `std` and uses `std::collections`. It has `#![forbid(unsafe_code)]`.

## Errors

- `GridError::InvalidAddress` is returned by `GridSession::set` for an address that is not
  canonical A1 notation. Nothing changes when it is returned.
- `CellResult::Error` carries a message for an error in a cell. It covers parse errors, cycles,
  division by zero and other evaluation errors. Errors do not stop evaluation of other cells.

## Determinism

Results are keyed by address in a `BTreeMap`, so output order is stable. The same sheet gives the
same results on every run. `evaluate_sheet` and a `GridSession` built from the same sheet give
the same results (a test checks this).

## Testing

```text
cargo test -p tpt-codex-grid
```

| File | What it covers |
|------|----------------|
| `tests/evaluate.rs` | formulas following their inputs, text and booleans, bad formulas, cycles, division by zero, empty sheets, session edits and their propagation, bad addresses, and agreement with one-shot evaluation |
| `tests/ranges.rs` | the rewrite of Excel ranges to the evaluator's syntax, and a `SUM` over a range |
| `tests/bindings.rs` | formulas in prose, edits reflected in text, documents without bindings, empty cells and errors not bound, number formatting, named cells, and common Excel functions |

A benchmark measures full and incremental evaluation:

```text
cargo run --release -p tpt-codex-grid --example bench_eval -- 1000
```

The argument is the number of rows. Each row has an input, a formula that uses it, and a formula
that uses that formula.

## License

Dual-licensed under MIT OR Apache-2.0. See [`../../LICENSE-MIT`](../../LICENSE-MIT) and
[`../../LICENSE-APACHE`](../../LICENSE-APACHE).
