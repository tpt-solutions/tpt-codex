// SPDX-License-Identifier: MIT OR Apache-2.0
//! Starter documents for `tpt new`.

/// Name, one-line description and text of each template.
pub const TEMPLATES: [(&str, &str, &str); 4] = [
    (
        "article",
        "an article with math, a footnote and a live table",
        include_str!("../templates/article.cdx"),
    ),
    (
        "report",
        "a report with sections and a figures table",
        include_str!("../templates/report.cdx"),
    ),
    (
        "slides",
        "a three-slide 16:9 presentation",
        include_str!("../templates/slides.cdx"),
    ),
    (
        "budget",
        "a budget grid with formulas",
        include_str!("../templates/budget.cdx"),
    ),
];

/// The text of the template called `name`.
pub fn template(name: &str) -> Option<&'static str> {
    TEMPLATES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, text)| *text)
}
