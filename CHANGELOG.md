# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Markdown reader and writer now cover bulleted and numbered lists, fenced code blocks, thematic
  breaks and standalone images. `Block` gains `List`, `Code` and `ThematicBreak` variants. Constructs
  that would read back differently are errors on read and refusals on write. See `docs/markdown-grammar.md`.
- Cargo workspace (resolver 2, edition 2024) with crates `tpt-codex-core`, `tpt-codex-views`,
  `tpt-codex-markdown`, `tpt-typeset`, `tpt-codex-interop` and `tpt-codex-app`.
- `tpt-codex-core` data model: `BlockId`, `SheetId`, `Rect`, `TextStyle`, `LayoutMode`,
  `MediaSource`, the `Block` enum (`Text`, `Grid`, `Media`, `Container`, `Query`), and `Document`
  with metadata and a schema version.
- `Document::validate` checks schema version, root existence, dangling children, and acyclicity.
- JSON serialization via serde, with deterministic ordering (`BTreeMap`).
- Licensing gate: `scripts/check_licenses.py` (Apache-2.0-only and `-sys` checks, with fixture tests)
  and `deny.toml` for `cargo deny check licenses`.
- SPDX header check: `scripts/check_spdx.py`.
- CI workflow (`.github/workflows/ci.yml`): licensing, tests on Linux/Windows/macOS, `no_std` and
  WASM builds.
- `MIT OR Apache-2.0` licensing files and project documentation.

- `tpt-codex-core`: editing operations (`Op`, `apply`) with inverses, cycle-safe moves, `History` with
  undo and redo, character-based `edit_text`, and `Clock` and `Storage` traits with in-memory helpers.
- `tpt-codex-markdown`: `from_markdown` and `to_markdown` for front matter, headings and paragraphs, with
  structured errors and a formal grammar (`docs/markdown-grammar.md`).
- `tpt-codex-views`: Flow layout with an injected `TextMeasure`, pagination with orphan and widow control,
  headers, footers and page numbers, and A4 and Letter paper sizes.
- `tpt-typeset`: heading outline with numbering, anchors and table of contents; `article` and `report`
  document classes; a math front end (`parse_math`) with line, column and hint errors.
- `tpt-codex-app`: `DocumentStore` for validated save and load, and `FileStorage` with atomic writes.
- Tests: randomized property tests for the model, edit and undo invariants, pagination invariants, a
  Markdown golden snapshot, and an end-to-end acceptance test.
- CI: cross-OS byte comparison of the golden output, and gate-script tests.
- Docs: TPT dependency audit (`docs/tpt-dependency-audit.md`) and versioning policy (`docs/versioning.md`).
- Benchmark: `tpt-codex-views` `examples/bench_layout.rs`.
- `scripts/check_licenses.py`: `--sys-roots` to check other crates for `-sys` closures.

### Fixed

- License gate: `MIT/Apache-2.0` and `Apache-2.0 / MIT` are read as OR; dev-only dependencies do not count
  toward the C-FFI rule.
- Python bytecode caches are no longer tracked.

### Decided

- `Unicode-3.0` is on the license allow-list. It is required by the serde derive toolchain
  (`unicode-ident`) and is permissive. This extends the list in the spec and needs maintainer sign-off.
