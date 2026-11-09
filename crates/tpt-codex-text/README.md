# tpt-codex-text

Font-backed text measurement for tpt-codex. It bundles two Noto Sans fonts, shapes text with
`tpt-text`, and gives the flow view, the math layout and the PDF writer the same widths and glyphs
on every platform.

## Status

Pre-release. Version `0.1.0-spec`, `publish = false`. The API is not stable and nothing is
published to crates.io.

## What it does

- **Measures text with the bundled font.** `FontMeasure::bundled()` loads Noto Sans Regular and
  Noto Sans Italic from the crate. `FontMeasure` implements the three measurement traits the
  workspace uses:
  - `tpt_codex_views::TextMeasure` (width and line height of a run of text, for the flow view),
  - `tpt_typeset::MathMetrics` (glyph advance, ascent and descent, and the layout constants read from
    the bundled STIX Two Math font's MATH table), and
  - `tpt_layout::text::TextMeasurer` (line boxes for wrapping, for the box layout engine).
- **Wraps text.** `line_boxes` breaks text at a width, using the same line breaks as the widths
  above.
- **Shapes text into glyphs.** `shape_lines` returns each line's glyphs with their ids, positions
  and advances, and the characters each glyph stands for. The PDF writer draws outlines from these
  glyphs and writes the characters back as an invisible text layer.
- **Emphasis.** Italic text uses the italic face. Bold text is shaped at weight 700 on the variable
  font's `wght` axis.
- **Heading sizes.** Body text is 11 points (`BODY_POINTS`). Headings are 2.0, 1.6, 1.3 and 1.1
  times that for levels 1, 2, 3 and 4 to 6.

## What it deliberately does not do

- It never looks up system fonts. The two bundled files are the only fonts, so layout does not
  depend on what is installed.
- It does not choose fonts from a style sheet. `FAMILY` is the only family name.
- It does not hyphenate. Hyphenation is in `tpt-codex-views`. This crate only keeps the hyphen when
  a line breaks at one.
- It does not lay out pages or paragraphs. The flow engine is in `tpt-codex-views`.

## Limitations

- **Bold outlines.** Bold text is measured and shaped at weight 700, so its advances match bold.
  The outlines come from the regular face at its default weight. A caller that draws
  `shape_lines` output draws bold text at regular weight. `FontMeasure::faces` shows which face
  each run uses.
- **One family.** Only Noto Sans is bundled. This crate chooses no other family and no fallback
  font, so text in a script that Noto Sans does not cover is measured with whatever `tpt-text`
  does with missing glyphs.
- **Flow engine.** The flow view uses its own engine. `tpt-layout`'s `layout_flow` is not used yet.
  This crate supplies only its `TextMeasurer`.
- **Fallback widths.** If the font cannot lay text out, `TextMeasure::width` returns an estimate
  (half an em per character). The font is checked when it is loaded, so this is not expected in
  practice.

## Where it sits

Depends on:

- `tpt-codex-core` (the text style type)
- `tpt-codex-views` (the `TextMeasure` trait)
- `tpt-typeset` (the `MathMetrics` trait and the math constants)
- `tpt-text` and `tpt-layout` (shaping, line breaking and the `TextMeasurer` trait; pinned by git
  revision in the workspace manifest)

Used by:

- `tpt-codex-pdf` (glyphs for the PDF outlines and the text layer)
- `tpt-codex-app` (the editor and paginated view)
- `tpt-codex-acceptance` (end-to-end tests)

The dependency pins and their status are in
[`../../docs/tpt-dependency-map.md`](../../docs/tpt-dependency-map.md).

## Features and key API

The crate has no Cargo features.

```rust,ignore
use tpt_codex_core::TextStyle;
use tpt_codex_text::FontMeasure;
use tpt_codex_views::TextMeasure;

let measure = FontMeasure::bundled()?; // TextError if the bundled font fails to load
let body = TextStyle::default();

let width_pt = measure.width("Hello, world", &body);
let line_h_pt = measure.line_height(&body);

// Glyphs for drawing: one entry per line, each with its runs and glyph positions.
let lines = measure.shape_lines("Hello, world", &body, Some(200.0))?;
for line in &lines {
    for run in &line.runs {
        for glyph in &run.glyphs {
            // glyph.glyph is the id in the face run.face; glyph.x and glyph.y are in points
            // from the start of the line and the baseline.
            let _ = (glyph.glyph, glyph.x, glyph.y, &glyph.text);
        }
    }
}
```

Public items:

- `FontMeasure` with `bundled()`, `faces()` and `shape_lines()`, plus the trait implementations
  above.
- `GlyphLine`, `GlyphRun` and `GlyphPlacement`, the output of `shape_lines`. A `GlyphPlacement`
  carries `glyph`, `x`, `y`, `advance` and `text`. Only the first glyph of a cluster carries the
  text of that cluster, so a ligature's characters appear once.
- `TextError`, with `Font` and `Layout` variants.
- Constants: `FAMILY` (`"Noto Sans"`), `NOTO_SANS_REGULAR`, `NOTO_SANS_ITALIC`, `BODY_POINTS`
  (11.0) and `BOLD_WEIGHT` (700.0).

## no_std and WASM

The crate uses `std` (`std::sync::Arc`, `std::collections`). It is not part of the `no_std` crates.
The bundled fonts are compiled into the binary with `include_bytes!`, so no file access is needed
at run time.

## Errors

`FontMeasure::bundled()` and `shape_lines()` return `TextError`:

- `Font` when a bundled font cannot be parsed. The font files are fixed, so this indicates a broken
  build.
- `Layout` when `tpt-text` cannot lay out the text.

The measurement traits do not return errors. They fall back to the estimates described above.

## Determinism

Widths and glyph positions depend only on the text, the style and the bundled fonts. The tests in
`tests/flow.rs` check that the same layout is produced twice. Positions use the
fixed-point type `Fx` from `tpt-text`, not floating-point layout.

## Testing

```text
cargo test -p tpt-codex-text
```

| File | What it covers |
|------|----------------|
| `src/lib.rs` (unit tests) | the bundled font files have the expected sizes; wider text measures wider; headings are larger; long text wraps within its width; bold is wider than regular and italic differs; wide glyphs advance further; a font-backed fraction lays out with real metrics |
| `tests/flow.rs` | the fixture document lays out and paginates with the bundled font, and the layout is deterministic |
| `tests/glyph_text.rs` | glyphs carry the characters of the text in order, and a ligature or a plain pair keeps its characters once |
| `tests/typography.rs` | kerned pairs measure narrower than their letters apart, and kerning and standard ligatures apply by default |

## Fonts

`fonts/NotoSans-Regular.ttf` and `fonts/NotoSans-Italic.ttf` are Noto Sans, both variable fonts,
under the SIL Open Font License 1.1. The licence text is in [`fonts/OFL.txt`](fonts/OFL.txt).

## License

Dual-licensed under MIT OR Apache-2.0 for the crate's code. The bundled fonts are under the SIL
Open Font License 1.1 (see above). See [`../../LICENSE-MIT`](../../LICENSE-MIT) and
[`../../LICENSE-APACHE`](../../LICENSE-APACHE).
