# tpt-codex-app

The application shell for tpt-codex: the editor's state and user interface, local document
persistence, one-click PDF export, and the presence state that shows other people's carets. The
editor is built on `tpt-appfront` and runs natively, so it can be driven and tested without a
browser. On `wasm32` it can also be mounted in a page.

## Status

Pre-release: version `0.1.0-spec`, `publish = false`. Everything described here is built and
tested. The limits are listed under [What it does not do](#what-it-does-and-what-it-does-not).

## What it does, and what it does not

It does:

* **Editor state.** `Editor` holds a `Document`, the caret, the selected grid cell and any slide
  block selection. `Editor::update` applies a `Msg` (typing, moving, editing a grid cell, inserting
  tables, lists, queries and slides, undo and redo, presenter mode, and more), and each change is
  one undoable step where the operation says so.
* **Editor view.** `Editor::view` returns an `tpt_appfront_core::UITree<Msg>`. It renders to
  semantic HTML through `tpt-appfront-html`, which the tests inspect. Grid blocks show evaluated
  cell values, and `[[sheet1.B4]]`-style bindings in prose show their values. Math is drawn with the
  math layout, and inline marks (emphasis, links, inline math) are drawn in place.
* **Paginated view.** `Editor::paginated_view` and `paginated_view_for_class` lay the document out
  as pages, using the page size and margins of the document's class, with headers and page-numbered
  footers.
* **Persistence.** `DocumentStore` saves and loads named documents as JSON through any
  `tpt_codex_core::Storage`. Every save and load validates the document, so an invalid document is
  never written and never returned. `FileStorage` is a file-system backend whose writes are atomic
  (a temporary file, then a rename).
* **PDF export.** `export::export_pdf` writes the document as a PDF on its class's page size, with a
  `Page N of M` footer.
* **Presence.** `Presence` keeps the newest caret of each peer for one replica, and produces the
  presence messages to send. The editor draws the peers' carets through `set_remote_cursors`.
* **Query forms.** `query_form` holds the draft behind a query block's form. The draft is editor
  state, not document state, so changing it is not undoable; applying it is.

It deliberately does not:

* Talk to the network. The relay connection is `tpt-codex-sync`'s job, and the browser page owns
  the socket. `Presence` only receives and produces messages.
* Choose a storage location for the user. The caller picks the `Storage` backend and key names.
* Keep documents in a database. Documents are whole JSON files; there is no partial save.
* Export pictures in PDF. `export_pdf` passes no image source, so every picture is drawn as its
  placeholder. The PDF crate can take an image source (`tpt_codex_pdf::export_with_images`), but
  this path does not use it.
* Edit the cells of a table of plain text. Such a table is shown as its rows.

## Where it sits in the workspace

Depends on:

* `tpt-codex-core` (the document model, `Storage`, `Replica`, `Message`)
* `tpt-codex-markdown`, `tpt-codex-views`, `tpt-typeset`, `tpt-codex-grid`, `tpt-codex-text`,
  `tpt-codex-pdf`
* `tpt-appfront-core` and `tpt-appfront-html`; on `wasm32` also `tpt-appfront-dom` and `web-sys`
* `serde_json`

Dev-dependencies: `tpt-codex-sync` (the presence test talks through a real relay) and
`tpt-codex-markdown`. Neither is part of the wasm build.

Used by:

* `tpt-codex-web`, which mounts the editor in the browser (`web::Controller` and the `mount_*` functions).
* `tpt-codex-acceptance`, the end-to-end checks.

The command-line tool (`tpt-codex-cli`) and the file crate (`tpt-codex-files`) do not depend on this
crate.

## Features and key public API

The table lists the public items and the module each one lives in. The most used are re-exported
from the crate root.

| Item | Module | What it is |
|------|--------|------------|
| `Editor`, `Msg`, `Arrow`, `CellFocus`, `RemoteCursor` | `editor` | The editor state, its messages, and the caret and cell types it reports. |
| `export::export_pdf(doc)` | `export` | One-click PDF export. Returns the bytes. |
| `DocumentStore<S>`, `PersistError` | `persistence` | Validated save and load over a `Storage`. |
| `FileStorage` | `file_storage` | A `Storage` on the local file system. |
| `Presence` | `presence` | Peer carets from relay messages, and the caret announcements to send. |
| `query_form::QueryDraft`, `Test`, `DraftChange`, `SOURCES`, `FIELDS`, `TESTS` | `query_form` | The query block form's values and the choices it offers. |
| `web::mount_sample`, `web::Controller` | `web` (`wasm32` only) | Mounts the editor in a page and returns a controller to send messages to. |

`Msg` covers the editing operations, and the presenter and slide operations. Some examples: `Type`,
`Backspace`, `Move`, `EditCell`, `InsertTable`, `AddListItem`, `InsertSlide`, `StartPresenting`,
`SetQuery`, `MoveSlide`, `Undo`, `Redo`. `Editor::update` is the only way to change the document.

A typical native session:

```rust,ignore
use tpt_codex_app::{DocumentStore, Editor, FileStorage, Msg};
use tpt_codex_core::Document;

// Edit a document in memory.
let mut editor = Editor::new(document);
editor.update(Msg::Type("Hello".into()));
editor.update(Msg::Undo);

// Save it to a directory, and read it back.
let mut store = DocumentStore::new(FileStorage::new("documents"));
store.save("notes", editor.document())?;
let loaded = store.load("notes")?;

// Export it as a PDF.
let pdf: Vec<u8> = tpt_codex_app::export::export_pdf(&loaded)?;
```

Names for `DocumentStore` must be non-empty and use only ASCII letters, digits, `-` and `_`.

A presence session, given a relay connection that the caller owns:

```rust,ignore
use tpt_codex_app::{Editor, Presence};
use tpt_codex_core::Message;

let mut presence = Presence::new(1);
// Each frame from the relay:
presence.receive(incoming_message);
// After each change of the caret, send whatever this returns:
if let Some(message) = presence.outbound_for(editor.caret()) {
    send(message);
}
// The other people's carets, to draw:
editor.set_remote_cursors(presence.remote_cursors());
```

## Platform notes

* The crate is `std`. It is not `no_std`. The headless crates it builds on (`tpt-codex-core`,
  `tpt-codex-views`, `tpt-codex-markdown`, `tpt-typeset`) are checked for `no_std` in the root
  `justfile`; this crate is not.
* On `wasm32-unknown-unknown` the `web` module and the DOM dependencies are compiled in. The
  demo page is built by `scripts/build-web.sh` (see the `tpt-codex-web` README).
* The crate forbids `unsafe` code, and warns on missing documentation.

## Errors

* `DocumentStore` returns `PersistError`: `InvalidName`, `NotFound`, `Storage` (the backend's
  error), `Json` (the stored bytes are not a document) or `Invalid` (the document fails
  validation). A load never returns a document that did not validate.
* `export_pdf` returns `Err(String)` when the document cannot be laid out or written as a PDF.
* `Editor::update` does not return errors. A command that cannot be applied, such as editing with
  no caret or setting a bad cell, leaves the document unchanged and sets a status message, which
  `Editor::status()` returns.
* `FileStorage` refuses keys that are absolute, contain `..` or a drive prefix, or are empty, with
  `InvalidInput`.

## Testing

```text
cargo test -p tpt-codex-app
```

| Test file | What it covers |
|-----------|----------------|
| `tests/acceptance.rs` | The headless path: Markdown in, saved, outlined, laid out, paginated, edited with undo, and written back to Markdown. |
| `tests/editor.rs` | The editor's messages and view: typing, caret, undo, grid cells and their formulas, tables, arrow keys, and the rendered HTML. |
| `tests/inserts.rs` | Inserting a list, a query and a slide text box, each as one undoable step. |
| `tests/lists.rs` | Adding and removing list items, switching a list between bulleted and numbered, and the list controls in the view. |
| `tests/media.rs` | SVG and URI media blocks, and that SVG markup cannot break out of its attribute. |
| `tests/persistence.rs` | Save and load through memory and the file system, overwrite and delete, refusal of invalid documents, corrupt JSON, and name and key checks. |
| `tests/presence_relay.rs` | Two editors exchanging carets over a real relay, including a late joiner. |
| `tests/queries.rs` | Setting a query block's source from a structured spec, and refusing an invalid one. |
| `tests/query_form.rs` | The query form: draft changes that do not touch the document, applying a draft, the `one of` test, and cached views. |
| `tests/reactive_view.rs` | That the cached view re-renders only the blocks that changed, and matches a fresh render. |
| `tests/typing_session.rs` | A headless typing session with the caret, undo, an equation block, and save and reload. |

Unit tests in `src/export.rs` and `src/presence.rs` check the PDF's page count and determinism, and
presence updates from peers.

The browser checks in `crates/tpt-codex-web/browser` run this crate's editor in Chrome.

## License

Dual-licensed under MIT OR Apache-2.0. See the root [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
