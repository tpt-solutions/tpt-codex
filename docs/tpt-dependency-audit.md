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
| `tpt-text` (`tpt-text/crates/tpt-text`) | MIT OR Apache-2.0 | yes | 16 dependencies | `self_cell` (Apache-2.0 OR GPL-2.0-only) |
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

1. **`self_cell` (via `tpt-text`).** The only permissive option is Apache-2.0 alone, which the spec
   forbids. Options: ask upstream to add an MIT option, vendor a copy under MIT, or replace the
   dependency. Until resolved, `tpt-text` and Phase 1 flow shaping are blocked.
2. **`tpt-crypto-stub` leaks into `tpt-model`.** The TODO asks that `*-stub` crates not reach our
   tree. `tpt-model` depends on `tpt-crypto-stub` as a normal dependency, which brings in `blake3`.
   This needs a fix in `tpt-model` (feature-gate the stub or drop it) before adoption.
3. **CC0-1.0 and MIT-0 alternatives.** `blake3` is `CC0-1.0 OR Apache-2.0 OR …` and
   `constant_time_eq` is `CC0-1.0 OR MIT-0 OR Apache-2.0`. Both can be satisfied by a permissive
   alternative, but neither license is on the allow-list. Adding them is a policy decision.
   `CC0-1.0` is a public-domain dedication. `MIT-0` is MIT without the attribution clause.

## Next steps

- Decide items 1–3 above.
- Once approved, run `cargo build --target thumbv7m-none-eabi` and `wasm32-unknown-unknown` on the
  candidate crates to verify the `no_std` attribute.
- Re-run this audit on every dependency bump.
