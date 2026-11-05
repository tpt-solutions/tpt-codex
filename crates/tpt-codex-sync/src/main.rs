// SPDX-License-Identifier: MIT OR Apache-2.0
//! The relay server. Usage: `tpt-codex-relay [address]`, default `127.0.0.1:8787`.

use std::net::TcpListener;
use std::process::ExitCode;

fn main() -> ExitCode {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8787".to_string());
    let listener = match TcpListener::bind(&addr) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match listener.local_addr() {
        Ok(local) => eprintln!("tpt-codex relay listening on {local} (ws://, no authentication)"),
        Err(e) => eprintln!("listening, address unknown: {e}"),
    }
    match tpt_codex_sync::serve(listener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("relay stopped: {e}");
            ExitCode::FAILURE
        }
    }
}
