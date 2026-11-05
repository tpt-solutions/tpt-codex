// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.docx` import: best-effort text and structure, on the in-house [`archive`](crate::archive) reader.
//!
//! What comes through: paragraphs in order, headings (`Heading1` to `Heading6`, and `Title` as level
//! 1), bold and italic when a whole paragraph has them, line and tab breaks, and the title and author
//! from the document properties. Tables are flattened to paragraphs. List items become plain
//! paragraphs. Images are not imported.
//!
//! Everything that does not come through is counted in the [`FidelityReport`], one item per kind.

use std::fmt;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tpt_codex_core::{Block, BlockId, Document, InlineRun, LayoutMode, Metadata, TextStyle};

use crate::archive::{Archive, ZipError};
use crate::fidelity::FidelityReport;

/// A `.docx` file that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxError(pub String);

impl fmt::Display for DocxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "docx: {}", self.0)
    }
}

impl std::error::Error for DocxError {}

impl From<ZipError> for DocxError {
    fn from(e: ZipError) -> Self {
        DocxError(e.to_string())
    }
}

/// The result of a Word import: the document and what was lost.
#[derive(Debug, Clone, PartialEq)]
pub struct DocxImport {
    /// The text as a document: one root flow container of text blocks, plus title and author.
    pub document: Document,
    /// What was flattened, approximated or dropped.
    pub report: FidelityReport,
}

/// Imports a `.docx` file.
pub fn import_docx(bytes: &[u8]) -> Result<DocxImport, DocxError> {
    let archive = Archive::open(bytes)?;
    let body = read_part(&archive, "word/document.xml")?;
    let parsed = parse_body(&body)?;

    let mut report = FidelityReport::new();
    if parsed.tables > 0 {
        report.approximated(
            "document",
            "tables",
            format!("{} table(s) flattened to paragraphs", parsed.tables),
        );
    }
    if parsed.list_items > 0 {
        report.approximated(
            "document",
            "lists",
            format!(
                "{} list item(s) kept as plain paragraphs; list markers are not kept",
                parsed.list_items
            ),
        );
    }
    if parsed.images > 0 {
        report.dropped(
            "document",
            "images",
            format!("{} image(s) not imported", parsed.images),
        );
    }

    let mut document = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=parsed.blocks.len() as u64).map(BlockId).collect(),
        },
    );
    for (i, block) in parsed.blocks.into_iter().enumerate() {
        document.insert(BlockId(i as u64 + 1), block);
    }
    document.metadata = read_metadata(&archive);
    Ok(DocxImport { document, report })
}

pub(crate) fn read_part(archive: &Archive<'_>, name: &str) -> Result<String, DocxError> {
    let bytes = archive.read(name)?;
    String::from_utf8(bytes).map_err(|_| DocxError(format!("'{name}' is not UTF-8")))
}

/// Title and author from `docProps/core.xml`. Missing or unreadable properties are left empty.
pub(crate) fn read_metadata(archive: &Archive<'_>) -> Metadata {
    let mut meta = Metadata::default();
    let Ok(xml) = read_part(archive, "docProps/core.xml") else {
        return meta;
    };
    let mut reader = Reader::from_str(&xml);
    let mut field: Option<&'static str> = None;
    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(e) => {
                field = match e.local_name().as_ref() {
                    b"title" => Some("title"),
                    b"creator" => Some("creator"),
                    _ => None,
                };
            }
            Event::Text(t) => {
                if let (Some(which), Ok(text)) = (field, t.unescape()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        match which {
                            "title" => meta.title = Some(text.to_string()),
                            _ => meta.author = Some(text.to_string()),
                        }
                    }
                }
            }
            Event::End(_) => field = None,
            Event::Eof => break,
            _ => {}
        }
    }
    meta
}

#[derive(Debug, Default, Clone)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
}

#[derive(Debug, Default)]
struct Parsed {
    blocks: Vec<Block>,
    tables: usize,
    list_items: usize,
    images: usize,
}

/// The paragraph being read, and the run inside it.
#[derive(Debug, Default)]
struct Paragraph {
    style: Option<String>,
    numbered: bool,
    runs: Vec<Run>,
}

fn on_off(e: &BytesStart<'_>) -> Result<bool, DocxError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.as_ref() == b"w:val" {
            let v = a.unescape_value().map_err(xml_err)?;
            return Ok(!matches!(v.as_ref(), "0" | "false" | "off"));
        }
    }
    Ok(true)
}

fn style_val(e: &BytesStart<'_>) -> Result<Option<String>, DocxError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.as_ref() == b"w:val" {
            return Ok(Some(a.unescape_value().map_err(xml_err)?.into_owned()));
        }
    }
    Ok(None)
}

pub(crate) fn xml_err(e: impl fmt::Display) -> DocxError {
    DocxError(format!("malformed XML: {e}"))
}

/// The heading level of a paragraph style, if it is one.
fn heading_level(style: &str) -> Option<u8> {
    let normalized: String = style
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized == "title" {
        return Some(1);
    }
    let digit = normalized.strip_prefix("heading")?;
    match digit.parse::<u8>() {
        Ok(n @ 1..=6) => Some(n),
        _ => None,
    }
}

fn is_list_style(style: &str) -> bool {
    style.to_ascii_lowercase().contains("list")
}

fn parse_body(xml: &str) -> Result<Parsed, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut out = Parsed::default();
    let mut para: Option<Paragraph> = None;
    let mut run: Option<Run> = None;
    let mut in_text = false;
    let mut table_depth = 0usize;

    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"p" => para = Some(Paragraph::default()),
                b"r" => run = Some(Run::default()),
                b"t" => in_text = true,
                b"tbl" => {
                    if table_depth == 0 {
                        out.tables += 1;
                    }
                    table_depth += 1;
                }
                b"drawing" | b"pict" => out.images += 1,
                b"b" | b"i" => apply_run_flag(&e, run.as_mut())?,
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"pStyle" => {
                    if let Some(p) = para.as_mut() {
                        p.style = style_val(&e)?;
                    }
                }
                b"numPr" => {
                    if let Some(p) = para.as_mut() {
                        p.numbered = true;
                    }
                }
                b"b" | b"i" => apply_run_flag(&e, run.as_mut())?,
                b"tab" => {
                    if let Some(r) = run.as_mut() {
                        r.text.push('\t');
                    }
                }
                b"br" => {
                    if let Some(r) = run.as_mut() {
                        r.text.push('\n');
                    }
                }
                b"drawing" | b"pict" => out.images += 1,
                _ => {}
            },
            Event::Text(t) if in_text => {
                let text = t.unescape().map_err(xml_err)?;
                if let Some(r) = run.as_mut() {
                    r.text.push_str(&text);
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"r" => {
                    if let (Some(r), Some(p)) = (run.take(), para.as_mut()) {
                        p.runs.push(r);
                    }
                }
                b"p" => {
                    if let Some(p) = para.take() {
                        finish_paragraph(p, &mut out);
                    }
                }
                b"tbl" => table_depth = table_depth.saturating_sub(1),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// `w:b` and `w:i` inside a run's properties. A run property on its own element sets the run.
fn apply_run_flag(e: &BytesStart<'_>, run: Option<&mut Run>) -> Result<(), DocxError> {
    let Some(r) = run else {
        return Ok(());
    };
    let on = on_off(e)?;
    match e.local_name().as_ref() {
        b"b" => r.bold = on,
        b"i" => r.italic = on,
        _ => {}
    }
    Ok(())
}

fn finish_paragraph(p: Paragraph, out: &mut Parsed) {
    // Each character with its marks. Whitespace at the ends is trimmed, as before.
    let chars: Vec<(char, bool, bool)> = p
        .runs
        .iter()
        .flat_map(|r| r.text.chars().map(move |c| (c, r.bold, r.italic)))
        .collect();
    let visible: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.0.is_whitespace())
        .map(|(i, _)| i)
        .collect();
    let (Some(&first), Some(&last)) = (visible.first(), visible.last()) else {
        return;
    };
    let chars = &chars[first..=last];
    let text: String = chars.iter().map(|c| c.0).collect();

    // Formatting that covers the whole paragraph is the paragraph's flag. The rest is runs.
    let all_bold = chars.iter().filter(|c| !c.0.is_whitespace()).all(|c| c.1);
    let all_italic = chars.iter().filter(|c| !c.0.is_whitespace()).all(|c| c.2);
    let mut runs: Vec<InlineRun> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let bold = chars[i].1 && !all_bold;
        let italic = chars[i].2 && !all_italic;
        let mut j = i + 1;
        while j < chars.len()
            && (chars[j].1 && !all_bold) == bold
            && (chars[j].2 && !all_italic) == italic
        {
            j += 1;
        }
        if bold || italic {
            runs.push(InlineRun {
                start: i,
                end: j,
                bold,
                italic,
                link: None,
                math: None,
            });
        }
        i = j;
    }

    let style = p.style.as_deref();
    let heading = style.and_then(heading_level);
    if style.is_some_and(is_list_style) || p.numbered {
        out.list_items += 1;
    }
    out.blocks.push(Block::Text {
        content: text,
        style: TextStyle {
            bold: all_bold,
            italic: all_italic,
            heading_level: heading,
            runs,
        },
    });
}
