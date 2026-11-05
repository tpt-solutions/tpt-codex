// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.docx` export: writes a document as a Word file, on the in-house [`archive`](crate::archive) writer.
//!
//! Text blocks keep their headings, bold and italic, line breaks and tabs, and the title and author
//! go into the document properties. Blocks the format cannot represent exactly are written as the
//! nearest paragraph, and the [`FidelityReport`] records each one.

use tpt_codex_core::{Block, BlockId, Document, MediaSource, Metadata};

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
/// paragraphs. Images, grids and queries are not written. The report says which.
pub fn export_docx(doc: &Document) -> Result<DocxExport, DocxError> {
    let mut report = FidelityReport::new();
    let mut body = String::new();
    let mut counts = Counts::default();

    let mut visit = |block: &Block| match block {
        Block::Text { content, style } => {
            body.push_str(&paragraph(
                content,
                style.heading_level,
                style.bold,
                style.italic,
            ));
        }
        Block::List { items, .. } => {
            counts.lists += 1;
            for item in items {
                if let Some(Block::Text { content, style }) = doc.get(*item) {
                    body.push_str(&paragraph(content, None, style.bold, style.italic));
                }
            }
        }
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
        Block::Grid { .. } | Block::Query(_) => counts.live += 1,
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

#[derive(Default)]
struct Counts {
    lists: usize,
    code: usize,
    math: usize,
    rules: usize,
    images: usize,
    live: usize,
}

impl Counts {
    fn report(&self, report: &mut FidelityReport) {
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
                format!("{} grid or query block(s) not written", self.live),
            );
        }
    }
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
        Some(block) => {
            visit(block);
            Ok(())
        }
        None => Err(DocxError(format!("block {} is missing", id.0))),
    }
}

/// One paragraph. Line breaks and tabs become `w:br` and `w:tab`, each in its own run.
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
