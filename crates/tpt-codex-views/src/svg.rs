// SPDX-License-Identifier: MIT OR Apache-2.0
//! SVG export of one slide. Text is wrapped with the same measure the Flow view uses, so the
//! lines match the layout. Other blocks are drawn as labelled boxes.

use alloc::format;
use alloc::string::String;
use core::fmt::Write;

use tpt_codex_core::{Block, Document, MediaSource};

use crate::canvas::{CanvasError, layout_canvas};
use crate::flow::wrap;
use crate::measure::TextMeasure;

/// Renders slide `slide` as an SVG document. Items without a frame are not drawn.
pub fn render_slide_svg(
    doc: &Document,
    slide: tpt_codex_core::BlockId,
    measure: &dyn TextMeasure,
) -> Result<String, CanvasError> {
    let layout = layout_canvas(doc, slide)?;
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = layout.width,
        h = layout.height
    );
    let _ = write!(
        out,
        r#"<rect x="0" y="0" width="{}" height="{}" fill="white"/>"#,
        layout.width, layout.height
    );
    for item in &layout.items {
        let Some(frame) = item.frame else { continue };
        match doc.get(item.block) {
            Some(Block::Text { content, style }) => {
                let step = measure.line_height(style);
                let lines = wrap(content, style, measure, frame.width);
                for (n, line) in lines.iter().enumerate() {
                    let y = frame.y + step * (n as f64 + 1.0);
                    if y > frame.y + frame.height + step {
                        break;
                    }
                    let _ = write!(
                        out,
                        r#"<text x="{}" y="{}" font-family="Noto Sans, sans-serif" font-size="{}" font-weight="{}" font-style="{}">{}</text>"#,
                        frame.x,
                        y,
                        step * 0.8,
                        if style.bold || style.heading_level.is_some() {
                            "bold"
                        } else {
                            "normal"
                        },
                        if style.italic { "italic" } else { "normal" },
                        escape(line)
                    );
                }
            }
            Some(Block::Media {
                source: MediaSource::Svg { markup },
                alt_text,
            }) => {
                // An SVG is drawn as an image, so any script inside it cannot run.
                let href =
                    alloc::format!("data:image/svg+xml;base64,{}", base64(markup.as_bytes()));
                let _ = write!(
                    out,
                    r#"<image x="{}" y="{}" width="{}" height="{}" href="{}" preserveAspectRatio="xMidYMid meet"><title>{}</title></image>"#,
                    frame.x,
                    frame.y,
                    frame.width,
                    frame.height,
                    href,
                    escape(alt_text.as_deref().unwrap_or("image"))
                );
            }
            other => {
                let label = match other {
                    Some(Block::Grid { .. }) => "grid",
                    Some(Block::Media { .. }) => "media",
                    Some(Block::Query(_)) => "query",
                    Some(_) => "block",
                    None => "missing",
                };
                let _ = write!(
                    out,
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="#888" stroke-dasharray="4 3"><title>{}</title></rect>"##,
                    frame.x,
                    frame.y,
                    frame.width,
                    frame.height,
                    escape(&format!("{label} {}", item.block.0))
                );
            }
        }
    }
    out.push_str("</svg>");
    Ok(out)
}

/// Standard base64 with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(TABLE[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Escapes text for XML element content and attribute values.
pub(crate) fn escape(text: &str) -> String {
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
