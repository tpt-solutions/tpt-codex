// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.docx` import: best-effort text and structure, on the in-house [`archive`](crate::archive) reader.
//!
//! What comes through: paragraphs in order, headings (`Heading1` to `Heading6`, and `Title` as level
//! 1), bold and italic runs (kept per run, not per paragraph), line and tab breaks, and the title and author
//! from the document properties. Tables become grids, one cell per table cell, with numbers kept as
//! numbers; a cell's text is never a formula. Merged cells are not kept. Lists come through as
//! lists, bulleted or numbered from `word/numbering.xml`, when the paragraph or its style numbers
//! it; nested levels are kept as items of their parent list. Images are not imported.
//!
//! Everything that does not come through is counted in the [`FidelityReport`], one item per kind.

use std::collections::HashMap;
use std::fmt;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tpt_codex_core::{
    Block, BlockId, CellInput, Document, InlineRun, LayoutMode, Metadata, Sheet, SheetId,
    TextStyle, address_of,
};

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
    let numbering = load_numbering(&archive)?;
    let parsed = parse_body(&body, &numbering)?;

    let mut report = FidelityReport::new();
    if parsed.merged > 0 {
        report.approximated(
            "document",
            "merged cells",
            format!(
                "{} merged cell(s) in {} table(s) kept as separate cells",
                parsed.merged, parsed.tables
            ),
        );
    }
    if parsed.nested_items > 0 {
        report.approximated(
            "document",
            "lists",
            format!(
                "{} nested list item(s) kept as items of their top-level list; the nesting is not kept",
                parsed.nested_items
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

    // Top-level blocks are the root's children. A list's items are blocks of their own, and are
    // not children of the root, as in the Markdown reader.
    let mut document = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: Vec::new(),
        },
    );
    let mut children = Vec::new();
    let mut next = 1u64;
    for top in parsed.blocks {
        match top {
            Top::Block(block) => {
                children.push(BlockId(next));
                document.insert(BlockId(next), block);
                next += 1;
            }
            Top::List {
                ordered,
                start,
                items,
                ..
            } => {
                let list = BlockId(next);
                next += 1;
                let mut ids = Vec::with_capacity(items.len());
                for item in items {
                    ids.push(BlockId(next));
                    document.insert(BlockId(next), item);
                    next += 1;
                }
                children.push(list);
                document.insert(
                    list,
                    Block::List {
                        ordered,
                        start,
                        items: ids,
                    },
                );
            }
        }
    }
    if let Some(Block::Container { children: root, .. }) = document.blocks.get_mut(&BlockId(0)) {
        *root = children;
    }
    for (id, sheet) in parsed.sheets {
        document.sheets.insert(id, sheet);
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

/// A top-level block as it is read: a block, or a list whose items are read in order.
#[derive(Debug)]
enum Top {
    Block(Block),
    List {
        /// The Word numbering instance, so that consecutive items of one list join it.
        num_id: String,
        ordered: bool,
        start: u32,
        items: Vec<Block>,
    },
}

#[derive(Debug, Default)]
struct Parsed {
    blocks: Vec<Top>,
    tables: usize,
    merged: usize,
    /// Items at a level below the top, which are kept in their list without their nesting.
    nested_items: usize,
    images: usize,
    /// The sheets of the tables, each shown by a grid block in `blocks`.
    sheets: Vec<(SheetId, Sheet)>,
    /// The rows of the table being read, while the reader is inside one.
    table: Option<Vec<Vec<String>>>,
}

/// The paragraph being read, and the run inside it.
#[derive(Debug, Default)]
struct Paragraph {
    style: Option<String>,
    /// The numbering instance the paragraph names itself, if any (`w:numPr/w:numId`).
    num_id: Option<String>,
    /// The level in that instance (`w:numPr/w:ilvl`), `0` for the top.
    level: u32,
    runs: Vec<Run>,
}

/// What `word/numbering.xml` and `word/styles.xml` say about lists.
#[derive(Debug, Default)]
struct Numbering {
    /// Numbering instance (`numId`) to its abstract definition (`abstractNumId`).
    instances: HashMap<String, String>,
    /// Abstract definition to its top level: `(ordered, start)`. Bullets and `none` are unordered.
    levels: HashMap<String, (bool, u32)>,
    /// Paragraph style id to the numbering instance the style gives its paragraphs.
    styles: HashMap<String, String>,
}

impl Numbering {
    /// `(ordered, start)` of a numbering instance, or `None` when the document does not define it.
    fn list_of(&self, num_id: &str) -> Option<(bool, u32)> {
        let abstract_id = self.instances.get(num_id)?;
        self.levels.get(abstract_id).copied()
    }
}

/// The value of an attribute by its local name (the part after any prefix).
fn attr(e: &BytesStart<'_>, name: &str) -> Result<Option<String>, DocxError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.local_name().as_ref() == name.as_bytes() {
            let value = a.unescape_value().map_err(xml_err)?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// The list definitions. A part that is missing counts as having none; one that cannot be read is
/// an error.
fn load_numbering(archive: &Archive<'_>) -> Result<Numbering, DocxError> {
    let mut numbering = match read_part(archive, "word/numbering.xml") {
        Ok(xml) => parse_numbering(&xml)?,
        Err(_) => Numbering::default(),
    };
    if let Ok(xml) = read_part(archive, "word/styles.xml") {
        numbering.styles = parse_styles(&xml)?;
    }
    Ok(numbering)
}

fn parse_numbering(xml: &str) -> Result<Numbering, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut out = Numbering::default();
    // The numbering instance and abstract definition being read, and the top level inside it.
    let mut num: Option<String> = None;
    let mut abstract_id: Option<String> = None;
    let mut top = false;
    let mut fmt: Option<String> = None;
    let mut start = 1u32;
    loop {
        let (e, opened) = match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(e) => {
                match e.local_name().as_ref() {
                    b"num" => num = None,
                    b"abstractNum" => abstract_id = None,
                    b"lvl" if top => {
                        if let Some(id) = &abstract_id {
                            let ordered =
                                fmt.as_deref().is_some_and(|f| f != "bullet" && f != "none");
                            out.levels.insert(id.clone(), (ordered, start));
                        }
                        top = false;
                    }
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.local_name().as_ref() {
            b"num" => {
                num = if opened { attr(&e, "numId")? } else { None };
            }
            b"abstractNumId" => {
                if let (Some(n), Some(value)) = (&num, attr(&e, "val")?) {
                    out.instances.insert(n.clone(), value);
                }
            }
            b"abstractNum" if opened => abstract_id = attr(&e, "abstractNumId")?,
            b"lvl" if opened => {
                top = attr(&e, "ilvl")?.as_deref() == Some("0");
                fmt = None;
                start = 1;
            }
            b"numFmt" if top => fmt = attr(&e, "val")?,
            b"start" if top => {
                if let Some(v) = attr(&e, "val")?.and_then(|v| v.parse().ok()) {
                    start = v;
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The numbering instance each paragraph style gives its paragraphs.
fn parse_styles(xml: &str) -> Result<HashMap<String, String>, DocxError> {
    let mut reader = Reader::from_str(xml);
    let mut out = HashMap::new();
    let mut style: Option<String> = None;
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) if e.local_name().as_ref() == b"style" => {
                style = attr(&e, "styleId")?;
            }
            Event::End(e) if e.local_name().as_ref() == b"style" => style = None,
            Event::Empty(e) if e.local_name().as_ref() == b"numId" => {
                if let (Some(id), Some(value)) = (&style, attr(&e, "val")?) {
                    out.insert(id.clone(), value);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
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

fn parse_body(xml: &str, numbering: &Numbering) -> Result<Parsed, DocxError> {
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
                        out.table = Some(Vec::new());
                    }
                    table_depth += 1;
                }
                // Only the rows and cells of the outermost table are kept; a nested table's text
                // lands in the cell that holds it.
                b"tr" if table_depth == 1 => {
                    if let Some(rows) = out.table.as_mut() {
                        rows.push(Vec::new());
                    }
                }
                b"tc" if table_depth == 1 => {
                    if let Some(row) = out.table.as_mut().and_then(|rows| rows.last_mut()) {
                        row.push(String::new());
                    }
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
                b"numId" => {
                    if let Some(p) = para.as_mut() {
                        p.num_id = style_val(&e)?;
                    }
                }
                b"ilvl" => {
                    if let Some(p) = para.as_mut() {
                        p.level = style_val(&e)?.and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                b"b" | b"i" => apply_run_flag(&e, run.as_mut())?,
                b"gridSpan" if table_depth == 1 && style_val(&e)?.as_deref() != Some("1") => {
                    out.merged += 1;
                }
                b"vMerge" if table_depth == 1 => out.merged += 1,
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
                        finish_paragraph(p, numbering, &mut out);
                    }
                }
                b"tbl" => {
                    table_depth = table_depth.saturating_sub(1);
                    if table_depth == 0 {
                        if let Some(rows) = out.table.take() {
                            add_table(rows, &mut out);
                        }
                    }
                }
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

fn finish_paragraph(p: Paragraph, numbering: &Numbering, out: &mut Parsed) {
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
        // Spaces at the edge of a run carry no emphasis that a reader can see, and Markdown
        // cannot write emphasis that starts or ends with one, so they are left out of the run.
        let mut start = i;
        let mut end = j;
        while start < end && chars[start].0.is_whitespace() {
            start += 1;
        }
        while end > start && chars[end - 1].0.is_whitespace() {
            end -= 1;
        }
        if (bold || italic) && start < end {
            runs.push(InlineRun {
                start,
                end,
                bold,
                italic,
                link: None,
                math: None,
                link_title: None,
            });
        }
        i = j;
    }

    let style = p.style.as_deref();
    let heading = style.and_then(heading_level);
    // A paragraph is in a list when its numbering names one (directly, or through its style). A
    // numbering of `0` removes the numbering, and an instance the document does not define is text.
    let num_id = p
        .num_id
        .clone()
        .or_else(|| style.and_then(|s| numbering.styles.get(s).cloned()))
        .filter(|id| id != "0");
    let list = num_id.as_deref().and_then(|id| {
        numbering
            .list_of(id)
            .map(|(ordered, start)| (id, ordered, start))
    });
    // Inside a table, a paragraph is a line of its cell. The cell's text is set when the table ends.
    if let Some(rows) = out.table.as_mut() {
        if let Some(cell) = rows.last_mut().and_then(|row| row.last_mut()) {
            if !cell.is_empty() {
                cell.push('\n');
            }
            cell.push_str(&text);
        }
        return;
    }
    let block = Block::Text {
        content: text,
        style: TextStyle {
            bold: all_bold,
            italic: all_italic,
            heading_level: heading,
            runs,
        },
    };
    match list {
        Some((id, ordered, start)) if heading.is_none() => {
            if p.level > 0 {
                out.nested_items += 1;
            }
            match out.blocks.last_mut() {
                Some(Top::List { num_id, items, .. }) if num_id == id => items.push(block),
                _ => out.blocks.push(Top::List {
                    num_id: id.to_string(),
                    ordered,
                    start,
                    items: vec![block],
                }),
            }
        }
        _ => out.blocks.push(Top::Block(block)),
    }
}

/// Adds a table as a grid block. Each cell's text is read as a number or a boolean when it is one,
/// and as text otherwise, so a cell that starts with `=` is never a formula. A table with no cells
/// adds nothing.
fn add_table(rows: Vec<Vec<String>>, out: &mut Parsed) {
    let height = rows.len() as u32;
    let width = rows.iter().map(Vec::len).max().unwrap_or(0) as u32;
    if height == 0 || width == 0 {
        return;
    }
    let mut sheet = Sheet::with_size(height, width);
    for (r, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let input = match CellInput::parse(text) {
                Ok(Some(CellInput::Formula { .. })) | Ok(None) | Err(_) => CellInput::Text {
                    value: text.to_string(),
                },
                Ok(Some(input)) => input,
            };
            sheet
                .cells
                .insert(address_of(c as u32 + 1, r as u32 + 1), input);
        }
    }
    let id = SheetId(out.sheets.len() as u64 + 1);
    out.sheets.push((id, sheet));
    out.blocks.push(Top::Block(Block::Grid {
        sheet_id: id,
        view_bounds: None,
    }));
}
