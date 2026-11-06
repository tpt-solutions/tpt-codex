# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- `tpt-codex-cli` crate and the `tpt` command: `convert` between `.cdx`/`.md`, `.docx`, `.xlsx`, `.pptx`
  (read only), `.json` and `.pdf` (write only), with the import's fidelity report; `check` (math,
  cross-references, figures, equations, tables, footnotes and, with `--bib`, citations; exit status 1 on
  errors); `watch` (rebuild on save); `diff` (word-level); `new` (article, report, slides, budget
  templates). Documented in `docs/quickstart.md`, `docs/cdx-cheatsheet.md` and
  `docs/migrating-from-office.md`.
- `tpt_codex_pdf::export_with_defaults`: the app's one-click export (A4, class margins, `Page N of M`
  footer), shared by the app and the CLI.

- Release workflow, `scripts/build-web.sh` and `.ps1`, a `justfile`, and a devcontainer.

- Live values in text: `[[sheet1.B4]]` shows that grid cell's computed value in the PDF, the Word export
  and the editor's paginated view (`tpt_typeset::resolve_bindings`, `tpt_codex_grid::grid_bindings` and
  `resolve_document`). `tpt check` reports a binding with no value.
- Formulas accept Excel's range syntax: `=SUM(B2:B9)`, with `$` references, is rewritten to the
  evaluator's `RANGE(B2, B9)` (`tpt_codex_grid::translate_ranges`). `tpt check` reports formula errors.
- Markdown: bold and italic on the same text, written and read as `***text***`. A block's own bold or
  italic flag is written as emphasis around its text (paragraphs, headings and list items), so a styled
  `.docx` converts to `.cdx`.

### Fixed

- `.docx` import no longer puts spaces at the edge of a bold or italic run, which Markdown cannot write.
- `to_markdown` writes a grid whose sheet stores no size (such as an imported one) at the size of its
  cells, instead of refusing it.
- Added the missing SPDX headers to the fuzz targets, which the header gate rejected.
- Crate docs and the README no longer describe finished work as "not yet implemented" or "scaffold".

- One-click PDF export: `tpt-codex-app::export::export_pdf` writes the document on its document class's page size,
  with a `Page N of M` footer. The demo page has an `Export PDF` button, and the browser test checks the download.
- `tpt-codex-pdf` text is drawn as glyph outlines (`tpt-canvas` pinned at `e451560`).

- `tpt-codex-text` crate: text measurement with the bundled Noto Sans Regular (OFL), shaped by
  `tpt-text`. It implements the Flow view's `TextMeasure` and `tpt-layout`'s `TextMeasurer`. The
  Flow view and pagination are tested on the fixture with it.
- Inline runs: `TextStyle.runs` holds emphasis and links on character ranges. Edits move them with
  the text. Markdown reads and writes `*em*`, `**strong**`, `[text](address)` and backslash escapes
  in paragraphs, headings and list items.
- Canvas and frame model: `Document.canvases` (slide sizes, `Canvas::WIDESCREEN` is 16:9) and
  `Document.frames` (child rectangles in absolute containers), with undoable `SetCanvas` and
  `SetFrame` ops and validation.
- Grid pagination: each grid row is a flow item, so pages break between rows, and
  `FlowOptions.header_rows` repeats the leading rows on continuation pages.
- `tpt-codex-core::sync`: `Replica` (tree and text CRDTs with an outbox, acknowledgements,
  de-duplication and held out-of-order operations), a `Transport` trait, and presence cursors. No
  network transport or server is included.
- Bold and italic text: the bundled font set now includes Noto Sans Italic, and bold is the
  variable font at weight 700.
- Editor: inline marks are drawn (links show their address as a tooltip, with no `href`), slides
  can be inserted and blocks placed on them, and the next table's size can be chosen.
- `tpt-codex-views::layout_canvas` and `slide_ids`: place a slide's children at their frames.
- Grid cells and sizes replicate in `Replica` as last-writer-wins values.
- `tpt-codex-core::VersionLog`: labelled snapshots, restore and a block-level diff.
- Repeated grid header rows: `Sheet.header_rows`, replicated with the grid's size. Replaces the
  document-wide `FlowOptions.header_rows`.
- `migrate_legacy_markdown` and an example command, to escape inline markers in files written
  before inline syntax was read.
- `Relay`: a per-document operation log for a replication server, with de-duplication and
  catch-up. No network transport yet.
- Editor: `ReplaceText` turns an input element's new text into the smallest undoable edit.
- `render_slide_svg`: one slide as SVG, with text wrapped by the same measure as the Flow view.
- Editor presenter mode: one slide at a time, with previous, next and stop.
- Editor: remote carets are drawn in their paragraphs (`set_remote_cursors`).
- Editor: `MoveBlock` and `ResizeBlock` move and resize slide children, clamped to the slide.
- Editor: `FilterQuery` builds an equality filter on a query block.
- Editor: flow and grid containers draw their children.
- `layout_flow_cached` and `FlowCache`: a flow layout that re-wraps only the text blocks that changed.
- Editor: `paginated_view` shows the document as pages.
- `tpt-codex-interop::place_sheet`: puts an imported sheet in the document as a grid block.
- `tpt-typeset::layout_math`: boxes, rules and scripts for a parsed formula, from injected character metrics.
- `FontMeasure` implements `MathMetrics` with the bundled font, and `math_svg` draws a laid-out formula as SVG.
- `check_references`: resolves links to heading anchors (`[text](#anchor)`) to section numbers, and reports the missing targets.
- `tpt-codex-sync`: a WebSocket transport (`WsTransport`, implementing `Transport`) and a relay
  server (`serve`, `spawn`), with the `tpt-codex-relay` binary. Clients join a document by name,
  get its log replayed, and see each other's operations and presence. No authentication yet.
- Citations: CSL-JSON library (`Library`), author-year text, bibliography, and a check for unknown
  keys. Markdown reads `[@key]` and writes it back.
- Footnotes: `Document.footnotes`, `[^label]` references and `[^label]: text` definitions in
  Markdown, with numbering in reading order and checks for unmatched references and unused
  definitions.
- Editor: `MoveSlide` reorders slides, as one undoable step. `SetFullscreen` marks the presenter
  stage for the host's full-screen call.
- The paginated view uses the page size and margins of the document's class, and draws headers and
  page-numbered footers.
- `render_slide_svg` draws embedded SVG as an image, so a script inside it cannot run. A golden
  file checks the slide's bytes.
- Word-level text diff (`diff_text`, `VersionLog::text_diff`).
- `.docx`: per-run bold and italic are written and read back. Mixed formatting in a paragraph is
  kept, not reported as lost. Links are written as their text, and the report says so.
- Acceptance tests for milestones 2 (up to the PDF), 4 (collaboration with live grids over the
  relay) and 5 (a real `.docx` and `.xlsx` opened, edited and written back).
- Mutation tests now cover citations, footnotes and the CSL-JSON reader.
- Browser: `tpt-codex-web` is a wasm module that mounts the editor on a page. A browser test
  (`crates/tpt-codex-web/browser`) drives it in Chrome: grid cells and formulas, undo, typing in a
  paragraph, mid-paragraph typing, arrow keys, and slides.
- Editor: a focused paragraph is a native text area, and each change is a `ReplaceText`. Paragraph
  marks and remote carets are drawn with containers, which the browser can style. Focusing a grid
  cell keeps the paragraph caret, so the cell keeps focus.
- Inline math: `$…$` in Markdown is an inline run with its TeX source. Checked by
  `check_inline_math`, drawn as math in the editor, and laid out with the math layout.
- Slide sections: `Document.sections`, `SetSection` (undoable), `slide_sections`, and the section
  name in the presenter.
- Demo page: a full-screen button for the presenter stage.
- Editor: a pages switch shows the document as pages, with a button back to editing. The demo
  page has it too.
- Browser checks: selecting with the keyboard and typing over the selection, and the pages switch.
- Grid `view_bounds` is a vertical window onto the sheet, in points from its top. The flow shows the
  header rows and the rows the window covers.
- Browser: `EditorHandle` (returned by `mount_sample`) lets the page send messages to the editor.
  Full screen is tracked both ways.
- Editor: a grid with a view window shows the same rows as the flow.
- Browser check: an input-method composition commits into the paragraph (22 checks).
- `Block::Query` source is editable in the editor (`EditQuery`); a source that does not run is
  rejected.
- `tpt-codex-core` caret and selection model (`Selection`): caret movement, insert over a range,
  backspace and delete. Each edit is one undoable step. Positions count characters.
- `History::last_changed` and `changed_blocks`: the blocks a step changed, for rebuilding only the
  affected projections.
- `Block::Math` for display math, read and written by `tpt-codex-markdown` as `$$` blocks. Syntax
  and writer refusals are in `docs/markdown-grammar.md`.
- `tpt-typeset::check_math_blocks`: parses every math block in a document and reports the failures
  with their block id and position.
- `tpt-codex-interop::FidelityReport`: records what an import kept, approximated or dropped, with a
  deterministic text form.
- Tests: headless typing session (caret, undo, equation block, save and reload), selection and
  change-tracking unit tests, math reader, writer and checker tests, and a seeded Markdown mutation
  test (4,000 inputs by default; 300,000 run clean locally).

- Editable tables: `Op::SetSheet` inserts, replaces or removes a sheet as one undoable step (removal is refused while a grid displays it). `Sheet` has an optional minimum `rows`/`cols` (omitted from JSON when zero), so an empty table can be typed into. The editor gets `Msg::InsertTable`, `Msg::FocusCell` and `Msg::MoveCell` (arrow keys), and an "Insert table" button.
- Benchmark `tpt-codex-grid/examples/bench_eval.rs`: full and incremental grid evaluation.
- Live grids: `Sheet` and `CellInput` in `tpt-codex-core`, stored in `Document::sheets`. `Op::SetCell`
  edits one cell as an undoable step. Validation checks addresses, formulas and that every
  `Block::Grid` names an existing sheet. The `sheets` key is left out of JSON when there are none.
- `tpt-codex-grid` (new, std): `evaluate_sheet` computes cell values with `tpt-lattice`. Errors stay in
  their own cell; cycles and bad formulas do not stop other cells.
- `tpt-typeset::run_query`: runs a `Block::Query` (JSON `tpt-query` form) against a host-supplied entity
  source.
- TPT dependencies pinned by rev in `Cargo.toml`: `tpt-query`, `tpt-lattice-core`, `tpt-lattice-evaluator`.
  `deny.toml` allows those two git sources. The license gate passes on the resolved closure (31 packages).

- `GridSession` in `tpt-codex-grid`: keeps an evaluator alive for one sheet. Edits recompute only the
  affected cells. Addresses must be canonical A1 notation, as in the core.
- `tpt-codex-app::Editor`: the editor's state and UI tree (`tpt-appfront-core`). `Msg` covers typing,
  caret movement, backspace, delete, focus, undo and redo. The view renders to HTML with
  `tpt-appfront-html`, and is tested natively.
- `tpt-codex-app::web::mount` (`wasm32` only): mounts the editor with `tpt-appfront-dom`. The app builds
  for `wasm32-unknown-unknown`, and CI checks it. The browser run is not yet done.
- Seeded mutation tests for the math parser (`tpt-typeset`) and document JSON (`tpt-codex-core`), next to
  the Markdown one.
- Pinned `tpt-appfront-core`, `tpt-appfront-html` and `tpt-appfront-dom` (rev `6146526`).

- Text CRDT for `Block::Text` (`tpt-codex-core::TextCrdt`): a replicated growable array of characters with
  Lamport stamps, tombstones and causal delivery (`TextOp`). Concurrent edits converge; `materialize` writes
  the converged text into a block as one undoable step. Convergence is tested over random edits and
  reordered delivery.
- `tpt-typeset::document_entities`: maps each block to a `tpt-query` entity, so `Block::Query` JSON runs over
  the document. Entity type ids are in `entity_type`.
- Editor: `Msg::EditCell` sets grid cells from typed text (`parse_cell_input`). The grid view shows an
  editable table. Seeded mutation tests cover the math parser and document JSON.

- Office stack written in-house (`tpt-codex-interop`), replacing the `zip`-based route that the license gate
  rejected:
  - `archive`: ZIP reader and writer. Reads stored and DEFLATE entries, checks CRC-32 and size limits
    (64 MiB per entry, 256 MiB total), and refuses encryption, ZIP64 and other methods. Writes DEFLATE entries
    with fixed timestamps, so output is deterministic.
  - `xlsx`: `import_xlsx` reads the first worksheet into a `Sheet` and reports what it could not keep.
    `export_xlsx` writes one worksheet, with cached formula values. Tested against openpyxl and Python
    `zipfile` fixtures (`fixtures/office/`, generated by `make_fixtures.py`), both directions.
  - Dependencies: `flate2` (pure-Rust backend), `crc32fast`, `quick-xml`. All pass the gate.

- `.docx` import (`tpt-codex-interop::import_docx`): in-house, on the `archive` module. Paragraphs, headings,
  bold and italic, breaks, title and author. Tables are flattened, list items become paragraphs, images are
  dropped. Each loss is recorded in the fidelity report. Tested on a python-docx file.

- Block-tree CRDT (`tpt-codex-core::TreeCrdt`): create, update, move and delete (to trash), with the move-
  operation rules of Kleppmann et al. (2021). Moves that would form a cycle are skipped on every replica, and
  late moves are applied in stamp order. Children are ordered by fractional keys (`key_between`).
  `project_document` turns a replica into a `Document`. Convergence and acyclicity are tested over random
  edits and out-of-order delivery.
- `.docx` export (`export_docx`) and `.pptx` import (`import_pptx`), both in-house on the `archive` module.
  Each loss is recorded in the fidelity report. Checked against python-docx and python-pptx files.

### Changed

- Bumped `tpt-appfront-core`, `-html` and `-dom` to `5d5dfd6` (web, TUI and canvas focus, keydown and pointer events).
- Grid cells in the editor select on focus, and arrow keys move the selection (`FocusCell`, `MoveCell`).

- Schema version is 2 (was 1). Version 2 covers the `List`, `Code`, `ThematicBreak` and `Math`
  block variants. No version 1 document was released, so no migration exists. See
  `docs/versioning.md`.

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
