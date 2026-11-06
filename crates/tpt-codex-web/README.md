# tpt-codex-web

The browser entry point: a `cdylib` that mounts the sample editor in a page.

## Build the demo

```sh
cargo build -p tpt-codex-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir target/web/pkg target/wasm32-unknown-unknown/debug/tpt_codex_web.wasm
cp crates/tpt-codex-web/web/index.html target/web/index.html
```

The `wasm-bindgen` CLI must be the same version as the `wasm-bindgen` library in `Cargo.lock`
(0.2.130 at the time of writing).

## Browser test

`browser/browser-test.mjs` opens `web/index.html` in Chrome through Playwright and checks the grid,
formulas, undo, paragraph typing, marks, and slides. It serves the files by routing requests for a
made-up origin, so it starts no server.

```sh
cd crates/tpt-codex-web/browser
npm install
set CHROME_PATH=C:\Program Files\Google\Chrome\Application\chrome.exe   # optional; Playwright's Chromium otherwise
set WEB_DIR=..\..\..\target\web                                          # the folder built above
npm test
```

`browser/presence-test.mjs` (`npm run test:presence`) opens two demo pages joined to a local relay
and checks that one page's caret is drawn on the other. It needs the relay binary:

```sh
cargo build -p tpt-codex-sync --bin tpt-codex-relay
```

It serves the pages from `localhost` on port 8799 and starts the relay on 127.0.0.1:18787. Chrome
blocks a page from reaching a local relay unless the page is served from a local origin too, so the
test does not use a made-up host name.
