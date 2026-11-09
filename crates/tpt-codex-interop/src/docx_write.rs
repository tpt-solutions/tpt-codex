// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.docx` export: writes a document as a Word file, on the in-house [`archive`](crate::archive) writer.
//!
//! Text blocks keep their headings, bold and italic, line breaks and tabs, and the title and author
//! go into the document properties. Blocks the format cannot represent exactly are written as the
//! nearest paragraph, and the [`FidelityReport`] records each one.

use tpt_codex_core::{
    Block, BlockId, CellInput, Document, MediaSource, Metadata, Sheet, TextStyle, address_of,
};

use crate::archive::ArchiveWriter;
use crate::docx::DocxError;
use crate::fidelity::FidelityReport;

/// The result of a Word export: the file, and what could not be written as it was.
#[derive(Debug, Clone, PartialEq)]
pub struct DocxExport {
    /// The `.docx` bytes. The same document always gives the same bytes.
    pub bytes: Vec<u8>,
    /// What was written in a closer form, or left out.
    pub report: FidelityReport,
}

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// Containers deeper than this are refused, so a malicious document cannot exhaust the stack.
const MAX_DEPTH: usize = 64;

/// Writes a document as a `.docx` file.
///
/// Text blocks keep their headings, bold and italic, line breaks and tabs. List items lose their
/// markers, code and math become plain paragraphs (math as its TeX source), rules become empty
/// paragraphs. Grids become Word tables of their cells' text; a formula cell is written as its
/// formula, so a caller that wants the values computes them first. Images and queries are not
/// written. The report says which.
pub fn export_docx(doc: &Document) -> Result<DocxExport, DocxError> {
    let mut report = FidelityReport::new();
    let mut body = String::new();
    let mut counts = Counts::default();

    let mut visit = |block: &Block| match block {
        Block::Text { content, style } => {
            counts.links += style.runs.iter().filter(|r| r.link.is_some()).count();
            body.push_str(&paragraph_styled(content, style));
        }
        Block::List { items, .. } => {
            counts.lists += 1;
            list_paragraphs(doc, items, &mut body, &mut counts);
        }
        Block::Quote { .. } => counts.quotes += 1,
        Block::Code { code, .. } => {
            counts.code += 1;
            body.push_str(&paragraph(code, None, false, false));
        }
        Block::Math { source } => {
            counts.math += 1;
            body.push_str(&paragraph(source, None, false, false));
        }
        Block::ThematicBreak => {
            counts.rules += 1;
            body.push_str(&paragraph("", None, false, false));
        }
        Block::Media { source, .. } => {
            if matches!(source, MediaSource::Uri { .. } | MediaSource::Svg { .. }) {
                counts.images += 1;
            }
        }
        Block::Grid { sheet_id, .. } => match doc.sheets.get(sheet_id) {
            Some(sheet) => {
                counts.tables += 1;
                counts.formulas += sheet
                    .cells
                    .values()
                    .filter(|c| matches!(c, CellInput::Formula { .. }))
                    .count();
                body.push_str(&table(sheet));
            }
            None => counts.live += 1,
        },
        Block::Query(_) => counts.live += 1,
        Block::Table { rows } => {
            counts.tables += 1;
            body.push_str(&text_table(rows));
        }
        Block::Container { .. } => {}
    };
    walk(doc, doc.root, &mut visit, 0)?;

    counts.report(&mut report);

    let mut zip = ArchiveWriter::new();
    zip.add("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec())?;
    zip.add("_rels/.rels", ROOT_RELS.as_bytes().to_vec())?;
    zip.add("word/_rels/document.xml.rels", DOC_RELS.as_bytes().to_vec())?;
    zip.add("word/styles.xml", STYLES.as_bytes().to_vec())?;
    zip.add(
        "word/document.xml",
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:document xmlns:w=\"{W}\"><w:body>{body}<w:sectPr/></w:body></w:document>"
        )
        .into_bytes(),
    )?;
    zip.add("docProps/core.xml", core_xml(&doc.metadata)?.into_bytes())?;
    Ok(DocxExport {
        bytes: zip.finish()?,
        report,
    })
}

/// The paragraphs of a list's items, sub-lists included, each one without its marker.
fn list_paragraphs(doc: &Document, items: &[BlockId], body: &mut String, counts: &mut Counts) {
    for item in items {
        match doc.get(*item) {
            Some(Block::Text { content, style }) => {
                counts.links += style.runs.iter().filter(|r| r.link.is_some()).count();
                body.push_str(&paragraph_styled(
                    content,
                    &TextStyle {
                        heading_level: None,
                        ..style.clone()
                    },
                ));
            }
            Some(Block::List { items, .. }) => list_paragraphs(doc, items, body, counts),
            _ => {}
        }
    }
}

#[derive(Default)]
struct Counts {
    quotes: usize,
    lists: usize,
    code: usize,
    math: usize,
    rules: usize,
    images: usize,
    live: usize,
    links: usize,
    tables: usize,
    formulas: usize,
}

impl Counts {
    fn report(&self, report: &mut FidelityReport) {
        if self.quotes > 0 {
            report.approximated(
                "document",
                "block quotes",
                format!("{} block quote(s) written as plain paragraphs", self.quotes),
            );
        }
        if self.formulas > 0 {
            report.approximated(
                "document",
                "formulas",
                format!(
                    "{} formula cell(s) in grids written as their formula text, not their value",
                    self.formulas
                ),
            );
        }
        if self.links > 0 {
            report.approximated(
                "document",
                "links",
                format!(
                    "{} link(s) written as their text; the addresses are not kept",
                    self.links
                ),
            );
        }
        if self.lists > 0 {
            report.approximated(
                "document",
                "lists",
                format!(
                    "{} list(s) written as paragraphs without markers",
                    self.lists
                ),
            );
        }
        if self.code > 0 {
            report.approximated(
                "document",
                "code",
                format!("{} code block(s) written as plain paragraphs", self.code),
            );
        }
        if self.math > 0 {
            report.approximated(
                "document",
                "math",
                format!("{} math block(s) written as their TeX source", self.math),
            );
        }
        if self.rules > 0 {
            report.approximated(
                "document",
                "rules",
                format!("{} rule(s) written as empty paragraphs", self.rules),
            );
        }
        if self.images > 0 {
            report.dropped(
                "document",
                "images",
                format!("{} image(s) not written", self.images),
            );
        }
        if self.live > 0 {
            report.dropped(
                "document",
                "live blocks",
                format!(
                    "{} query block(s) or grid(s) without a sheet not written",
                    self.live
                ),
            );
        }
    }
}

/// Column width in twentieths of a point (about 3.5 cm).
const COLUMN_WIDTH: u32 = 2000;

/// A grid as a Word table: one cell per grid cell, with the cell's text. Empty cells are empty
/// paragraphs. A sheet with no cells is an empty table of one cell, which Word needs.
fn table(sheet: &Sheet) -> String {
    let (rows, cols) = sheet.extent();
    let (rows, cols) = (rows.max(1), cols.max(1));
    word_table(rows, cols, |row, col| {
        sheet
            .cells
            .get(&address_of(col, row))
            .map(CellInput::to_text)
            .unwrap_or_default()
    })
}

/// A Word table of plain text: the rows of a `Block::Table`.
fn text_table(rows: &[Vec<String>]) -> String {
    let cols = rows.first().map_or(1, Vec::len).max(1);
    word_table(rows.len() as u32, cols as u32, |row, col| {
        rows[row as usize - 1]
            .get(col as usize - 1)
            .cloned()
            .unwrap_or_default()
    })
}

/// A Word table with `rows` by `cols` cells, numbered from 1, whose text is `text(row, col)`.
fn word_table(rows: u32, cols: u32, text: impl Fn(u32, u32) -> String) -> String {
    let border = |side: &str| {
        format!("<w:{side} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>")
    };
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders>{}{}{}{}{}{}</w:tblBorders></w:tblPr><w:tblGrid>",
        border("top"),
        border("left"),
        border("bottom"),
        border("right"),
        border("insideH"),
        border("insideV"),
    );
    for _ in 0..cols {
        out.push_str(&format!("<w:gridCol w:w=\"{COLUMN_WIDTH}\"/>"));
    }
    out.push_str("</w:tblGrid>");
    for row in 1..=rows {
        out.push_str("<w:tr>");
        for col in 1..=cols {
            out.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{COLUMN_WIDTH}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
                paragraph(&text(row, col), None, false, false)
            ));
        }
        out.push_str("</w:tr>");
    }
    // A paragraph after the table keeps it from running into the next block.
    out.push_str("</w:tbl><w:p/>");
    out
}

/// Visits the leaf blocks of the tree below `id`, in order. Containers are entered, not visited.
fn walk(
    doc: &Document,
    id: BlockId,
    visit: &mut dyn FnMut(&Block),
    depth: usize,
) -> Result<(), DocxError> {
    if depth > MAX_DEPTH {
        return Err(DocxError("containers are nested too deeply".into()));
    }
    match doc.get(id) {
        Some(Block::Container { children, .. }) => {
            for child in children {
                walk(doc, *child, visit, depth + 1)?;
            }
            Ok(())
        }
        // A quote is visited, so it can be reported, and its children are then visited in turn.
        Some(block @ Block::Quote { children }) => {
            visit(block);
            for child in children {
                walk(doc, *child, visit, depth + 1)?;
            }
            Ok(())
        }
        Some(block) => {
            visit(block);
            Ok(())
        }
        None => Err(DocxError(format!("block {} is missing", id.0))),
    }
}

/// One paragraph. Line breaks and tabs become `w:br` and `w:tab`, each in its own run.
/// A paragraph with each character's marks: the paragraph's own flags, plus its inline runs.
/// Links are written as their text.
fn paragraph_styled(content: &str, style: &TextStyle) -> String {
    let mut out = String::from("<w:p>");
    if let Some(level) = style.heading_level.filter(|l| (1..=6).contains(l)) {
        out.push_str(&format!(
            "<w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>"
        ));
    }
    let marks: Vec<(bool, bool)> = content
        .chars()
        .enumerate()
        .map(|(i, _)| {
            let (mut bold, mut italic) = (style.bold, style.italic);
            for run in style.runs.iter().filter(|r| i >= r.start && i < r.end) {
                bold |= run.bold;
                italic |= run.italic;
            }
            (bold, italic)
        })
        .collect();
    let mut run = String::new();
    let mut current: Option<(bool, bool)> = None;
    for (c, mark) in content.chars().zip(marks) {
        if current != Some(mark) {
            flush_marked(&mut run, current, &mut out);
            current = Some(mark);
        }
        match c {
            '\n' => {
                flush_marked(&mut run, current, &mut out);
                out.push_str(&format!("<w:r>{}<w:br/></w:r>", rpr_for(current)));
            }
            '\t' => {
                flush_marked(&mut run, current, &mut out);
                out.push_str(&format!("<w:r>{}<w:tab/></w:r>", rpr_for(current)));
            }
            c => run.push_str(&escape_char(c)),
        }
    }
    flush_marked(&mut run, current, &mut out);
    out.push_str("</w:p>");
    out
}

fn rpr_for(mark: Option<(bool, bool)>) -> String {
    let (bold, italic) = mark.unwrap_or((false, false));
    let mut props = String::new();
    if bold {
        props.push_str("<w:b/>");
    }
    if italic {
        props.push_str("<w:i/>");
    }
    if props.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{props}</w:rPr>")
    }
}

fn flush_marked(run: &mut String, mark: Option<(bool, bool)>, out: &mut String) {
    if !run.is_empty() {
        out.push_str(&format!(
            "<w:r>{}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
            rpr_for(mark),
            run
        ));
        run.clear();
    }
}

fn paragraph(text: &str, heading: Option<u8>, bold: bool, italic: bool) -> String {
    let mut out = String::from("<w:p>");
    if let Some(level) = heading.filter(|l| (1..=6).contains(l)) {
        out.push_str(&format!(
            "<w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr>"
        ));
    }
    let mut props = String::new();
    if bold {
        props.push_str("<w:b/>");
    }
    if italic {
        props.push_str("<w:i/>");
    }
    let rpr = if props.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{props}</w:rPr>")
    };
    let mut run = String::new();
    for c in text.chars() {
        match c {
            '\n' => {
                flush_run(&mut run, &rpr, &mut out);
                out.push_str(&format!("<w:r>{rpr}<w:br/></w:r>"));
            }
            '\t' => {
                flush_run(&mut run, &rpr, &mut out);
                out.push_str(&format!("<w:r>{rpr}<w:tab/></w:r>"));
            }
            c => run.push_str(&escape_char(c)),
        }
    }
    flush_run(&mut run, &rpr, &mut out);
    out.push_str("</w:p>");
    out
}

fn flush_run(run: &mut String, rpr: &str, out: &mut String) {
    if !run.is_empty() {
        out.push_str(&format!(
            "<w:r>{rpr}<w:t xml:space=\"preserve\">{run}</w:t></w:r>"
        ));
        run.clear();
    }
}

fn escape_char(c: char) -> String {
    match c {
        '&' => "&amp;".into(),
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        '"' => "&quot;".into(),
        '\'' => "&apos;".into(),
        c => c.to_string(),
    }
}

/// Escapes a whole string, refusing characters XML 1.0 forbids.
fn escape_text(text: &str) -> Result<String, DocxError> {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') {
            return Err(DocxError(format!(
                "character U+{:04X} cannot be written to XML",
                c as u32
            )));
        }
        out.push_str(&escape_char(c));
    }
    Ok(out)
}

fn core_xml(meta: &Metadata) -> Result<String, DocxError> {
    let mut props = String::new();
    if let Some(title) = &meta.title {
        props.push_str(&format!("<dc:title>{}</dc:title>", escape_text(title)?));
    }
    if let Some(author) = &meta.author {
        props.push_str(&format!(
            "<dc:creator>{}</dc:creator>",
            escape_text(author)?
        ));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\">{props}</cp:coreProperties>"
    ))
}

const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
</Types>";

const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
</Relationships>";

const DOC_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
</Relationships>";

/// Heading styles, so Word shows `Heading1` to `Heading6` as headings.
const STYLES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"0\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading2\"><w:name w:val=\"heading 2\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"1\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"28\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading3\"><w:name w:val=\"heading 3\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"2\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"26\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading4\"><w:name w:val=\"heading 4\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"3\"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading5\"><w:name w:val=\"heading 5\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"4\"/></w:pPr><w:rPr><w:b/><w:i/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading6\"><w:name w:val=\"heading 6\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"5\"/></w:pPr><w:rPr><w:i/></w:rPr></w:style>\
</w:styles>";
