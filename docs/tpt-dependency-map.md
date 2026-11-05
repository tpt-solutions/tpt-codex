# TPT dependency map

Ecosystem map of the `tpt-solutions` crates that tpt-codex depends on. Every crate is expected to be
`MIT OR Apache-2.0`. Verify each entry against its `Cargo.toml` before depending on it.

**Status:** planning document. None of these crates are added as dependencies yet. Each is still
unverified against the audit items in [todo.md](../todo.md).

| Crate(s) | Role in tpt-codex | Used by | Verified? |
| --- | --- | --- | --- |
| `tpt-model` | Schema and model primitives. **Blocked:** four `*-stub` crates are unconditional normal dependencies (see audit) | core | no |
| `tpt-text` | Text shaping, pinned `83bc13c`. **Wired in** through `tpt-codex-text`, with the bundled Noto Sans Regular (the font decision) | text (measure) | yes (gate) |
| `tpt-layout` | Box, flow and page layout, pinned `21361d1`. **Wired in** through `tpt-codex-text` (its `TextMeasurer`). The Flow view does not use its engine yet | text | yes (gate) |
| `tpt-canvas`, `tpt-canvas-pdf` | Drawing commands; the PDF backend lives inside the `tpt-canvas` repo. Not wired in: PDF glyph runs are placeholders upstream | views, typeset, app | no |
| `tpt-lattice-core`, `-evaluator` | Live grids and LES formulas. **Wired in** (`tpt-codex-grid`, pinned `d723321`). The evaluator is std-only | grid | yes (gate) |
| `tpt-lattice-import-xlsx`, `-export-xlsx` | Replaced by in-house `archive` and `xlsx` in `tpt-codex-interop`; not used | — | no |
| `tpt-lattice-parser`, `-crdt`, `-wasm` | Pulled in by the evaluator (parser); the others are not used yet | — | parser: gate only |
| `tpt-replicate` | Offline-first CRDT sync. Gate clean, but its CRDT types come from its own `stubs` module, so not adopted. Text CRDT written in `tpt-codex-core` instead | core (phase 4) | no |
| `tpt-appfront-core`, `-html`, `-dom` | UI tree, HTML rendering, and WASM DOM mount. **Wired in** (`tpt-codex-app`, pinned `5d5dfd6`) | app | yes (gate) |
| `tpt-appfront-canvas`, `-webview` | Native desktop backends. Fail the gate (GUI stacks) | — | no |
| `tpt-doc-word`, `-spreadsheet` | Office import. Fail the gate (`zopfli`, and `ryu` for spreadsheet) | — | no |
| `tpt-doc-core`, `-pdf`, `-convert` | Office document crates. Gate OK, not yet wired in | — | gate only |
| `tpt-query` | `Block::Query` evaluation (json feature). **Wired in** (`tpt-typeset`, pinned `6b06d02`) | typeset | yes (gate, no_std) |
| `tpt-version` | Semantic diff and merge. Gate clean; depends on `tpt-crypto` | core (phase 4) | gate only |

## Checks required before adoption

- `no_std` support, and `wasm32-unknown-unknown` support.
- No `-sys` or C-FFI transitive dependencies.
- Full transitive license audit (`cargo tree`, `cargo deny check licenses`).
- Pin by `rev` until the crates are published to crates.io.

## Naming corrections against the spec

- `tpt-lattice-import` is `tpt-lattice-import-xlsx` / `tpt-lattice-export-xlsx`.
- `tpt-canvas-pdf` is inside the `tpt-canvas` repository.
