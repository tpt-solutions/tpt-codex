# tpt-typeset

The semantic layer of tpt-codex: heading outlines and sections, cross-references, citations,
footnotes, labels, math parsing and box layout, document queries, and `[[name]]` data binding.
It works on a `tpt_codex_core::Document` and returns checks, numbers and boxes that the editor,
the PDF exporter and the CLI use.

## Status

Pre-release. Version `0.1.0-spec`, `publish = false`. The API is not stable and nothing is
published to crates.io.

## What it does

- **Outline and sections.** `Outline` numbers headings, gives each a unique anchor, builds a table
  of contents and resolves anchors. `semantic_tree` groups the blocks under each heading into
  nested `Section`s, with their footnotes and citations, and `section_boxes` reads each section's
  extent back from a laid-out flow column.
- **Document classes.** `DocumentClass` is a typed schema for `article` and `report`
  (numbering depth, TOC depth, page height, margins). `class_of` reads a document's class.
- **Cross-references.** `check_references` resolves `[text](#anchor)` links to section numbers.
  `check_figure_references` and `check_label_references` do the same for figures (`{#fig:id}`),
  equations (`{#eq:id}`) and tables (a grid with `label="tab:id"`), numbering each kind from 1 in
  reading order.
- **Citations.** `Library` reads CSL-JSON (`Library::from_csl_json`) or BibTeX
  (`Library::from_bibtex`). `author_year`, `bibliography` and `check_citations` use it.
- **Footnotes.** `footnote_numbers`, `reading_order`, `check_footnotes` and `unreferenced_footnotes`
  number footnotes in reading order. `footnote_notes` builds the notes that a page reserves room
  for, using a `TextMeasure`.
- **Math.** `parse_math` reads a LaTeX-style subset into a `MathNode` tree with line, column and
  hint on error. `layout_math` turns a tree into a `MathBox` (boxes, rules and glyphs) from
  injected `MathMetrics`. `math_svg` draws a box as SVG. `check_math_blocks` and
  `check_inline_math` parse every display and inline formula in a document and report failures.
  The layout constants (axis height, rule thickness, shifts and gaps) come from the MATH table of
  the bundled STIX Two Math font through `math_constants`; `MathConstants::default` has fixed
  fallbacks.
- **Data binding.** `Bindings` maps names to text. `resolve_bindings` replaces `[[name]]` in text
  blocks with its value, `check_bindings` reports names that have no value, and `grid_cell_name`
  gives the name of a grid cell (`sheet1.B4`). The caller supplies every value; this crate does not
  evaluate formulas (`tpt-codex-grid` does).
- **Queries.** `run_query` runs a `Block::Query` (the JSON form of a `tpt-query` query) against an
  entity source. `document_entities` maps the document's blocks to query entities, with the ids in
  `entity_type`. `query_bindings` runs every query in a document and binds each result as `q<id>`
  to its row count (`"3 rows"`). `query_builder` builds the JSON from a structured `QuerySpec`
  (equality, inequality and one-of conditions, a sort and a limit), so a host need not write JSON.
- **Slides.** `slide_sections` groups slides by the document's `sections`.
- **Bibliography and math fonts.** The BibTeX reader converts common TeX accents and letter
  commands (`{\"O}zt\"urk` reads as `Öztürk`).

## What it deliberately does not do

- It does not lay out pages or paragraphs. The flow engine is in `tpt-codex-views`, and the
  `tpt-layout` box tree is not used by this crate. Conversion to `tpt-layout` boxes is not
  implemented.
- It does not evaluate spreadsheet formulas. `tpt-codex-grid` computes grid values and passes them
  in as bindings.
- It does not read files or fonts from the system. The only font is the one bundled in
  `assets/`.
- It does not hyphenate. Hyphenation is in `tpt-codex-views`.

## Limitations

- **Math subset.** The parser covers numbers, single-letter identifiers, operators, groups,
  `\frac`, `\sqrt`, `^` and `_`, and a fixed list of symbol commands (`symbol_glyph`). Anything
  else is an error with a hint. Math glyph advances come from the text font, not the math font.
- **BibTeX.** Only `author`, `title`, and `year` (or the year of `date`) are kept. Name particles
  are handled only for the common `von` rule. Other commands in values are dropped.
- **CSL-JSON.** Only the fields used for author-year citations are read.
- **Document classes.** Only `article` and `report` exist.
- **Query conditions.** The builder offers equality, inequality and one-of conditions. Other
  filters need JSON written by hand.

## Where it sits

Depends on:

- `tpt-codex-core` (the document model, re-exported as `tpt_typeset::core`)
- `tpt-codex-views` (flow layout for section boxes and footnote notes)
- `tpt-query` (with its `json` feature, for `Block::Query`)
- `serde_json` and `ttf-parser` (for reading the MATH table)

Used by:

- `tpt-codex-grid` (bindings, query bindings and the grid cell names)
- `tpt-codex-text` (its math metrics use `MathConstants` and `MathMetrics`)
- `tpt-codex-pdf`, `tpt-codex-cli` and `tpt-codex-app` (the export, checks and editor)
- `tpt-codex-acceptance` (end-to-end tests)

The document formats that produce its input are described in
[`../../docs/markdown-grammar.md`](../../docs/markdown-grammar.md). The dependency pins are in
[`../../docs/tpt-dependency-map.md`](../../docs/tpt-dependency-map.md).

## Features and key API

The crate has no Cargo features of its own. `tpt-query` is depended on with its `json` feature.

```rust,ignore
use tpt_typeset::{
    Bindings, Library, Outline, bibliography, check_references, parse_math, resolve_bindings,
    semantic_tree,
};

// Headings, numbers and the table of contents.
let outline = Outline::of(&doc);
for line in outline.toc(2) {
    // Each line has its number and title; print or render them.
}

// Cross-references and the semantic tree.
let missing = check_references(&doc);
let tree = semantic_tree(&doc);

// Citations from a CSL-JSON file.
let library = Library::from_csl_json(csl_text)?;
let entries = bibliography(&library, &["smith2020", "jones2021"]);

// Math: parse, then lay out with metrics from a font.
let nodes = parse_math(r"\frac{a^2}{b}")?;
let boxed = layout_math(&nodes, &metrics, 11.0);

// Data binding: fill [[name]] with values the caller computed.
let mut values = Bindings::new();
values.set("sheet1.B4", "42");
let resolved = resolve_bindings(&doc, &values);
```

The `query_builder` module builds a query's JSON:

```rust,ignore
use tpt_typeset::query_builder::{Condition, QuerySpec, build_source};

let spec = QuerySpec::new(1) // entity type id, see tpt_typeset::entity_type
    .with(Condition::Equals { field: "kind".into(), value: "heading".into() })
    .sorted_by("content", false)
    .limited_to(10);
let source = build_source(&spec);
```

## no_std and WASM

The crate is `#![no_std]` with `alloc`, and `#![forbid(unsafe_code)]`. The root README lists it
among the crates that build for `thumbv7m-none-eabi` and `wasm32-unknown-unknown`. The CI
workflow checks those builds.

## Errors

Each parser and checker returns structured errors rather than panicking:

- `MathError` carries a 1-based line and column, a message and a hint. Nesting is limited to
  `MAX_DEPTH` (64) levels.
- `CslError` (from `Library::from_csl_json` and `from_bibtex`) carries a line and column.
- `QueryRunError` is either `Parse` (with a hint) or `Execute` (from `tpt-query`).
- `UnknownClass` names a class that does not exist.
- Checks such as `check_references` and `check_bindings` return one entry per reference or
  binding, with missing targets or values marked, and do not stop at the first problem.

## Determinism

Section numbers, anchors, footnote numbers and the output of `document_entities` depend only on
the document. Maps are `BTreeMap`s, so iteration order is stable. `query_bindings` runs every
query in the document and drops the ones that do not run.

## Testing

```text
cargo test -p tpt-typeset
```

The integration tests in `tests/` cover:

| File | What it covers |
|------|----------------|
| `outline.rs`, `semantic.rs`, `sections.rs`, `boxes.rs` | numbering, anchors, TOC, sections and section boxes |
| `references.rs`, `labels.rs`, `figures.rs` | cross-reference resolution and numbering |
| `citations.rs`, `bibtex.rs`, `mutation_csl.rs` | CSL-JSON and BibTeX readers, citations, seeded CSL mutations |
| `footnotes.rs`, `footnote_notes.rs` | numbering and footnote notes |
| `math.rs`, `math_blocks.rs`, `math_layout.rs`, `math_svg.rs`, `inline_math.rs` | math parsing, layout, SVG and checks |
| `math_font.rs` | the font's MATH constants, and the SVG of a fraction with a superscript against `tests/golden/frac_sup.svg` |
| `bindings.rs` | `[[name]]` resolution and checks |
| `query.rs`, `query_builder.rs`, `document_query.rs` | query execution, the builder and entity mapping |
| `errors.rs` | structured errors and their hints |
| `mutation.rs` | seeded mutation test of the math parser (no panics, same result twice) |
| `milestone2.rs` | a Markdown document read, numbered and checked up to the page (PDF export is not tested here) |

The crate also has unit tests in `src/math_font.rs` and `src/query_builder.rs`.

## Assets

`assets/STIXTwoMath-Regular.otf` is STIX Two Math 2.13, under the SIL Open Font License 1.1
(`assets/OFL.txt`). Its checksums and source are in [`assets/README.md`](assets/README.md).

## License

Dual-licensed under MIT OR Apache-2.0. See [`../../LICENSE-MIT`](../../LICENSE-MIT) and
[`../../LICENSE-APACHE`](../../LICENSE-APACHE).
