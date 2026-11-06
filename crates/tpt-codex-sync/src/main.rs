// SPDX-License-Identifier: MIT OR Apache-2.0
//! The relay server. Usage: `tpt-codex-relay --help`.

use std::net::TcpListener;
use std::process::ExitCode;

use tpt_codex_sync::{USAGE, parse_args, serve_with};

fn main() -> ExitCode {
    let config = match parse_args(std::env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            if message == USAGE {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let listener = match TcpListener::bind(&config.addr) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("cannot listen on {}: {e}", config.addr);
            return ExitCode::FAILURE;
        }
    };
    match listener.local_addr() {
        Ok(local) => eprintln!(
            "tpt-codex relay listening on {local} (ws://, no authentication; max message {} bytes, max {} operations per document)",
            config.limits.max_message_bytes, config.limits.max_ops_per_doc
        ),
        Err(e) => eprintln!("listening, address unknown: {e}"),
    }
    match serve_with(listener, config.limits) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("relay stopped: {e}");
            ExitCode::FAILURE
        }
    }
}
