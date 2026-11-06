// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{check_figure_references, check_references, figure_entries};

const SRC: &str = "# Intro\n\n\
    ![Plan](plan.png) {#fig:plan}\n\n\
    See [the plan](#fig:plan) and [the chart](#fig:chart) and [nothing](#fig:missing).\n\n\
    ![Unlabelled](plain.png)\n\n\
    ![Chart](chart.png) {#fig:chart width=50%}\n";

#[test]
fn labelled_images_are_numbered_in_reading_order() {
    let doc = from_markdown(SRC).unwrap();
    let figures = figure_entries(&doc);
    let labels: Vec<(&str, u32)> = figures
        .iter()
        .map(|f| (f.label.as_str(), f.number))
        .collect();
    assert_eq!(labels, [("fig:plan", 1), ("fig:chart", 2)]);
}

#[test]
fn figure_references_resolve_to_numbers_and_missing_ones_are_reported() {
    let doc = from_markdown(SRC).unwrap();
    let refs = check_figure_references(&doc);
    let found: Vec<(&str, Option<u32>)> =
        refs.iter().map(|r| (r.label.as_str(), r.number)).collect();
    assert_eq!(
        found,
        [
            ("fig:plan", Some(1)),
            ("fig:chart", Some(2)),
            ("fig:missing", None)
        ]
    );
}

#[test]
fn figure_links_are_not_reported_as_heading_references() {
    let doc = from_markdown(SRC).unwrap();
    assert!(check_references(&doc).is_empty());
}

#[test]
fn figures_inside_slides_are_numbered_too() {
    let src = "::: canvas { ratio=\"16:9\" }\n\n![Plan](plan.png) {#fig:plan}\n:::\n\n\
        ![Chart](chart.png) {#fig:chart}\n";
    let doc = from_markdown(src).unwrap();
    let figures = figure_entries(&doc);
    assert_eq!(figures.len(), 2);
    assert_eq!(figures[0].label, "fig:plan");
    assert_eq!(figures[1].number, 2);
}
