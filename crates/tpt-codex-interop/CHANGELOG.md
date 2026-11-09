# Changelog

All notable changes to this crate are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate adheres to semantic versioning once it is released.

## [Unreleased]

### Added

- `.docx` import, read in-house: paragraphs, headings, bold and italic runs, line and tab breaks, title and author, tables as grids, and bulleted or numbered lists with their nested levels.
- `.docx` export: headings, bold and italic runs, line breaks, tabs, title and author, and grids as Word tables. Each thing written in a closer form, or left out, is recorded in the report.
- `.xlsx` import of the first worksheet (`import_xlsx`) or of every worksheet (`import_xlsx_workbook`), with cell types and formulas kept, and merged cells, cross-sheet formulas and other unsupported features recorded in the report.
- `.xlsx` export of one sheet as one worksheet, with each formula's computed value written.
- `.pptx` import, data only: each slide becomes a heading and its text paragraphs. Pictures, tables, speaker notes and formatting are recorded in the report.
- `.pdf` import, read in-house: the text as paragraphs and headings, in reading order, with title and author. It reads classic and stream cross-reference tables, object streams, incremental updates, and falls back to scanning a file whose tables are damaged. It reads the PDFs this project writes as the text they show. Encrypted files are refused.
- `place_sheet`: puts an imported sheet in a document as a grid block, as one undoable step.
- `FidelityReport`, with `FidelityItem` and `Fidelity` (exact, approximated, dropped), and a deterministic text form. Every importer fills one in.
- ZIP container reader (`Archive`) and writer (`ArchiveWriter`), with stored and DEFLATE entries, CRC-32 checks, and size limits.

### Changed

- `.docx` tables are imported as grids, one cell per table cell, instead of flattened paragraphs. Merged cells are reported as approximated.
- `.docx` lists are imported as lists, instead of plain paragraphs.

### Fixed

- `.docx` import no longer puts spaces at the edge of a bold or italic run.
