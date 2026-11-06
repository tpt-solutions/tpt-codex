// SPDX-License-Identifier: MIT OR Apache-2.0
#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_typeset::{Library, bibliography};

// Both bibliography formats, and the formatted list, must never panic.
fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else {
        return;
    };
    for library in [Library::from_csl_json(text), Library::from_bibtex(text)]
        .into_iter()
        .flatten()
    {
        let _ = bibliography(&library, &["k", "smith2020"]);
    }
});
