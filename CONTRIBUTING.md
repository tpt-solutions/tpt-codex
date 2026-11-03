# Contributing to tpt-codex

Thanks for your interest. This is a solo-maintained project in an early phase, so please open an
issue before starting larger changes.

## Before you open a pull request

Run the same checks CI runs:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/check_licenses.py < <(cargo metadata --format-version 1)
python3 scripts/check_spdx.py
```

## Rules

1. **Licensing is non-negotiable.** Do not add a dependency unless its license is on the allow-list in
   [deny.toml](deny.toml) and [scripts/check_licenses.py](scripts/check_licenses.py). Apache-2.0-only,
   GPL, AGPL, SSPL and BSL dependencies are rejected. Do not add `-sys` crates to core or view crates.
2. **Keep the core headless.** `tpt-codex-core`, `tpt-codex-views`, `tpt-codex-markdown` and
   `tpt-typeset` are `no_std` + `alloc`, with no platform I/O.
3. **No `unsafe`.** Every crate sets `#![forbid(unsafe_code)]`.
4. **SPDX headers.** Every `.rs` file starts with `// SPDX-License-Identifier: MIT OR Apache-2.0`.
5. **Determinism.** Use ordered collections (`BTreeMap`) where iteration order reaches serialized output.
6. **Tests with changes.** Add or update tests for behavior changes. Validation failures need a test each.

## Scope

Work follows the phase plan in [todo.md](todo.md). Items in the "Explicit non-goals" section are
out of scope for v1, and pull requests that add them will be declined.

## Commit messages

Use short imperative subjects. Describe the why in the body where it isn't obvious.

## License of contributions

By contributing, you agree that your contribution is dual-licensed under MIT OR Apache-2.0, as
described in the [README](README.md#license).
