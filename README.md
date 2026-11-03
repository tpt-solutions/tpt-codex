# tpt-codex

A unified, deterministic, strictly-licensed document and data workspace.

tpt-codex replaces fragmented office suites with one headless data model. A document is a
tree (DAG) of structured blocks. The same data can be projected as a flowing document, a live
spreadsheet, a presentation canvas, or a byte-exact PDF, and it runs in the browser via
WebAssembly.

> **Status: Phase 0 (foundation).** The workspace, the licensing gate, and the core data model
> exist. There is no user-facing application yet. See [todo.md](todo.md) for the phased plan and
> [spec.txt](spec.txt) for the design specification.

## Design principles

- **Single source of truth.** Data is decoupled from presentation. The UI is a reactive projection of the block tree.
- **Deterministic rendering.** The same model yields the same output on native, WASM, and PDF.
- **Strict licensing.** Dependencies must be MIT, 0BSD, BSD-2/3-Clause, Unicode-DFS-2016, Unicode-3.0, Zlib, or dual-licensed with an MIT option. Apache-2.0-only, GPL, AGPL, SSPL and BSL are rejected by CI.
- **Headless, `no_std` core.** Core model, views, Markdown and typesetting are `no_std` + `alloc`. Platform I/O sits behind trait boundaries.

## Workspace layout

| Crate | Purpose | `no_std` |
| --- | --- | --- |
| `tpt-codex-core` | Block data model, IDs, validation, serialization | yes |
| `tpt-codex-views` | Projection engines: Flow, Grid, Canvas, Paginated | yes (scaffold) |
| `tpt-codex-markdown` | Markdown and Codex Script parser/serializer | yes (scaffold) |
| `tpt-typeset` | Semantic typesetting engine | yes (scaffold) |
| `tpt-codex-interop` | Import/export for `.docx`, `.xlsx`, `.pdf` | no (scaffold) |
| `tpt-codex-app` | WASM-first application shell | no (scaffold) |

## Building and testing

Requires a stable Rust toolchain (edition 2024, Rust 1.85+). `rust-toolchain.toml` installs the
required components and targets automatically.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# no_std / WASM builds of the headless crates
cargo build -p tpt-codex-core -p tpt-codex-views -p tpt-codex-markdown -p tpt-typeset --target thumbv7m-none-eabi
cargo build -p tpt-codex-core -p tpt-codex-views -p tpt-codex-markdown -p tpt-typeset --target wasm32-unknown-unknown

# Licensing and header gates
cargo metadata --format-version 1 | python3 scripts/check_licenses.py
python3 -m unittest scripts/test_check_licenses.py
python3 scripts/check_spdx.py
```

## License

Dual-licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option. Copyright © 2026 TPT Solutions.

### Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

Every Rust source file begins with the header `// SPDX-License-Identifier: MIT OR Apache-2.0`.
