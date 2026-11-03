# TPT dependency map

Ecosystem map of the `tpt-solutions` crates that tpt-codex depends on. Every crate is expected to be
`MIT OR Apache-2.0`. Verify each entry against its `Cargo.toml` before depending on it.

**Status:** planning document. None of these crates are added as dependencies yet. Each is still
unverified against the audit items in [todo.md](../todo.md).

| Crate(s) | Role in tpt-codex | Used by | Verified? |
| --- | --- | --- | --- |
| `tpt-model` | Schema and model primitives (`*-stub` crates must not leak into the tree) | core | no |
| `tpt-text` | Text shaping | views, typeset | no |
| `tpt-layout` | Flow layout, pagination, box trees | views, typeset | no |
| `tpt-canvas`, `tpt-canvas-pdf` | Drawing commands; the PDF backend lives inside the `tpt-canvas` repo | views, typeset, app | no |
| `tpt-lattice-core`, `-evaluator`, `-parser`, `-crdt`, `-wasm` | Live grids and LES formulas | core, views | no |
| `tpt-lattice-import-xlsx`, `tpt-lattice-export-xlsx` | `.xlsx` import/export (spec's "tpt-lattice-import") | interop | no |
| `tpt-replicate` | Offline-first CRDT sync | core (phase 4) | no |
| `tpt-appfront-core`, `-dom`, `-canvas`, `-html`, `-webview` | UI shell (pick the subset needed) | app | no |
| `tpt-doc-core`, `-word`, `-spreadsheet`, `-pdf`, `-convert` | Office format import | interop | no |
| `tpt-query` | `Block::Query` evaluation | core, views | no |
| `tpt-version` | Semantic diff and merge | core (phase 4) | no |

## Checks required before adoption

- `no_std` support, and `wasm32-unknown-unknown` support.
- No `-sys` or C-FFI transitive dependencies.
- Full transitive license audit (`cargo tree`, `cargo deny check licenses`).
- Pin by `rev` until the crates are published to crates.io.

## Naming corrections against the spec

- `tpt-lattice-import` is `tpt-lattice-import-xlsx` / `tpt-lattice-export-xlsx`.
- `tpt-canvas-pdf` is inside the `tpt-canvas` repository.
