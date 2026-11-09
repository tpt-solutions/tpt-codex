# tpt-codex-files

Opens and saves tpt-codex documents by file name, from and to bytes. The file's extension decides the format, so the `tpt` command-line tool and the browser page follow the same rules. The crate does no file-system work itself: the caller reads the bytes and writes the result.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and this crate is not published (`publish = false`). The API may change before a release. See the [crate changelog](CHANGELOG.md).

## What it does

| Extension | Open | Save |
| --- | --- | --- |
| `.cdx`, `.md`, `.markdown` | Yes. Errors give `name:line:column: error: message (hint: …)`. | Yes, as Markdown (Codex Script) |
| `.docx` | Yes, with a fidelity report | Yes, with a fidelity report. Formula cells and `[[…]]` bindings are written as their computed values. |
| `.xlsx` | Yes. Every worksheet becomes a heading and a grid. | Yes. The first grid of the document is written as the one worksheet. |
| `.pptx` | Yes, text only, with a fidelity report | No, and the error says so |
| `.json` | Yes, the document model as JSON | Yes, pretty-printed |
| `.pdf` | Yes, its text, with a fidelity report | Yes. Bindings are resolved, and a text layer is added. Pictures are drawn only with `save_with_images`. |

Extensions are matched without regard to case. An unknown extension is an error that lists the accepted types.

Every opened document is checked with `Document::validate` before it is returned, so a file that opens here is a valid document.

## What it deliberately does not do

- It does not read or write paths. The caller passes the file name for its extension and the bytes.
- It does not decide where a picture comes from. `save_with_images` takes a closure that returns the bytes for a picture's address. `save` is the same call with no pictures, and the browser page uses `save`.
- It does not write `.pptx`, and it does not write a `.xlsx` with more than one worksheet. The other worksheets stay in the document.

## Where it sits in the workspace

Depends on:

- `tpt-codex-core` (the document model and `Document::validate`);
- `tpt-codex-markdown` (Markdown and Codex Script reading and writing);
- `tpt-codex-interop` (`.docx`, `.xlsx`, `.pptx` and `.pdf` readers and writers, and the fidelity report);
- `tpt-codex-pdf` (PDF export);
- `tpt-codex-grid` (resolving bindings and computed cells before `.docx` and `.pdf` saves);
- `serde_json` (with `std`), for `.json`.

Used by:

- `tpt-codex-cli`, which reads and writes files for the `tpt` command, and passes picture bytes from the document's folder;
- `tpt-codex-web`, the browser page, which opens and saves files the user chooses.

The browser page is a wasm module (`tpt-codex-web`), so this crate is built for `wasm32` as well. It needs `std`, and it works on the bytes it is given.

## Key public API

| Item | Purpose |
| --- | --- |
| `format_of(name) -> Result<Format, String>` | The format of a file name |
| `Format` | `Markdown`, `Docx`, `Xlsx`, `Pptx`, `Json`, `Pdf` |
| `open(name, bytes) -> Result<Opened, String>` | `Opened { document, report: Option<FidelityReport> }`. The report is `None` for formats read exactly (Markdown and JSON). |
| `save(doc, name) -> Result<Saved, String>` | `Saved { bytes, report: Option<FidelityReport> }` |
| `save_with_images(doc, name, &dyn Fn(&str) -> Option<Vec<u8>>)` | As `save`, and a PDF draws each picture whose address the closure resolves |
| `Fidelity`, `FidelityReport` | Re-exported from `tpt-codex-interop` |

Errors are `String` values that start with the file name, so they can be shown to a user as they are.

Example (not compiled as a doctest):

```rust,ignore
use tpt_codex_files::{open, save, save_with_images};

// Open a file's bytes and show what the import left out.
let opened = open("report.docx", &bytes)?;
if let Some(report) = &opened.report {
    println!("{report}");
}

// Save as PDF, drawing pictures from the folder of the document.
let folder = std::path::Path::new("notes");
let saved = save_with_images(&opened.document, "report.pdf", &|uri| {
    std::fs::read(folder.join(uri)).ok()
})?;
std::fs::write("report.pdf", &saved.bytes)?;
```

`save` is the same call with no pictures, so every picture is an empty box.

## Determinism and security

Saving the same document under the same name gives the same bytes, because each writer below it is deterministic. Opening is strict: an invalid document is refused, and a file that cannot be read is an error, not a partial document. Limits on archive size and entry count are in `tpt-codex-interop`, and the PDF picture decoder has its own limits in `tpt-codex-pdf`.

## Testing

```
cargo test -p tpt-codex-files
```

`tests/pdf.rs` has one test: a Markdown document saved as PDF opens again with its headings and paragraphs, through the same `open` path as any other file. The other formats are tested in the crates that implement them (`tpt-codex-interop`, `tpt-codex-pdf`, `tpt-codex-markdown`).

## Limitations

- `.pptx` is read only, and its import is data only.
- A `.xlsx` save writes one worksheet, the first grid of the document.
- The PDF writer draws pictures only when the caller gives their bytes. The browser page has no files, so its PDFs show empty boxes for pictures.
- Errors are text, not typed values, so a caller cannot match on the kind of failure.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE).

For the user-facing rules, see [the migration guide](../../docs/migrating-from-office.md) and the [quickstart](../../docs/quickstart.md).
