# tpt-codex

A unified, deterministic, strictly-licensed document and data workspace.

tpt-codex replaces fragmented office suites with one headless data model. A document is a
tree (DAG) of structured blocks. The same data can be projected as a flowing document, a live
spreadsheet, a presentation canvas, or a byte-exact PDF, and it runs in the browser via
WebAssembly.

> **Status: pre-release (`0.1.0-spec`).** The data model, Markdown/`.cdx` reader and writer,
> typesetting, paginated PDF export, collaboration (CRDT and relay), and `.docx`/`.xlsx`/`.pptx`
> interop are implemented and tested. The browser editor is a demo on a sample document. See
> [todo.md](todo.md) for what is left and [spec.txt](spec.txt) for the design specification.

## Design principles

- **Single source of truth.** Data is decoupled from presentation. The UI is a reactive projection of the block tree.
- **Deterministic rendering.** The same model yields the same output on native, WASM, and PDF.
- **Strict licensing.** Shipped dependencies must be MIT, 0BSD, BSD-2/3-Clause, Unicode-DFS-2016, Unicode-3.0, Zlib, or dual-licensed with an MIT option. Apache-2.0-only, GPL, AGPL, SSPL and BSL are rejected by CI. Development-only dependencies (tests, benchmarks, examples) are not checked.
- **Headless, `no_std` core.** Core model, views, Markdown and typesetting are `no_std` + `alloc`. Platform I/O sits behind trait boundaries.

## Workspace layout

| Crate | Purpose | `no_std` |
| --- | --- | --- |
| `tpt-codex-core` | Block data model, undo history, CRDT replicas, version log | yes |
| `tpt-codex-views` | Flow, Paginated and Canvas (slide) layout, hyphenation, SVG slides | yes |
| `tpt-codex-markdown` | Markdown and `.cdx` reader/writer | yes |
| `tpt-typeset` | Outline, math, citations, footnotes, cross-references, queries, bindings | yes |
| `tpt-codex-grid` | Live grid evaluation with `tpt-lattice` | no |
| `tpt-codex-text` | Bundled fonts and text measurement with `tpt-text` | no |
| `tpt-codex-pdf` | Deterministic PDF export of documents and slides | no |
| `tpt-codex-interop` | `.docx`, `.xlsx`, `.pptx` import and `.docx`, `.xlsx` export | no |
| `tpt-codex-cli` | The `tpt` command: convert, check, watch, diff, new | no |
| `tpt-codex-app` | Editor state and UI tree on `tpt-appfront` | no |
| `tpt-codex-web` | Browser entry point (wasm) and demo page | no |
| `tpt-codex-sync` | WebSocket transport and `tpt-codex-relay` server | no |
| `tpt-codex-acceptance` | One end-to-end test file per milestone | no |

More: [Markdown grammar](docs/markdown-grammar.md) · [versioning](docs/versioning.md) ·
[dependency map](docs/tpt-dependency-map.md) · [browser demo](crates/tpt-codex-web/README.md) ·
[relay server](crates/tpt-codex-sync/README.md)

## Using it

```sh
cargo install --path crates/tpt-codex-cli
tpt new article                       # start from a template
tpt check article.cdx                 # broken references, bad math, missing footnotes
tpt convert article.cdx -o article.pdf
tpt watch article.cdx -o article.pdf  # rebuild on save
tpt convert report.docx -o report.cdx --report
```

See the [quickstart](docs/quickstart.md), the [`.cdx` cheat-sheet](docs/cdx-cheatsheet.md) and
[migrating from Office](docs/migrating-from-office.md).

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
