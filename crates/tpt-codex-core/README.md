# tpt-codex-core

The headless data model of tpt-codex. A document is a directed acyclic graph of `Block` nodes
held in a `Document`. This crate defines that graph, the operations that change it, undo and
redo, the text and tree CRDTs that let two replicas edit it and converge, a version log, and the
three-way merge of diverged documents. It has no layout, no rendering and no I/O.

## Status

Pre-release. The workspace is at version `0.1.0-spec` and the crate is `publish = false`. The
public API is still moving, and the document schema is versioned (`SCHEMA_VERSION`, currently
`4`; see [docs/versioning.md](../../docs/versioning.md)). A build reads only the schema versions it
knows, and refuses a newer one.

## What it does

- **Document model.** `Block` has these variants: `Text` (characters plus a `TextStyle` with
  inline runs), `Grid` (a live sheet, optionally a window onto it), `Media`, `Container` (flow,
  grid or absolute layout, with children), `Query` (stores its source text only), `List` (ordered
  or bulleted, items may be sub-lists), `Code`, `Quote`, `Table` (plain-text rows), `ThematicBreak`
  and `Math` (display math, stored as source). A `Document` holds the blocks, a root, metadata
  (title, author, date, class), sheets, frames and canvases for slides, slide sections, footnotes,
  figure and equation and table labels, and named cells.
- **Edits.** `Op` is one change: `AddBlock`, `RemoveBlock`, `SetBlock`, `InsertChild`,
  `RemoveChild`, `MoveChild`, `SetCell`, `SetSheet`, `SetFrame`, `SetCanvas` and `SetSection`.
  `apply` performs one and returns its inverse. `History` groups operations into steps and gives
  `commit`, `undo` and `redo`, with a failed step rolled back. `changed_blocks` reports which
  blocks a set of operations touched, so a host can rebuild only those.
- **Text editing.** `Selection` holds a caret or a range, with `move_caret`, `select_all`,
  `insert_text`, `delete_backward` and `delete_forward`. Positions count `char`s. Helpers
  `replace_chars`, `edit_text`, `remap_runs` and `clip_runs` keep inline runs on the right
  characters when text changes.
- **Lists.** `lists::add_item`, `lists::remove_item` and `lists::set_kind` return the operations
  for a list edit (`add_item` also returns the new item's id).
- **Sheets.** `Sheet` stores cell inputs (`CellInput`) by A1 address, a minimum size, and header
  rows. `address_of`, `address_position`, `is_valid_address` and `is_formula_source` handle
  addresses. Evaluation is not here; `tpt-codex-grid` does it.
- **Replication.** `TextCrdt` (characters with Lamport stamps and tombstones), `TreeCrdt` (the
  block tree, with moves that cannot form a cycle and fractional child keys from `key_between`),
  and `project_document`, which turns a replica back into a `Document`. `Replica` combines them
  with an outbox, acknowledgements, duplicate suppression and held out-of-order operations. Grid
  cells and sheet sizes are last-writer-wins by stamp. `Session` drives a `Replica` over a
  `Transport`, and `Message` carries operations, acknowledgements and presence cursors.
- **Relay log.** `Relay` keeps an ordered, de-duplicated log per document name, for a server to
  replay to late joiners.
- **Versions and diff.** `VersionLog` saves labelled snapshots, restores them, and reports a
  block-level diff (`diff_documents`) and a word-level text diff (`diff_text`).
- **Merge.** `three_way_merge` and `three_way_merge_with` combine two documents that both diverged
  from a base. A `Policy` (`KeepOurs`, `KeepTheirs` or `Manual`) settles conflicts, and each
  `Conflict` says where it was and whether it was resolved.
- **Host hooks.** The `Clock` and `Storage` traits are supplied by the host. `FixedClock` and
  `MemoryStorage` are in-memory implementations for tests and deterministic output.

## What it deliberately does not do

- No layout, measuring or drawing. Those are `tpt-codex-views` and the text and PDF crates.
- No network. `Transport` is the boundary; a WebSocket transport and relay server are in
  `tpt-codex-sync`.
- No query language. `Block::Query` keeps its source; `tpt-query` runs it.
- No formula evaluation. `tpt-codex-grid` evaluates sheets.
- No file formats. Markdown is `tpt-codex-markdown`; Office and PDF are the interop, files and
  pdf crates.
- No interpretation of math source. `tpt-typeset` parses it.

Known limits of the merge: a block that one side removed from its container, while the other side
edited it, keeps the edited value as an unreferenced block if that edit is chosen. It is not put
back into the container. Attribute entries (frames, footnotes, sections, figures, canvases) of a
block that does not survive the merge are dropped without a conflict.

## Where it sits

`tpt-codex-core` depends only on `serde` (no default features, with `derive` and `alloc`) and
has `serde_json` as a dev-dependency for its JSON tests.

Depended on by: `tpt-codex-views`, `tpt-codex-markdown`, `tpt-codex-grid`, `tpt-codex-interop`,
`tpt-codex-text`, `tpt-codex-pdf`, `tpt-codex-files`, `tpt-codex-sync`, `tpt-typeset`,
`tpt-codex-app`, `tpt-codex-cli` and `tpt-codex-acceptance`.

## Key public API

```rust,ignore
use tpt_codex_core::{
    Block, BlockId, Document, History, LayoutMode, Op, Replica, TextStyle,
};

// A document whose root is a flow container with no children yet.
let root = Block::Container { layout_mode: LayoutMode::Flow, children: Vec::new() };
let mut doc = Document::new(BlockId(0), root);

// One undoable step: add a paragraph and attach it to the root.
let mut history = History::new();
history.commit(&mut doc, vec![
    Op::AddBlock {
        id: BlockId(1),
        block: Block::Text { content: "Hello".into(), style: TextStyle::default() },
    },
    Op::InsertChild { parent: BlockId(0), index: 0, child: BlockId(1) },
])?;
history.undo(&mut doc)?; // the paragraph is removed again
doc.validate()?;

// Two replicas: each edits its own copy and sends operations to the other.
let mut alice = Replica::new(1);
let (node, envelopes) = alice.create_block(
    tpt_codex_core::ROOT, 0,
    Block::Text { content: String::new(), style: TextStyle::default() },
)?;
```

The main entry points are:

- `Document::new`, `Document::get`, `Document::insert`, `Document::validate`
- `apply`, `History::{new, commit, undo, redo}`, `changed_blocks`
- `Selection`, `insert_text`, `delete_backward`, `delete_forward`
- `Replica::{new, create_block, insert_text, delete_text, set_cell, set_sheet_size, receive, pending, acknowledge}`
- `Session::{new, send, pump, disconnect, reconnect}` with a `Transport` implementation
- `Relay::{new, publish, since, len}`
- `VersionLog::{new, save, restore, diff, text_diff}`, `diff_documents`, `diff_text`
- `three_way_merge`, `three_way_merge_with`, `Policy`, `Merge`, `Conflict`

The full list is in `src/lib.rs`.

## no_std, std and WASM

The crate is `#![no_std]` with `extern crate alloc`, and `#![forbid(unsafe_code)]`. It uses
`BTreeMap` and `BTreeSet` throughout, so iteration and serialization order are deterministic. The
root README lists `cargo build` checks for `thumbv7m-none-eabi` and `wasm32-unknown-unknown` that
include this crate.

## Errors

Each area has its own error type, and none of them panic on bad input:

- `ValidationError`: a document that breaks the schema, for example an unsupported schema version,
  a missing root, a dangling child, a cycle, or an invalid label, cell name or footnote.
  `Document::validate` returns it.
- `EditError`: an operation that cannot apply, such as a removal of the root, a child that does not
  exist, a block that is not text, a sheet that a grid still shows, or an address out of range.
- `TreeError`, `TextCrdtError`, `SyncError`: a local change to a replica that was refused, such as
  an invalid cell address or a formula that does not start with `=`.
- `MergeError::Invalid`: the merged document failed validation.

## Determinism and security

There is no randomness and no reading of the clock inside the crate. Callers pass a `Clock` where
they need a time. Replica identifiers are supplied by the caller, and each editing session should
have its own.

## Testing

```sh
cargo test -p tpt-codex-core
```

- `tests/model.rs`, `ops.rs`, `positions.rs`, `selection.rs`, `text.rs`, `inline_runs.rs`: the
  data model, operations, undo and redo, caret movement, and text edits.
- `tests/lists.rs`, `quotes_and_sublists.rs`, `sheet.rs`, `cell_names.rs`, `labels.rs`: list
  helpers, the schema-version-3 and -4 shapes, sheets, cell names and labels.
- `tests/tree_crdt.rs`, `text_crdt.rs`, `sync.rs`, `session.rs`, `merge.rs`, `versions.rs`:
  replication, convergence under reordered and duplicated delivery, the session outbox, merging
  and version diffs.
- `tests/properties.rs`, `json_mutation.rs`: randomized property tests for the model, and a
  seeded mutation test of document JSON.
- `tests/platform.rs`: the `Clock` and `Storage` helpers.

The fuzz harness under `fuzz/` is excluded from the workspace. Its `document_json` target
exercises this crate's JSON reader.

## Licence

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
