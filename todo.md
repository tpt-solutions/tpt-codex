# tpt-codex TODO

License: MIT OR Apache-2.0 · Copyright © 2026 TPT Solutions · Zero Apache-2.0-only dependencies.
Source spec: `spec.txt`. Week estimates are from the spec (solo maintainer).

Legend: `[ ]` todo · `[~]` partly done · `[x]` done

---

## Phase 0 — Foundation & Gates (Weeks 1-2)

### Repo & licensing
- [~] `git init`, create GitHub repo under `tpt-solutions`, add `.gitignore`, `rust-toolchain.toml`, `rustfmt.toml` — _local parts done; GitHub repo not yet created_
- [x] Add `LICENSE-MIT` and `LICENSE-APACHE` (copyright TPT Solutions 2026)
- [x] Set `license = "MIT OR Apache-2.0"`, `authors = ["TPT Solutions"]` in `[workspace.package]`
- [x] Add SPDX header (`MIT OR Apache-2.0`) convention + check script (`scripts/check_spdx.py`)
- [x] README (mission, license section with standard dual-license + contribution clause), CONTRIBUTING, SECURITY, CHANGELOG
- [x] Write the ecosystem-wide "TPT dependency map" doc (see "Verified TPT dependencies" below) — `docs/tpt-dependency-map.md` (entries still unverified)

### Workspace scaffold
- [x] Cargo workspace root (resolver 2, edition 2024, shared deps/lints)
- [x] `tpt-codex-core` (`no_std` + alloc)
- [x] `tpt-codex-views` (`no_std` + alloc) — scaffold only
- [x] `tpt-codex-markdown` (`no_std` + alloc) — scaffold only
- [x] `tpt-typeset` (`no_std` + alloc for parsing/AST) — NEW, does not exist in org — scaffold only
- [x] `tpt-codex-interop` (std) — scaffold only
- [x] `tpt-codex-app` (WASM-first, std) — scaffold only
- [x] Add `no_std` build check for core/views/markdown/typeset (thumbv7m and wasm32-unknown-unknown, both pass locally)

### Verified TPT dependencies (github.com/tpt-solutions, all `MIT OR Apache-2.0`)
- [~] Add as git deps pinned by `rev` (crates.io publication unconfirmed — re-check, then switch to versions) — _added for the crates in use: `tpt-query`, `tpt-lattice-core`, `tpt-lattice-evaluator` (pinned in `Cargo.toml`). The rest are pinned in `docs/tpt-dependency-map.md`, not yet wired in._
- [x] `tpt-model` (check `*-stub` crates don't leak into our dep tree) — _done upstream: the stub crates are folded into `tpt-model` (`59699b3`, on GitHub master). `cargo test -p tpt-model` passes and the license gate passes on its metadata. tpt-codex does not depend on it yet_
- [x] `tpt-text` — _wired in at `83bc13c` through `tpt-codex-text`, which bundles Noto Sans (OFL): Regular and Italic files, both variable. Bold is the same font at weight 700 on the `wght` axis. Static files were not available_
- [~] `tpt-layout` — _wired in at `21361d1`: `tpt-codex-text` implements its `TextMeasurer` with `tpt-text`. The Flow view still uses its own engine, so `layout_flow` from `tpt-layout` is not used yet_
- [x] `tpt-canvas` + `tpt-canvas-pdf` (PDF backend is a crate inside the `tpt-canvas` repo, not a separate repo) — _pinned at `e451560`, where glyph runs are filled as outlines, so PDF text is drawn. `tpt-codex-pdf` uses it_
- [~] `tpt-lattice` workspace crates: `-core`, `-evaluator`, `-parser`, `-crdt`, `-wasm` — _`-core` and `-evaluator` added (grid evaluation in `tpt-codex-grid`; the evaluator is std-only, so it is not in `no_std` crates). `-parser` comes in transitively. `-crdt` and `-wasm` not audited_
- [x] `tpt-lattice-import-xlsx` / `-export-xlsx` (spec's "tpt-lattice-import" = these) — _replaced by our own stack in `tpt-codex-interop`: `archive` (ZIP reader and writer, stored and DEFLATE, CRC and size limits) and `xlsx` (reader and writer, on `quick-xml`). `zip`, `calamine` and the lattice xlsx crates are no longer used. Tested against openpyxl and Python `zipfile` fixtures, both directions_
- [ ] `tpt-replicate` — _gate clean, but its CRDT types (`UirNode`, `ActorId`, `CrdtCompatible`) come from its own `stubs` module. Not adopted: a stub type would enter the sync format. Text CRDT written in `tpt-codex-core` instead (see Phase 4)_
- [~] `tpt-appfront` crates: `-core`, `-dom`, `-canvas`, `-html`, `-webview` (pick which are needed) — _audited per crate: `-core`, `-dom`, `-html` gate OK and in use. `-canvas` and `-webview` fail the gate (Apache-only GUI stacks: winit, glutin, tao). They are native-desktop crates, outside the WASM-first decision_
- [~] `tpt-doc` crates: `-core`, `-word`, `-spreadsheet`, `-pdf`, `-convert` — _upstream fixed: `tpt-doc` master (`cc16a28`) uses `zip` with `deflate-flate2-zlib-rs`, and `-pdf` and `-convert` pass the license gate on their own closures. Not adopted: `.docx` and `.xlsx` are in-house, and `tpt-doc-pdf` writes PDFs only (no parser)_
- [x] `tpt-query` — _added: `tpt-typeset::run_query` runs `Block::Query` JSON with the `json` feature. Gate clean, `no_std` build verified_
- [ ] `tpt-version` (for semantic diff/merge) — _gate clean; depends on `tpt-crypto-hash`/`-sig` (git). Not wired in: Phase 4_
- [~] Audit each for `no_std` support, wasm32 support, and `-sys`/C-FFI transitive deps — _`-sys` closure checked for TPT crates; `no_std`/wasm builds not verified (needs approval to compile external code)_
- [~] Audit transitive licenses (`cargo tree`, `cargo deny`) — _license gate run on the resolved closure (31 packages): OK. `cargo deny` not run (not installed). `deny.toml` now allows the two pinned TPT git sources._

### Strict licensing CI gate
- [x] `deny.toml`: allow MIT, 0BSD, BSD-2-Clause, BSD-3-Clause, Unicode-DFS-2016, Unicode-3.0, Zlib (Apache-2.0 omitted: `X OR Apache-2.0` passes via X)
- [x] `deny.toml`: deny GPL-2.0, GPL-3.0, AGPL-3.0, SSPL, BSL, Apache-2.0-only — _`cargo deny` itself not run locally (not installed)_
- [x] Ban `*-sys` crates in core + views crates (script checks the dependency closure)
- [x] Script: parse `Cargo.lock` / `cargo metadata` and fail on any new Apache-2.0-only crate (`scripts/check_licenses.py`)
- [x] Script tests (fixtures with an Apache-only crate must fail) — `scripts/test_check_licenses.py`
- [x] Decide handling of `Apache-2.0 WITH LLVM-exception` and `Unicode-3.0` — Unicode-3.0 allowed (serde derive needs it); WITH-exceptions rejected. _Signed off: recommendation applied (2026-11-04)._

### CI (GitHub Actions)
- [x] `cargo deny check licenses` (required on all PRs) — workflow written; not yet run on GitHub
- [x] Apache-only script gate — workflow written; not yet run on GitHub
- [x] `cargo fmt --check`, `cargo clippy -D warnings` — workflow written; passes locally
- [x] `cargo test` (Linux, Windows, macOS) — workflow written; passes locally on Windows only
- [x] `wasm32-unknown-unknown` build — workflow written; passes locally
- [x] `no_std` build check — workflow written; passes locally
- [ ] Branch protection requiring all above

### Data model foundation
- [x] Define IDs & primitives: `BlockId`, `SheetId`, `Rect`, `TextStyle`, `LayoutMode` (Flow/Grid/Absolute), `MediaSource`
- [x] Define `Block` enum: `Text`, `Grid`, `Media`, `Container` (+ `Query`)
- [x] Add `Block::Query` (mentioned in spec §4 note but missing from the enum) and define its shape — `Query { source: String }`; language owned by `tpt-query`
- [x] `Document` type: DAG of blocks, root, metadata (front matter)
- [x] `Block::Math` for display math; schema version bumped to 2 (covers `List`, `Code`, `ThematicBreak`, `Math`) — _no v1 document was released, so no migration_
- [x] Sheet cell storage: `Sheet`, `CellInput`, `Op::SetCell` (undoable), validation of addresses, formulas and `Block::Grid` sheet references. `sheets` is omitted from JSON when empty, so the schema version is unchanged
- [~] `tpt-model` schema + versioning for the Block tree — _partial: own `schema_version` in core; tpt-model not used_
- [x] Basic serialization / deserialization (serde JSON, deterministic `BTreeMap` order)
- [x] DAG invariants (no cycles, dangling `BlockId`s) + validation (`Document::validate`)
- [x] Unit + property tests for model — `tests/model.rs` and randomized `tests/properties.rs`

**Milestone 0:** CI is green, license gate demonstrably blocks an Apache-only dep, `Block` tree round-trips through serialization.

---

## Phase 1 — Hybrid Document MVP (Weeks 3-6)

- [x] `tpt-codex-app` shell on `tpt-appfront`, builds to WASM — _editor state and UI tree (`Editor`, `Msg`) render to semantic HTML and are tested natively. `web::mount` mounts the editor with `tpt-appfront-dom`; the app builds for `wasm32-unknown-unknown` and CI checks it. Not yet run in a browser_
- [~] Core ⇄ UI reactive wiring (tree change → projection update) — _partial: `History::last_changed` names the blocks each step changed. The editor re-renders from a signal after each message (`web::mount`). No per-block projection cache yet_
- [x] **Flow View** in `tpt-codex-views`: shape text via `tpt-text`, lay out via `tpt-layout` (block/inline flow, margin collapsing, typographic hierarchy) — _partial: flow layout with injected `TextMeasure`; `tpt-codex-text` supplies the `tpt-text` measure. Decided, as agreed: keep our own flow engine and do not adopt `tpt-layout`'s. Adjacent margins collapse, and headings get more space above and a larger size (`tests/margins.rs`)_
- [~] Text editing: caret, selection, input, undo/redo — _partial: checked in Chrome (`crates/tpt-codex-web/browser`, 22 checks): typing in a focused paragraph (a native text area), typing in the middle, selecting with the keyboard and typing over the selection, an input-method composition committing into the paragraph, undo, grid cells with typed values and recalculation, and arrow keys. The model's caret is set at the end of each edit, not from the browser's selection; that is the decision, as the typing is diff-based_
- [x] `TextStyle` editing (bold/italic/headings/lists etc.) — _done: bold/italic/heading via `SetBlock`; list insert, add and remove item, and the bullet/number toggle are app commands (`crates/tpt-codex-app/tests/lists.rs`)_
- [~] **Grid View**: swap to `tpt-lattice` canvas/WebGL renderer when `Block::Grid` is focused — _partial: the editor shows an editable table (`data_grid` plus one input per cell). No canvas or WebGL renderer_
- [~] Cell editing + LES formula entry + real-time evaluation (`tpt-lattice-evaluator`) — _partial: `Op::SetCell` (undoable); `GridSession` recomputes affected cells; the editor's `EditCell` message parses typed text (number, boolean, `=formula`, text) and shows the grid as a table. Cell focus and arrow keys work in the editor: each grid input has `on_focus` (`FocusCell`) and `on_keydown` (arrows become `MoveCell`). These need the appfront `web-events` work (merged to appfront master, not yet pushed); appfront is pinned to `5d5dfd6` (pushed to tpt-solutions/tpt-appfront master)_
- [~] Inline embed of a live grid in text flow (`view_bounds`), plus full-sheet reference — _partial: `view_bounds` is a vertical window onto the sheet, in points from its top. The flow and the editor both show the header rows and the rows the window covers. Columns are not laid out in the flow. A full-sheet reference is not done_
- [x] Insert-table command in the UI — _"Insert table" button adds an empty 3×3 `Block::Grid` as one undoable step (`Op::SetSheet` + `AddBlock` + `InsertChild`); `Sheet` has a minimum `rows`/`cols`, so empty cells can be typed into. Table size is fixed at 3×3 until a size control exists_
- [~] `Block::Query` evaluation via `tpt-query` — _partial: `tpt-typeset::run_query` runs JSON queries, and the editor shows each query's matching blocks or its error. The source is editable, and `FilterQuery` builds an equality filter on `kind`, `content` or `language`. No richer query builder yet_
- [x] `Block::Media` rendering (images, SVG, `tpt-canvas` drawing commands) with alt text — _the editor draws media with its alt text. SVG and `tpt-canvas` commands are not drawn yet_
- [x] `Block::Container` rendering for Flow/Grid layout modes — _Flow and Grid containers draw their children in order, with a class for the host to lay them out. Absolute containers are slides_
- [x] Trait boundaries for platform I/O, rendering, clock, storage (keep core headless) — _`Clock`, `Storage`, `TextMeasure` traits; rendering via `TextMeasure` only_
- [x] Local persistence (save/load document) — _JSON via `DocumentStore` over `FileStorage`; validated on save and load_
- [ ] Native desktop shell smoke test (via appfront webview/tui, if used) — _not started: `tpt-appfront-webview` fails the gate. WASM-first is the decision, and the browser run now works_

**Milestone 1:** A user types a paragraph, inserts a live calculation table, and it updates reactively in the browser via WASM.

---

## Phase 2 — Markdown & Typesetting (Weeks 6-8)

### tpt-codex-markdown
- [x] `from_markdown(&str) -> Result<Document, ParseError>` — front matter, headings, paragraphs; other syntax is a structured error
- [x] `to_markdown(&Document) -> Result<String, SerializeError>` — refuses anything it cannot write back exactly
- [x] CommonMark mapping: headings, paragraphs, lists, code blocks, thematic breaks, images, links and emphasis _(done: inline runs in `TextStyle.runs`; `*em*`, `**strong**`, `[text](address)` and escapes in paragraphs, headings and list items. Other inline syntax is an error. The editor view does not show the marks yet)_
- [x] Codex-Flavored Markdown: `::: grid { id="..." }` blocks (table body → lattice sheet) — _done: read and write in `tpt-codex-markdown`, with round-trip and error tests_
- [x] `::: canvas { ratio="16:9" }` slide containers — _done: read and write in `tpt-codex-markdown`, with round-trip and error tests_
- [x] `$$ ... $$` math blocks (+ inline math) — _display math: reader, writer and `Block::Math`. Inline math: `$…$` is an inline run with its TeX source (`InlineRun.math`), checked by `check_inline_math` and drawn as math in the editor_
- [~] Citations `[@key]`, cross-refs `[#id]`, figure attrs `{#fig:id width=80%}` — _partial: `[@key]` and `[#id]`-style cross-refs are done as links (`[text](#anchor)`, `[text](@key)`), as agreed. Figure attributes are not_
- [x] `.cdx` ("Codex Script") format: front matter (title, author, date, class) — simple `key: value` subset of YAML, not full YAML
- [x] Define & document the grammar formally (spec examples are not exhaustive) — _`docs/markdown-grammar.md`_
- [x] Structured parse errors: line/column + fix suggestion (for the supported subset; column is 1 for line-level errors)
- [~] Round-trip + snapshot tests; fuzz the parser — _round-trip (incl. lists, code, images, math, inline math, citations, footnotes), randomized and golden snapshot tests done; seeded mutation tests for Markdown, math, document JSON and CSL-JSON. A libFuzzer harness is in `fuzz/` (Markdown, JSON, math, bibliographies, xlsx, docx, pptx). It type-checks on stable; running it needs nightly_

### tpt-typeset
- [~] Semantic AST: chapters, sections, footnotes, citations, TOC, cross-refs — _partial: heading outline and numbering; no footnotes, citations or chapter objects_
- [ ] AST → `tpt-layout` BoxTree conversion — _not done: layout uses the in-house flow engine, not `tpt-layout`_
- [x] TOC generation — _`Outline::toc` with depth limit_
- [~] Cross-reference resolution (headings, figures, tables, equations) — _partial: a link to `#anchor` is a cross-reference. `check_references` resolves each one to its heading number, and reports the missing ones. Figures, tables and equations are not in the model_
- [x] Footnote placement — _done: footnotes are in the model (`Document.footnotes`), read and written as `[^label]` references and `[^label]: text` definitions. `check_footnotes` numbers them in reading order. `footnote_notes` makes one note per footnote at its first reference, and `paginate_with_notes` places each note at the foot of the page where its anchor falls. The PDF export and the paginated view use it (`tests/footnote_notes.rs`, `tests/notes.rs`)_
- [x] Citations + bibliography (decide source format, e.g. BibTeX/CSL-JSON) — _CSL-JSON, as agreed. `Library` reads it, `[@key]` and `[text](@key)` cite, `check_citations` reports unknown keys, `bibliography` lists the cited entries sorted by author and year. BibTeX is read too, by `Library::from_bibtex`, with TeX accents converted for the common letters (`tests/bibtex.rs`)_
- [x] Document classes (`article` etc.) as a typed, declarative schema (no macro language) — _`DocumentClass` (article, report) typed schema; not yet used by layout_
- [x] Typography: kerning, ligatures, hyphenation (via `tpt-text`) — _partial: kerning and standard ligatures come from shaping with the bundled font (`tpt-codex-text` tests). Hyphenation is in-house (`tpt-codex-views::hyphenate`): our own rules and exception list, used by the PDF export and the paginated view. It is rule-based, not dictionary-based, so quality is approximate_
- [~] **Math layout engine** (spec references a "new typesetting crate" — not in the crate list): parse LaTeX-style math, box/glue layout, fonts with MATH table — _partial: parser and document check (`check_math_blocks`). `layout_math` lays out boxes, rules and scripts from character metrics (`MathMetrics`). `FontMeasure` supplies them from the bundled font, and `math_svg` draws a laid-out formula as SVG. The layout reads the font's MATH constants (`math_constants`), with defaults when a font has none_
- [x] Better math error messages — _line, column, message and hint on each error_
- [~] Data binding: embed `tpt-lattice` grids, `tpt-query` results, variables — _partial: `tpt-typeset`'s `Bindings` binds `[[name]]` to query results and grid cells, and checks them. Embedding a live grid in text is in the grid row above_
- [~] Structured, actionable errors ("Line 42: unclosed environment") — _partial: Markdown and math errors carry line, column and hint_
- [~] Incremental compilation / watch mode (via `tpt-layout` incremental relayout) — _partial: `layout_flow_cached` with a `FlowCache` re-wraps only the text blocks that changed (tested by counting measurements). Watch mode and `tpt-layout` relayout are not done_
- [~] WASM in-browser live preview — _partial: the editor runs in Chrome as a wasm module (`tpt-codex-web`), with a demo page and a browser test. It is an editor, not a preview of a rendered page; the paginated view is not on the page yet_
- [x] Semantic diff/merge integration via `tpt-version` — _in-house: word-level diff, block-level diff and `three_way_merge` (`tpt-codex-core::merge`) are done, and `tpt merge` runs it on files. `tpt-version` is not used_

**Milestone 2:** A `.cdx` file with Markdown, math, and cross-references exports to a beautifully typeset PDF.
(Note: depends on the PDF backend in Phase 3 — either reorder, or ship a minimal `tpt-canvas-pdf` hookup here.)

---

## Phase 3 — PDF Pipeline (Weeks 7-10)

- [~] **Paginated View**: wire `tpt-layout` pagination to the hybrid document — _partial: pagination on the injected flow layout; the editor has a paginated view (`paginated_view`) with page numbers, lines and grid rows. Not `tpt-layout`; no chrome in the editor_
- [x] Orphan/widow control — _`min_lines` on both sides of a split paragraph_
- [x] Repeated table headers across pages — _`Sheet.header_rows` sets how many leading rows of a grid repeat at the top of each continuation page. It is per grid, and it replicates with the grid's size_
- [x] Page size, margins, headers/footers, page numbers — _`PageGeometry`, `PaperSize` (A4, Letter), `render_chrome` with `{page}`/`{pages}`_
- [x] Render grids into pages (page breaking for large grids) — _each grid row is a flow item (`FlowKind::GridRow`), so pages break between rows and header rows repeat. The host draws the cell values; the views crate only places rows_
- [x] Output drawing commands to `tpt-canvas-pdf` — _done in `tpt-codex-pdf`: glyph runs are filled as outlines (`tpt-canvas` `e451560`), so the PDF shows the text as laid out_
- [x] Font embedding / subsetting for PDF — _done for the text layer: the bundled font is embedded as a CID font, subset to the glyphs used (the variation and layout tables are dropped, glyph ids kept), so a page adds about 50–100 KB. Outlines are still how text is drawn; see the text layer below_
- [x] Text layer: the PDF's words as invisible text over the outlines — _done in `tpt-codex-pdf/src/text_layer.rs`: each glyph is written in render mode 3 at its outline's position, with a ToUnicode map. The file is an incremental update, so the outlines part is unchanged. Checked with `pdftotext` and `pypdf` (`tests/text_layer.rs`, `src/subset.rs`). Known: `pypdf` reads the bullet marker as U+FFFD, `pdftotext` reads it correctly_
- [x] One-click PDF export in the app — _an `Export PDF` button on the demo page downloads `document.pdf`, on the document class's page size, with a `Page N of M` footer (`tpt-codex-app/src/export.rs`). The browser test checks the download_
- [x] Byte-exact determinism: fixed timestamps/IDs, stable object ordering — _partial: ordered maps, fixed clock and deterministic layout; no PDF output yet_ — _same input gives the same bytes; the golden files are compared in tests_
- [~] Accessible PDF basics (tags, alt text from `Media.alt_text`) — _partial, in the text layer's update: a tagged file has a structure tree (document, headings `H1`–`H6`, paragraphs, lists as `L`/`LI`/`LBody`, figures with `/Alt`), a parent tree, marked content that is balanced, headers and footers as artifacts, `MarkInfo`, `Lang` (`en`, as the model records no language yet), and the title in `Info` and `DisplayDocTitle`. Checked with `pdfinfo` (Tagged: yes) and `pypdf`. Not done: grid tables (cells are paragraphs of the grid), images (the export draws only placeholders, so an image's alt text sits on its placeholder), nested lists (a sub-list is a sibling `L`, not inside its item), reading-order checks against a real accessibility checker, and a language per document_
- [ ] PDF text layer for pictures: images and placeholder boxes are drawn as before, with no text or alt text in the file_
- [x] Golden-file PDF tests — _`tpt-codex-pdf/golden/page.pdf` and `slide.pdf`, checked in `tests/export.rs`_

**Milestone 3:** One-click export of the Phase 1 hybrid document to a byte-exact, paginated PDF.

---

## Phase 4 — Canvas & Collaboration (Weeks 11-14)

### Canvas View (slides)
- [x] `Container` with `LayoutMode::Absolute` and fixed aspect ratio (16:9 etc.) — _`Document.canvases` holds each slide's size (`Canvas::WIDESCREEN` is 16:9), undoable with `Op::SetCanvas`. `layout_canvas` places the children, and the editor inserts and draws slides_
- [x] Explicit X/Y positioning of children, render via `tpt-canvas` — _`Document.frames` holds each child's rectangle (`Op::SetFrame`); the editor places blocks on a slide. Drawing goes through the slide's HTML, not `tpt-canvas`_
- [x] Slide navigation, presenter/fullscreen mode — _presenter mode shows one slide at a time, with previous, next, stop, and the section name. Full screen is tracked both ways: the demo page's button marks the stage in the editor (`EditorHandle.set_fullscreen`), and leaving full screen clears it. Browser-tested_
- [x] Slide ordering / sections — _slides can be reordered (`MoveSlide`, undoable). Each slide can be put in a section (`SetSection`, undoable), kept in `Document.sections`. Consecutive slides with the same section form a group (`slide_sections`)_
- [x] Select / move / resize of canvas children — _done: a press selects a child and starts a drag; the child follows the pointer and moves on release as one undoable step (`PressSlideBlock`, `DragSlideBlock`, `DropSlideBlock`). Browser-tested. Uses appfront `be649e1`, pushed and pinned_
- [x] Slides → PDF export — _partial: `render_slide_svg` exports one slide as SVG, with text wrapped by the Flow view's measure. PDF is blocked upstream_ — _`export_slide` writes one slide to PDF_

### Collaboration (`tpt-replicate`)
- [x] CRDT operations on the Block tree (insert/delete/move/update) — _move-operation tree CRDT in `tpt-codex-core` (`tree_crdt.rs`): create, update (last writer wins), move (Kleppmann et al. 2021 undo-do-redo), delete to trash. `project_document` turns a replica into a `Document`. Not yet wired into `History` or a transport_
- [x] Concurrent text editing semantics for `Block::Text` — _replicated growable array (RGA) of characters in `tpt-codex-core` (`text_crdt.rs`), with stamps, tombstones and causal delivery. `materialize` writes the converged text into the block as one undoable step. `Replica` (`sync.rs`) sends it with the tree operations; no network transport yet_
- [x] Merge live grids (`tpt-lattice-crdt`) with the document CRDT — _in-house: `Replica` replicates cells and grid sizes as last-writer-wins registers ordered by stamp. `tpt-lattice-crdt` is not used_
- [x] Move-operation conflict handling (DAG must stay acyclic) — _a move that would make a node its own ancestor is skipped on every replica. Late moves are applied in stamp order by undoing and redoing newer moves. Tested with a concurrent cycle and random delivery_
- [~] Offline-first sync + reconnect — _partial: `Replica` keeps an outbox of unacknowledged operations (`pending`, `acknowledge`), resends are de-duplicated, and out-of-order operations are held. `Relay` (`relay.rs`) keeps one copy of each operation per document and serves late replicas. Not yet tied to a network transport_
- [x] Transport layer (WebSocket) behind a trait; sync server decision — _done for WebSocket: `Transport` is in `tpt-codex-core`; `WsTransport` and the `tpt-codex-relay` binary (`tungstenite`, MIT OR Apache-2.0) are in `tpt-codex-sync`. The browser page talks to the relay over its own WebSocket. WebRTC is later, as agreed_
- [x] Presence / cursors — _done: cursors per replica (`Replica::set_cursor`, `cursor_of`, `cursors_in`), newest update wins, not queued. The editor draws remote carets in their paragraphs, fed from the relay in the browser (`presence-test.mjs`, 6 checks)_
- [x] Version history via `tpt-version` — _in-house, as agreed: `VersionLog` keeps labelled snapshots, restores them, and diffs blocks and words (`text_diff`). `tpt-version` is not used_

**Milestone 4:** Multi-user collaborative editing of a document containing live grids.

---

## Phase 5 — Interop (after v1; spec lists crate but no schedule)

- [x] `.docx` import → Block tree (best-effort) — _in-house `docx` reader in `tpt-codex-interop`, on our `archive` module: paragraphs, headings (`Heading1`–`6`, `Title`), bold and italic when a whole paragraph has them, line and tab breaks, title and author. Tables are flattened, list items become paragraphs, images are dropped. Each loss is one fidelity item. Tested on a python-docx file. Chosen over `tpt-doc-word` because the need is small (see the tpt-doc note)_
- [x] `.xlsx` import via `tpt-lattice-import-xlsx` → `Block::Grid` — _in-house `xlsx` reader in `tpt-codex-interop`. The first worksheet is imported into a `Sheet` with a `FidelityReport`, and `place_sheet` puts it in the document as one undoable step. Dates and formats come in as plain numbers. `import_xlsx_workbook` imports every worksheet, and each is placed as its own grid (`m5_interop` acceptance test)_
- [ ] `.pdf` text/structure extraction via `tpt-doc-pdf` — _blocked on a decision: no TPT crate parses PDFs, and `tpt-doc-pdf` writes PDFs only. Needs a new in-house parser, or an approved dependency_
- [x] `.pptx` import (data only, no animations) — _in-house `pptx` reader: each slide becomes a level-2 heading (its title placeholder, or `Slide N`) and its text. Pictures, tables, speaker notes and formatting are reported. Tested on a python-pptx file_
- [x] Export adapters: `.docx`, `.xlsx` — _`.xlsx` export of the first sheet (formulas with cached values, deterministic bytes), checked by openpyxl. `.docx` export keeps headings, bold, italic and per-run emphasis; links are written as text and the report says so. Other blocks are written as their nearest paragraph and reported_
- [x] Import fidelity report (what was dropped/approximated) — _`FidelityReport` in `tpt-codex-interop` with a text form; the docx, pptx and xlsx importers all fill it_
- [~] Corpus-based tests with sample real-world files — _partial: files written by openpyxl, python-docx, python-pptx and Python's zipfile (`fixtures/office/`). No real-world corpus yet, since that needs files from you_
- [x] Verify no new Apache-only transitive deps arrive via interop — _the license gate runs on the resolved closure, which includes interop, and passes_

**Milestone 5:** Open a real `.docx`/`.xlsx` and get a usable editable document.

---

## Cross-cutting: Testing & Determinism

- [~] Golden/byte-identical render tests: native vs WASM vs PDF — _partial: golden JSON compared across OSes in CI; the slide SVG is compared byte for byte with a golden file; no WASM or PDF comparison_
- [x] Determinism CI job (run on Linux/Windows/macOS and compare hashes) — _in `.github/workflows/ci.yml`; not yet run on GitHub_
- [x] Property tests for CRDT convergence — _text RGA (three replicas, 200 rounds; 20,000 ran clean) and block tree (150 rounds; 5,000 ran clean), with random edits, out-of-order delivery, and an acyclicity check after every step_
- [~] Fuzzing: Markdown/.cdx parser, math parser, deserializer — _seeded mutation tests for Markdown (now including citations and footnotes), math, document JSON, and CSL-JSON (`tpt-typeset/tests/mutation_csl.rs`). No cargo-fuzz harness yet; that needs a nightly toolchain and libFuzzer_
- [x] Benchmarks: layout, eval, large documents — _`tpt-codex-views/examples/bench_layout.rs` (layout and pagination) and `tpt-codex-grid/examples/bench_eval.rs` (full and incremental evaluation; 1,000 rows: 6.4 ms full, 0.9 ms after one input)_
- [x] Test fixtures / sample documents repo dir — _`fixtures/` with a sample and its golden JSON_
- [~] Per-phase milestone acceptance tests — _partial: Milestone 0 covered; Milestone 1's headless path covered; Milestone 2 covered up to the page (Markdown, math, cross-references, footnotes, citations; PDF blocked); Milestone 4 covered (two people editing text and live grids through the relay); Milestone 5 covered (a real `.docx` and `.xlsx` opened, edited, written and read back). The one-click export is on the demo page and browser-tested; `m3_paginated` tests the export crate_

## Release (minimal)
- [x] Versioning policy (`0.1.0-spec` → `0.1.0`) — _`docs/versioning.md`_
- [ ] Publish crates once all TPT deps are on crates.io (git deps can't be published)

## Live-calc documents
- [x] `[[sheet1.B4]]` in prose shows the grid cell's computed value in PDF, `.docx` and the paginated view; the editor view keeps the name — _`tpt-typeset::resolve_bindings`, `tpt-codex-grid::{grid_bindings, resolve_document}`_
- [x] Excel range syntax in formulas (`B2:B9`, `$B$2:$B$9`) — _rewritten to `RANGE(B2, B9)`, which is what `tpt-lattice` parses; found because the starter templates' formulas were parse errors_
- [x] `tpt check` evaluates grids: formula errors and bindings with no value are errors
- [ ] Names for cells other than `sheet<id>.<address>`, and inline formulas in prose
- [ ] Query results as bindings (`[[q1]]`)

## Gaps found in the review, not built yet
- [x] `.cdx`: nested lists and block quotes — _done in schema version 3: `Block::Quote`, and a list item may be a sub-list. Markdown reads and writes both; the flow view, PDF and editor show them. Tables outside a grid (GFM tables) are not done_
- [ ] `.cdx`: tables outside a grid (GFM tables)
- [ ] The editor's paginated view shows list markers inline, without the indent of nested items or quoted blocks
- [ ] `.cdx`: link titles (`[t](url "title")`) — the inline run has no title field
- [x] `.docx` tables as grids (import and export) — _done: a table becomes a `Block::Grid` with its cells, numbers kept as numbers; merged cells are reported. Grids are written back as Word tables. Lists still lose their markers_
- [ ] `.docx` import: lists as lists, instead of plain paragraphs
- [x] `.pdf` import: the text, read in-house (`tpt-codex-interop/src/pdf/`) — _done: objects and cross-reference tables (classic, streams, object streams, incremental updates, a scan when the tables are damaged), Flate and ASCII filters, PNG predictors, fonts with ToUnicode maps or a Latin-1 guess, form XObjects. Paragraphs and headings from line spacing and type size. The report counts images, drawings, page numbers, emphasis and guessed characters. Encrypted files are refused. Checked on our own PDFs and on a Chromium PDF, with a seeded damage test and a fuzz target. Not done: bold and italic as marks, list structure, table structure, columns, scanned pages (no OCR), other stream filters (LZW, DCT, JBIG2)_
- [ ] Formula language: Excel function names and syntax beyond ranges. Imported `.xlsx` formulas are reported as approximated where the evaluator differs

## Adoption tooling
- [x] `tpt` CLI: `convert`, `check`, `watch`, `diff`, `merge`, `new` — _`crates/tpt-codex-cli`; templates are `article`, `report`, `slides`, `budget`_
- [x] Quickstart, `.cdx` cheat-sheet, migrating-from-Office guide — _`docs/`_
- [x] Release workflow (`.github/workflows/release.yml`: `tpt` and the relay for Linux, Windows and macOS on a `v*` tag, with checksums) — _written, not yet run on GitHub_
- [x] `scripts/build-web.sh` and `.ps1` (check the `wasm-bindgen` version, build, copy the page), `justfile`, `.devcontainer`
- [x] Relay stop on ctrl-c — _not needed: ctrl-c ends the process and the relay keeps no state on disk. No dependency added for it_
- [x] Web demo opens and saves real files, with autosave — _`tpt-codex-files` (bytes in, bytes out) behind `EditorHandle.open_file` and `save_file`; browser-tested in Chrome_
- [x] `.cdx` writer: whole-paragraph bold or italic, and bold+italic with `***` — _a flag is written as emphasis around the text and read back as a run; `docx` → `.cdx` now works for styled files_

---

## Explicit non-goals (V1 scope guards — do not build)
- No VBA/macro compatibility (use LES formulas or sandboxed WASM UDFs)
- No pixel-perfect legacy animation import (.pptx/.docx is data extraction only)
- No native video/audio editing or playback

## Open questions / spec inconsistencies to resolve
- [x] Unmatched `*` or `[` in Markdown: error, not literal. _Agreed. Files written before inline syntax was read can be migrated with `migrate_legacy_markdown` (or the `migrate_markdown` example), which escapes the markers and lists the changed lines._
- [x] Table header rows: per grid, not per document. _Agreed. `Sheet.header_rows`._
- [x] Sync transport and server: WebSocket relay first, WebRTC later. _Agreed. Relay logic is in place; the WebSocket library and host are still to decide._
- [x] Editor input: a native input element mapped onto the caret model, not per-keystroke messages. _Agreed. The model side (`ReplaceText`) is in place; browser wiring is to verify._
- [x] Version history: keep the in-house `VersionLog` until `tpt-version` is unblocked. _Agreed._
- [x] Flow layout: keep the in-house engine, not `tpt-layout`. _Agreed._
- [x] Grid merge: keep the in-house register CRDT, not `tpt-lattice-crdt`. _Agreed._
- [ ] Pin TPT git dependencies to pushed commits, and create the GitHub repo. _Agreed in principle; needs your go-ahead, since pushing and creating a repo are outward-facing._
- [x] `Block::Query` is referenced but not defined in the enum — now defined
- [x] Spec crate names differ from the real repos (`tpt-lattice-import` → `tpt-lattice-import-xlsx`; `tpt-canvas-pdf` lives in `tpt-canvas`) — recorded in `docs/tpt-dependency-map.md`
- [x] `tpt-typeset` was marked "no_std: ✅" with deps on `tpt-layout`/`tpt-canvas` — _resolved: `tpt-typeset` now depends on `tpt-query` (json) instead, and builds for `wasm32-unknown-unknown` and `thumbv7m-none-eabi`. `tpt-layout`/`tpt-canvas` are not used yet_
- [x] "Zero Apache-only" rule vs. TPT repos showing "Apache-2.0" on GitHub — _verified: every TPT crate in the resolved closure is `MIT OR Apache-2.0` in its manifest, and the gate passes. Re-verify on every dep bump._
- [x] Phase 2 PDF milestone depends on Phase 3 — _decided (recommendation): Milestone 2 is accepted as Markdown, math and outline; PDF export stays in Phase 3. Upstream glyph outlines are now pinned (`tpt-canvas` `e451560`)._
- [x] WASM UDF sandbox: mentioned in non-goals but not scheduled — _decided (recommendation): out of v1, as the non-goals list says. Revisit after v1._
- [x] Font decision: bundle a fixed font set in the binary and never use system fonts, so layout is the same everywhere. Noto Sans (OFL) Regular and Italic, bold by weight. _Recommendation applied; the open question had not been written down, and `tpt-text`'s entry referred to it._
- [x] Footnote definitions in the flow view: kept as editable body text. The paginated view and the PDF draw them as notes at the foot of the page. _Decided by following the recommendation: no editing path for note text was added._
- [x] Spec marks `tpt-codex-app` as "WASM-first" — decide native desktop story — _decided (recommendation): WASM-first, as the spec says. A native shell comes later, through `tpt-appfront` webview._
