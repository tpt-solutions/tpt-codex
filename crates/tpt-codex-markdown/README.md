# tpt-codex-markdown

The reader and writer for the Markdown and Codex Script (`.cdx`) text form of a tpt-codex
document. `from_markdown` reads a supported subset of Markdown, with tpt-codex extensions, into a
`tpt-codex-core` `Document`. `to_markdown` writes a document back in the same subset. The two are
built on the same line predicates, so a document the writer accepts reads back as the same
document. Anything outside the subset is a structured error on read and a refusal on write, so a
round trip never drops data silently.

The supported grammar is described in [docs/markdown-grammar.md](../../docs/markdown-grammar.md).
This README gives the overview; that document is the authority on what is accepted.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and the crate is `publish = false`. The
subset is deliberately smaller than CommonMark: a construct whose reading would differ from
CommonMark is an error, not a silent change.

## What it does

On read (`from_markdown`):

- YAML-style front matter with `title`, `author`, `date` and `class`.
- ATX headings (`#` to `######`), paragraphs with inline emphasis (`*em*`, `**strong**`,
  `***both***`), links (`[text](address "title")`), and backslash escapes.
- Bulleted and numbered lists, with sub-lists nested to any depth.
- Block quotes (`>`), which can hold any blocks, including lists and other quotes.
- Fenced code blocks (backtick or tilde), stored exactly as written.
- Thematic breaks, and standalone images with figure attributes (`{#fig:id width=80%}`).
- Display math (`$$` blocks, multi-line or one-line), with optional equation labels.
- Grids (`::: grid { id="N" }`) with cells, header rows, table labels and cell names.
- Slides (`::: canvas { ratio="W:H" }`), whose body is ordinary blocks.
- Pipe tables outside a grid, read as plain text (`Block::Table`).
- Citations (`[@key]`), footnote references and definitions (`[^label]`), cross-references to
  headings, figures, equations and tables (`[text](#anchor)`, `[text](#fig:id)`), and bindings
  (`[[name]]`), which the host fills with a value. A grid cell's value fills `[[sheet1.B4]]` or a
  cell name in the PDF and Word export and the paginated view.

On write (`to_markdown`): the same constructs, in the writer's canonical form. It escapes the
characters that would otherwise start syntax, sorts cell names, and refuses what it cannot write
back exactly, with a `SerializeError` that names the block. Refusals include queries (`Block::Query`
has no Markdown form yet), nested containers, slide frames, and grids with view bounds.

Also:

- `migrate_legacy_markdown` escapes inline markers in files written before inline syntax was read,
  and reports the changed lines. Run it once per file.

## What it deliberately does not do

- It is not a general Markdown renderer or a CommonMark implementation. HTML, indented code blocks,
  setext headings, loose lists, reference-style links, and other containers are read errors.
- It stores no block ids. A document read from Markdown has the root as `BlockId(0)`, and the other
  blocks are numbered in reading order (a list's items before the list). Writing and reading again
  keeps the ids only when the document already used that numbering.
- It does not interpret math source or query source. `tpt-typeset` parses math, and `tpt-query`
  owns the query language.
- It does not render or lay out anything.

## Where it sits

Depends on `tpt-codex-core` only, and uses `serde_json` as a dev-dependency for its golden test.

Depended on by `tpt-codex-acceptance`, `tpt-codex-app`, `tpt-codex-files`, `tpt-codex-grid`,
`tpt-codex-pdf`, `tpt-codex-text` and `tpt-typeset`, and by the `tpt` command line through those
crates. The browser demo opens `.cdx` and `.md` files through `tpt-codex-files`.

## Key public API

```rust,ignore
use tpt_codex_markdown::{from_markdown, to_markdown, migrate_legacy_markdown};

let source = std::fs::read_to_string("report.cdx")?;

// Read. A ParseError has a 1-based line and column, a message and an optional hint.
// Its Display form is "line 12, column 1: ... (hint: ...)".
let document = from_markdown(&source)?;

// Write. A SerializeError names the child index it refused, when there is one.
let text = to_markdown(&document)?;
let again = from_markdown(&text)?; // reads back as the same document

// Repair a file written before inline syntax was read.
let migrated = migrate_legacy_markdown(&source);
println!("changed lines: {:?}", migrated.changed_lines);
```

The public items are `from_markdown`, `to_markdown`, `migrate_legacy_markdown` with `Migrated`,
`ParseError` and `SerializeError`.

## no_std, std and WASM

The crate is `#![no_std]` with `extern crate alloc`, and `#![forbid(unsafe_code)]`. The root README
lists `cargo build` checks for `thumbv7m-none-eabi` and `wasm32-unknown-unknown` that include it.
The `migrate_markdown` example uses `std` for file access; the library does not.

## Errors

- `ParseError` (on read): `line` and `column` (1-based), `message`, and an optional `suggestion`.
  Its `Display` form is `line N, column M: message (hint: ...)`. Front-matter problems, unknown
  containers, unsupported syntax and an unclosed block are all reported this way, never as a panic.
- `SerializeError` (on write): `index`, the position of the block in the root container when the
  problem is with a block, and `reason`.

## Determinism and security

Reading and writing are pure functions of their input: no clock, no randomness and no hash maps,
so the output is byte-for-byte the same on every platform. The golden fixture is compared exactly.
The reader reports malformed input as a `ParseError`. The seeded mutation test checks that it never
panics on mutated input (it runs the reader under `catch_unwind`).

## Testing

```sh
cargo test -p tpt-codex-markdown
cargo run -p tpt-codex-markdown --example migrate_markdown -- old.md new.md
```

- `tests/markdown.rs`: the core subset, front matter, headings, paragraphs, lists, code, rules,
  images, inline syntax, and the read and write errors.
- `tests/golden.rs`: a fixture reads to a known JSON document and writes back byte for byte.
- `tests/containers.rs`: grids, slides and figure attributes, with exact round trips and reader
  errors that give the line.
- `tests/labels.rs`: equation and table labels, round trips, and writer refusals.
- `tests/quotes_and_sublists.rs`: block quotes and nested lists.
- `tests/mutation.rs`: a seeded mutation test (edits to valid input, no external fuzzer) that checks
  the reader never panics, that a document it accepts validates, and that a document the writer
  accepts writes the same text again and reads back without error.

The fuzz harness under `fuzz/` (excluded from the workspace) has a `markdown` target.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
