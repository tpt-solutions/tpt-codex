# PDF fixtures

`chromium-print.pdf` is a file written by another program, Chromium (through Playwright's
`page.pdf`, A4, no background), from this HTML:

```html
<h1>Quarterly report</h1>
<p>The first paragraph has <b>bold</b> and <i>italic</i> words, and it wraps across more than one line so that the import can join the lines back into a paragraph with the words in order.</p>
<p>A second paragraph, with ligatures: first, office, efficient, flow.</p>
<ul><li>First item</li><li>Second item</li></ul>
```

It has compressed streams, object streams and a cross-reference stream, and embedded subset fonts
with ToUnicode maps. The tests in `crates/tpt-codex-interop/tests/pdf.rs` read it.

`sample.jpg` is a 16 by 16 pixel gradient, written with Pillow at quality 90. The picture tests
decode it.

`rgb.png`, `grey16.png`, `shapes.png` and `palette.png` are small PNG files written with Pillow:
an opaque RGB picture, a 16-bit grey one, one with a translucent half, and a palette picture.
