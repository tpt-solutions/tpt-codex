# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace also has a root
`CHANGELOG.md` that covers the whole project.

## [Unreleased]

### Added

- `Editor` and `Msg`: the editor's state and its messages, with a view built as a `tpt-appfront` UI
  tree that renders to HTML. Undo and redo, caret movement, and typing.
- Local persistence: `DocumentStore`, which validates every save and load, and `FileStorage`, a
  file-system backend with atomic writes.
- One-click PDF export: `export::export_pdf`, on the page size and margins of the document's class,
  with a `Page N of M` footer.
- Editable tables: insert a table, edit grid cells, and move between cells with the arrow keys.
  Grid cells show their evaluated values, and `[[sheet1.B4]]` and named cells such as `[[Total]]`
  show their values in prose.
- Lists: insert a list, add and remove items, switch between bulleted and numbered, and draw
  sub-lists and block quotes.
- Editable query blocks, with a query form for the conditions, sort order and limit. The form's
  `one of` test is included. Applying a form is one undoable step.
- Slides: insert slides and text boxes, select, move and resize blocks, drag blocks with the
  pointer, reorder slides, put slides in sections, and present one slide at a time.
- The paginated view, which draws pages with headers and page-numbered footers.
- Inline marks drawn in place, with link addresses as tooltips, and inline math drawn with the math
  layout.
- Live presence: `Presence` keeps each peer's newest caret, and the editor draws the remote carets.
- Mounting in a browser page on `wasm32` (`web::mount_sample`, `web::Controller`), including opening
  and saving files through the controller.

### Changed

- A focused paragraph in the browser is a native text area. Each change to it is applied as a
  `ReplaceText` message.
- A table of plain text is shown as its rows. Its cells are not edited in the editor.

### Fixed

- Focusing a grid cell keeps the paragraph caret, so the cell keeps focus.
- Crate docs no longer describe finished work as "not yet implemented".
