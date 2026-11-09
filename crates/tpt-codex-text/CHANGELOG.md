# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Text measurement with the bundled Noto Sans Regular (SIL Open Font License), shaped by `tpt-text`.
  `FontMeasure` implements the flow view's `TextMeasure` and `tpt-layout`'s `TextMeasurer`.
- Bundled Noto Sans Italic. Bold is the same variable font at weight 700.
- Math metrics: `FontMeasure` implements `MathMetrics`, with the layout constants of the bundled
  STIX Two Math font.
- `FontMeasure::shape_lines`, which returns each line's glyphs with their positions, advances and
  the characters each one stands for, and `FontMeasure::faces` for the faces those glyphs use.
- A glyph at a hyphenated line break carries "-", so the hyphen is in the text.

### Changed

- `GlyphPlacement` has two new fields, `advance` and `text` (the characters of its cluster), and no
  longer implements `Copy`. Code that built or copied glyph placements needs updating.
