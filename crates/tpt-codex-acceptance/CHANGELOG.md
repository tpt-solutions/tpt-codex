# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- New test-only crate with one test file per milestone in `todo.md`: `m0_foundation`,
  `m1_editor`, `m2_markdown_typeset`, `m3_paginated`, `m4_collaboration` and `m5_interop`.
- Golden render tests (`golden.rs`) for the slide SVG and the paginated layout, compared byte for
  byte with the files in `golden/`. `TPT_UPDATE_GOLDEN=1` rewrites them.
- Milestone 4: two replicas edit a paragraph and a live grid through a relay and converge.
- Milestone 5: a real `.docx` and `.xlsx` from `fixtures/office` are opened, edited, written back and
  read again.

### Changed

- The milestone 2 and 3 tests cover PDF export, including the one-click export being byte-exact
  across runs.
