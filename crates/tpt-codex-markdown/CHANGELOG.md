# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide history is in the
root `CHANGELOG.md`.

## [Unreleased]

### Added

- `from_markdown` and `to_markdown`: a reader and writer for the supported subset, with structured
  errors (`ParseError`, `SerializeError`) and the grammar in `docs/markdown-grammar.md`.
- Front matter, ATX headings, paragraphs, bulleted and numbered lists, fenced code blocks, thematic
  breaks and standalone images.
- Inline emphasis, strong, bold italic, links with optional titles, backslash escapes, citations
  (`[@key]`), footnote references and definitions, and bindings (`[[name]]`).
- Display math blocks (`$$`), in multi-line and one-line form, and inline math.
- Block quotes, and nested lists (a sub-list under an item).
- Grids (`::: grid`) with header rows, table labels and cell names; slides (`::: canvas`).
- Figure attributes on images (`{#fig:id width=80%}`), and equation labels on display math.
- Pipe tables outside a grid, read and written exactly as plain text.
- `migrate_legacy_markdown`, which escapes inline markers in files written before inline syntax was
  read, and reports the changed lines. Includes the `migrate_markdown` example command.

### Changed

- The reader and writer cover bulleted and numbered lists, fenced code blocks, thematic breaks and
  standalone images. Constructs that would read back differently are errors on read and refusals on
  write.
- Block quotes and nested lists were errors before and now read and write. The writer refuses an
  empty quote, a sub-list first in its list, and two adjacent sub-lists of one kind. Documents with
  these need schema version 3.
- Bold and italic text are written as emphasis around the text (`**text**`, `*text*`, `***text***`),
  so a styled `.docx` converts to `.cdx`.

### Fixed

- `to_markdown` writes a grid whose sheet stores no size (such as an imported one) at the size of its
  cells, instead of refusing it.
