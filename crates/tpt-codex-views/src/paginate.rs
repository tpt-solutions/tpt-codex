// SPDX-License-Identifier: MIT OR Apache-2.0
//! Paginated projection: splits a flow layout into fixed-size pages.
//!
//! Rules:
//! - Items are never split, except text lines inside one paragraph.
//! - A paragraph split across pages keeps at least [`PageGeometry::min_lines`]
//!   lines on each side (orphan and widow control).
//! - A block taller than a page is placed alone on a page rather than dropped.
//! - A [`Note`] (a footnote) is placed at the foot of the page where its anchor block first
//!   appears. Its height is kept free on that page, so the text above it moves up or onto the next
//!   page to make room. A note is never split. A note whose anchor is not in the flow is not
//!   placed.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use tpt_codex_core::{BlockId, TextStyle};

use crate::flow::{FlowItem, FlowKind, FlowLayout, wrap};
use crate::measure::TextMeasure;

/// The header rows of the grid that `block` belongs to, in order.
fn header_rows_of(items: &[FlowItem], block: BlockId) -> Vec<&FlowItem> {
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

/// A footnote, laid out and ready to be placed at the foot of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The block that refers to the note. The note goes on the page where this block first appears.
    pub anchor: BlockId,
    /// The lines of the note. `y` is measured from the top of the note.
    pub items: Vec<FlowItem>,
    /// The height of the note: the lines' total height.
    pub height: f64,
}

/// Lays out `content` as the lines of a note that refers from `anchor`. `block` is the block that
/// holds the note's text, and `content` is what is shown, including any number the host adds.
pub fn layout_note(
    anchor: BlockId,
    block: BlockId,
    content: &str,
    style: &TextStyle,
    measure: &dyn TextMeasure,
    width: f64,
) -> Note {
    let line_height = measure.line_height(style);
    let items: Vec<FlowItem> = wrap(content, style, measure, width, false)
        .into_iter()
        .enumerate()
        .map(|(i, text)| FlowItem {
            block,
            y: i as f64 * line_height,
            height: line_height,
            kind: FlowKind::Line {
                text,
                style: style.clone(),
                marker: None,
                indent: 0.0,
            },
        })
        .collect();
    let height = items.len() as f64 * line_height;
    Note {
        anchor,
        items,
        height,
    }
}

/// One page of a paginated document.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// 1-based page number.
    pub number: usize,
    /// Items on this page. `y` is measured from the top of the content area.
    pub items: Vec<FlowItem>,
    /// The footnotes on this page, placed above the bottom margin. `y` is measured from the top
    /// of the content area, as for `items`.
    pub notes: Vec<FlowItem>,
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
    paginate_with_notes(flow, geometry, &[])
}

/// Splits `flow` into pages, placing each note at the foot of the page where its anchor appears.
/// With no notes this is [`paginate`].
pub fn paginate_with_notes(
    flow: &FlowLayout,
    geometry: PageGeometry,
    notes: &[Note],
) -> Result<Vec<Page>, PageError> {
    let avail = geometry.height - geometry.margin_top - geometry.margin_bottom;
    if !avail.is_finite() || avail <= 0.0 {
        return Err(PageError::InvalidGeometry);
    }
    if geometry.min_lines == 0 {
        return Err(PageError::InvalidMinLines);
    }
    let min = geometry.min_lines;

    // A block that is shown as a note (a footnote's text) is not also placed in the body.
    let owned: Vec<FlowItem>;
    let items: &[FlowItem] = if notes.is_empty() {
        &flow.items
    } else {
        let note_blocks: BTreeSet<BlockId> = notes
            .iter()
            .flat_map(|n| n.items.iter().map(|i| i.block))
            .collect();
        owned = flow
            .items
            .iter()
            .filter(|item| !note_blocks.contains(&item.block))
            .cloned()
            .collect();
        &owned
    };

    let mut pager = Pager::new(avail, notes);
    let mut i = 0;

    while i < items.len() {
        let item = &items[i];
        match &item.kind {
            FlowKind::Box => {
                if pager.needs_new_page(item.height, item.block) {
                    pager.new_page();
                }
                pager.place(item);
                i += 1;
            }
            FlowKind::GridRow { header, .. } => {
                if pager.needs_new_page(item.height, item.block) {
                    pager.new_page();
                    // A grid that continues on this page starts with its header rows again.
                    if !header {
                        for head in header_rows_of(items, item.block) {
                            pager.place(head);
                        }
                    }
                }
                pager.place(item);
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

                let fit = fit_count(run, pager.room() - pager.pending(item.block));
                if fit == run_len {
                    for line in run {
                        pager.place(line);
                    }
                    i = run_end;
                    continue;
                }

                let page_empty = pager.current_is_empty();
                // Never leave a single line behind at the bottom of a page.
                if fit < min && !page_empty {
                    pager.new_page();
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
                        pager.new_page();
                        continue;
                    }
                    // On an empty page with a paragraph shorter than 2 * min, the split
                    // is forced: no layout can keep `min` lines on both sides.
                }
                for line in &run[..take] {
                    pager.place(line);
                }
                i += take;
                pager.new_page();
            }
        }
    }

    Ok(pager.finish())
}

/// The state of pagination: the page being filled, the room left on it, and the notes waiting for
/// their anchors.
struct Pager<'a> {
    avail: f64,
    pages: Vec<Page>,
    current: Page,
    /// Height used by items on the current page.
    used: f64,
    /// The notes placed on the current page, in order, and their total height.
    placed_notes: Vec<&'a Note>,
    reserved: f64,
    /// Notes whose anchor has not been placed yet, by anchor.
    waiting: BTreeMap<BlockId, Vec<&'a Note>>,
}

impl<'a> Pager<'a> {
    fn new(avail: f64, notes: &'a [Note]) -> Self {
        let mut waiting: BTreeMap<BlockId, Vec<&'a Note>> = BTreeMap::new();
        for note in notes {
            waiting.entry(note.anchor).or_default().push(note);
        }
        Self {
            avail,
            pages: Vec::new(),
            current: Page {
                number: 1,
                items: Vec::new(),
                notes: Vec::new(),
            },
            used: 0.0,
            placed_notes: Vec::new(),
            reserved: 0.0,
            waiting,
        }
    }

    /// The room left on the current page for items, after the notes placed on it.
    fn room(&self) -> f64 {
        self.avail - self.used - self.reserved
    }

    /// The height of the notes that `block` would bring to the current page.
    fn pending(&self, block: BlockId) -> f64 {
        self.waiting
            .get(&block)
            .map_or(0.0, |notes| notes.iter().map(|n| n.height).sum())
    }

    fn current_is_empty(&self) -> bool {
        self.current.items.is_empty()
    }

    /// Whether an item of `height` from `block` must start a new page. Never true on an empty page.
    fn needs_new_page(&self, height: f64, block: BlockId) -> bool {
        height + self.pending(block) > self.room() && !self.current_is_empty()
    }

    /// Places `item` at the current height, and places the notes its block brings.
    fn place(&mut self, item: &FlowItem) {
        let mut placed = item.clone();
        placed.y = self.used;
        self.current.items.push(placed);
        self.used += item.height;
        if let Some(notes) = self.waiting.remove(&item.block) {
            for note in notes {
                self.reserved += note.height;
                self.placed_notes.push(note);
            }
        }
    }

    /// Writes the current page's notes at the foot of the page and starts the next page.
    fn new_page(&mut self) {
        let page = self.close_current();
        self.pages.push(page);
    }

    /// The current page with its notes placed at the foot, and a fresh page in its place.
    fn close_current(&mut self) -> Page {
        let mut top = self.avail - self.reserved;
        for note in self.placed_notes.drain(..) {
            for item in &note.items {
                let mut placed = item.clone();
                placed.y = top + item.y;
                self.current.notes.push(placed);
            }
            top += note.height;
        }
        let next = Page {
            number: self.current.number + 1,
            items: Vec::new(),
            notes: Vec::new(),
        };
        let done = core::mem::replace(&mut self.current, next);
        self.used = 0.0;
        self.reserved = 0.0;
        done
    }

    fn finish(mut self) -> Vec<Page> {
        if !self.current.items.is_empty() || self.pages.is_empty() {
            let last = self.close_current();
            self.pages.push(last);
        }
        self.pages
    }
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
