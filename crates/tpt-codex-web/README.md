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
