# Fuzz harness

Seven libFuzzer targets for the code that reads untrusted input: Markdown, saved document JSON,
math source, bibliographies (CSL-JSON and BibTeX), xlsx, docx and pptx.

This crate is excluded from the main workspace, so libFuzzer and its nightly-only build do not
enter the licence-checked dependency tree.

Run one target with a nightly toolchain:

    rustup toolchain install nightly
    cargo +nightly fuzz run markdown --manifest-path fuzz/Cargo.toml

Or type-check all targets on stable:

    cargo check --manifest-path fuzz/Cargo.toml --bins

Crashing inputs are written to `fuzz/artifacts/`. Corpora are written to `fuzz/corpus/`. Both are
ignored by git.
