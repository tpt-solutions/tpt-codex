# Changelog

All notable changes to this crate are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate adheres to semantic versioning once it is released.

## [Unreleased]

### Added

- `open` and `save` by file extension, from and to bytes, for `.cdx`, `.md`, `.markdown`, `.docx`, `.xlsx`, `.pptx`, `.json` and `.pdf`. `format_of` gives the format of a name. Extensions are matched without regard to case.
- `.pptx` opens as text only. It cannot be saved.
- `.xlsx` opens with every worksheet as a heading and a grid. It saves the first grid as one worksheet.
- `.pdf` opens with its text, with a fidelity report. It saves with a text layer.
- `.docx` opens and saves with a fidelity report. Formula cells and `[[…]]` bindings are written as their computed values.
- `save_with_images`: a PDF draws each picture whose address the caller can resolve to bytes.
- Every opened document is checked with `Document::validate` before it is returned.
- `Fidelity` and `FidelityReport` are re-exported from `tpt-codex-interop`.
