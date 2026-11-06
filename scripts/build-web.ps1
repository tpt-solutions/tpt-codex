# SPDX-License-Identifier: MIT OR Apache-2.0
# Builds the browser demo into target\web (open target\web\index.html through a local server).
#
# Usage: scripts\build-web.ps1 [-Profile debug|release]
param([ValidateSet('debug', 'release')][string]$Profile = 'debug')
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# The wasm-bindgen CLI must match the library version in Cargo.lock exactly.
$lock = Get-Content Cargo.lock
$index = [array]::IndexOf($lock, 'name = "wasm-bindgen"')
$want = ($lock[$index + 1] -replace '^version = "(.*)"$', '$1')
$have = ''
if (Get-Command wasm-bindgen -ErrorAction SilentlyContinue) {
    $have = (wasm-bindgen --version).Split(' ')[1]
}
if ($have -ne $want) {
    Write-Error "wasm-bindgen CLI is '$have', but Cargo.lock needs $want. Install it with: cargo install wasm-bindgen-cli --version $want --locked"
}

rustup target add wasm32-unknown-unknown | Out-Null
$flags = @()
if ($Profile -eq 'release') { $flags += '--release' }
cargo build -p tpt-codex-web --target wasm32-unknown-unknown @flags
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
wasm-bindgen --target web --out-dir target/web/pkg "target/wasm32-unknown-unknown/$Profile/tpt_codex_web.wasm"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item crates/tpt-codex-web/web/index.html target/web/index.html -Force
Write-Host 'built target\web (serve it, for example: python -m http.server -d target\web 8000)'
