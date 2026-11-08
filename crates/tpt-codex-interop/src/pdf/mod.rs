// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.pdf` import: the text of a PDF, as paragraphs and headings.
//!
//! The file is read in-house: its objects and cross-reference tables ([`objects`]), the fonts and
//! their character maps ([`font`]), and the content stream of each page ([`content`]). The text
//! is grouped into lines, the lines into paragraphs, and the largest type sizes become headings.
//!
//! What is not imported is counted in the [`FidelityReport`]: images, drawings (lines and shapes),
//! characters with no way to read them, and characters read from a font with no character map,
//! which are a guess. Encrypted files are refused.
//!
//! The PDFs this project writes carry a text layer, so they read back as the text they show.

mod content;
mod font;
mod objects;

use std::fmt;

use tpt_codex_core::{Block, BlockId, Document as CoreDocument, LayoutMode, Metadata, TextStyle};

use crate::fidelity::FidelityReport;
use content::{Glyph, PageContent, read_page};
use objects::{Dict, Document, Obj};

/// The most levels of the page tree that are followed.
const MAX_PAGE_DEPTH: usize = 32;
/// Lines whose baselines are closer than this share a line, as a fraction of the font size.
const SAME_LINE: f64 = 0.5;
/// Lines whose gap is more than this many font sizes start a new paragraph.
const PARAGRAPH_GAP: f64 = 1.45;
/// A line whose type is this many times the body size is a heading.
const HEADING_RATIO: f64 = 1.2;
/// Lines this close to the bottom of the page, in points, are a footer, where a page number is.
const FOOTER_HEIGHT: f64 = 60.0;

/// A PDF that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// The file has no `%PDF-` header.
    NotPdf,
    /// The file is encrypted, and its text cannot be read without a password.
    Encrypted,
    /// The file is damaged, in the way named.
    Damaged(&'static str),
    /// The file uses a feature this reader does not have.
    Unsupported(String),
    /// The file has no pages.
    NoPages,
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPdf => write!(f, "not a PDF file (no %PDF- header)"),
            Self::Encrypted => write!(f, "the PDF is encrypted; its text cannot be read"),
            Self::Damaged(what) => write!(f, "the PDF is damaged: {what}"),
            Self::Unsupported(what) => write!(f, "the PDF uses {what}, which is not supported"),
            Self::NoPages => write!(f, "the PDF has no pages"),
        }
    }
}

impl std::error::Error for PdfError {}

/// The result of a PDF import: the document and what was not imported.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfImport {
    /// The text as a document: paragraphs and headings in reading order, with the title and
    /// author from the file's information dictionary.
    pub document: CoreDocument,
    /// What was left out, or read with a guess.
    pub report: FidelityReport,
}

/// Reads the text of a PDF file.
pub fn import_pdf(bytes: &[u8]) -> Result<PdfImport, PdfError> {
    let doc = Document::open(bytes)?;
    let root = doc
        .trailer
        .get("Root")
        .map(|o| doc.resolve(o))
        .and_then(|o| doc.dict_of(&o))
        .ok_or(PdfError::Damaged("the trailer has no catalog"))?;
    let pages_root = doc.entry(&root, "Pages");
    let mut pages = Vec::new();
    collect_pages(&doc, &pages_root, &Dict::new(), &mut pages, 0)?;
    if pages.is_empty() {
        return Err(PdfError::NoPages);
    }

    let mut report = FidelityReport::new();
    let mut page_lines: Vec<Vec<Line>> = Vec::with_capacity(pages.len());
    let (mut images, mut drawings, mut unmapped, mut guessed, mut damaged, mut emphasis) =
        (0, 0, 0, 0, 0, 0);
    let mut page_numbers = 0usize;
    for page in &pages {
        let content = match doc.decode_contents(&page.contents) {
            Ok(bytes) => bytes,
            Err(_) => {
                damaged += 1;
                page_lines.push(Vec::new());
                continue;
            }
        };
        let read = match read_page(&doc, &content, &page.resources) {
            Ok(read) => read,
            Err(_) => {
                damaged += 1;
                page_lines.push(Vec::new());
                continue;
            }
        };
        images += read.images;
        drawings += read.drawings;
        unmapped += read.unmapped;
        guessed += read.glyphs.iter().filter(|g| g.guessed).count();
        emphasis += read.glyphs.iter().filter(|g| g.emphasis).count();
        let mut page = lines(&read);
        page.retain(|line| {
            let number = line.y < FOOTER_HEIGHT && is_page_number(&line.text);
            page_numbers += usize::from(number);
            !number
        });
        page_lines.push(page);
    }

    if images > 0 {
        report.dropped(
            "document",
            "images",
            format!("{images} image(s) not imported"),
        );
    }
    if drawings > 0 {
        report.dropped(
            "document",
            "drawings",
            format!("{drawings} drawn path(s) and shape(s) not imported"),
        );
    }
    if unmapped > 0 {
        report.dropped(
            "document",
            "characters",
            format!("{unmapped} character(s) with no way to read them were skipped"),
        );
    }
    if guessed > 0 {
        report.approximated(
            "document",
            "fonts",
            format!("{guessed} character(s) read from a font with no character map, as a guess"),
        );
    }
    if page_numbers > 0 {
        report.dropped(
            "document",
            "page numbers",
            format!("{page_numbers} page number(s) in the footers not imported"),
        );
    }
    if emphasis > 0 {
        report.approximated(
            "document",
            "emphasis",
            format!("{emphasis} character(s) in bold or italic were read as plain text"),
        );
    }
    if damaged > 0 {
        report.dropped(
            "document",
            "pages",
            format!("{damaged} page(s) with damaged content were skipped"),
        );
    }

    let blocks = paragraphs(&page_lines);
    let mut document = CoreDocument::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=blocks.len() as u64).map(BlockId).collect(),
        },
    );
    for (i, block) in blocks.into_iter().enumerate() {
        document.insert(BlockId(i as u64 + 1), block);
    }
    document.metadata = metadata(&doc);
    Ok(PdfImport { document, report })
}

/// A page's content and resources, with the resources it inherits from its ancestors.
struct Page {
    contents: Vec<Obj>,
    resources: Dict,
}

/// Walks the page tree from `node`, adding the pages in order.
fn collect_pages(
    doc: &Document<'_>,
    node: &Obj,
    inherited: &Dict,
    out: &mut Vec<Page>,
    depth: usize,
) -> Result<(), PdfError> {
    if depth > MAX_PAGE_DEPTH {
        return Err(PdfError::Damaged("the page tree is nested too deeply"));
    }
    let Some(dict) = doc.dict_of(node) else {
        return Ok(());
    };
    let mut inherited = inherited.clone();
    if let Some(resources) = doc.dict_of(&doc.entry(&dict, "Resources")) {
        inherited.insert("Resources".into(), Obj::Dict(resources));
    }
    match dict.get("Type").and_then(Obj::as_name) {
        Some("Pages") => {
            if let Obj::Array(kids) = doc.entry(&dict, "Kids") {
                for kid in &kids {
                    collect_pages(doc, kid, &inherited, out, depth + 1)?;
                }
            }
        }
        _ => {
            let contents = match doc.entry(&dict, "Contents") {
                Obj::Array(items) => items,
                Obj::Null => Vec::new(),
                other => vec![other],
            };
            let resources = match inherited.get("Resources") {
                Some(Obj::Dict(d)) => d.clone(),
                _ => Dict::new(),
            };
            out.push(Page {
                contents: contents.iter().map(|c| doc.resolve(c)).collect(),
                resources,
            });
        }
    }
    Ok(())
}

/// A line of text: its characters, its baseline, and its type size.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    text: String,
    /// The baseline, in points from the bottom of the page.
    y: f64,
    size: f64,
}

/// The lines of one page, top to bottom. Glyphs on one baseline make a line; a space is put
/// between glyphs that have a gap on the page.
fn lines(read: &PageContent) -> Vec<Line> {
    let mut glyphs: Vec<&Glyph> = read.glyphs.iter().filter(|g| g.size > 0.0).collect();
    glyphs.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut groups: Vec<Vec<&Glyph>> = Vec::new();
    for glyph in glyphs {
        let joins = groups.last().and_then(|g| g.first()).is_some_and(|first| {
            (first.y - glyph.y).abs() <= SAME_LINE * first.size.max(glyph.size)
        });
        if joins {
            if let Some(group) = groups.last_mut() {
                group.push(glyph);
            }
        } else {
            groups.push(vec![glyph]);
        }
    }
    groups
        .into_iter()
        .map(|mut group| {
            group.sort_by(|a, b| a.x.total_cmp(&b.x));
            let size = group.iter().map(|g| g.size).fold(0.0, f64::max);
            let y = group.iter().map(|g| g.y).sum::<f64>() / group.len() as f64;
            let mut text = String::new();
            let mut end: Option<f64> = None;
            for glyph in &group {
                if let Some(end) = end {
                    if glyph.x - end > 0.2 * glyph.size
                        && !text.ends_with(' ')
                        && !glyph.text.starts_with(' ')
                    {
                        text.push(' ');
                    }
                }
                text.push_str(&glyph.text);
                end = Some(glyph.x + glyph.width);
            }
            Line {
                text: text.trim().to_string(),
                y,
                size,
            }
        })
        .filter(|line| !line.text.is_empty())
        .collect()
}

/// The document's blocks: headings, and paragraphs of joined lines.
fn paragraphs(pages: &[Vec<Line>]) -> Vec<Block> {
    // The body size is the one most characters are set in.
    let mut sizes: Vec<(f64, usize)> = Vec::new();
    for line in pages.iter().flatten() {
        let key = (line.size * 10.0).round() / 10.0;
        match sizes.iter_mut().find(|(s, _)| *s == key) {
            Some((_, n)) => *n += line.text.chars().count(),
            None => sizes.push((key, line.text.chars().count())),
        }
    }
    let body = sizes
        .iter()
        .max_by_key(|(_, n)| *n)
        .map_or(0.0, |(s, _)| *s);
    // Heading sizes, largest first, get levels 1 to 6.
    let mut heading_sizes: Vec<f64> = sizes
        .iter()
        .map(|(s, _)| *s)
        .filter(|s| body > 0.0 && *s >= body * HEADING_RATIO)
        .collect();
    heading_sizes.sort_by(|a, b| b.total_cmp(a));
    let level_of = |size: f64| -> Option<u8> {
        let key = (size * 10.0).round() / 10.0;
        heading_sizes
            .iter()
            .position(|s| *s == key)
            .map(|i| (i + 1).min(6) as u8)
    };

    let mut blocks: Vec<Block> = Vec::new();
    // The paragraph being built: its text, its size, and its heading level.
    let mut current: Option<(String, f64, Option<u8>)> = None;
    let flush = |current: &mut Option<(String, f64, Option<u8>)>, blocks: &mut Vec<Block>| {
        if let Some((content, _, level)) = current.take() {
            blocks.push(Block::Text {
                content,
                style: TextStyle {
                    heading_level: level,
                    ..TextStyle::default()
                },
            });
        }
    };
    for page in pages {
        // A new page always starts a new paragraph.
        flush(&mut current, &mut blocks);
        let mut previous: Option<(f64, f64)> = None;
        for line in page {
            let level = level_of(line.size);
            let gap_break =
                previous.is_some_and(|(y, size)| y - line.y > PARAGRAPH_GAP * size.max(line.size));
            let kind_changes = current
                .as_ref()
                .is_some_and(|(_, size, lvl)| *lvl != level || (*size - line.size).abs() > 0.5);
            // A list item is a paragraph of its own, as the list is laid out line by line.
            let starts_item = starts_list_item(&line.text);
            if current.is_some() && (gap_break || kind_changes || starts_item) {
                flush(&mut current, &mut blocks);
            }
            match current.as_mut() {
                Some((content, _, _)) => join(content, &line.text),
                None => current = Some((line.text.clone(), line.size, level)),
            }
            previous = Some((line.y, line.size));
        }
    }
    flush(&mut current, &mut blocks);
    blocks
}

/// Whether a line begins a list item: a bullet, a dash or star, or a number with a full stop or a
/// bracket, each followed by a space.
fn starts_list_item(text: &str) -> bool {
    match text.chars().next() {
        Some('\u{2022}' | '\u{25E6}' | '\u{25AA}') => true,
        Some('-' | '*') => text[1..].starts_with(' '),
        Some(c) if c.is_ascii_digit() => {
            let rest = text.trim_start_matches(|c: char| c.is_ascii_digit());
            rest.starts_with(". ") || rest.starts_with(") ")
        }
        _ => false,
    }
}

/// Appends a wrapped line to a paragraph. A line that ends in a hyphen and is followed by a lower
/// case start is a word broken at the line end, so it is joined without a space.
fn join(content: &mut String, next: &str) {
    let continues_word =
        content.ends_with('-') && next.chars().next().is_some_and(|c| c.is_lowercase());
    if !continues_word {
        content.push(' ');
    }
    content.push_str(next);
}

/// The title and author from the information dictionary, when the file has them.
fn metadata(doc: &Document<'_>) -> Metadata {
    let mut meta = Metadata::default();
    let Some(info) = doc.trailer.get("Info").and_then(|o| doc.dict_of(o)) else {
        return meta;
    };
    meta.title = text_string(doc, &info, "Title");
    meta.author = text_string(doc, &info, "Author");
    meta
}

/// A text string from a dictionary: UTF-16 with a byte-order mark, or Latin-1 otherwise.
fn text_string(doc: &Document<'_>, dict: &Dict, key: &str) -> Option<String> {
    let Obj::Str(bytes) = doc.entry(dict, key) else {
        return None;
    };
    let text = if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| char::from(b)).collect()
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Whether a footer line is a page number: `7`, `Page 7`, or `Page 7 of 12`.
fn is_page_number(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    let digits = |w: &str| !w.is_empty() && w.len() <= 6 && w.bytes().all(|b| b.is_ascii_digit());
    match words.as_slice() {
        [n] => digits(n),
        ["Page", n] => digits(n),
        ["Page", n, "of", m] => digits(n) && digits(m),
        _ => false,
    }
}
