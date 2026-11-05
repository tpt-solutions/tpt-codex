// SPDX-License-Identifier: MIT OR Apache-2.0
//! SVG for a laid-out formula.

use alloc::format;
use alloc::string::String;
use core::fmt::Write;

use crate::math_layout::{MathBox, MathItem};

/// Renders `b` as an SVG document in `family`. The baseline is placed `ascent` below the top.
pub fn math_svg(b: &MathBox, family: &str) -> String {
    let height = b.ascent + b.descent;
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = b.width,
        h = height
    );
    for item in &b.items {
        match item {
            MathItem::Glyph { x, y, ch, size } => {
                let _ = write!(
                    out,
                    r#"<text x="{}" y="{}" font-family="{}" font-size="{}">{}</text>"#,
                    x,
                    b.ascent - y,
                    escape(family),
                    size,
                    escape(&format!("{ch}"))
                );
            }
            MathItem::Rule {
                x,
                y,
                width,
                height,
            } => {
                let _ = write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}"/>"#,
                    x,
                    b.ascent - (y + height),
                    width,
                    height
                );
            }
        }
    }
    out.push_str("</svg>");
    out
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}
