// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fonts for text extraction: which characters each code stands for, and how wide each glyph is.
//!
//! A font's characters come from its `ToUnicode` map when it has one. Without one, a simple font's
//! codes are read as Latin-1 (or by the glyph names of its encoding), which is a guess, and the
//! caller reports it. A composite font without a map has no characters to read.

use std::collections::BTreeMap;

use super::objects::{Dict, Document, Obj, decode_stream};

/// The widths a font has when it gives none: half an em.
const DEFAULT_SIMPLE_WIDTH: f64 = 500.0;
/// The widths a composite font has when it gives none: one em (its `/DW` default).
const DEFAULT_COMPOSITE_WIDTH: f64 = 1000.0;
/// The most codes one range of a character map may name. A larger range is a sign of a bad map.
const MAX_RANGE: u32 = 65_536;

/// A font, as far as text extraction needs it.
#[derive(Debug, Clone)]
pub struct Font {
    /// The font is bold, from its name.
    bold: bool,
    /// The font is italic, from its name.
    italic: bool,
    /// Whether codes are read as pairs of bytes (a composite font).
    composite: bool,
    /// Bytes per code.
    code_bytes: usize,
    /// The characters of each code, from the font's `ToUnicode` map.
    to_unicode: BTreeMap<u32, String>,
    /// The width of each code, in thousandths of an em.
    widths: BTreeMap<u32, f64>,
    /// The width of a code the widths do not give.
    default_width: f64,
    /// For a simple font, the glyph name of each code, from its encoding.
    names: BTreeMap<u32, String>,
}

impl Font {
    /// The font that a font object (a dictionary, or a reference to one) describes.
    pub fn load(doc: &Document<'_>, object: &Obj) -> Font {
        let dict = doc.dict_of(object).unwrap_or_default();
        let composite = dict.get("Subtype").and_then(Obj::as_name) == Some("Type0");
        let (code_space, to_unicode) = load_to_unicode(doc, &dict);
        let mut font = if composite {
            load_composite(doc, &dict, code_space, to_unicode)
        } else {
            load_simple(doc, &dict, to_unicode)
        };
        // A font's weight and slant are in its name, as in `Helvetica-Bold` or `Times-Italic`.
        let name = dict
            .get("BaseFont")
            .and_then(Obj::as_name)
            .unwrap_or("")
            .to_ascii_lowercase();
        font.bold = name.contains("bold") || name.contains("black") || name.contains("heavy");
        font.italic = name.contains("italic") || name.contains("oblique");
        font
    }

    /// Whether the font is bold or italic, from its name.
    pub fn is_emphasis(&self) -> bool {
        self.bold || self.italic
    }

    /// The codes in a string of bytes, in order.
    pub fn codes(&self, bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks(self.code_bytes.max(1))
            .map(|chunk| chunk.iter().fold(0u32, |acc, &b| acc << 8 | u32::from(b)))
            .collect()
    }

    /// Bytes per code.
    pub fn code_bytes(&self) -> usize {
        self.code_bytes
    }

    /// The characters a code stands for, and whether they are a guess. `None` when the font gives
    /// no way to read the code.
    pub fn text(&self, code: u32) -> Option<(String, bool)> {
        if let Some(text) = self.to_unicode.get(&code) {
            return Some((text.clone(), false));
        }
        if self.composite {
            return None;
        }
        if let Some(name) = self.names.get(&code) {
            return name_to_char(name).map(|c| (c.to_string(), true));
        }
        // A simple font with no map: the code is read as Latin-1, which is right for the usual
        // encodings in the printable range and a guess everywhere else.
        match code {
            0x20..=0x7E | 0xA0..=0xFF => char::from_u32(code).map(|c| (c.to_string(), true)),
            _ => None,
        }
    }

    /// The width of a code, in thousandths of an em.
    pub fn width(&self, code: u32) -> f64 {
        self.widths
            .get(&code)
            .copied()
            .unwrap_or(self.default_width)
    }
}

/// The `ToUnicode` map of a font: its code-space width, and its entries.
fn load_to_unicode(doc: &Document<'_>, dict: &Dict) -> (Option<usize>, BTreeMap<u32, String>) {
    let Some(stream) = dict.get("ToUnicode").map(|o| doc.resolve(o)) else {
        return (None, BTreeMap::new());
    };
    let Obj::Stream(stream) = stream else {
        return (None, BTreeMap::new());
    };
    match decode_stream(doc, &stream) {
        Ok(data) => parse_cmap(&data),
        Err(_) => (None, BTreeMap::new()),
    }
}

fn load_composite(
    doc: &Document<'_>,
    dict: &Dict,
    code_space: Option<usize>,
    to_unicode: BTreeMap<u32, String>,
) -> Font {
    let descendant = dict
        .get("DescendantFonts")
        .map(|o| doc.resolve(o))
        .and_then(|o| match o {
            Obj::Array(items) => items.first().cloned(),
            _ => None,
        })
        .and_then(|o| doc.dict_of(&o))
        .unwrap_or_default();
    let default_width = descendant
        .get("DW")
        .and_then(Obj::as_f64)
        .unwrap_or(DEFAULT_COMPOSITE_WIDTH);
    let widths = match doc.entry(&descendant, "W") {
        Obj::Array(items) => cid_widths(doc, &items),
        _ => BTreeMap::new(),
    };
    // Identity encodings are two bytes a code; a map that names its code space says so.
    let code_bytes = code_space.unwrap_or(2);
    Font {
        bold: false,
        italic: false,
        composite: true,
        code_bytes,
        to_unicode,
        widths,
        default_width,
        names: BTreeMap::new(),
    }
}

/// The `/W` array of a composite font: `c [w1 w2 ...]` for consecutive codes, or `c1 c2 w` for a
/// range of codes with one width.
fn cid_widths(doc: &Document<'_>, items: &[Obj]) -> BTreeMap<u32, f64> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < items.len() {
        let Some(first) = doc.resolve(&items[i]).as_i64() else {
            break;
        };
        match items.get(i + 1).map(|o| doc.resolve(o)) {
            Some(Obj::Array(widths)) => {
                for (k, w) in widths.iter().enumerate() {
                    if let Some(w) = doc.resolve(w).as_f64() {
                        out.insert((first + k as i64) as u32, w);
                    }
                }
                i += 2;
            }
            Some(second) => {
                let (Some(last), Some(w)) = (
                    second.as_i64(),
                    items.get(i + 2).and_then(|o| doc.resolve(o).as_f64()),
                ) else {
                    break;
                };
                if last >= first && last - first < i64::from(MAX_RANGE) {
                    for code in first..=last {
                        out.insert(code as u32, w);
                    }
                }
                i += 3;
            }
            None => break,
        }
    }
    out
}

fn load_simple(doc: &Document<'_>, dict: &Dict, to_unicode: BTreeMap<u32, String>) -> Font {
    let first_char = doc.entry(dict, "FirstChar").as_i64().unwrap_or(0);
    let mut widths = BTreeMap::new();
    if let Obj::Array(items) = doc.entry(dict, "Widths") {
        for (k, w) in items.iter().enumerate() {
            if let Some(w) = doc.resolve(w).as_f64() {
                widths.insert((first_char + k as i64) as u32, w);
            }
        }
    }
    let mut names = BTreeMap::new();
    if let Obj::Dict(encoding) = doc.entry(dict, "Encoding") {
        if let Obj::Array(differences) = doc.entry(&encoding, "Differences") {
            let mut code = 0i64;
            for item in &differences {
                match doc.resolve(item) {
                    Obj::Int(n) => code = n,
                    Obj::Name(name) => {
                        names.insert(code as u32, name);
                        code += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    Font {
        bold: false,
        italic: false,
        composite: false,
        code_bytes: 1,
        to_unicode,
        widths,
        default_width: DEFAULT_SIMPLE_WIDTH,
        names,
    }
}

/// The characters of a glyph name: a letter or digit, `uniXXXX`, or one of the punctuation names
/// that the standard encodings use.
fn name_to_char(name: &str) -> Option<char> {
    if let Some(hex) = name.strip_prefix("uni") {
        return u32::from_str_radix(hex.get(..4)?, 16)
            .ok()
            .and_then(char::from_u32);
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        return Some(first);
    }
    Some(match name {
        "space" => ' ',
        "period" => '.',
        "comma" => ',',
        "hyphen" => '-',
        "colon" => ':',
        "semicolon" => ';',
        "question" => '?',
        "exclam" => '!',
        "parenleft" => '(',
        "parenright" => ')',
        "slash" => '/',
        "quotedbl" => '"',
        "quoteright" => '\u{2019}',
        "quoteleft" => '\u{2018}',
        "endash" => '\u{2013}',
        "emdash" => '\u{2014}',
        "bullet" => '\u{2022}',
        _ => return None,
    })
}

/// The code-space width and the entries of a `ToUnicode` CMap. The entries come from `bfchar`
/// lines (one code to one string) and `bfrange` lines (a run of codes, counted up from one string,
/// or each given its own).
pub fn parse_cmap(data: &[u8]) -> (Option<usize>, BTreeMap<u32, String>) {
    let tokens = tokens(data);
    let mut code_bytes = None;
    let mut map = BTreeMap::new();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) if w == "begincodespacerange" => {
                i += 1;
                while let Some(Token::Hex(lo)) = tokens.get(i) {
                    code_bytes.get_or_insert(lo.len().max(1));
                    i += 2;
                }
            }
            Token::Word(w) if w == "beginbfchar" => {
                i += 1;
                while let (Some(Token::Hex(src)), Some(dst)) = (tokens.get(i), tokens.get(i + 1)) {
                    if let Token::Hex(dst) = dst {
                        map.insert(hex_code(src), utf16_text(dst));
                    }
                    i += 2;
                }
            }
            Token::Word(w) if w == "beginbfrange" => {
                i += 1;
                while let (Some(Token::Hex(lo)), Some(Token::Hex(hi))) =
                    (tokens.get(i), tokens.get(i + 1))
                {
                    let (lo, hi) = (hex_code(lo), hex_code(hi));
                    i += 2;
                    match tokens.get(i) {
                        Some(Token::Hex(dst)) => {
                            let units = utf16_units(dst);
                            if hi >= lo && hi - lo < MAX_RANGE {
                                for (k, code) in (lo..=hi).enumerate() {
                                    let mut u = units.clone();
                                    if let Some(last) = u.last_mut() {
                                        *last = last.wrapping_add(k as u16);
                                    }
                                    map.insert(code, String::from_utf16_lossy(&u));
                                }
                            }
                            i += 1;
                        }
                        Some(Token::Open) => {
                            i += 1;
                            let mut code = lo;
                            while let Some(Token::Hex(dst)) = tokens.get(i) {
                                if code <= hi {
                                    map.insert(code, utf16_text(dst));
                                }
                                code = code.wrapping_add(1);
                                i += 1;
                            }
                            i += 1; // the closing bracket
                        }
                        _ => {}
                    }
                }
            }
            _ => i += 1,
        }
    }
    (code_bytes, map)
}

/// The value of a code's bytes, read big-endian.
fn hex_code(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, &b| acc << 8 | u32::from(b))
}

fn utf16_units(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks(2)
        .map(|c| {
            if c.len() == 2 {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from(c[0])
            }
        })
        .collect()
}

fn utf16_text(bytes: &[u8]) -> String {
    String::from_utf16_lossy(&utf16_units(bytes))
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Hex(Vec<u8>),
    Word(String),
    Open,
    Close,
}

/// The tokens of a CMap that matter here: hexadecimal strings, brackets, and words. Names,
/// numbers and strings are read as words and ignored.
fn tokens(data: &[u8]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            b'<' if data.get(i + 1) != Some(&b'<') => {
                let end = data[i + 1..]
                    .iter()
                    .position(|&c| c == b'>')
                    .map_or(data.len(), |p| i + 1 + p);
                let digits: Vec<u8> = data[i + 1..end]
                    .iter()
                    .filter_map(|&c| super::objects::hex_value(c))
                    .collect();
                let bytes = digits
                    .chunks(2)
                    .map(|p| p[0] << 4 | p.get(1).copied().unwrap_or(0))
                    .collect();
                out.push(Token::Hex(bytes));
                i = end + 1;
            }
            b'[' => {
                out.push(Token::Open);
                i += 1;
            }
            b']' => {
                out.push(Token::Close);
                i += 1;
            }
            b if b.is_ascii_whitespace()
                || b == b'<'
                || b == b'>'
                || b == b'('
                || b == b')'
                || b == b'/' =>
            {
                i += 1;
            }
            _ => {
                let start = i;
                while i < data.len()
                    && !data[i].is_ascii_whitespace()
                    && !matches!(data[i], b'<' | b'>' | b'[' | b']' | b'/' | b'(' | b')')
                {
                    i += 1;
                }
                out.push(Token::Word(
                    String::from_utf8_lossy(&data[start..i]).into_owned(),
                ));
            }
        }
    }
    out
}
