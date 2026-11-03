// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headers, footers and page numbers, and standard paper sizes.

use alloc::string::String;
use alloc::vec::Vec;

use crate::paginate::Page;

/// A paper size in points (1/72 inch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaperSize {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl PaperSize {
    /// ISO A4, 210 x 297 mm.
    pub const A4: PaperSize = PaperSize {
        width: 595.28,
        height: 841.89,
    };
    /// US Letter, 8.5 x 11 in.
    pub const LETTER: PaperSize = PaperSize {
        width: 612.0,
        height: 792.0,
    };

    /// Width left for text between the left and right margins. Never negative.
    pub fn column_width(&self, margin_left: f64, margin_right: f64) -> f64 {
        (self.width - margin_left - margin_right).max(0.0)
    }
}

/// Header and footer templates. `{page}` is the page number and `{pages}` the total.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageChrome {
    /// Text above the content, or `None` for no header.
    pub header: Option<String>,
    /// Text below the content, or `None` for no footer.
    pub footer: Option<String>,
}

/// The header and footer text for one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedChrome {
    /// Page number, 1-based.
    pub page: usize,
    /// Rendered header, if the template has one.
    pub header: Option<String>,
    /// Rendered footer, if the template has one.
    pub footer: Option<String>,
}

/// Fills `{page}` and `{pages}` in `template`.
pub fn render_template(template: &str, page: usize, pages: usize) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(stripped) = after.strip_prefix("{page}") {
            out.push_str(&alloc::format!("{page}"));
            rest = stripped;
        } else if let Some(stripped) = after.strip_prefix("{pages}") {
            out.push_str(&alloc::format!("{pages}"));
            rest = stripped;
        } else {
            out.push('{');
            rest = &after[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Renders the chrome for every page.
pub fn render_chrome(pages: &[Page], chrome: &PageChrome) -> Vec<RenderedChrome> {
    let total = pages.len();
    pages
        .iter()
        .map(|p| RenderedChrome {
            page: p.number,
            header: chrome
                .header
                .as_deref()
                .map(|t| render_template(t, p.number, total)),
            footer: chrome
                .footer
                .as_deref()
                .map(|t| render_template(t, p.number, total)),
        })
        .collect()
}
