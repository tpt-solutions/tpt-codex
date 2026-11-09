# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- The `tpt` command: `convert`, `check`, `watch`, `diff`, `merge` and `new`, with `--help`.
- `tpt merge <BASE> <OURS> <THEIRS> -o <OUTPUT> [--prefer ours|theirs]`: a three-way merge of text
  files. Blocks are matched with the base before merging, so an added paragraph does not confuse it.
  Unresolved conflicts exit with status 1.
- `tpt new <TEMPLATE>` with four starter documents (`article`, `report`, `slides`, `budget`) and
  `tpt new --list`.
- `convert` reads `.pdf` files as text (paragraphs and headings) and writes `.pdf` files with
  pictures drawn from the input's folder.
- `tpt check --bib FILE` checks citations against a CSL-JSON or BibTeX bibliography.

### Changed

- `convert` and `watch` read and write files through `tpt-codex-files`, which the browser demo also
  uses.
- `tpt check` reports `[[name]]` bindings with no value, and says how grid cells and queries are
  named.
- `tpt check` reports grid formula errors.
