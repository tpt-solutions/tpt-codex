// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_codex_interop::import_pdf;

fuzz_target!(|data: &[u8]| {
    let _ = import_pdf(data);
});
