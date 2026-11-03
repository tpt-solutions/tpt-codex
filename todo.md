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
- [ ] Add as git deps pinned by `rev` (crates.io publication unconfirmed — re-check, then switch to versions) — _not added: audit found blockers (see docs/tpt-dependency-audit.md)_
- [ ] `tpt-model` (check `*-stub` crates don't leak into our dep tree) — _audited: `tpt-crypto-stub` leaks `blake3` (CC0/MIT-0 alternatives) into the closure; blocked_
- [ ] `tpt-text` — _audited: `self_cell` is Apache-2.0 OR GPL-2.0-only; blocked_
- [ ] `tpt-layout` — _audited: 0 dependencies, license clean; not yet added (no_std build unverified)_
- [ ] `tpt-canvas` + `tpt-canvas-pdf` (PDF backend is a crate inside the `tpt-canvas` repo, not a separate repo) — _audited: license clean, no `-sys`; not yet added_
- [ ] `tpt-lattice` workspace crates: `-core`, `-evaluator`, `-parser`, `-crdt`, `-wasm` — _audited `-core` and `-evaluator`: clean; `-parser`, `-crdt`, `-wasm` not audited_
- [ ] `tpt-lattice-import-xlsx` / `-export-xlsx` (spec's "tpt-lattice-import" = these)
- [ ] `tpt-replicate` — _audited: 0 dependencies, license clean; not yet added_
- [ ] `tpt-appfront` crates: `-core`, `-dom`, `-canvas`, `-html`, `-webview` (pick which are needed) — _not audited_
- [ ] `tpt-doc` crates: `-core`, `-word`, `-spreadsheet`, `-pdf`, `-convert` — _not audited_
- [ ] `tpt-query` — _audited: 0 dependencies, license clean; not yet added_
- [ ] `tpt-version` (for semantic diff/merge) — _audited: license clean, no `-sys`; not yet added_
- [ ] Audit each for `no_std` support, wasm32 support, and `-sys`/C-FFI transitive deps — _`-sys` closure checked for TPT crates; `no_std`/wasm builds not verified (needs approval to compile external code)_
- [ ] Audit transitive licenses (`cargo tree`, `cargo deny`) — _gate run on the resolved TPT closures; `cargo deny` not run (not installed)_

### Strict licensing CI gate
- [x] `deny.toml`: allow MIT, 0BSD, BSD-2-Clause, BSD-3-Clause, Unicode-DFS-2016, Unicode-3.0, Zlib (Apache-2.0 omitted: `X OR Apache-2.0` passes via X)
- [x] `deny.toml`: deny GPL-2.0, GPL-3.0, AGPL-3.0, SSPL, BSL, Apache-2.0-only — _`cargo deny` itself not run locally (not installed)_
- [x] Ban `*-sys` crates in core + views crates (script checks the dependency closure)
- [x] Script: parse `Cargo.lock` / `cargo metadata` and fail on any new Apache-2.0-only crate (`scripts/check_licenses.py`)
- [x] Script tests (fixtures with an Apache-only crate must fail) — `scripts/test_check_licenses.py`
- [x] Decide handling of `Apache-2.0 WITH LLVM-exception` and `Unicode-3.0` — Unicode-3.0 allowed (serde derive needs it); WITH-exceptions rejected. _Needs maintainer sign-off._

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
- [ ] `tpt-model` schema + versioning for the Block tree — _partial: own `schema_version` in core; tpt-model not used_
- [x] Basic serialization / deserialization (serde JSON, deterministic `BTreeMap` order)
- [x] DAG invariants (no cycles, dangling `BlockId`s) + validation (`Document::validate`)
- [x] Unit + property tests for model — `tests/model.rs` and randomized `tests/properties.rs`

**Milestone 0:** CI is green, license gate demonstrably blocks an Apache-only dep, `Block` tree round-trips through serialization.

---

## Phase 1 — Hybrid Document MVP (Weeks 3-6)

- [ ] `tpt-codex-app` shell on `tpt-appfront`, builds to WASM — _blocked on tpt-appfront audit_
- [ ] Core ⇄ UI reactive wiring (tree change → projection update) — _partial: `Op`, `History` and change inputs exist; no UI or subscription layer_
- [ ] **Flow View** in `tpt-codex-views`: shape text via `tpt-text`, lay out via `tpt-layout` (block/inline flow, margin collapsing, typographic hierarchy) — _partial: flow layout with injected `TextMeasure` and greedy wrap; not `tpt-text`/`tpt-layout`, no margin collapsing_
- [ ] Text editing: caret, selection, input, undo/redo — _partial: undo/redo and character-range edits in core; no caret, selection or input UI_
- [ ] `TextStyle` editing (bold/italic/headings/lists etc.) — _partial: bold/italic/heading via `SetBlock`; lists not in the model_
- [ ] **Grid View**: swap to `tpt-lattice` canvas/WebGL renderer when `Block::Grid` is focused
- [ ] Cell editing + LES formula entry + real-time evaluation (`tpt-lattice-evaluator`)
- [ ] Inline embed of a live grid in text flow (`view_bounds`), plus full-sheet reference
- [ ] Insert-table command in the UI
- [ ] `Block::Query` evaluation via `tpt-query`
- [ ] `Block::Media` rendering (images, SVG, `tpt-canvas` drawing commands) with alt text
- [ ] `Block::Container` rendering for Flow/Grid layout modes — _partial: Flow done; Grid and Absolute render as placeholder boxes_
- [x] Trait boundaries for platform I/O, rendering, clock, storage (keep core headless) — _`Clock`, `Storage`, `TextMeasure` traits; rendering via `TextMeasure` only_
- [x] Local persistence (save/load document) — _JSON via `DocumentStore` over `FileStorage`; validated on save and load_
- [ ] Native desktop shell smoke test (via appfront webview/tui, if used)

**Milestone 1:** A user types a paragraph, inserts a live calculation table, and it updates reactively in the browser via WASM.

---

## Phase 2 — Markdown & Typesetting (Weeks 6-8)

### tpt-codex-markdown
- [x] `from_markdown(&str) -> Result<Document, ParseError>` — front matter, headings, paragraphs; other syntax is a structured error
- [x] `to_markdown(&Document) -> Result<String, SerializeError>` — refuses anything it cannot write back exactly
- [ ] CommonMark mapping: headings, paragraphs _(done)_; lists, code blocks, links, emphasis, images _(not yet — needs model variants or inline support)_
- [ ] Codex-Flavored Markdown: `::: grid { id="..." }` blocks (table body → lattice sheet)
- [ ] `::: canvas { ratio="16:9" }` slide containers
- [ ] `$$ ... $$` math blocks (+ inline math)
- [ ] Citations `[@key]`, cross-refs `[#id]`, figure attrs `{#fig:id width=80%}`
- [x] `.cdx` ("Codex Script") format: front matter (title, author, date, class) — simple `key: value` subset of YAML, not full YAML
- [x] Define & document the grammar formally (spec examples are not exhaustive) — _`docs/markdown-grammar.md`_
- [x] Structured parse errors: line/column + fix suggestion (for the supported subset; column is 1 for line-level errors)
- [ ] Round-trip + snapshot tests; fuzz the parser — _round-trip, randomized and golden snapshot tests done; fuzzing not yet (smoke test only)_

### tpt-typeset
- [ ] Semantic AST: chapters, sections, footnotes, citations, TOC, cross-refs — _partial: heading outline and numbering; no footnotes, citations or chapter objects_
- [ ] AST → `tpt-layout` BoxTree conversion
- [x] TOC generation — _`Outline::toc` with depth limit_
- [ ] Cross-reference resolution (headings, figures, tables, equations) — _partial: heading anchors resolve; figures, tables and equations not in the model_
- [ ] Footnote placement
- [ ] Citations + bibliography (decide source format, e.g. BibTeX/CSL-JSON)
- [x] Document classes (`article` etc.) as a typed, declarative schema (no macro language) — _`DocumentClass` (article, report) typed schema; not yet used by layout_
- [ ] Typography: kerning, ligatures, hyphenation (via `tpt-text`)
- [ ] **Math layout engine** (spec references a "new typesetting crate" — not in the crate list): parse LaTeX-style math, box/glue layout, fonts with MATH table — _partial: parser only (`parse_math`); layout needs `tpt-text`_
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
- [ ] Output drawing commands to `tpt-canvas-pdf`
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
- [ ] CRDT operations on the Block tree (insert/delete/move/update) — _partial: `Op` insert, delete, move and update with inverses; not a CRDT_
- [ ] Concurrent text editing semantics for `Block::Text`
- [ ] Merge live grids (`tpt-lattice-crdt`) with the document CRDT
- [ ] Move-operation conflict handling (DAG must stay acyclic) — _partial: local moves reject cycles; concurrent conflict handling not designed_
- [ ] Offline-first sync + reconnect
- [ ] Transport layer (WebSocket/WebRTC) behind a trait; sync server decision
- [ ] Presence / cursors
- [ ] Version history via `tpt-version`

**Milestone 4:** Multi-user collaborative editing of a document containing live grids.

---

## Phase 5 — Interop (after v1; spec lists crate but no schedule)

- [ ] `.docx` import via `tpt-doc-word` → Block tree (best-effort data extraction)
- [ ] `.xlsx` import via `tpt-lattice-import-xlsx` → `Block::Grid`
- [ ] `.pdf` text/structure extraction via `tpt-doc-pdf`
- [ ] `.pptx` import (data only, no animations)
- [ ] Export adapters: `.docx`, `.xlsx`
- [ ] Import fidelity report (what was dropped/approximated)
- [ ] Corpus-based tests with sample real-world files
- [ ] Verify no new Apache-only transitive deps arrive via interop

**Milestone 5:** Open a real `.docx`/`.xlsx` and get a usable editable document.

---

## Cross-cutting: Testing & Determinism

- [ ] Golden/byte-identical render tests: native vs WASM vs PDF — _partial: golden JSON compared across OSes in CI; no WASM or PDF comparison_
- [x] Determinism CI job (run on Linux/Windows/macOS and compare hashes) — _in `.github/workflows/ci.yml`; not yet run on GitHub_
- [ ] Property tests for CRDT convergence
- [ ] Fuzzing: Markdown/.cdx parser, math parser, deserializer — _smoke tests only (random input, no panics); no fuzz harness_
- [ ] Benchmarks: layout, eval, large documents — _partial: layout and pagination benchmark (`examples/bench_layout.rs`); no eval benchmark_
- [x] Test fixtures / sample documents repo dir — _`fixtures/` with a sample and its golden JSON_
- [ ] Per-phase milestone acceptance tests — _partial: Milestone 0 covered; end-to-end headless test added; Milestones 1 to 5 blocked_

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
- [ ] `tpt-typeset` was marked "no_std: ✅" with deps on `tpt-layout`/`tpt-canvas` — confirm those are `no_std` — _still open: the TPT crates it names are not built here_
- [ ] "Zero Apache-only" rule vs. TPT repos showing "Apache-2.0" on GitHub — all verified dual in Cargo.toml; re-verify on every dep bump — _partial: audit done; three blockers listed in docs/tpt-dependency-audit.md_
- [ ] Phase 2 PDF milestone depends on Phase 3
- [ ] WASM UDF sandbox: mentioned in non-goals but not scheduled
- [ ] Spec marks `tpt-codex-app` as "WASM-first" — decide native desktop story
