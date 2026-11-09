# tpt-codex-web

The browser entry point for tpt-codex. It builds to a WebAssembly module (a `cdylib`, also an
`rlib`) that mounts the editor in a page, plus a static demo page (`web/index.html`) and the
Chrome checks that drive it.

## Status

Pre-release: version `0.1.0-spec`, `publish = false`. The demo page edits a built-in sample
document. It is a demonstration of the editor in the browser, not a hosted product.

## What it does, and what it does not

It does:

* Exports `mount_sample` and `mount_sample_as`, which mount the sample editor in a page element
  and return an `EditorHandle` the page drives.
* Lets the handle open and save files through `tpt-codex-files`: `.cdx`, `.md`, `.docx`, `.xlsx`,
  `.pptx` and `.pdf` (read only) and `.json` on open, and `.cdx`, `.md`, `.docx`, `.xlsx`, `.json` and `.pdf`
  on save. An open reports what the import dropped or approximated.
* Exports the document as PDF bytes (`export_pdf`), on the page size of its document class.
* Exchanges presence with a relay. The demo page can join a relay named in its URL and draws the
  other people's carets. It does not send or receive document operations through the relay.
* Keeps the demo document in the browser's local storage between visits (autosave, see below).

It deliberately does not:

* Serve files. Any static server will do; the root `justfile` uses Python's `http.server`.
* Sync documents. The demo page joins for presence only. Operations stay on the page.
* Authenticate anyone or protect the relay connection. The page's relay URL is whatever the
  query string gives it.
* Build for anything but `wasm32`. On other targets the library compiles to nothing, and the
  binding functions exist only on `wasm32`.

## Where it sits in the workspace

* Depends on `tpt-codex-app` (the editor and its wasm mount), `tpt-codex-files` (open and save of
  real file formats) and `wasm-bindgen`.
* Nothing in the workspace depends on it. It is the outermost crate of the browser build.
* The relay it talks to is `tpt-codex-sync`'s `tpt-codex-relay` binary. The browser does not link
  that crate; the page speaks the same JSON over a WebSocket.

## Exported API

These are `wasm-bindgen` exports, visible to JavaScript after `wasm-bindgen --target web`.

| Export | What it does |
|--------|--------------|
| `mount_sample(container_id)` | Mounts the sample editor in the element with that id, as replica 1. Returns an `EditorHandle`. |
| `mount_sample_as(container_id, replica)` | As above, as replica `replica`. Pages that share a relay document each need their own number. |
| `EditorHandle.set_fullscreen(on)` | Marks the presenter stage full screen, or not. The page calls the browser's full-screen API itself. |
| `EditorHandle.set_pages(on)` | Shows the document as pages, or as the editable flow. |
| `EditorHandle.receive_sync(text)` | Takes one relay frame as text. Presence moves the other carets. A frame that is not a message is an error the page can ignore. |
| `EditorHandle.pending_sync()` | The frames to send to the relay, one per line, or an empty string. |
| `EditorHandle.announce_again()` | Sends this editor's caret again. The page calls it once it has joined, because the relay keeps no presence. |
| `EditorHandle.replica()` | This editor's replica number. |
| `EditorHandle.open_file(name, bytes)` | Replaces the document with the file's contents. Returns the fidelity report as text, or an empty string when nothing was lost. On error the document is unchanged. |
| `EditorHandle.save_file(name)` | The document as the bytes of a file named `name`; the extension picks the format. |
| `EditorHandle.export_pdf()` | The document as PDF bytes. |

Errors come back to JavaScript as strings. Keep the handle for as long as the editor is on the page.

## The demo page

`web/index.html` is a single page with its script inline. It:

* Mounts the sample editor, and offers Open (or drop a file), Save, Export PDF and Full screen.
  The editor's own controls inside the page include Pages and presenter mode.
* Autosaves the document to `localStorage` under the key `tpt-codex-autosave`, using the same
  `.json` form as `save_file`, and restores it on the next visit. A failing storage call is
  ignored, so the page still works without it. Start over, which appears when a saved copy is
  restored, clears the saved copy and brings back the sample. The `fresh` query parameter skips
  the restore.
* Joins a relay when the URL has a `relay` parameter. The parameters are:

| Parameter | Meaning |
|-----------|---------|
| `relay` | The WebSocket URL of a relay, for example `ws://localhost:8787`. Without it the page does not connect. |
| `doc` | The document name to join. Default `demo`. |
| `replica` | The replica number for this page. Default `1`. Give each page on the same document its own number. |
| `fresh` | Ignore the autosaved copy. |

Autosave is per origin and per browser, and it is not encrypted. It holds the whole document in
plain text.

## Build the demo

The script does the steps below and checks the `wasm-bindgen` version for you:

```sh
scripts/build-web.sh                 # add `release` for a release build
just serve                           # builds, then serves http://localhost:8000
```

On Windows PowerShell:

```powershell
scripts\build-web.ps1                # or: scripts\build-web.ps1 -Profile release
python -m http.server -d target/web 8000
```

By hand:

```sh
cargo build -p tpt-codex-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir target/web/pkg target/wasm32-unknown-unknown/debug/tpt_codex_web.wasm
cp crates/tpt-codex-web/web/index.html target/web/index.html
```

The `wasm-bindgen` CLI must be the same version as the `wasm-bindgen` library in `Cargo.lock`
(0.2.130 at the time of writing). Install a matching CLI with
`cargo install wasm-bindgen-cli --version 0.2.130 --locked`.

## Run the presence demo

Start a relay, then open the demo page with the `relay` parameter, once per replica:

```text
cargo run -p tpt-codex-sync --bin tpt-codex-relay
http://localhost:8000/?relay=ws://127.0.0.1:8787&doc=demo&replica=1
http://localhost:8000/?relay=ws://127.0.0.1:8787&doc=demo&replica=2
```

Relay options and the security notes are in the `tpt-codex-sync` README. The relay has no
authentication, so keep it on the loopback address.

## Testing

Both browser checks need the demo built into `target/web` first. They use Playwright's
`playwright-core` and drive Chrome. Install the test dependencies once:

```sh
cd crates/tpt-codex-web/browser
npm install
```

`browser-test.mjs` (`npm test`) opens `web/index.html` and checks the sample document, grid cells
and formulas, undo, paragraph typing, mid-paragraph typing, arrow keys, input-method composition,
marks, slides and presenter mode, pages, lists, query blocks, text boxes on slides, PDF export,
opening and saving files (including a bad file and a `.docx` and `.pdf`), autosave and Start
over. It serves the files by routing requests for a made-up origin (`http://tpt-demo.test`), so
it starts no server.

`presence-test.mjs` (`npm run test:presence`) opens two demo pages joined to a local relay and
checks that one page's caret is drawn on the other. It starts the relay binary itself, so build it
first:

```sh
cargo build -p tpt-codex-sync --bin tpt-codex-relay
```

It serves the pages from `localhost` on port 8799 and starts the relay on 127.0.0.1:18787. Chrome
blocks a page from reaching a local relay unless the page is served from a local origin too, so the
test does not use a made-up host name.

Environment variables for both scripts:

| Variable | Meaning |
|----------|---------|
| `WEB_DIR` | The folder with the built demo. Default `target/web` at the repository root. |
| `CHROME_PATH` | A Chrome executable to use instead of Playwright's Chromium. Optional. |
| `RELAY_BIN` | The relay binary for the presence test. Default `target/debug/tpt-codex-relay`. |
| `RELAY_PORT`, `WEB_PORT` | The ports for the presence test. Defaults 18787 and 8799. |

In PowerShell, set them with `$env:CHROME_PATH = "C:\Program Files\Google\Chrome\Application\chrome.exe"`;
in `cmd`, use `set CHROME_PATH=...`.

## Errors

* A wasm function that fails returns a `JsValue` error carrying the message. The demo page shows
  it in the status line.
* `open_file` leaves the document unchanged when the file cannot be read. A file that opens with
  losses returns the report rather than failing.
* `receive_sync` fails on a frame that is not a message, such as the relay's join reply.

## License

Dual-licensed under MIT OR Apache-2.0. See the root [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
