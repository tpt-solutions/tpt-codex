# tpt-codex TODO

License: MIT OR Apache-2.0 · Copyright © 2026 TPT Solutions · Zero Apache-2.0-only dependencies.
Source spec: `spec.txt`. Week estimates are from the spec (solo maintainer).

Legend: `[ ]` todo · `[x]` done

---

## Phase 0 — Foundation & Gates (Weeks 1-2)

### Repo & licensing
- [ ] `git init`, create GitHub repo under `tpt-solutions`, add `.gitignore`, `rust-toolchain.toml`, `rustfmt.toml` — _local parts done; GitHub repo not yet created_
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
- [ ] Add as git deps pinned by `rev` (crates.io publication unconfirmed — re-check, then switch to versions) — _added for the crates in use: `tpt-query`, `tpt-lattice-core`, `tpt-lattice-evaluator` (pinned in `Cargo.toml`). The rest are pinned in `docs/tpt-dependency-map.md`, not yet wired in._
- [ ] `tpt-model` (check `*-stub` crates don't leak into our dep tree) — _blocked upstream: `tpt-crypto-stub`, `tpt-primitives-stub`, `tpt-uir-stub` and `tpt-validex-stub` are unconditional normal dependencies of `tpt-model`. Fix in the `tpt-model` repo (feature-gate or drop them). Its `blake3` leak is already gone on GitHub master_
- [ ] `tpt-text` — _self_cell fix is on GitHub master (`83bc13c`). Not wired in: needs the font decision (see open questions)_
- [ ] `tpt-layout` — _gate clean (0 dependencies). Not wired in: flow text needs `tpt-text` and a font decision_
- [ ] `tpt-canvas` + `tpt-canvas-pdf` (PDF backend is a crate inside the `tpt-canvas` repo, not a separate repo) — _not wired in: `tpt-canvas-pdf` writes glyph runs as placeholder comments, so PDF output would have no text until upstream draws them_
- [ ] `tpt-lattice` workspace crates: `-core`, `-evaluator`, `-parser`, `-crdt`, `-wasm` — _`-core` and `-evaluator` added (grid evaluation in `tpt-codex-grid`; the evaluator is std-only, so it is not in `no_std` crates). `-parser` comes in transitively. `-crdt` and `-wasm` not audited_
- [x] `tpt-lattice-import-xlsx` / `-export-xlsx` (spec's "tpt-lattice-import" = these) — _replaced by our own stack in `tpt-codex-interop`: `archive` (ZIP reader and writer, stored and DEFLATE, CRC and size limits) and `xlsx` (reader and writer, on `quick-xml`). `zip`, `calamine` and the lattice xlsx crates are no longer used. Tested against openpyxl and Python `zipfile` fixtures, both directions_
- [ ] `tpt-replicate` — _gate clean, but its CRDT types (`UirNode`, `ActorId`, `CrdtCompatible`) come from its own `stubs` module. Not adopted: a stub type would enter the sync format. Text CRDT written in `tpt-codex-core` instead (see Phase 4)_
- [ ] `tpt-appfront` crates: `-core`, `-dom`, `-canvas`, `-html`, `-webview` (pick which are needed) — _audited per crate: `-core`, `-dom`, `-html` gate OK and in use. `-canvas` and `-webview` fail the gate (Apache-only GUI stacks: winit, glutin, tao). They are native-desktop crates, outside the WASM-first decision_
- [ ] `tpt-doc` crates: `-core`, `-word`, `-spreadsheet`, `-pdf`, `-convert` — _gate results: `-core`, `-pdf`, `-convert` OK. `-word` passes once `zip` uses `deflate-flate2` instead of `deflate` (commit `5a37c6f` on branch `gate-zip-deflate-flate2` in `tpt-solutions/tpt-doc`, also as a patch in the session scratchpad). The push was refused: this account has no write access to `tpt-doc`. Until the branch is pushed, `-word` cannot be pinned, and it is not needed. `-spreadsheet` also needs `ryu` via `csv`, and is not needed (xlsx is in-house)_
- [x] `tpt-query` — _added: `tpt-typeset::run_query` runs `Block::Query` JSON with the `json` feature. Gate clean, `no_std` build verified_
- [ ] `tpt-version` (for semantic diff/merge) — _gate clean; depends on `tpt-crypto-hash`/`-sig` (git). Not wired in: Phase 4_
- [ ] Audit each for `no_std` support, wasm32 support, and `-sys`/C-FFI transitive deps — _`-sys` closure checked for TPT crates; `no_std`/wasm builds not verified (needs approval to compile external code)_
- [ ] Audit transitive licenses (`cargo tree`, `cargo deny`) — _license gate run on the resolved closure (31 packages): OK. `cargo deny` not run (not installed). `deny.toml` now allows the two pinned TPT git sources._

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
- [ ] `tpt-model` schema + versioning for the Block tree — _partial: own `schema_version` in core; tpt-model not used_
- [x] Basic serialization / deserialization (serde JSON, deterministic `BTreeMap` order)
- [x] DAG invariants (no cycles, dangling `BlockId`s) + validation (`Document::validate`)
- [x] Unit + property tests for model — `tests/model.rs` and randomized `tests/properties.rs`

**Milestone 0:** CI is green, license gate demonstrably blocks an Apache-only dep, `Block` tree round-trips through serialization.

---

## Phase 1 — Hybrid Document MVP (Weeks 3-6)

- [x] `tpt-codex-app` shell on `tpt-appfront`, builds to WASM — _editor state and UI tree (`Editor`, `Msg`) render to semantic HTML and are tested natively. `web::mount` mounts the editor with `tpt-appfront-dom`; the app builds for `wasm32-unknown-unknown` and CI checks it. Not yet run in a browser_
- [ ] Core ⇄ UI reactive wiring (tree change → projection update) — _partial: `History::last_changed` names the blocks each step changed. The editor re-renders from a signal after each message (`web::mount`). No per-block projection cache yet_
- [ ] **Flow View** in `tpt-codex-views`: shape text via `tpt-text`, lay out via `tpt-layout` (block/inline flow, margin collapsing, typographic hierarchy) — _partial: flow layout with injected `TextMeasure` and greedy wrap; not `tpt-text`/`tpt-layout`, no margin collapsing_
- [ ] Text editing: caret, selection, input, undo/redo — _partial: `Selection` (caret movement, insert, backspace, delete, each undoable) in core; `Editor` maps messages to these. Keyboard, IME and key bindings in the browser are not wired yet_
- [ ] `TextStyle` editing (bold/italic/headings/lists etc.) — _partial: bold/italic/heading via `SetBlock`; lists are in the model (`Block::List`) but there are no list-editing commands_
- [ ] **Grid View**: swap to `tpt-lattice` canvas/WebGL renderer when `Block::Grid` is focused — _partial: the editor shows an editable table (`data_grid` plus one input per cell). No canvas or WebGL renderer_
- [ ] Cell editing + LES formula entry + real-time evaluation (`tpt-lattice-evaluator`) — _partial: `Op::SetCell` (undoable); `GridSession` recomputes affected cells; the editor's `EditCell` message parses typed text (number, boolean, `=formula`, text) and shows the grid as a table. Cell focus and arrow keys work in the editor: each grid input has `on_focus` (`FocusCell`) and `on_keydown` (arrows become `MoveCell`). These need the appfront `web-events` work (merged to appfront master, not yet pushed); appfront is pinned to `5d5dfd6` (pushed to tpt-solutions/tpt-appfront master)_
- [ ] Inline embed of a live grid in text flow (`view_bounds`), plus full-sheet reference
- [x] Insert-table command in the UI — _"Insert table" button adds an empty 3×3 `Block::Grid` as one undoable step (`Op::SetSheet` + `AddBlock` + `InsertChild`); `Sheet` has a minimum `rows`/`cols`, so empty cells can be typed into. Table size is fixed at 3×3 until a size control exists_
- [ ] `Block::Query` evaluation via `tpt-query` — _partial: `tpt-typeset::run_query` runs JSON queries. `document_entities` maps each block to an entity (type ids in `entity_type`), so queries run over the document. No query UI or query-result rendering yet_
- [ ] `Block::Media` rendering (images, SVG, `tpt-canvas` drawing commands) with alt text
- [ ] `Block::Container` rendering for Flow/Grid layout modes — _partial: Flow done; Grid and Absolute render as placeholder boxes_
- [x] Trait boundaries for platform I/O, rendering, clock, storage (keep core headless) — _`Clock`, `Storage`, `TextMeasure` traits; rendering via `TextMeasure` only_
- [x] Local persistence (save/load document) — _JSON via `DocumentStore` over `FileStorage`; validated on save and load_
- [ ] Native desktop shell smoke test (via appfront webview/tui, if used) — _not started: `tpt-appfront-webview` fails the gate. WASM-first is the decision, so the browser run comes first_

**Milestone 1:** A user types a paragraph, inserts a live calculation table, and it updates reactively in the browser via WASM.

---

## Phase 2 — Markdown & Typesetting (Weeks 6-8)

### tpt-codex-markdown
- [x] `from_markdown(&str) -> Result<Document, ParseError>` — front matter, headings, paragraphs; other syntax is a structured error
- [x] `to_markdown(&Document) -> Result<String, SerializeError>` — refuses anything it cannot write back exactly
- [ ] CommonMark mapping: headings, paragraphs, lists, code blocks, thematic breaks, images _(done)_; links, emphasis _(not yet — needs inline runs in the model)_
- [ ] Codex-Flavored Markdown: `::: grid { id="..." }` blocks (table body → lattice sheet)
- [ ] `::: canvas { ratio="16:9" }` slide containers
- [ ] `$$ ... $$` math blocks (+ inline math) — _block math done: reader, writer and `Block::Math`; inline math not yet_
- [ ] Citations `[@key]`, cross-refs `[#id]`, figure attrs `{#fig:id width=80%}`
- [x] `.cdx` ("Codex Script") format: front matter (title, author, date, class) — simple `key: value` subset of YAML, not full YAML
- [x] Define & document the grammar formally (spec examples are not exhaustive) — _`docs/markdown-grammar.md`_
- [x] Structured parse errors: line/column + fix suggestion (for the supported subset; column is 1 for line-level errors)
- [ ] Round-trip + snapshot tests; fuzz the parser — _round-trip (incl. lists, code, images, math), randomized and golden snapshot tests done; a seeded mutation test runs 4,000 inputs by default (300,000 ran clean); no cargo-fuzz harness yet_

### tpt-typeset
- [ ] Semantic AST: chapters, sections, footnotes, citations, TOC, cross-refs — _partial: heading outline and numbering; no footnotes, citations or chapter objects_
- [ ] AST → `tpt-layout` BoxTree conversion
- [x] TOC generation — _`Outline::toc` with depth limit_
- [ ] Cross-reference resolution (headings, figures, tables, equations) — _partial: heading anchors resolve; figures, tables and equations not in the model_
- [ ] Footnote placement
- [ ] Citations + bibliography (decide source format, e.g. BibTeX/CSL-JSON)
- [x] Document classes (`article` etc.) as a typed, declarative schema (no macro language) — _`DocumentClass` (article, report) typed schema; not yet used by layout_
- [ ] Typography: kerning, ligatures, hyphenation (via `tpt-text`)
- [ ] **Math layout engine** (spec references a "new typesetting crate" — not in the crate list): parse LaTeX-style math, box/glue layout, fonts with MATH table — _partial: parser and a document-level check of math blocks (`check_math_blocks`); layout needs `tpt-text`_
- [x] Better math error messages — _line, column, message and hint on each error_
- [ ] Data binding: embed `tpt-lattice` grids, `tpt-query` results, variables
- [ ] Structured, actionable errors ("Line 42: unclosed environment") — _partial: Markdown and math errors carry line, column and hint_
- [ ] Incremental compilation / watch mode (via `tpt-layout` incremental relayout)
- [ ] WASM in-browser live preview
- [ ] Semantic diff/merge integration via `tpt-version`

**Milestone 2:** A `.cdx` file with Markdown, math, and cross-references exports to a beautifully typeset PDF.
(Note: depends on the PDF backend in Phase 3 — either reorder, or ship a minimal `tpt-canvas-pdf` hookup here.)

---

## Phase 3 — PDF Pipeline (Weeks 7-10)

- [ ] **Paginated View**: wire `tpt-layout` pagination to the hybrid document — _partial: pagination on the injected flow layout; not `tpt-layout`_
- [x] Orphan/widow control — _`min_lines` on both sides of a split paragraph_
- [ ] Repeated table headers across pages
- [x] Page size, margins, headers/footers, page numbers — _`PageGeometry`, `PaperSize` (A4, Letter), `render_chrome` with `{page}`/`{pages}`_
- [ ] Render grids into pages (page breaking for large grids) — _grids are fixed-height placeholder boxes; no page breaking inside grids_
- [ ] Output drawing commands to `tpt-canvas-pdf` — _blocked upstream: glyph runs are placeholders in `tpt-canvas-pdf`_
- [ ] Font embedding / subsetting for PDF
- [ ] One-click PDF export in the app
- [ ] Byte-exact determinism: fixed timestamps/IDs, stable object ordering — _partial: ordered maps, fixed clock and deterministic layout; no PDF output yet_
- [ ] Accessible PDF basics (tags, alt text from `Media.alt_text`) — optional stretch
- [ ] Golden-file PDF tests

**Milestone 3:** One-click export of the Phase 1 hybrid document to a byte-exact, paginated PDF.

---

## Phase 4 — Canvas & Collaboration (Weeks 11-14)

### Canvas View (slides)
- [ ] `Container` with `LayoutMode::Absolute` and fixed aspect ratio (16:9 etc.) — _blocked: the model has no child positions_
- [ ] Explicit X/Y positioning of children, render via `tpt-canvas`
- [ ] Slide navigation, presenter/fullscreen mode
- [ ] Slide ordering / sections
- [ ] Select / move / resize of canvas children
- [ ] Slides → PDF export

### Collaboration (`tpt-replicate`)
- [x] CRDT operations on the Block tree (insert/delete/move/update) — _move-operation tree CRDT in `tpt-codex-core` (`tree_crdt.rs`): create, update (last writer wins), move (Kleppmann et al. 2021 undo-do-redo), delete to trash. `project_document` turns a replica into a `Document`. Not yet wired into `History` or a transport_
- [x] Concurrent text editing semantics for `Block::Text` — _replicated growable array (RGA) of characters in `tpt-codex-core` (`text_crdt.rs`), with stamps, tombstones and causal delivery. `materialize` writes the converged text into the block as one undoable step. Not yet connected to a transport_
- [ ] Merge live grids (`tpt-lattice-crdt`) with the document CRDT
- [x] Move-operation conflict handling (DAG must stay acyclic) — _a move that would make a node its own ancestor is skipped on every replica. Late moves are applied in stamp order by undoing and redoing newer moves. Tested with a concurrent cycle and random delivery_
- [ ] Offline-first sync + reconnect
- [ ] Transport layer (WebSocket/WebRTC) behind a trait; sync server decision
- [ ] Presence / cursors
- [ ] Version history via `tpt-version`

**Milestone 4:** Multi-user collaborative editing of a document containing live grids.

---

## Phase 5 — Interop (after v1; spec lists crate but no schedule)

- [x] `.docx` import → Block tree (best-effort) — _in-house `docx` reader in `tpt-codex-interop`, on our `archive` module: paragraphs, headings (`Heading1`–`6`, `Title`), bold and italic when a whole paragraph has them, line and tab breaks, title and author. Tables are flattened, list items become paragraphs, images are dropped. Each loss is one fidelity item. Tested on a python-docx file. Chosen over `tpt-doc-word` because the need is small (see the tpt-doc note)_
- [ ] `.xlsx` import via `tpt-lattice-import-xlsx` → `Block::Grid` — _first worksheet imported into a `Sheet` with a `FidelityReport` (merges, other sheets, error values, shared formulas, named ranges). Dates and formats come in as plain numbers (styles are not read). Placing the sheet as a `Block::Grid` is left to the caller_
- [ ] `.pdf` text/structure extraction via `tpt-doc-pdf`
- [x] `.pptx` import (data only, no animations) — _in-house `pptx` reader: each slide becomes a level-2 heading (its title placeholder, or `Slide N`) and its text. Pictures, tables, speaker notes and formatting are reported. Tested on a python-pptx file_
- [ ] Export adapters: `.docx`, `.xlsx` — _`.xlsx` export of the first sheet (formulas with cached values, deterministic bytes), checked by openpyxl. `.docx` export (headings, bold and italic, breaks, title and author; other blocks written as their nearest paragraph and reported), checked by python-docx and by our reader_
- [x] Import fidelity report (what was dropped/approximated) — _`FidelityReport` in `tpt-codex-interop` with a text form; the docx, pptx and xlsx importers all fill it_
- [ ] Corpus-based tests with sample real-world files — _partial: files written by openpyxl, python-docx, python-pptx and Python's zipfile (`fixtures/office/`). No real-world corpus yet_
- [x] Verify no new Apache-only transitive deps arrive via interop — _the license gate runs on the resolved closure, which includes interop, and passes_

**Milestone 5:** Open a real `.docx`/`.xlsx` and get a usable editable document.

---

## Cross-cutting: Testing & Determinism

- [ ] Golden/byte-identical render tests: native vs WASM vs PDF — _partial: golden JSON compared across OSes in CI; no WASM or PDF comparison_
- [x] Determinism CI job (run on Linux/Windows/macOS and compare hashes) — _in `.github/workflows/ci.yml`; not yet run on GitHub_
- [x] Property tests for CRDT convergence — _text RGA (three replicas, 200 rounds; 20,000 ran clean) and block tree (150 rounds; 5,000 ran clean), with random edits, out-of-order delivery, and an acyclicity check after every step_
- [ ] Fuzzing: Markdown/.cdx parser, math parser, deserializer — _seeded mutation tests for all three: Markdown (`tpt-codex-markdown/tests/mutation.rs`), math (`tpt-typeset/tests/mutation.rs`) and document JSON (`tpt-codex-core/tests/json_mutation.rs`). Default runs are 4k, 20k and 5k inputs; 300k, 300k and 200k ran clean. No cargo-fuzz harness yet_
- [x] Benchmarks: layout, eval, large documents — _`tpt-codex-views/examples/bench_layout.rs` (layout and pagination) and `tpt-codex-grid/examples/bench_eval.rs` (full and incremental evaluation; 1,000 rows: 6.4 ms full, 0.9 ms after one input)_
- [x] Test fixtures / sample documents repo dir — _`fixtures/` with a sample and its golden JSON_
- [ ] Per-phase milestone acceptance tests — _partial: Milestone 0 covered; Milestone 1's headless path (typing with a caret, undo, equation block, save and reload) covered; its UI and Milestones 2 to 5 blocked_

## Release (minimal)
- [x] Versioning policy (`0.1.0-spec` → `0.1.0`) — _`docs/versioning.md`_
- [ ] Publish crates once all TPT deps are on crates.io (git deps can't be published)

---

## Explicit non-goals (V1 scope guards — do not build)
- No VBA/macro compatibility (use LES formulas or sandboxed WASM UDFs)
- No pixel-perfect legacy animation import (.pptx/.docx is data extraction only)
- No native video/audio editing or playback

## Open questions / spec inconsistencies to resolve
- [x] `Block::Query` is referenced but not defined in the enum — now defined
- [x] Spec crate names differ from the real repos (`tpt-lattice-import` → `tpt-lattice-import-xlsx`; `tpt-canvas-pdf` lives in `tpt-canvas`) — recorded in `docs/tpt-dependency-map.md`
- [x] `tpt-typeset` was marked "no_std: ✅" with deps on `tpt-layout`/`tpt-canvas` — _resolved: `tpt-typeset` now depends on `tpt-query` (json) instead, and builds for `wasm32-unknown-unknown` and `thumbv7m-none-eabi`. `tpt-layout`/`tpt-canvas` are not used yet_
- [x] "Zero Apache-only" rule vs. TPT repos showing "Apache-2.0" on GitHub — _verified: every TPT crate in the resolved closure is `MIT OR Apache-2.0` in its manifest, and the gate passes. Re-verify on every dep bump._
- [x] Phase 2 PDF milestone depends on Phase 3 — _decided (recommendation): Milestone 2 is accepted as Markdown, math and outline; PDF export stays in Phase 3. Blocked further by upstream glyph output in `tpt-canvas-pdf`._
- [x] WASM UDF sandbox: mentioned in non-goals but not scheduled — _decided (recommendation): out of v1, as the non-goals list says. Revisit after v1._
- [x] Spec marks `tpt-codex-app` as "WASM-first" — decide native desktop story — _decided (recommendation): WASM-first, as the spec says. A native shell comes later, through `tpt-appfront` webview._
