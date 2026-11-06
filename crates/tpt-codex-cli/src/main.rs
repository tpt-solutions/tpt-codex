// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `tpt` command. Usage: `tpt --help`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = tpt_codex_cli::run(&args, &mut std::io::stdout(), &mut std::io::stderr());
    ExitCode::from(status as u8)
}
