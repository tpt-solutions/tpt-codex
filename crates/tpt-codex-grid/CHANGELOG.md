# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Live-grid evaluation with `tpt-lattice`: `evaluate_sheet` computes a sheet's cells, and
  `GridSession` keeps an evaluator for one sheet, so an edit recomputes only the affected cells.
- Errors stay in their own cell. A bad formula, a cycle or a division by zero does not stop the
  other cells.
- Excel range syntax in formulas, such as `=SUM(B2:B9)`, rewritten to the evaluator's syntax
  (`translate_ranges`).
- Grid values in prose: `grid_bindings` binds each cell as `sheet<id>.<address>` and under its
  name, `all_bindings` adds the query results, and `resolve_document` fills `[[name]]` with these
  values for output that is not live.
- `with_computed_cells`, which gives each formula cell its computed value, for formats with no
  formulas such as Word tables.
- `format_number`, which shows numbers in prose without trailing zeros.
- Benchmark `examples/bench_eval.rs` for full and incremental evaluation.

### Fixed

- Formulas written with Excel range syntax were parse errors and are now evaluated.
