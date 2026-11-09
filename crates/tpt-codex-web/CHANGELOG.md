# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The workspace also has a root
`CHANGELOG.md` that covers the whole project.

## [Unreleased]

### Added

- A WebAssembly module that mounts the editor on a page: `mount_sample`, `mount_sample_as` and the
  `EditorHandle` the page drives.
- A static demo page (`web/index.html`) and a Chrome browser test (`browser/`) that drives it.
- Presence over a relay: `receive_sync`, `pending_sync`, `announce_again` and `replica` on the handle,
  and the `relay`, `doc` and `replica` URL parameters on the demo page. A presence test joins two pages
  to a local relay.
- `set_pages` (show the document as pages or as the editable flow) and `set_fullscreen` (mark the
  presenter stage full screen, with the browser's full-screen call made by the page).
- `export_pdf`, and an Export PDF button on the demo page.
- Opening and saving real files on the demo page, through `tpt-codex-files`: open `.cdx`, `.md`,
  `.docx`, `.xlsx`, `.pptx`, `.pdf` and `.json`, and save `.cdx`, `.docx`, `.xlsx`, `.pdf` and
  `.json`. An open shows the import's fidelity report.
- Autosave of the demo document to local storage, with a Start over button that restores the sample.
