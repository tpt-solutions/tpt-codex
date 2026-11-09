# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide history is in the
root `CHANGELOG.md`.

## [Unreleased]

### Added

- Flow view (`layout_flow`): one column of positioned items, wrapped with an injected
  `TextMeasure`, with a fixed-pitch `MonospaceMeasure` for tests.
- Paginated view (`paginate`): pages with orphan and widow control, and `PageGeometry`.
- Headers, footers and page numbers (`render_chrome`, `render_template`, `PageChrome`), and
  `PaperSize` with A4 and Letter.
- Footnotes at the foot of the page (`layout_note`, `paginate_with_notes`).
- Grids in the flow: one item per row, pages that break between rows, and header rows that repeat
  on continuation pages. A grid with a view window shows the rows the window covers.
- Canvas view (`layout_canvas`, `slide_ids`): a slide's children at their frames, in z-order.
- SVG slide renderer (`render_slide_svg`), with text wrapped by the same measure as the flow, and
  embedded SVG drawn as an image.
- Incremental layout (`layout_flow_cached`, `FlowCache`), which re-wraps only the text that changed.
- Hyphenation of long words at line ends (`layout_flow_hyphenated`), with in-house rules.
- Layout benchmark example (`examples/bench_layout.rs`).

### Changed

- Block quotes are indented from the margin, and sub-lists are indented under their item, in the
  flow. Top-level output is unchanged.
- Tables outside a grid are laid out as one line per row.
- Grid header rows are read from each sheet's `header_rows`, replacing the document-wide setting in
  `FlowOptions`.
- Media blocks can be given their real height through `FlowCache::set_media_height`, so a picture
  takes its own space in the flow.
- Lists are drawn as text, with markers on the first line of each item.
