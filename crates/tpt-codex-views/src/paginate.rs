// SPDX-License-Identifier: MIT OR Apache-2.0
//! Paginated projection: splits a flow layout into fixed-size pages.
//!
//! Rules:
//! - Items are never split, except text lines inside one paragraph.
//! - A paragraph split across pages keeps at least [`PageGeometry::min_lines`]
//!   lines on each side (orphan and widow control).
//! - A block taller than a page is placed alone on a page rather than dropped.

use alloc::vec::Vec;

use crate::flow::{FlowItem, FlowKind, FlowLayout};

/// The header rows of the grid that `block` belongs to, in order.
fn header_rows_of(items: &[FlowItem], block: tpt_codex_core::BlockId) -> Vec<&FlowItem> {
    items
        .iter()
        .filter(|it| it.block == block && matches!(it.kind, FlowKind::GridRow { header: true, .. }))
        .collect()
}

/// Page size and margins, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    /// Page height.
    pub height: f64,
    /// Space above the content on every page.
    pub margin_top: f64,
    /// Space below the content on every page.
    pub margin_bottom: f64,
    /// Minimum lines of a split paragraph kept at the bottom of a page and at the top of the next.
    pub min_lines: usize,
}

/// One page of a paginated document.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// 1-based page number.
    pub number: usize,
    /// Items on this page. `y` is measured from the top of the content area.
    pub items: Vec<FlowItem>,
}

/// Why pagination was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum PageError {
    /// The margins leave no room for content, or the geometry is not finite.
    InvalidGeometry,
    /// `min_lines` must be at least 1.
    InvalidMinLines,
}

impl core::fmt::Display for PageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidGeometry => f.write_str("page geometry leaves no room for content"),
            Self::InvalidMinLines => f.write_str("min_lines must be at least 1"),
        }
    }
}

impl core::error::Error for PageError {}

/// Splits `flow` into pages.
pub fn paginate(flow: &FlowLayout, geometry: PageGeometry) -> Result<Vec<Page>, PageError> {
    let avail = geometry.height - geometry.margin_top - geometry.margin_bottom;
    if !avail.is_finite() || avail <= 0.0 {
        return Err(PageError::InvalidGeometry);
    }
    if geometry.min_lines == 0 {
        return Err(PageError::InvalidMinLines);
    }
    let min = geometry.min_lines;

    let mut pages: Vec<Page> = Vec::new();
    let mut current = Page {
        number: 1,
        items: Vec::new(),
    };
    let mut used = 0.0; // height used on the current page
    let items = &flow.items;
    let mut i = 0;

    while i < items.len() {
        let item = &items[i];
        match &item.kind {
            FlowKind::Box => {
                if used + item.height > avail && !current.items.is_empty() {
                    push_page(&mut pages, &mut current, &mut used);
                }
                place(&mut current, item, used);
                used += item.height;
                i += 1;
            }
            FlowKind::GridRow { header, .. } => {
                if used + item.height > avail && !current.items.is_empty() {
                    push_page(&mut pages, &mut current, &mut used);
                    // A grid that continues on this page starts with its header rows again.
                    if !header {
                        for head in header_rows_of(items, item.block) {
                            place(&mut current, head, used);
                            used += head.height;
                        }
                    }
                }
                place(&mut current, item, used);
                used += item.height;
                i += 1;
            }
            FlowKind::Line { .. } => {
                // The run of lines that belong to the same paragraph.
                let run_end = (i..items.len())
                    .take_while(|&j| {
                        matches!(items[j].kind, FlowKind::Line { .. })
                            && items[j].block == item.block
                    })
                    .last()
                    .map_or(i + 1, |j| j + 1);
                let run = &items[i..run_end];
                let run_len = run.len();

                let fit = fit_count(run, avail - used);
                if fit == run_len {
                    for line in run {
                        place(&mut current, line, used);
                        used += line.height;
                    }
                    i = run_end;
                    continue;
                }

                let page_empty = current.items.is_empty();
                // Never leave a single line behind at the bottom of a page.
                if fit < min && !page_empty {
                    push_page(&mut pages, &mut current, &mut used);
                    continue; // re-evaluate the run on the fresh page
                }
                let mut take = fit.max(1);
                // Never leave fewer than `min` lines alone at the top of the next page.
                if run_len - take < min {
                    let candidate = run_len - min;
                    if candidate >= min && candidate >= 1 {
                        // Move lines onto the next page so both sides keep `min` lines.
                        take = candidate;
                    } else if !page_empty {
                        // The paragraph is too short to split with `min` lines on each side.
                        push_page(&mut pages, &mut current, &mut used);
                        continue;
                    }
                    // On an empty page with a paragraph shorter than 2 * min, the split
                    // is forced: no layout can keep `min` lines on both sides.
                }
                for line in &run[..take] {
                    place(&mut current, line, used);
                    used += line.height;
                }
                i += take;
                push_page(&mut pages, &mut current, &mut used);
            }
        }
    }

    if !current.items.is_empty() || pages.is_empty() {
        pages.push(current);
    }
    Ok(pages)
}

/// How many leading lines of `run` fit in `space`.
fn fit_count(run: &[FlowItem], space: f64) -> usize {
    let mut total = 0.0;
    let mut n = 0;
    for line in run {
        if total + line.height > space {
            break;
        }
        total += line.height;
        n += 1;
    }
    n
}

fn place(page: &mut Page, item: &FlowItem, used: f64) {
    let mut placed = item.clone();
    placed.y = used;
    page.items.push(placed);
}

fn push_page(pages: &mut Vec<Page>, current: &mut Page, used: &mut f64) {
    let next = Page {
        number: current.number + 1,
        items: Vec::new(),
    };
    let done = core::mem::replace(current, next);
    pages.push(done);
    *used = 0.0;
}
