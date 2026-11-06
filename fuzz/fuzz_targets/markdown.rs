#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_codex_markdown::{from_markdown, to_markdown};

// Any input that parses must give a valid document, and writing it must not panic.
fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else {
        return;
    };
    if let Ok(doc) = from_markdown(text) {
        assert!(
            doc.validate().is_ok(),
            "parsed Markdown gave an invalid document"
        );
        let _ = to_markdown(&doc);
    }
});
