# tpt-codex-interop

Import and export adapters for the Office and PDF formats that tpt-codex has to meet: `.docx` and `.xlsx` in both directions, `.pptx` for reading, and `.pdf` for reading. Every importer returns the document it made together with a `FidelityReport` that says what it kept exactly, what it approximated and what it left out.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and this crate is not published (`publish = false`). The API may change before a release. See the [crate changelog](CHANGELOG.md).

## What it does

| Format | Import | Export |
| --- | --- | --- |
| `.docx` | Paragraphs, headings (`Heading1`–`6`, `Title`), bold and italic runs, line and tab breaks, lists from `word/numbering.xml`, tables as grids, title and author | Headings, bold and italic runs, line breaks, tabs, title and author, grids as Word tables, formula cells as their formula text. Images and queries are not written. |
| `.xlsx` | The first worksheet (`import_xlsx`) or every worksheet (`import_xlsx_workbook`) as a `Sheet`, with cell types and formulas | One `Sheet` as one worksheet, with each formula's computed value and inline text. |
| `.pptx` | Each slide as a level-2 heading (its title, or `Slide N`) and its text paragraphs | Not supported. |
| `.pdf` | The text as paragraphs and headings, read in reading order, with title and author from the information dictionary | Not supported here. PDF export is `tpt-codex-pdf`. |

Reading is in-house. The ZIP container (`archive`) is written here, rather than taken from the `zip` crate. XML is parsed with `quick-xml`, DEFLATE with `flate2` (pure-Rust backend) and CRC-32 with `crc32fast`. The PDF reader (`pdf/`) reads objects, cross-reference tables and streams itself.

Helpers:

- `place_sheet` puts an imported sheet into a document as a grid block, as one undoable step.
- `Archive` and `ArchiveWriter` are the ZIP reader and writer, for other crates that need them.
- `FidelityReport` is the shared report type, with a deterministic text form through `Display`.

## What it deliberately does not do

- It does not evaluate formulas. An imported formula keeps its text, and a cached value from the file is kept where the format has one.
- It does not read cell formatting in `.xlsx` (dates and percentages arrive as their stored numbers, and the report says so).
- It does not render, lay out or animate slides from `.pptx`. The import is data only, as the spec asks.
- It does not open encrypted PDFs, OCR scanned pages, or read PDF forms or pictures.
- It does not write `.pptx` or `.pdf`.

## Where it sits in the workspace

Depends on:

- `tpt-codex-core` (the document model, re-exported as `tpt_codex_interop::core`);
- `tpt-codex-grid` (sheets and cell values);
- `flate2`, `crc32fast`, `quick-xml`.

Used by:

- `tpt-codex-files`, which maps file extensions to these functions for the CLI and the browser page;
- `tpt-codex-acceptance`, for the milestone tests.

## Key public API

| Item | Purpose |
| --- | --- |
| `import_docx(&[u8]) -> Result<DocxImport, DocxError>` | `DocxImport { document, report }` |
| `export_docx(&Document) -> Result<DocxExport, DocxError>` | `DocxExport { bytes, report }`. The same document gives the same bytes. |
| `import_xlsx(&[u8]) -> Result<XlsxImport, XlsxError>` | First worksheet as `XlsxImport { sheet, report }` |
| `import_xlsx_workbook(&[u8]) -> Result<XlsxWorkbook, XlsxError>` | `XlsxWorkbook { sheets: Vec<XlsxSheet { name, sheet }>, report }` |
| `export_xlsx(&Sheet) -> Result<Vec<u8>, XlsxError>` | One worksheet as `.xlsx` bytes |
| `import_pptx(&[u8]) -> Result<PptxImport, DocxError>` | `PptxImport { document, report }` |
| `import_pdf(&[u8]) -> Result<PdfImport, PdfError>` | `PdfImport { document, report }` |
| `place_sheet(doc, history, sheet, parent, index) -> Result<SheetPlacement, PlaceError>` | Adds the sheet and a grid block that shows it, as one undoable step |
| `FidelityReport`, `FidelityItem`, `Fidelity` | `Exact`, `Approximated`, `Dropped`. `exact`, `approximated`, `dropped`, `items`, `count`, `is_lossless` |
| `Archive::open`, `Archive::names`, `Archive::read` | ZIP reader. Stored and DEFLATE entries only. |
| `ArchiveWriter::new`, `add`, `finish` | ZIP writer. DEFLATE entries, ASCII names, a fixed timestamp. |
| `MAX_ENTRY_BYTES`, `MAX_ENTRIES`, `MAX_TOTAL_BYTES` | Limits of the ZIP reader: 64 MiB an entry, 4,096 entries, 256 MiB in all |

Example (not compiled as a doctest):

```rust,ignore
use tpt_codex_interop::{import_docx, export_docx};

let import = import_docx(&bytes)?;
for item in import.report.items() {
    println!("{item:?}");
}
// Anything the report counts as dropped was not imported.
let lossless = import.report.is_lossless();

let export = export_docx(&import.document)?;
std::fs::write("out.docx", &export.bytes)?;
```

The error types are plain values. `DocxError`, `XlsxError`, `PdfError` and `ZipError` implement `Display`. `import_pptx` reports its errors as `DocxError`, because it shares the ZIP and XML readers with the `.docx` importer.

## Fidelity reports

Each importer records one item per feature it meets, with its location, the feature, and for approximations and drops a note. The report is the answer to "what did the import lose?", and the command-line tool and the browser page show it after opening a file. A report with no dropped or approximated items is lossless.

## Security and limits

The ZIP reader refuses an entry larger than 64 MiB, more than 4,096 entries, or more than 256 MiB in total. Every entry is checked against its declared size and CRC-32. Encrypted entries, ZIP64, and compression methods other than stored and DEFLATE are errors, not guesses. The ZIP writer refuses unsafe and duplicate entry names. The `fuzz/` directory has targets for the `.docx`, `.xlsx`, `.pptx` and `.pdf` readers.

## Testing

```
cargo test -p tpt-codex-interop
```

- `tests/office.rs`: the ZIP reader and writer (stored and DEFLATE entries, checksums, truncation, missing and unsafe names), and the `.xlsx` reader on openpyxl and Python `zipfile` fixtures.
- `tests/corpus.rs`: the `.xlsx` and `.docx` fixtures written by openpyxl and python-docx, in `fixtures/office/` (sheet order, cell types, merged ranges, cross-sheet formulas, styled runs).
- `tests/docx.rs`: `.docx` import: paragraphs and headings, tables as grids, Word tables written back and read again, title and author, the report, and lists.
- `tests/docx_export.rs`: `.docx` export: styles, breaks, tabs and special characters, lists, code, math and rules, the report, control characters refused, and package parts.
- `tests/pptx.rs`: `.pptx` import: slide headings and text, the report, and a slide without a title.
- `tests/pdf.rs`: `.pdf` import: text paragraphs and headings, compressed streams, ToUnicode maps, images counted, forms, incremental updates, object streams, damaged cross-reference tables, encrypted files refused, non-PDF files refused, and lists.
- `tests/place.rs`: `place_sheet`, including undo and the refused positions.
- `tests/fidelity.rs`: the report's counts and its text form.
- `tests/milestone5.rs`: a `.docx` and an `.xlsx` opened, edited and written back, and emphasis and links surviving a round trip.

## Limitations

- `.docx`: images are not imported. Merged cells are reported as approximated. Lists are written to `.docx` as paragraphs without markers, and the export report says so.
- `.docx`: parts of the package that are not listed above are not read.
- `.xlsx`: merged cells, array and shared formulas (where unsupported), cross-sheet formulas and error values are reported. Named ranges are reported. Cell formatting is not read.
- `.pptx`: pictures, tables, speaker notes and formatting are reported, not imported.
- `.pdf`: images, drawings and page numbers are counted, not imported. Bold and italic are read as plain text, and the report says so. Only the Flate and ASCII filters are decoded, so LZW, DCT and JBIG2 streams are not read. Scanned pages have no text and import as an empty document. Encrypted files are refused.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE).

For the user-facing side of these formats, see [the migration guide](../../docs/migrating-from-office.md).
