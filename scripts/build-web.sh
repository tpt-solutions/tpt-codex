#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Builds the browser demo into target/web (open target/web/index.html through a local server).
#
# Usage: scripts/build-web.sh [debug|release]
set -euo pipefail
cd "$(dirname "$0")/.."

profile="${1:-debug}"
case "$profile" in
  debug) flag="" ;;
  release) flag="--release" ;;
  *) echo "usage: $0 [debug|release]" >&2; exit 2 ;;
esac

# The wasm-bindgen CLI must match the library version in Cargo.lock exactly.
want="$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p' | head -1)"
have="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)"
if [ "$have" != "$want" ]; then
  echo "wasm-bindgen CLI is '${have:-missing}', but Cargo.lock needs $want." >&2
  echo "install it with: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p tpt-codex-web --target wasm32-unknown-unknown $flag
wasm-bindgen --target web --out-dir target/web/pkg \
  "target/wasm32-unknown-unknown/$profile/tpt_codex_web.wasm"
cp crates/tpt-codex-web/web/index.html target/web/index.html
echo "built target/web (serve it, for example: python3 -m http.server -d target/web 8000)"
