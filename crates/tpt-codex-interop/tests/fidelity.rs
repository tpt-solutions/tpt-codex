// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_interop::{Fidelity, FidelityReport};

#[test]
fn empty_report_is_lossless() {
    let report = FidelityReport::new();
    assert!(report.is_lossless());
    assert_eq!(report.to_string(), "0 exact, 0 approximated, 0 dropped");
}

#[test]
fn counts_and_lossless_flag() {
    let mut report = FidelityReport::new();
    report.exact("page 1", "paragraph");
    report.exact("page 1", "heading");
    assert!(report.is_lossless());

    report.approximated("page 1", "heading colour", "headings are plain text");
    assert!(!report.is_lossless());
    assert_eq!(report.count(Fidelity::Exact), 2);
    assert_eq!(report.count(Fidelity::Approximated), 1);
    assert_eq!(report.count(Fidelity::Dropped), 0);
    assert_eq!(report.items().len(), 3);
}

#[test]
fn text_form_lists_problems_most_serious_first_and_skips_exact_items() {
    let mut report = FidelityReport::new();
    report.exact("page 1", "paragraph");
    report.approximated("page 1", "heading colour", "headings are plain text");
    report.dropped(
        "page 3",
        "chart",
        "charts are not imported; kept the caption",
    );
    report.dropped("page 4", "comment", "");

    assert_eq!(
        report.to_string(),
        "dropped: chart (page 3): charts are not imported; kept the caption\n\
         dropped: comment (page 4)\n\
         approximated: heading colour (page 1): headings are plain text\n\
         1 exact, 1 approximated, 2 dropped"
    );
}

#[test]
fn text_form_keeps_recording_order_within_one_outcome() {
    let mut report = FidelityReport::new();
    report.dropped("p2", "b", "");
    report.dropped("p1", "a", "");
    let text = report.to_string();
    let first = text.find("(p2)").unwrap();
    let second = text.find("(p1)").unwrap();
    assert!(first < second);
}
