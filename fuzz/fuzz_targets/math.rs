#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_typeset::parse_math;

// Math source comes from documents, so the parser must return an error, never panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        let _ = parse_math(text);
    }
});
