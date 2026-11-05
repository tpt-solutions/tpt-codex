// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.pptx` import: the text of each slide, on the in-house [`archive`](crate::archive) reader.
//!
//! Data only, as the spec asks: no layout, animation or theme. Each slide becomes a level-2 heading
//! (its title placeholder, or `Slide N` when it has none), followed by the slide's text paragraphs in
//! the order they appear. Speaker notes, pictures, tables and text formatting are recorded in the
//! [`FidelityReport`].

use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};

use crate::archive::Archive;
use crate::docx::{DocxError, read_metadata, read_part, xml_err};
use crate::fidelity::FidelityReport;

/// The result of a presentation import: the document and what was lost.
#[derive(Debug, Clone, PartialEq)]
pub struct PptxImport {
    /// One heading per slide, each followed by its text.
    pub document: Document,
    /// What was left out or flattened.
    pub report: FidelityReport,
}

/// Imports the text of a `.pptx` file.
pub fn import_pptx(bytes: &[u8]) -> Result<PptxImport, DocxError> {
    let archive = Archive::open(bytes)?;
    let presentation = read_part(&archive, "ppt/presentation.xml")?;
    let rels = parse_rels(&read_part(&archive, "ppt/_rels/presentation.xml.rels")?)?;
    let slide_ids = parse_slide_ids(&presentation)?;

    let mut report = FidelityReport::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut totals = Totals::default();

    for (number, rid) in slide_ids.iter().enumerate() {
        let target = rels
            .get(rid)
            .ok_or_else(|| DocxError(format!("slide relationship '{rid}' is missing")))?;
        let part = format!("ppt/{target}");
        let slide = parse_slide(&read_part(&archive, &part)?)?;
        totals.add(&slide);

        let title = if slide.title.is_empty() {
            format!("Slide {}", number + 1)
        } else {
            slide.title.clone()
        };
        blocks.push(Block::Text {
            content: title,
            style: TextStyle {
                bold: false,
                italic: false,
                heading_level: Some(2),
            },
        });
        for paragraph in slide.body {
            blocks.push(Block::Text {
                content: paragraph,
                style: TextStyle::default(),
            });
        }
    }
    totals.notes += count_notes(&archive)?;

    if !slide_ids.is_empty() {
        report.approximated(
            "presentation",
            "slides",
            format!(
                "{} slide(s) imported as text; layout, positions and theme are not kept",
                slide_ids.len()
            ),
        );
    }
    if totals.tables > 0 {
        report.approximated(
            "presentation",
            "tables",
            format!("{} table(s) flattened to paragraphs", totals.tables),
        );
    }
    if totals.formatted > 0 {
        report.approximated(
            "presentation",
            "text formatting",
            format!(
                "{} paragraph(s) had bold or italic runs; the formatting is dropped",
                totals.formatted
            ),
        );
    }
    if totals.pictures > 0 {
        report.dropped(
            "presentation",
            "pictures",
            format!("{} picture(s) not imported", totals.pictures),
        );
    }
    if totals.notes > 0 {
        report.dropped(
            "presentation",
            "speaker notes",
            format!("{} notes page(s) with text not imported", totals.notes),
        );
    }

    let mut document = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=blocks.len() as u64).map(BlockId).collect(),
        },
    );
    for (i, block) in blocks.into_iter().enumerate() {
        document.insert(BlockId(i as u64 + 1), block);
    }
    document.metadata = read_metadata(&archive);
    Ok(PptxImport { document, report })
}

#[derive(Default)]
struct Totals {
    tables: usize,
    pictures: usize,
    formatted: usize,
    notes: usize,
}

impl Totals {
    fn add(&mut self, slide: &Slide) {
        self.tables += slide.tables;
        self.pictures += slide.pictures;
        self.formatted += slide.formatted;
    }
}

#[derive(Default)]
struct Slide {
    title: String,
    body: Vec<String>,
    tables: usize,
    pictures: usize,
    formatted: usize,
}

fn attr(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>, DocxError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.as_ref() == key {
            return Ok(Some(a.unescape_value().map_err(xml_err)?.into_owned()));
        }
    }
    Ok(None)
}

/// `(relationship id, target)` for every relationship.
fn parse_rels(xml: &str) -> Result<BTreeMap<String, String>, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut rels = BTreeMap::new();
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, b"Id")?, attr(&e, b"Target")?) {
                    rels.insert(id, target);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rels)
}

/// The relationship ids of the slides, in presentation order.
fn parse_slide_ids(xml: &str) -> Result<Vec<String>, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut ids = Vec::new();
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"sldId" => {
                let rid = attr(&e, b"r:id")?
                    .ok_or_else(|| DocxError("slide without a relationship id".into()))?;
                ids.push(rid);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(ids)
}

/// Reads one slide: its title placeholder, its text paragraphs, and what it contains that is not text.
fn parse_slide(xml: &str) -> Result<Slide, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut slide = Slide::default();
    let mut para: Option<String> = None;
    let mut para_formatted = false;
    let mut in_text = false;
    let mut title_shape = false;
    let mut title_parts: Vec<String> = Vec::new();

    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"sp" => title_shape = false,
                b"ph" => title_shape |= is_title(&e)?,
                b"p" => para = Some(String::new()),
                b"rPr" => {
                    if attr(&e, b"b")?.as_deref() == Some("1")
                        || attr(&e, b"i")?.as_deref() == Some("1")
                    {
                        para_formatted = true;
                    }
                }
                b"t" => in_text = true,
                b"tbl" => slide.tables += 1,
                b"pic" => slide.pictures += 1,
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"ph" => title_shape |= is_title(&e)?,
                b"br" => {
                    if let Some(p) = para.as_mut() {
                        p.push('\n');
                    }
                }
                b"rPr" => {
                    if attr(&e, b"b")?.as_deref() == Some("1")
                        || attr(&e, b"i")?.as_deref() == Some("1")
                    {
                        para_formatted = true;
                    }
                }
                b"pic" => slide.pictures += 1,
                _ => {}
            },
            Event::Text(t) if in_text => {
                let text = t.unescape().map_err(xml_err)?;
                if let Some(p) = para.as_mut() {
                    p.push_str(&text);
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    if let Some(text) = para.take() {
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            if title_shape {
                                title_parts.push(text);
                            } else {
                                slide.body.push(text);
                            }
                        }
                    }
                    if std::mem::take(&mut para_formatted) {
                        slide.formatted += 1;
                    }
                }
                b"sp" => title_shape = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    slide.title = title_parts.join(" ");
    Ok(slide)
}

/// A placeholder that holds the slide's title.
fn is_title(e: &BytesStart<'_>) -> Result<bool, DocxError> {
    Ok(matches!(
        attr(e, b"type")?.as_deref(),
        Some("title" | "ctrTitle")
    ))
}

/// Counts notes pages that contain text.
fn count_notes(archive: &Archive<'_>) -> Result<usize, DocxError> {
    let mut count = 0;
    for name in archive.names().map(str::to_string).collect::<Vec<_>>() {
        if name.starts_with("ppt/notesSlides/") && name.ends_with(".xml") {
            let xml = read_part(archive, &name)?;
            let text = parse_slide(&xml)?;
            if !text.body.is_empty() || !text.title.is_empty() {
                count += 1;
            }
        }
    }
    Ok(count)
}
