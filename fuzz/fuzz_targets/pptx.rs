#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_codex_interop::import_pptx;

fuzz_target!(|data: &[u8]| {
    let _ = import_pptx(data);
});
