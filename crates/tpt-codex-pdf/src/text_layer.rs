// SPDX-License-Identifier: MIT OR Apache-2.0
//! The text layer: the words of a PDF, drawn invisibly over its outlines.
//!
//! Text is drawn as glyph outlines, so a page shows the text but cannot search, select or read it.
//! This adds the text back. Each glyph is written again in an invisible text object (render mode 3)
//! at the position of its outline, with the bundled font embedded as a CID font and a ToUnicode map
//! that says which characters each glyph stands for.
//!
//! The file is extended by an incremental update: the page objects are rewritten to use the new
//! content and fonts, and the new objects follow the original bytes, which are not changed. A
//! reader that ignores the layer draws the same page.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

use crate::subset::subset_truetype;

/// One glyph of the text layer, where it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct TextGlyph {
    /// The glyph id in its face.
    pub glyph: u32,
    /// The glyph origin, in points from the left edge of the page.
    pub x: f32,
    /// The baseline, in points from the top edge of the page. Positive is down.
    pub y: f32,
    /// The glyph's horizontal advance, in points.
    pub advance: f32,
    /// The characters the glyph stands for, when it starts a cluster. Empty otherwise.
    pub text: String,
}

/// A run of glyphs from one face at one size.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    /// The index of the face in the faces given to [`add_text_layer`].
    pub face: usize,
    /// The size in points.
    pub size: f32,
    /// The glyphs, in drawing order.
    pub glyphs: Vec<TextGlyph>,
}

/// The text layer could not be added to the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayerError(pub String);

impl fmt::Display for TextLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text layer: {}", self.0)
    }
}

impl std::error::Error for TextLayerError {}

fn fail<T>(message: impl Into<String>) -> Result<T, TextLayerError> {
    Err(TextLayerError(message.into()))
}

/// Adds the text layer to `pdf`, a file of one page per entry of `pages`, each `page_height`
/// points tall, as written by the export. `faces` holds the font file of each face the runs index.
///
/// The original bytes are kept; the page objects are rewritten, and the new objects follow them.
pub fn add_text_layer(
    pdf: &[u8],
    page_height: f32,
    pages: &[Vec<TextRun>],
    faces: &[&[u8]],
) -> Result<Vec<u8>, TextLayerError> {
    let xref = start_xref(pdf)?;
    let mut next = declared_size(pdf)?;

    let used: BTreeSet<usize> = pages.iter().flatten().map(|run| run.face).collect();
    if let Some(&face) = used.iter().find(|&&f| f >= faces.len()) {
        return fail(format!(
            "a run uses face {face}, but {} faces were given",
            faces.len()
        ));
    }

    let mut objects: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut fonts: BTreeMap<usize, FontObjects> = BTreeMap::new();
    for &face in &used {
        let glyphs: BTreeSet<u32> = pages
            .iter()
            .flatten()
            .filter(|run| run.face == face)
            .flat_map(|run| run.glyphs.iter().map(|g| g.glyph))
            .collect();
        let file = subset_truetype(faces[face], &glyphs)
            .map_err(|e| TextLayerError(format!("face {face}: {e}")))?;
        let font = FontObjects::allocate(&mut next);
        font.push_objects(face, &file, pages, &mut objects);
        fonts.insert(face, font);
    }
    let font_resources: String = fonts
        .iter()
        .map(|(face, font)| format!("/TPTText{face} {} 0 R ", font.type0))
        .collect();

    for (index, runs) in pages.iter().enumerate() {
        let page_object = 3 + 2 * index as u64;
        let content = 4 + 2 * index as u64;
        let text_stream = next;
        next += 1;

        let page = object_text(pdf, page_object)?;
        if page.contains("/Font") {
            return fail(format!("page {index} already has fonts"));
        }
        let contents = format!("/Contents {content} 0 R");
        let resources = "/Resources << ";
        if !page.contains(&contents) || !page.contains(resources) {
            return fail(format!(
                "page {index} is not in the form the export writes (no contents or resources)"
            ));
        }
        let rewritten = page
            .replacen(
                &contents,
                &format!("/Contents [{content} 0 R {text_stream} 0 R]"),
                1,
            )
            .replacen(
                resources,
                &format!("{resources}/Font << {font_resources}>> "),
                1,
            );
        objects.push((page_object, rewritten.into_bytes()));
        objects.push((
            text_stream,
            stream_bytes("", content_stream(runs, page_height).as_bytes()),
        ));
    }
    Ok(append(pdf, &objects, next, xref))
}

/// The numbers of one embedded font's objects.
struct FontObjects {
    type0: u64,
    descendant: u64,
    descriptor: u64,
    file: u64,
    to_unicode: u64,
}

impl FontObjects {
    fn allocate(next: &mut u64) -> Self {
        let font = Self {
            type0: *next,
            descendant: *next + 1,
            descriptor: *next + 2,
            file: *next + 3,
            to_unicode: *next + 4,
        };
        *next += 5;
        font
    }

    /// The five objects of the font: the composite font, its descendant, the descriptor, the
    /// font file, and the ToUnicode map. Glyph widths and characters come from the runs.
    fn push_objects(
        &self,
        face: usize,
        font_file: &[u8],
        pages: &[Vec<TextRun>],
        out: &mut Vec<(u64, Vec<u8>)>,
    ) {
        let name = format!("TPTText{face}");
        // The first width and the first characters seen for each glyph.
        let mut widths: BTreeMap<u32, f32> = BTreeMap::new();
        let mut characters: BTreeMap<u32, String> = BTreeMap::new();
        for run in pages.iter().flatten().filter(|r| r.face == face) {
            for g in &run.glyphs {
                if run.size > 0.0 {
                    widths
                        .entry(g.glyph)
                        .or_insert(g.advance / run.size * 1000.0);
                }
                if !g.text.is_empty() {
                    characters.entry(g.glyph).or_insert_with(|| g.text.clone());
                }
            }
        }
        let mut w_entries = String::new();
        for (glyph, width) in &widths {
            let _ = write!(w_entries, " {glyph} [{}]", fmt(*width));
        }
        out.push((
            self.type0,
            format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H \
                 /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
                self.descendant, self.to_unicode
            )
            .into_bytes(),
        ));
        out.push((
            self.descendant,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} \
                 /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor {} 0 R /DW 1000 /W [{w_entries} ] /CIDToGIDMap /Identity >>",
                self.descriptor
            )
            .into_bytes(),
        ));
        out.push((
            self.descriptor,
            format!(
                "<< /Type /FontDescriptor /FontName /{name} /Flags 32 \
                 /FontBBox [-1000 -1000 2000 2000] /ItalicAngle 0 /Ascent 800 /Descent -200 \
                 /CapHeight 700 /StemV 80 /FontFile2 {} 0 R >>",
                self.file
            )
            .into_bytes(),
        ));
        out.push((
            self.file,
            stream_bytes(&format!("/Length1 {}", font_file.len()), font_file),
        ));
        out.push((
            self.to_unicode,
            stream_bytes("", to_unicode(&characters).as_bytes()),
        ));
    }
}

/// The invisible text of one page: each glyph in a text object of its own, at its position.
fn content_stream(runs: &[TextRun], page_height: f32) -> String {
    let mut out = String::from("q\n");
    for run in runs {
        for g in &run.glyphs {
            let _ = writeln!(
                out,
                "BT 3 Tr /TPTText{face} {size} Tf 1 0 0 1 {x} {y} Tm <{glyph:04X}> Tj ET",
                face = run.face,
                size = fmt(run.size),
                x = fmt(g.x),
                y = fmt(page_height - g.y),
                glyph = g.glyph,
            );
        }
    }
    out.push_str("Q\n");
    out
}

/// A ToUnicode map from glyph ids to the characters they stand for.
fn to_unicode(characters: &BTreeMap<u32, String>) -> String {
    let entries: Vec<(&u32, &String)> = characters.iter().collect();
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in entries.chunks(100) {
        let _ = writeln!(out, "{} beginbfchar", chunk.len());
        for (glyph, text) in chunk {
            let units: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(out, "<{glyph:04X}> <{units}>");
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out
}

/// A stream object: its dictionary with the length and `extra` entries, then the data.
fn stream_bytes(extra: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< /Length {} {extra} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

/// Writes the new objects after the original bytes, with a cross-reference section for them and a
/// trailer that points back at the original one.
fn append(pdf: &[u8], objects: &[(u64, Vec<u8>)], size: u64, previous_xref: usize) -> Vec<u8> {
    let mut out = pdf.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut offsets: BTreeMap<u64, usize> = BTreeMap::new();
    for (number, body) in objects {
        offsets.insert(*number, out.len());
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n");
    // Consecutive object numbers share one subsection.
    let numbers: Vec<u64> = offsets.keys().copied().collect();
    let mut start = 0;
    while start < numbers.len() {
        let mut end = start;
        while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
            end += 1;
        }
        out.extend_from_slice(format!("{} {}\n", numbers[start], end - start + 1).as_bytes());
        for number in &numbers[start..=end] {
            out.extend_from_slice(format!("{:010} 00000 n \n", offsets[number]).as_bytes());
        }
        start = end + 1;
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {size} /Root 1 0 R /Prev {previous_xref} >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    out
}

/// The offset in the `startxref` line at the end of the file.
fn start_xref(pdf: &[u8]) -> Result<usize, TextLayerError> {
    let at = rfind(pdf, b"startxref").ok_or_else(|| TextLayerError("no startxref".into()))?;
    number_at(pdf, at + b"startxref".len())
        .ok_or_else(|| TextLayerError("no offset after startxref".into()))
}

/// The `/Size` of the last trailer: the number of object numbers the file uses.
fn declared_size(pdf: &[u8]) -> Result<u64, TextLayerError> {
    let trailer = rfind(pdf, b"trailer").ok_or_else(|| TextLayerError("no trailer".into()))?;
    let at = find_from(pdf, trailer, b"/Size")
        .ok_or_else(|| TextLayerError("no /Size in the trailer".into()))?;
    number_at(pdf, at + b"/Size".len())
        .map(|n| n as u64)
        .ok_or_else(|| TextLayerError("no number after /Size".into()))
}

/// The text of object `number`: what lies between `N 0 obj` and `endobj`, trimmed.
fn object_text(pdf: &[u8], number: u64) -> Result<String, TextLayerError> {
    let header = format!("{number} 0 obj");
    let mut from = 0;
    while let Some(at) = find_from(pdf, from, header.as_bytes()) {
        // An object header starts a line: the number is not the end of a longer one.
        if at == 0 || pdf[at - 1] == b'\n' {
            let body_start = at + header.len();
            let body_end = find_from(pdf, body_start, b"endobj")
                .ok_or_else(|| TextLayerError(format!("object {number} has no endobj")))?;
            let text = std::str::from_utf8(&pdf[body_start..body_end])
                .map_err(|_| TextLayerError(format!("object {number} is not text")))?;
            return Ok(text.trim().to_string());
        }
        from = at + 1;
    }
    fail(format!("object {number} is missing"))
}

fn find_from(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// The unsigned integer that starts at or after `from`, skipping white space.
fn number_at(pdf: &[u8], from: usize) -> Option<usize> {
    let rest = pdf.get(from..)?;
    let skip = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let digits: Vec<u8> = rest[skip..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .copied()
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// A number for a PDF content stream: up to three decimals, no exponent.
fn fmt(value: f32) -> String {
    let text = format!("{value:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}
