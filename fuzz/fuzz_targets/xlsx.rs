#![no_main]
use libfuzzer_sys::fuzz_target;
use tpt_codex_interop::{import_xlsx, import_xlsx_workbook};

// Workbooks are untrusted files: reading them must return an error, never panic.
fuzz_target!(|data: &[u8]| {
    let _ = import_xlsx(data);
    let _ = import_xlsx_workbook(data);
});
