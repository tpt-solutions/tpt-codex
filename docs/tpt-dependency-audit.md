# TPT dependency audit (Phase 0)

Audit of the `tpt-solutions` crates against the strict licensing gate and the no-C-FFI rule.
Method: shallow-cloned each repository, resolved the candidate crates by path in a scratch
workspace, then ran `cargo metadata` through [scripts/check_licenses.py](../scripts/check_licenses.py)
with `--sys-roots` set to the TPT crates. Nothing was added to this repository's dependencies.

## Scope and limits

- **Verified:** repositories exist on GitHub. The crates are not on crates.io (index lookups return 404),
  so they can only be consumed as git or path dependencies. License gate and `-sys` closure were run
  against the resolved graph.
- **Not verified:** `no_std` and `wasm32` builds of the TPT crates. Building them runs their build
  scripts and compiles external code, which needs an explicit go-ahead. The `no_std` attribute in their
  source is a claim, not a check.

## Results

| Crate | License | `no_std` attribute in source | Resolved closure (normal deps) | License problems in closure |
| --- | --- | --- | --- | --- |
| `tpt-model` (`tpt-model/crates/tpt-model`) | MIT OR Apache-2.0 | yes | 22 dependencies, includes `tpt-crypto-stub` | `blake3`, `constant_time_eq` (CC0 / MIT-0 alternatives) |
| `tpt-text` (`tpt-text/crates/tpt-text`) | MIT OR Apache-2.0 | yes | 16 dependencies | none on local `master` of the tpt-text clone (commits `9a110a6`, `f0ebd33`; not pushed); `self_cell` removed |
| `tpt-layout` | MIT OR Apache-2.0 | yes | 0 dependencies | none |
| `tpt-canvas` (`tpt-canvas/crates/tpt-canvas`) | MIT OR Apache-2.0 | yes | 1 dependency | none |
| `tpt-canvas-pdf` | MIT OR Apache-2.0 | no | 3 dependencies | none |
| `tpt-lattice-core` | MIT OR Apache-2.0 | yes | 1 dependency | none |
| `tpt-lattice-evaluator` | MIT OR Apache-2.0 | no | 19 dependencies | none |
| `tpt-query` | MIT OR Apache-2.0 | yes | 0 dependencies | none |
| `tpt-version` | MIT OR Apache-2.0 | yes | 11 dependencies | none |
| `tpt-replicate` | MIT OR Apache-2.0 | yes | 0 dependencies | none |

No resolved closure contains a `-sys` crate. Not yet audited: `tpt-appfront-*` (only `-core`, `-dom`,
`-canvas`, `-html`, `-webview` are needed, and none of them is audited yet), `tpt-doc-*`, `tpt-lattice-*`
crates other than the rows above.

"Closure" counts the packages reached through normal (non-dev) dependency edges, excluding the crate itself. The license gate checks the same set: packages reached from workspace members through normal or build edges. Development-only dependencies are not checked.

## Blockers and decisions needed

1. **`self_cell` (via `tpt-text`): resolved.** Committed to local `master` of the tpt-text clone
   (`9a110a6`, `f0ebd33`, `9c794eb`; not pushed). Font bytes are owned, character maps are read once,
   and `LayoutSession` keeps shaping faces across paragraphs. Latin, Arabic and short-paragraph layout are
   at or below the old `self_cell` cache speed on this machine.
2. **`blake3` and `constant_time_eq` (via `tpt-crypto-stub` → `tpt-model`): resolved.** Committed to local
   `master` of the tpt-model clone (`f0a8abe`; not pushed). The default digest is SHA-256 (`sha2`) and keyed
   digests are HMAC-SHA256 (`hmac`). Envelope format 2 names the digest, and format 1 (BLAKE3) envelopes are
   refused. Neither `blake3` nor `constant_time_eq` appears in the lock file anymore.
3. **tpt-model development tooling: resolved by policy.** `criterion` and `proptest` (development-only) pull in
   Apache-2.0-only crates. Development dependencies are outside the gate, so the shipped closure is what counts.

## Update 2026-11-04: status of the blockers

The TPT repositories are on GitHub and the audit's local commits are on their `master` branches.

- **`self_cell` (tpt-text)**: resolved on GitHub master (`83bc13c`).
- **`blake3` (tpt-model)**: resolved on GitHub master (`0c999f3`).
- **tpt-model development tooling**: unchanged; development-only dependencies are outside the gate.
- **New blocker: `*-stub` crates in tpt-model.** `tpt-crypto-stub`, `tpt-primitives-stub`, `tpt-uir-stub`
  and `tpt-validex-stub` are unconditional normal dependencies of `tpt-model`. The rule is that stubs must
  not reach the shipped closure. Fix in the `tpt-model` repository, then re-run the gate. Until then
  `tpt-model` is not adopted.
- **New finding: `tpt-canvas-pdf` glyph runs are placeholders.** PDF output would have no text. Not adopted
  until upstream draws glyph runs.
- **Adopted, pinned by rev:** `tpt-query` (typeset), `tpt-lattice-core` and `tpt-lattice-evaluator` (grid).
  The license gate passes on the resolved closure of 31 packages, and no `-sys` crate is in it. The
  `no_std` WASM and thumbv7m builds of core, views, markdown and typeset pass.
- **Cargo noise:** cargo prints `invalid character` errors for the template manifests in the TPT git
  checkouts (`templates/*/Cargo.toml`). The build still succeeds (exit 0). Nothing in this repository
  can fix it.

## Update 2026-11-04 (second pass): doc, appfront and xlsx

Each crate was probed alone through the gate, on its resolved closure.

| Crate | Gate | Reason |
| --- | --- | --- |
| `tpt-doc-core`, `-pdf`, `-convert` | OK | adopted-ready |
| `tpt-doc-word` | fails | `zopfli` (Apache-2.0 only) via `zip`'s `deflate` feature |
| `tpt-doc-spreadsheet` | fails | `zopfli`, and `ryu` (`Apache-2.0 OR BSL-1.0`; BSL not allowed) |
| `tpt-appfront-core`, `-dom`, `-html` | OK | adopted (app shell, WASM-first) |
| `tpt-appfront-canvas`, `-webview` | fails | Apache-only GUI stacks (`winit`, `glutin`, `tao`) |
| `tpt-lattice-import-xlsx` / `-export-xlsx` | fails | `calamine` uses `zip` with `deflate`, which pulls `zopfli`; `zip`'s default features add `bzip2`, `lzma`, `ppmd` and `aes` |

The `zip` problem needs upstream fixes: `zip` with `default-features = false` and `deflate-flate2`, in
`tpt-doc` and `tpt-lattice`. `calamine` needs the same change in its own manifest, or a different reader.
These are not fixable from this repository, because cargo unifies features across the graph.

## Update 2026-11-05: the zip problem, solved in-house

The Office stack no longer uses the `zip` crate, `calamine`, or the lattice xlsx crates. `tpt-codex-interop`
has its own ZIP container code (`archive`) and an xlsx reader and writer (`xlsx`). The new dependencies are
`flate2` with the `rust_backend` feature (MIT OR Apache-2.0, with `miniz_oxide` under MIT OR Zlib OR Apache-2.0),
`crc32fast` (MIT OR Apache-2.0) and `quick-xml` (MIT). Each passes the license gate on its own and in the
resolved closure (`zip` and `zopfli` are not in `Cargo.lock`). The `tpt-doc-word` and `-spreadsheet` crates are
still blocked by their own `zip` features. The same `archive` module could read `.docx` parts.

## Update 2026-11-05: tpt-doc, and a small docx reader

- **tpt-doc, `zip` feature.** `zip` 9's `deflate` feature enables `zopfli` (Apache-2.0 only) and `zlib-rs`.
  Switching the workspace to `deflate-flate2` passes the gate for `tpt-doc-word`, `-core`, `-pdf` and
  `-convert`. The change is one line, committed on branch `gate-zip-deflate-flate2` in `tpt-solutions/tpt-doc`
  (not `master`). The push was refused: this account has no write access to that repository. Someone with access
  needs to push the branch, or apply the patch. Until then the crate cannot be pinned.
- **tpt-doc-spreadsheet** also fails on `ryu` (`Apache-2.0 OR BSL-1.0`), which comes in through `csv`, unconditionally.
  It is not needed, because xlsx is covered in-house.
- **docx.** We need text, headings and basic formatting from `.docx`, and `tpt-doc-word` is not needed for that.
  A small reader on our own `archive` module covers it, so we don't depend on `tpt-doc-word`. Its loss items are
  recorded in the fidelity report.

## Update 2026-11-06: tpt-model stub blocker cleared

- The `*-stub` crates are folded into `tpt-model` as modules (`uir`, `crypto`, `validex`, `primitives`), and
  `tpt-uir-stub`, `tpt-validex-stub`, `tpt-crypto-stub`, `tpt-primitives-stub` and `tpt-testkit-stub` are removed
  from the workspace. Commit `59699b3` is on GitHub master. Its normal dependencies are serde, uuid, sha2, hmac
  and `tpt-model-derive`.
- Checked locally: `cargo test -p tpt-model` passes (all suites), `cargo tree -p tpt-model` shows no stub crate,
  and `scripts/check_licenses.py` passes on its `cargo metadata`.
- tpt-codex does not depend on `tpt-model` yet, so its closure is unchanged and the gate for this repository is not affected.

## Update 2026-11-06: upstream fixes checked on GitHub

- **`tpt-canvas` glyph runs.** Fixed upstream at `e451560` (glyph runs with a glyph source are filled as outline
  paths). Our `Cargo.toml` already pins that rev for `tpt-canvas` and `tpt-canvas-pdf`, so the PDF export draws text.
- **`tpt-doc` zip feature.** Fixed on tpt-doc `master` (`cc16a28`, `zip` with `deflate-flate2-zlib-rs`). The license
  gate passes on the closures of `tpt-doc-pdf` (9 packages) and `tpt-doc-convert` (22 packages). The
  `gate-zip-deflate-flate2` branch mentioned above is no longer needed. Not adopted: `.docx` and `.xlsx` are in-house.
- **`tpt-doc-pdf` has no parser.** It writes PDFs only, so it cannot provide `.pdf` import. That needs a new parser
  or another dependency, which is a decision for you.

## Next steps

- No licensing decisions are open.
- Push the local commits when ready: tpt-text `master` (`9c794eb` and earlier) and tpt-model `master` (`f0a8abe`).
- Once approved, run `cargo build --target thumbv7m-none-eabi` and `wasm32-unknown-unknown` on the
  candidate crates to verify the `no_std` attribute.
- Re-run this audit on every dependency bump.
