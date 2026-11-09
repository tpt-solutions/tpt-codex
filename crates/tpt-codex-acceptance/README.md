# tpt-codex-acceptance

End-to-end, headless acceptance tests for tpt-codex. The crate has no library code. It holds one
test file per milestone in [`todo.md`](../../todo.md), plus golden render tests and shared helpers.

Status: pre-release, version 0.1.0-spec, `publish = false`. This is a test-only crate: it is not
published and nothing depends on it.

## What it is for

Each milestone in `todo.md` has an acceptance statement, such as "A `.cdx` file with Markdown, math,
and cross-references exports to a beautifully typeset PDF". Each `tests/m<N>_*.rs` file turns the
headless part of its milestone into tests that drive the real crates together: the document model,
the editor, the Markdown reader, the typesetter, the PDF exporter, the relay and the Office
importers. The parts that need a browser or a human are noted in the file's header and tested
elsewhere.

The tests read their input files from the repository's `fixtures/office` directory and keep everything
else in memory. Only the golden tests write files, and only when `TPT_UPDATE_GOLDEN` is set. Nothing
uses the network.

## Test files

| File | Milestone (todo.md) | What it covers |
| --- | --- | --- |
| `m0_foundation.rs` | 0: foundation | A `Block` tree is saved and loaded through `DocumentStore` over an in-memory store and comes back equal, including trees with grids and frames. The CI and license-gate parts are checked by the workflow and `scripts/`. |
| `m1_editor.rs` | 1: editor | Drives the editor's messages (`tpt_codex_app::Editor`, `Msg`) headlessly: type a paragraph, insert a live calculation table, enter formulas, check the computed values and the view, and undo. The browser half is `crates/tpt-codex-web/browser`. |
| `m2_markdown_typeset.rs` | 2: Markdown and typesetting | A `.cdx` file with Markdown, math and cross-references is read, checked (math, references, bibliography), written back exactly, laid out and paginated. Also exports it to a PDF, and checks that the export is repeatable. |
| `m3_paginated.rs` | 3: paginated PDF | A hybrid document (text, a live grid and a media block) paginates across pages and repeats the grid's header rows on continuation pages. The same document lays out to the same pages every time, and one-click export gives the same PDF bytes on repeated runs. |
| `m4_collaboration.rs` | 4: collaboration | Two replicas edit one paragraph and one live grid at the same time through a relay, and converge on the same text and values. A replica that joins late catches up from the relay. |
| `m5_interop.rs` | 5: interop | A real `.docx` and `.xlsx` from `fixtures/office` open as editable documents; every worksheet is placed as a live grid; a cell is edited; the result writes back out and reads again. |
| `golden.rs` | (cross-cutting) | Renders a fixed slide to SVG and a fixed report to paginated text, and compares both byte for byte with `golden/slide.svg` and `golden/paginated.txt`. |

Shared helpers are in `tests/common/mod.rs`: a fixed text measure (6 points per character, 14 points
per line), an in-memory `Storage`, a PDF page counter, and a reader for `fixtures/office`.

Golden files are updated deliberately: set `TPT_UPDATE_GOLDEN=1` and run the golden tests, then review
the diff.

## Where it sits in the workspace

The crate depends on the crates it exercises: `tpt-codex-app`, `tpt-codex-core`, `tpt-codex-grid`,
`tpt-codex-interop`, `tpt-codex-markdown`, `tpt-codex-pdf`, `tpt-codex-text`, `tpt-codex-views` and
`tpt-typeset`. Nothing depends on it. It does not depend on the [`tpt` command](../tpt-codex-cli/README.md),
which has its own tests.

## Running it

```sh
cargo test -p tpt-codex-acceptance                      # every milestone and the golden tests
cargo test -p tpt-codex-acceptance --test m3_paginated  # one milestone file
cargo test -p tpt-codex-acceptance --test golden        # the golden render tests
```

CI runs the whole workspace with `cargo test --workspace`, which includes this crate.

When a change to rendering is deliberate, regenerate the goldens with
`TPT_UPDATE_GOLDEN=1 cargo test -p tpt-codex-acceptance --test golden`, and review the changed files
in `golden/` before committing them.

## Adding a milestone file

Name the file `m<N>_<topic>.rs` after the milestone in `todo.md`, and put the milestone's acceptance
statement in its header comment, as the existing files do. Test only the headless parts; say in the
header where the rest is covered.

## Testing scope and limits

- The tests are headless. Browser behaviour is tested by `crates/tpt-codex-web/browser`, in Chrome,
  outside `cargo test`.
- The golden tests run natively. Their header says the wasm32 build of the file is compiled by CI.
  The CI workflow currently builds only the core, views, markdown, typeset and app crates for
  wasm32, so that compile check does not cover this file.
- The license gate and CI workflow are not run by these tests.

## License

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
