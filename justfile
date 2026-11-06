# Common tasks. Install `just` with `cargo install just`, then run `just` to list them.

default:
    @just --list

# Everything CI checks, apart from the license gate and the cross-OS comparison.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    python3 scripts/check_spdx.py

# The license gate.
licenses:
    cargo metadata --format-version 1 | python3 scripts/check_licenses.py
    python3 -m unittest scripts/test_check_licenses.py

# Builds of the headless crates for the targets CI checks.
no-std:
    cargo build -p tpt-codex-core -p tpt-codex-views -p tpt-codex-markdown -p tpt-typeset --target thumbv7m-none-eabi
    cargo build -p tpt-codex-core -p tpt-codex-views -p tpt-codex-markdown -p tpt-typeset --target wasm32-unknown-unknown

# Installs the `tpt` command.
install:
    cargo install --path crates/tpt-codex-cli --locked

# Builds the browser demo into target/web.
web profile="debug":
    bash scripts/build-web.sh {{profile}}

# Serves the browser demo at http://localhost:8000.
serve: web
    python3 -m http.server -d target/web 8000

# Runs the collaboration relay on 127.0.0.1:8787.
relay:
    cargo run -p tpt-codex-sync --bin tpt-codex-relay
