# Security Policy

## Supported versions

tpt-codex has not had a public release. Only the `master` branch is supported.

## Reporting a vulnerability

Please do not open a public issue for security problems.

Report privately to TPT Solutions by email at the maintainer address listed on the
[TPT Solutions GitHub organisation](https://github.com/tpt-solutions). Include:

- a description of the issue and its impact,
- steps or a minimal document that reproduces it,
- the commit you tested against.

You will get an acknowledgement as soon as the maintainer is able to respond. Please allow time for a
fix before public disclosure.

## Scope notes

- Documents are untrusted input. Parsing and deserialization must not panic or allocate without bound
  on malformed input. Fuzzing of the Markdown/.cdx parser and deserializer is planned (todo.md).
- The core crates forbid `unsafe` code and must not depend on `-sys` (C FFI) crates.
