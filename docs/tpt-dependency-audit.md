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

## Next steps

- No licensing decisions are open.
- Push the local commits when ready: tpt-text `master` (`9c794eb` and earlier) and tpt-model `master` (`f0a8abe`).
- Once approved, run `cargo build --target thumbv7m-none-eabi` and `wasm32-unknown-unknown` on the
  candidate crates to verify the `no_std` attribute.
- Re-run this audit on every dependency bump.
