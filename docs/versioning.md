# Versioning and schema policy

## Crate versions

The workspace is at `0.1.0-spec`, the design-spec stage. The first public release is `0.1.0`.

- `0.1.0-spec` → `0.1.0`: when all TPT dependencies are published to crates.io (git dependencies cannot
  be published), the license gate passes on the published graph, and the Phase 1 milestone is met.
- Before `0.1.0`, minor versions may break compatibility. Note each break in `CHANGELOG.md`.
- After `0.1.0`, follow semantic versioning. Adding a variant to a public enum is breaking unless the enum
  is `#[non_exhaustive]`. The public enums (`Block`, `Op`, `EditError`, `ValidationError`, `MathNode`,
  `DocumentClass`) are not marked yet. Mark them before `0.1.0`.
- Crates are `publish = false` until the TPT dependencies are on crates.io.

## Document schema

Every serialized `Document` carries `schema_version`. The current value is `SCHEMA_VERSION` in
`tpt-codex-core` (`1`).

Rules:

1. **Reading.** A build reads only the schema versions it knows. `Document::validate` returns
   `ValidationError::UnsupportedSchemaVersion` for anything else. Nothing is read partially.
2. **Compatible additions** do not change the version. These are a new optional field with
   `#[serde(default)]`, or a new `Metadata` key that older readers can ignore. Older readers must still
   read such documents.
3. **Incompatible changes** bump `SCHEMA_VERSION`. These are a removed or renamed field, a changed
   meaning, or a new `Block` variant that older readers cannot represent. When this happens, add a
   migration from the old version and a fixture for each version (see `fixtures/`).
4. **Golden files.** `fixtures/notes.golden.json` pins the JSON form of the reader's output. A change to
   it needs a reason in the commit message and, for an incompatible change, a version bump.
5. **Markdown is not versioned.** It is the readable interchange format. The Markdown grammar in
   `docs/markdown-grammar.md` changes by documentation, and writer refusals protect data that cannot be
   represented yet.

## What is not yet decided

- Migration tooling. There is no older schema to migrate from yet.
- The `tpt-model` schema and its versioning. The `tpt-model` dependency is blocked (see
  `docs/tpt-dependency-audit.md`). If it is adopted, its schema version replaces or wraps `schema_version`.
