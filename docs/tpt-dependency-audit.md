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

"Closure" counts the packages reached through normal (non-dev) dependency edges, excluding the crate itself.

## Blockers and decisions needed

1. **`self_cell` (via `tpt-text`): resolved, committed to local `master`, not pushed.** `tpt-text` owns its
   font bytes and builds shapers once per paragraph. The full `tpt-text` closure passes the gate. Remaining
   cost: about one face parse (about 1.2 ms for Noto Sans) per font per paragraph, so latin layout is about
   25 to 30 percent slower than the old cache. Removing that needs a caller-owned layout session. Tracked in
   `tpt-text`'s `TODO.md`.
2. **`blake3` (via `tpt-crypto-stub` → `tpt-model`): blocked, needs a decision.** `blake3` is licensed
   `CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception`. It has no MIT option, and CC0 is not allowed,
   so it cannot be used anywhere in the graph, including tests and examples. Removing it means changing
   `tpt-model`'s default digest. That changes envelope hashes. Options: `sha2` (MIT OR Apache-2.0), or
   `blake2` (MIT OR Apache-2.0), which keeps the BLAKE family. Either needs a digest identifier in the
   envelope so existing hashes can still be checked. **Recommendation:** `sha2` with HMAC-SHA256 from
   `hmac` for keyed mode. Both pass the gate, both are standard, users can check hashes with ordinary tools,
   and most CPUs accelerate SHA-256. `blake2` is the alternative if the BLAKE design matters more.
3. **`constant_time_eq` (via `blake3`).** Goes away with item 2.

## Next steps

- Decide item 2 (digest choice and envelope compatibility).
- Review the `replace-self-cell` branch in the tpt-text clone, then push it if approved.
- Once approved, run `cargo build --target thumbv7m-none-eabi` and `wasm32-unknown-unknown` on the
  candidate crates to verify the `no_std` attribute.
- Re-run this audit on every dependency bump.
