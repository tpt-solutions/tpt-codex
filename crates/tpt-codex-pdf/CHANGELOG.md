# Changelog

All notable changes to this crate are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate adheres to semantic versioning once it is released.

## [Unreleased]

### Added

- PDF export of a paginated document, on A4 with the margins of the document's class (`export_with_defaults`), or on any paper size and margins (`export_document`). Optional header and footer (`export_document_with_chrome`), with a `Page N of M` footer by default.
- PDF export of one slide as one page the size of the slide (`export_slide`).
- Text layer: each exported page carries its words as invisible text over the outlines, so the text can be searched, selected and read. The bundled font is embedded as a subset CID font, with a ToUnicode map. The layer is an incremental update, and the original bytes are unchanged.
- Tagged document export: a structure tree with headings, paragraphs, list items, tables (`Table`, `TR`, `TD`, `TH`) and figures with alternative text. Headers and footers are marked as artifacts. The file gets its language, title and author. Slides are not tagged.
- Pictures: PNG and JPEG pictures are drawn at the column width or their own width, whichever is less, through `export_with_images` and `export_document_with_images`. Pictures that cannot be found or decoded are drawn as empty boxes.
- Bold text is drawn with a thin stroke over the regular outlines, because the bundled font has no bold outlines.
- Lists are drawn as text with their markers.
- Footnotes are drawn at the foot of the page where their reference falls.
- Live values: `[[sheet1.B4]]` and named cells in prose show the computed value in the export.
- Deterministic output: the same document and options give the same bytes.

### Changed

- Block quotes and sub-lists are indented, and a list marker sits one list indent left of its text.

### Fixed

- Text is drawn as glyph outlines, so exported PDFs show the text as laid out.
