# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Cargo workspace (resolver 2, edition 2024) with crates `tpt-codex-core`, `tpt-codex-views`,
  `tpt-codex-markdown`, `tpt-typeset`, `tpt-codex-interop` and `tpt-codex-app`.
- `tpt-codex-core` data model: `BlockId`, `SheetId`, `Rect`, `TextStyle`, `LayoutMode`,
  `MediaSource`, the `Block` enum (`Text`, `Grid`, `Media`, `Container`, `Query`), and `Document`
  with metadata and a schema version.
- `Document::validate` checks schema version, root existence, dangling children, and acyclicity.
- JSON serialization via serde, with deterministic ordering (`BTreeMap`).
- Licensing gate: `scripts/check_licenses.py` (Apache-2.0-only and `-sys` checks, with fixture tests)
  and `deny.toml` for `cargo deny check licenses`.
- SPDX header check: `scripts/check_spdx.py`.
- CI workflow (`.github/workflows/ci.yml`): licensing, tests on Linux/Windows/macOS, `no_std` and
  WASM builds.
- `MIT OR Apache-2.0` licensing files and project documentation.

### Decided

- `Unicode-3.0` is on the license allow-list. It is required by the serde derive toolchain
  (`unicode-ident`) and is permissive. This extends the list in the spec and needs maintainer sign-off.
