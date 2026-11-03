#!/usr/bin/env bash
# Runs the strict licensing gate (Apache-only + -sys checks) against the workspace.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo metadata --format-version 1 | python3 scripts/check_licenses.py
