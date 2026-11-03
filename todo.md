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
- [ ] Add as git deps pinned by `rev` (crates.io publication unconfirmed — re-check, then switch to versions)
- [ ] `tpt-model` (check `*-stub` crates don't leak into our dep tree)
- [ ] `tpt-text`
- [ ] `tpt-layout`
- [ ] `tpt-canvas` + `tpt-canvas-pdf` (PDF backend is a crate inside the `tpt-canvas` repo, not a separate repo)
- [ ] `tpt-lattice` workspace crates: `-core`, `-evaluator`, `-parser`, `-crdt`, `-wasm`
- [ ] `tpt-lattice-import-xlsx` / `-export-xlsx` (spec's "tpt-lattice-import" = these)
- [ ] `tpt-replicate`
- [ ] `tpt-appfront` crates: `-core`, `-dom`, `-canvas`, `-html`, `-webview` (pick which are needed)
- [ ] `tpt-doc` crates: `-core`, `-word`, `-spreadsheet`, `-pdf`, `-convert`
- [ ] `tpt-query`
- [ ] `tpt-version` (for semantic diff/merge)
- [ ] Audit each for `no_std` support, wasm32 support, and `-sys`/C-FFI transitive deps
- [ ] Audit transitive licenses (`cargo tree`, `cargo deny`)

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
- [ ] `tpt-model` schema + versioning for the Block tree
- [x] Basic serialization / deserialization (serde JSON, deterministic `BTreeMap` order)
- [x] DAG invariants (no cycles, dangling `BlockId`s) + validation (`Document::validate`)
- [x] Unit + property tests for model — `tests/model.rs` and randomized `tests/properties.rs`

**Milestone 0:** CI is green, license gate demonstrably blocks an Apache-only dep, `Block` tree round-trips through serialization.

---

## Phase 1 — Hybrid Document MVP (Weeks 3-6)

- [ ] `tpt-codex-app` shell on `tpt-appfront`, builds to WASM
- [ ] Core ⇄ UI reactive wiring (tree change → projection update)
- [ ] **Flow View** in `tpt-codex-views`: shape text via `tpt-text`, lay out via `tpt-layout` (block/inline flow, margin collapsing, typographic hierarchy)
- [ ] Text editing: caret, selection, input, undo/redo
- [ ] `TextStyle` editing (bold/italic/headings/lists etc.)
- [ ] **Grid View**: swap to `tpt-lattice` canvas/WebGL renderer when `Block::Grid` is focused
- [ ] Cell editing + LES formula entry + real-time evaluation (`tpt-lattice-evaluator`)
- [ ] Inline embed of a live grid in text flow (`view_bounds`), plus full-sheet reference
- [ ] Insert-table command in the UI
- [ ] `Block::Query` evaluation via `tpt-query`
- [ ] `Block::Media` rendering (images, SVG, `tpt-canvas` drawing commands) with alt text
- [ ] `Block::Container` rendering for Flow/Grid layout modes
- [ ] Trait boundaries for platform I/O, rendering, clock, storage (keep core headless)
- [ ] Local persistence (save/load document)
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
- [ ] Define & document the grammar formally (spec examples are not exhaustive)
- [x] Structured parse errors: line/column + fix suggestion (for the supported subset; column is 1 for line-level errors)
- [ ] Round-trip + snapshot tests; fuzz the parser — _round-trip tests done (including randomized); snapshots and fuzzing not yet_

### tpt-typeset
- [ ] Semantic AST: chapters, sections, footnotes, citations, TOC, cross-refs
- [ ] AST → `tpt-layout` BoxTree conversion
- [ ] TOC generation
- [ ] Cross-reference resolution (headings, figures, tables, equations)
- [ ] Footnote placement
- [ ] Citations + bibliography (decide source format, e.g. BibTeX/CSL-JSON)
- [ ] Document classes (`article` etc.) as a typed, declarative schema (no macro language)
- [ ] Typography: kerning, ligatures, hyphenation (via `tpt-text`)
- [ ] **Math layout engine** (spec references a "new typesetting crate" — not in the crate list): parse LaTeX-style math, box/glue layout, fonts with MATH table
- [ ] Better math error messages
- [ ] Data binding: embed `tpt-lattice` grids, `tpt-query` results, variables
- [ ] Structured, actionable errors ("Line 42: unclosed environment")
- [ ] Incremental compilation / watch mode (via `tpt-layout` incremental relayout)
- [ ] WASM in-browser live preview
- [ ] Semantic diff/merge integration via `tpt-version`

**Milestone 2:** A `.cdx` file with Markdown, math, and cross-references exports to a beautifully typeset PDF.
(Note: depends on the PDF backend in Phase 3 — either reorder, or ship a minimal `tpt-canvas-pdf` hookup here.)

---

## Phase 3 — PDF Pipeline (Weeks 7-10)

- [ ] **Paginated View**: wire `tpt-layout` pagination to the hybrid document
- [ ] Orphan/widow control
- [ ] Repeated table headers across pages
- [ ] Page size, margins, headers/footers, page numbers
- [ ] Render grids into pages (page breaking for large grids)
- [ ] Output drawing commands to `tpt-canvas-pdf`
- [ ] Font embedding / subsetting for PDF
- [ ] One-click PDF export in the app
- [ ] Byte-exact determinism: fixed timestamps/IDs, stable object ordering
- [ ] Accessible PDF basics (tags, alt text from `Media.alt_text`) — optional stretch
- [ ] Golden-file PDF tests

**Milestone 3:** One-click export of the Phase 1 hybrid document to a byte-exact, paginated PDF.

---

## Phase 4 — Canvas & Collaboration (Weeks 11-14)

### Canvas View (slides)
- [ ] `Container` with `LayoutMode::Absolute` and fixed aspect ratio (16:9 etc.)
- [ ] Explicit X/Y positioning of children, render via `tpt-canvas`
- [ ] Slide navigation, presenter/fullscreen mode
- [ ] Slide ordering / sections
- [ ] Select / move / resize of canvas children
- [ ] Slides → PDF export

### Collaboration (`tpt-replicate`)
- [ ] CRDT operations on the Block tree (insert/delete/move/update)
- [ ] Concurrent text editing semantics for `Block::Text`
- [ ] Merge live grids (`tpt-lattice-crdt`) with the document CRDT
- [ ] Move-operation conflict handling (DAG must stay acyclic)
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

- [ ] Golden/byte-identical render tests: native vs WASM vs PDF
- [ ] Determinism CI job (run on Linux/Windows/macOS and compare hashes)
- [ ] Property tests for CRDT convergence
- [ ] Fuzzing: Markdown/.cdx parser, math parser, deserializer
- [ ] Benchmarks: layout, eval, large documents
- [ ] Test fixtures / sample documents repo dir
- [ ] Per-phase milestone acceptance tests

## Release (minimal)
- [ ] Versioning policy (`0.1.0-spec` → `0.1.0`)
- [ ] Publish crates once all TPT deps are on crates.io (git deps can't be published)

---

## Explicit non-goals (V1 scope guards — do not build)
- No VBA/macro compatibility (use LES formulas or sandboxed WASM UDFs)
- No pixel-perfect legacy animation import (.pptx/.docx is data extraction only)
- No native video/audio editing or playback

## Open questions / spec inconsistencies to resolve
- [x] `Block::Query` is referenced but not defined in the enum — now defined
- [x] Spec crate names differ from the real repos (`tpt-lattice-import` → `tpt-lattice-import-xlsx`; `tpt-canvas-pdf` lives in `tpt-canvas`) — recorded in `docs/tpt-dependency-map.md`
- [ ] `tpt-typeset` was marked "no_std: ✅" with deps on `tpt-layout`/`tpt-canvas` — confirm those are `no_std`
- [ ] "Zero Apache-only" rule vs. TPT repos showing "Apache-2.0" on GitHub — all verified dual in Cargo.toml; re-verify on every dep bump
- [ ] Phase 2 PDF milestone depends on Phase 3
- [ ] WASM UDF sandbox: mentioned in non-goals but not scheduled
- [ ] Spec marks `tpt-codex-app` as "WASM-first" — decide native desktop story
