#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_codex_core::Document;

// Saved documents are read back from disk, so any JSON must be rejected or validated, never panic.
fuzz_target!(|data: &[u8]| {
    if let Ok(doc) = serde_json::from_slice::<Document>(data) {
        let _ = doc.validate();
    }
});
