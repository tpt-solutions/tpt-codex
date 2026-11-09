# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace-wide history is in the
root `CHANGELOG.md`.

## [Unreleased]

### Added

- Document data model: `Block` (text, grid, media, container, query, list, code, quote, table,
  thematic break and display math), `Document` with metadata, and `BlockId` and `SheetId`.
- Editing: `Op` and `apply` with inverses, cycle-safe moves, and `History` with undo and redo.
  `changed_blocks` reports the blocks an edit touched.
- Character-based text editing (`edit_text`, `replace_chars`) and a caret and selection model
  (`Selection`, `insert_text`, `delete_backward`, `delete_forward`).
- Inline runs on text blocks: emphasis and links on character ranges that move with the text.
- Live grids: `Sheet`, `CellInput`, and `Op::SetCell` and `Op::SetSheet`, with an optional minimum
  size and header rows. Address helpers for A1 notation.
- Canvas and frame model for slides: `Canvas` (with `Canvas::WIDESCREEN`), frames for children of
  absolute containers, slide sections and `SetSection`.
- Footnotes, figure attributes, equation and table labels, and cell names, each validated.
- Block-tree CRDT (`TreeCrdt`) and text CRDT (`TextCrdt`), with `project_document` and `materialize`.
- Replication: `Replica` (an outbox, acknowledgements, duplicate suppression and held out-of-order
  operations), a `Transport` trait, `Session`, presence cursors, and `Relay`, a per-document
  operation log for a server.
- Three-way merge of two diverged documents (`three_way_merge`, `three_way_merge_with`), with
  `Policy` and a list of conflicts.
- `VersionLog` with labelled snapshots, restore, a block-level diff and a word-level text diff.
- `History` tracks the last changed blocks and sheets, so a host can rebuild only what changed.
- `Clock` and `Storage` traits, with `FixedClock` and `MemoryStorage`.
- JSON serialization through serde, in a deterministic order.

### Changed

- Document schema version 4. Adds `Block::Table` (plain-text rows outside a grid) and
  `Document.cell_names`. Builds that read only version 3 refuse version 4 documents.
- Document schema version 3. Adds `Block::Quote` and sub-lists (a `List` block as an item of
  another list). Builds that read only version 2 refuse version 3 documents.
- Document schema version 2. Adds `Block::List`, `Block::Code`, `Block::ThematicBreak` and
  `Block::Math`. No version 1 document was released.
- Repeated grid header rows are stored on `Sheet.header_rows` and replicated with the grid's size,
  replacing a document-wide header setting in the flow options.
