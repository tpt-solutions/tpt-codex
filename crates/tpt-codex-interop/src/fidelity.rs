// SPDX-License-Identifier: MIT OR Apache-2.0
//! Import fidelity report: what an importer kept, approximated, or dropped.
//!
//! Every importer records one [`FidelityItem`] per feature it meets. The report is the
//! user-facing answer to "what did the import lose?", so it is plain data with a
//! deterministic text form.

use std::fmt::{self, Write};

/// How faithfully one feature was imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fidelity {
    /// The feature has the same meaning in the document model.
    Exact,
    /// The feature was imported with a different but close meaning, for example a
    /// coloured heading becomes a plain heading.
    Approximated,
    /// The feature could not be represented and was left out.
    Dropped,
}

impl Fidelity {
    /// A short label used in the text form.
    pub fn label(self) -> &'static str {
        match self {
            Fidelity::Exact => "exact",
            Fidelity::Approximated => "approximated",
            Fidelity::Dropped => "dropped",
        }
    }
}

/// One feature met during an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FidelityItem {
    /// Where the feature is in the source, for example `page 3` or `sheet "Q1"!B4`.
    pub location: String,
    /// What the feature is, for example `footnote` or `chart`.
    pub feature: String,
    /// How it was imported.
    pub outcome: Fidelity,
    /// What was done, and why. Empty for exact features.
    pub note: String,
}

/// The features of one import, in the order they were recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FidelityReport {
    items: Vec<FidelityItem>,
}

impl FidelityReport {
    /// An empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a feature that came through unchanged.
    pub fn exact(&mut self, location: impl Into<String>, feature: impl Into<String>) {
        self.push(location, feature, Fidelity::Exact, "");
    }

    /// Records a feature that was imported with an approximate meaning.
    pub fn approximated(
        &mut self,
        location: impl Into<String>,
        feature: impl Into<String>,
        note: impl Into<String>,
    ) {
        self.push(location, feature, Fidelity::Approximated, note);
    }

    /// Records a feature that was left out.
    pub fn dropped(
        &mut self,
        location: impl Into<String>,
        feature: impl Into<String>,
        note: impl Into<String>,
    ) {
        self.push(location, feature, Fidelity::Dropped, note);
    }

    fn push(
        &mut self,
        location: impl Into<String>,
        feature: impl Into<String>,
        outcome: Fidelity,
        note: impl Into<String>,
    ) {
        self.items.push(FidelityItem {
            location: location.into(),
            feature: feature.into(),
            outcome,
            note: note.into(),
        });
    }

    /// All recorded items, in order.
    pub fn items(&self) -> &[FidelityItem] {
        &self.items
    }

    /// The number of items with the given outcome.
    pub fn count(&self, outcome: Fidelity) -> usize {
        self.items.iter().filter(|i| i.outcome == outcome).count()
    }

    /// `true` when nothing was approximated or dropped.
    pub fn is_lossless(&self) -> bool {
        self.count(Fidelity::Approximated) == 0 && self.count(Fidelity::Dropped) == 0
    }
}

impl fmt::Display for FidelityReport {
    /// One line per item that is not exact, then a count line:
    ///
    /// ```text
    /// dropped: chart (page 3): charts are not imported; kept the caption
    /// approximated: heading colour (page 1): headings are plain text
    /// 2 exact, 1 approximated, 1 dropped
    /// ```
    ///
    /// Exact items are counted but not listed, so the report stays short for large files.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines: Vec<(Fidelity, String)> = Vec::new();
        for item in &self.items {
            if item.outcome == Fidelity::Exact {
                continue;
            }
            let mut line = format!(
                "{}: {} ({})",
                item.outcome.label(),
                item.feature,
                item.location
            );
            if !item.note.is_empty() {
                let _ = write!(line, ": {}", item.note);
            }
            lines.push((item.outcome, line));
        }
        // Dropped first, then approximated, so the most serious problems come first.
        lines.sort_by_key(|(outcome, _)| core::cmp::Reverse(*outcome));
        for (_, line) in &lines {
            writeln!(f, "{line}")?;
        }
        write!(
            f,
            "{} exact, {} approximated, {} dropped",
            self.count(Fidelity::Exact),
            self.count(Fidelity::Approximated),
            self.count(Fidelity::Dropped)
        )
    }
}
