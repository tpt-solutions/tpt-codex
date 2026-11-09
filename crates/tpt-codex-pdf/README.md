# tpt-codex-pdf

PDF export for tpt-codex. It writes a paginated document, or one slide, as a PDF file. Text is drawn as glyph outlines, so the page looks as the layout measured it, and an invisible text layer is added so the words can be searched, selected and read. A document export is also tagged, so a screen reader or an accessibility checker can tell headings, paragraphs, lists, tables and figures apart.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and this crate is not published (`publish = false`). The API may change before a release. See the [crate changelog](CHANGELOG.md) for what has changed so far.

## What it does

- Lays out a document with `tpt-codex-views` and writes it as one column of pages, with an optional header and footer (the default footer is `Page {page} of {pages}`).
- Exports a single slide as one page the size of the slide.
- Draws every text run as glyph outlines, using the bundled Noto Sans font through `tpt-codex-text` and `tpt-text`. Writing the file is done by `tpt-canvas` and `tpt-canvas-pdf`.
- Adds a text layer as an incremental update after the outlines. Each glyph is written again in an invisible text object, the bundled font is embedded as a subset CID font, and a ToUnicode map says which characters each glyph stands for. The original bytes are not changed, and a reader that ignores the layer draws the same page.
- Tags a document export: a structure tree with `Document`, `H1` to `H6`, `P`, `L`/`LI`/`LBody` for lists, `Table`/`TR`/`TD`/`TH` for grids, and `Figure` with alternative text for pictures. Headers and footers are marked as artifacts. The file gets its language (always `en`, because the model records none), its title and its author from the document metadata, and the tagged marker.
- Draws PNG and JPEG pictures, sized to the column or their own width, whichever is less. Pictures are kept at 200 dpi at most.
- Draws footnotes at the foot of the page where their reference falls, lists with their markers, block quotes and sub-lists indented, and bold text with a thin stroke.
- Replaces `[[sheet1.B4]]`-style bindings with the computed value of the cell, and `[[Total]]` names, before drawing. The document in the caller is not changed.

## What it deliberately does not do

- It does not read PDFs. Reading is in `tpt-codex-interop`.
- It does not embed fonts other than the bundled one, and it does not choose fonts from the system.
- It does not write PDF/A, encrypted files, forms or link annotations. A link's text is drawn, but its address is not written into the file.
- It does not load picture files itself. The caller supplies the bytes of each picture through an image source, so the crate never touches the file system and it builds the same way in a browser.

## Where it sits in the workspace

Depends on:

- `tpt-codex-core` (the document model), `tpt-codex-views` (layout and pagination), `tpt-codex-text` (font measurement and shaping), `tpt-codex-grid` (cell values for bindings), `tpt-typeset` (document class page settings and footnotes);
- `tpt-text` (glyph runs), `tpt-canvas` (drawing commands), `tpt-canvas-pdf` (the PDF writer, inside the `tpt-canvas` repository);
- `flate2` (inflating and deflating picture data and the text layer's streams).

Used by:

- `tpt-codex-files`, which `tpt-codex-cli` and `tpt-codex-web` use to save `.pdf` files;
- `tpt-codex-app`, for the one-click export on the demo page;
- `tpt-codex-acceptance`, for the milestone tests.

## Key public API

The crate root exports these items. Layout types such as `PaperSize` and `PageChrome` come from `tpt-codex-views`, and the font measure from `tpt-codex-text`.

| Item | Purpose |
| --- | --- |
| `export_with_defaults(&Document)` | The app's one-click export: A4, the margins of the document's class (article margins when it has none), the bundled font and the `Page N of M` footer. Pictures are drawn as empty boxes. |
| `export_with_images(&Document, ImageSource)` | The same, with pictures drawn from the source. |
| `export_document(&Document, PaperSize, Margins, &FontMeasure)` | A paginated document with no header or footer. |
| `export_document_with_chrome(...)` | As above, with a `PageChrome` header and footer. |
| `export_document_with_images(...)` | As above, with pictures. |
| `export_slide(&Document, BlockId, &FontMeasure)` | One slide as one page. Slides are not tagged. |
| `Margins` | Space in points on each side. `Margins::all(points)` sets one value on every side. |
| `ImageSource<'a>` | `&dyn Fn(&str) -> Option<Vec<u8>>`: the bytes of the picture at a URI, or `None` to draw an empty box. |
| `DEFAULT_FOOTER` | The footer template used by `export_with_defaults`. |
| `ExportError` | `Layout`, `Page`, `Canvas`, `Text` or `TextLayer`, each wrapping the error of the step that failed. |
| `TextLayerError` | The text layer could not be added to the file. |

Example (not compiled as a doctest):

```rust,ignore
use tpt_codex_pdf::{export_with_defaults, export_with_images};

// The app's default export: A4, class margins, page numbers.
let pdf: Vec<u8> = export_with_defaults(&doc)?;

// With pictures: each picture's URI is looked up in a folder.
let base = std::path::Path::new("notes");
let pdf = export_with_images(&doc, &|uri| std::fs::read(base.join(uri)).ok())?;
```

`export_slide` takes the measure, which comes from `tpt-codex-text`:

```rust,ignore
let measure = tpt_codex_text::FontMeasure::bundled()?;
let pdf = tpt_codex_pdf::export_slide(&doc, slide_id, &measure)?;
```

The output is deterministic: the same document and options give the same bytes. The crate uses ordered maps and a fixed clock for this, and tests compare the bytes of repeated exports.

## Pictures

The reader is in-house (`picture.rs`) and takes PNG and JPEG only.

- A JPEG with 8-bit samples and one or three components is embedded as the file has it.
- A PNG is embedded with its rows when it is 8 or 16 bits a channel with no transparency and no interlacing. Any other PNG (a palette, transparency, low bit depths) is decoded to 8-bit RGB with a separate alpha channel.
- A picture larger than 10,000 pixels on a side, or 40 million pixels in all, is not drawn. So is one whose inflated data would exceed 200 MiB.
- A picture that cannot be found, or does not decode, is drawn as an empty box and its figure is still tagged.

Not drawn: SVG pictures, CMYK JPEGs and JPEGs with other sample sizes. The file size limit for a picture is the caller's to apply; the command-line tool skips files over 64 MB.

## Limitations

- Bold text is an approximation: the regular outlines are stroked with a thin line, because the bundled font has no bold outlines of its own. Its width is the bold width the layout measured.
- Slides are not tagged, and their text layer has no structure.
- The language is always `en`. The model has no per-document language yet.
- The tagging has not been checked against a full accessibility checker, so it is not claimed to meet any accessibility standard.

## Errors

Every export function returns `Result<Vec<u8>, ExportError>`. The error says which step failed. Pictures never cause an error: a picture that cannot be read is drawn as an empty box.

## Testing

```
cargo test -p tpt-codex-pdf
```

- `tests/export.rs`: page count, page splitting, determinism, bold, headers and footers, footnotes, lists, grid values, and the golden files `golden/page.pdf` and `golden/slide.pdf` (the outlines only; regenerate with `TPT_UPDATE_GOLDEN=1` and review the diff).
- `tests/text_layer.rs`: the layer is an incremental update, its text is invisible and uses an embedded CID font, the ToUnicode map is right, and each page keeps its outlines.
- `tests/tagged.rs`: the tagged marker, language and title, headings and paragraphs, lists, balanced marked content, figures with alternative text, artifacts for headers and footers, slides left untagged, and grids as tables.
- `tests/pictures.rs`: pictures drawn when their bytes are found, an empty box otherwise, figure tags either way, and the drawing changing without the text layer changing.
- Unit tests in `src/picture.rs` cover the PNG and JPEG readers.

The tests read the output bytes and do not run a PDF reader. The text layer was checked by hand with `pdftotext` and `pypdf` during development.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE).

See also [the dependency map](../../docs/tpt-dependency-map.md) for how `tpt-canvas-pdf` is pinned.
