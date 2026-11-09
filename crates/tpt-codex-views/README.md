# tpt-codex-views

The projection engines of tpt-codex. A projection turns a `tpt-codex-core` `Document` into
positioned items that a host can draw. This crate has the Flow view (one column), the Paginated
view (the column split into pages, with footnotes, headers, footers and page numbers), the
Canvas view for slides (children placed at their frames), and an SVG renderer for one slide. It
is `no_std` and does no drawing and no font work itself: text is measured through a trait the
host supplies.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and the crate is `publish = false`. Every
item listed below is built and tested. The layout is our own: there is no external layout engine.

## What it does

- **Flow view.** `layout_flow` lays the root flow container out as one column of `FlowItem`s in
  reading order. Text blocks are wrapped to `FlowOptions::width` with a `TextMeasure`. A list item
  carries its marker (a bullet or a number) on its first line, and sub-lists are indented a further
  list indent (`LIST_INDENT`, 18 points). Block quotes are indented from the margin. A table is one
  line per row. A grid is one item per row, and a grid's header rows (`Sheet.header_rows`) are marked
  so they can repeat. A grid with `view_bounds` shows only the rows in its window (`grid_rows_shown`).
  Media, code, display math, thematic breaks and queries are reserved as opaque boxes of
  `FlowOptions::box_height`, which the host draws. A media block can be given its real height through
  `FlowCache::set_media_height`.
- **Incremental layout.** `layout_flow_cached` with a `FlowCache` re-wraps only the text blocks whose
  content, style or column width changed. Its result is the same as a plain layout.
- **Hyphenation.** `layout_flow_hyphenated`, or a `FlowCache::hyphenated()`, breaks a long word at a
  hyphenation point when it does not fit. The rules are in-house (`hyphenate` module, ASCII letters
  only). They are an approximation: they have no dictionary and do not know affixes or stress.
- **Paginated view.** `paginate` splits a flow into `Page`s of a `PageGeometry` (page height, top and
  bottom margins, and `min_lines` of orphan and widow control). A split paragraph keeps at least
  `min_lines` lines on each side of the break. A grid's rows break between rows, and continuation pages
  repeat its header rows. `paginate_with_notes` places each `Note` (a footnote, laid out by
  `layout_note`) at the foot of the page where its anchor first appears.
- **Headers, footers and paper.** `render_chrome` fills a `PageChrome` template for each page;
  `render_template` substitutes `{page}` and `{pages}`. `PaperSize` gives `A4` and `LETTER` in points,
  and `column_width` gives the text width between two margins.
- **Canvas view.** `layout_canvas` returns a slide's children with their frames, in z-order. A slide
  with no canvas size is 16:9 (`Canvas::WIDESCREEN`, 960 by 540 points). `slide_ids` lists the slides
  of a document in order.
- **SVG slide.** `render_slide_svg` draws one slide as an SVG string, with text wrapped by the same
  measure as the Flow view. An embedded SVG picture is drawn as an `<image>` with a data URI, so a
  script inside it does not run.

## What it deliberately does not do

- No font loading or shaping. `TextMeasure` is the boundary. `tpt-codex-text` supplies a font-backed
  measure, and `tpt-codex-pdf` draws the pages as a PDF.
- No drawing of boxes, grids or pictures in the Flow view. It returns positions and kinds; the host
  draws them.
- No editing and no state beyond the optional `FlowCache`.
- No columns, floats or wrapping of text around a picture in the flow. A picture's height is the
  space it takes, and the text moves to make room in the PDF export.
- No inline images. An image is a block.
- No hyphenation outside ASCII English.

## Where it sits

Depends on `tpt-codex-core` only (re-exported as `tpt_codex_views::core`). Depended on by
`tpt-codex-text`, `tpt-codex-pdf`, `tpt-codex-app`, `tpt-typeset` and `tpt-codex-acceptance`.
`tpt-codex-text` supplies the font-backed measure and `tpt-codex-pdf` draws the pages.

## Key public API

```rust,ignore
use tpt_codex_views::{
    FlowOptions, MonospaceMeasure, PageChrome, PageGeometry, PaperSize,
    layout_flow, paginate, render_chrome, render_slide_svg,
};

let measure = MonospaceMeasure { char_width: 6.0, line_height: 14.0 };
let options = FlowOptions {
    width: PaperSize::A4.column_width(72.0, 72.0),
    block_spacing: 6.0,
    box_height: 40.0,
    row_height: 20.0,
};
let flow = layout_flow(&doc, &measure, options)?;

let geometry = PageGeometry {
    height: PaperSize::A4.height,
    margin_top: 72.0,
    margin_bottom: 72.0,
    min_lines: 2,
};
let pages = paginate(&flow, geometry)?;

let chrome = PageChrome {
    header: None,
    footer: Some("Page {page} of {pages}".into()),
};
let rendered = render_chrome(&pages, &chrome);

// One slide, as SVG.
let svg = render_slide_svg(&doc, slide_id, &measure)?;
```

The main entry points are `layout_flow`, `layout_flow_hyphenated`, `layout_flow_cached`,
`FlowCache`, `paginate`, `paginate_with_notes`, `layout_note`, `render_chrome`, `render_template`,
`layout_canvas`, `slide_ids`, `render_slide_svg`, `grid_rows_shown`, and the `TextMeasure` trait
with the fixed-pitch `MonospaceMeasure`.

`MonospaceMeasure` is for tests and benchmarks. Its width is `char_width` times the number of
characters, and a heading level is scaled up (level 1 is twice the body size). Real output uses a
font-backed measure.

## no_std, std and WASM

The crate is `#![no_std]` with `extern crate alloc`, and `#![forbid(unsafe_code)]`. The root README
lists `cargo build` checks for `thumbv7m-none-eabi` and `wasm32-unknown-unknown` that include it.
The benchmark in `examples/bench_layout.rs` uses `std` and is not part of the library.

## Errors

Each entry point returns a typed error and does not panic on a bad document:

- `LayoutError`: a column width that is not positive, a root that is not a flow container, or a
  child id that does not exist.
- `PageError`: a page geometry with no space for text, or a `min_lines` the geometry cannot hold.
- `CanvasError`: the block is not an absolute container (a slide).

## Determinism

Layout depends only on the document, the measure and the options. Output is in reading order and
uses no hash maps, so the same input gives the same items. The slide SVG is checked byte for byte
against a golden file (`tests/golden/slide.svg`).

## Testing

```sh
cargo test -p tpt-codex-views
cargo run --release -p tpt-codex-views --example bench_layout -- 20000
```

- `tests/views.rs`: flow wrapping, long words, hard breaks, heading sizes, boxes for non-text
  blocks, nested containers, layout errors, and the page template.
- `tests/margins.rs`: adjacent bottom and top margins collapse to the larger one, and headings get
  more space.
- `tests/lists.rs`, `tests/quotes_and_sublists.rs`: list markers, indentation, and quotes.
- `tests/grid_pages.rs`: grid rows as flow items, row breaks, and repeated header rows.
- `tests/hyphenation.rs`: hyphenated breaks.
- `tests/flow_cache.rs`: the cached layout matches the plain one and re-measures only what changed.
- `tests/notes.rs`: footnotes placed at the foot of the page.
- `tests/chrome.rs`: headers, footers and page numbers.
- `tests/canvas.rs`, `tests/slide_svg.rs`, `tests/slide_golden.rs`: slide layout, SVG output, and
  the golden file. Set `TPT_UPDATE_GOLDEN=1` to rewrite the golden file after a deliberate change.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
