// SPDX-License-Identifier: MIT OR Apache-2.0
//! `.xlsx` import and export for live grids, on the in-house [`archive`](crate::archive) reader.
//!
//! Import reads the first worksheet into a [`Sheet`]. Anything the model cannot hold is recorded in
//! the [`FidelityReport`]: other worksheets, merged cells, array and shared formulas, error values,
//! named ranges. Cell formatting is not read, so dates and percentages come in as their stored
//! numbers, and the report says so.
//!
//! Export writes one worksheet. Formulas carry their computed value, so the file shows numbers
//! before a spreadsheet recalculates. Text is written inline, so there is no shared string table.

use std::collections::BTreeMap;
use std::fmt;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tpt_codex_core::{CellInput, Sheet, is_valid_address};
use tpt_codex_grid::{CellResult, evaluate_sheet};

use crate::archive::{Archive, ArchiveWriter, ZipError};
use crate::fidelity::FidelityReport;

/// A workbook that could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XlsxError(pub String);

impl fmt::Display for XlsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xlsx: {}", self.0)
    }
}

impl std::error::Error for XlsxError {}

impl From<ZipError> for XlsxError {
    fn from(e: ZipError) -> Self {
        XlsxError(e.to_string())
    }
}

/// The result of an import: the sheet and what was lost on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct XlsxImport {
    /// The first worksheet.
    pub sheet: Sheet,
    /// What the import kept, approximated or dropped.
    pub report: FidelityReport,
}

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Imports the first worksheet of an `.xlsx` file.
pub fn import_xlsx(bytes: &[u8]) -> Result<XlsxImport, XlsxError> {
    let archive = Archive::open(bytes)?;
    let mut report = FidelityReport::new();

    let workbook = parse_workbook(&read_text(&archive, "xl/workbook.xml")?)?;
    if workbook.sheets.is_empty() {
        return Err(XlsxError("the workbook has no worksheets".into()));
    }
    if workbook.defined_names > 0 {
        report.dropped(
            "workbook",
            "named ranges",
            format!("{} named range(s) not imported", workbook.defined_names),
        );
    }
    let rels = parse_rels(&read_text(&archive, "xl/_rels/workbook.xml.rels")?)?;
    let shared = if archive.names().any(|n| n == "xl/sharedStrings.xml") {
        parse_shared_strings(&read_text(&archive, "xl/sharedStrings.xml")?)?
    } else {
        Vec::new()
    };

    let (first_name, first_rid) = &workbook.sheets[0];
    for (other, _) in workbook.sheets.iter().skip(1) {
        report.dropped(
            format!("sheet \"{other}\""),
            "worksheet",
            "only the first worksheet is imported",
        );
    }
    let target = rels
        .get(first_rid)
        .ok_or_else(|| XlsxError(format!("no relationship for sheet '{first_name}'")))?;
    let sheet_xml = read_text(&archive, &resolve_target(target))?;
    let parsed = parse_sheet(&sheet_xml, &shared)?;

    let location = |address: &str| format!("sheet \"{first_name}\"!{address}");
    if parsed.merged > 0 {
        report.approximated(
            format!("sheet \"{first_name}\""),
            "merged cells",
            format!(
                "{} merged region(s) not represented; each keeps its top-left value only",
                parsed.merged
            ),
        );
    }

    let mut sheet = Sheet::default();
    for cell in parsed.cells {
        match cell.kind {
            CellKind::Input(input) => {
                sheet.cells.insert(cell.address, input);
            }
            CellKind::Approximated(input, note) => {
                report.approximated(location(&cell.address), "formula", note);
                sheet.cells.insert(cell.address, input);
            }
            CellKind::Dropped(note) => {
                report.dropped(location(&cell.address), "cell", note);
            }
            CellKind::Empty => {}
        }
    }
    Ok(XlsxImport { sheet, report })
}

/// Writes a sheet as a one-worksheet `.xlsx` file.
///
/// Formulas are written with their computed value when it is a number, text or boolean.
pub fn export_xlsx(sheet: &Sheet) -> Result<Vec<u8>, XlsxError> {
    let results = evaluate_sheet(sheet);
    let mut cells: Vec<(usize, usize, &String, &CellInput)> = Vec::new();
    for (address, input) in &sheet.cells {
        let (col, row) = split_address(address)
            .ok_or_else(|| XlsxError(format!("'{address}' is not a valid cell address")))?;
        cells.push((row, col, address, input));
    }
    cells.sort_by_key(|&(row, col, _, _)| (row, col));

    let mut data = String::new();
    let mut current_row = 0usize;
    for (row, _, address, input) in &cells {
        if *row != current_row {
            if current_row != 0 {
                data.push_str("</row>");
            }
            data.push_str(&format!("<row r=\"{row}\">"));
            current_row = *row;
        }
        data.push_str(&cell_xml(address, input, results.get(*address))?);
    }
    if current_row != 0 {
        data.push_str("</row>");
    }
    let sheet_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <worksheet xmlns=\"{NS_MAIN}\"><sheetData>{data}</sheetData></worksheet>"
    );

    let mut zip = ArchiveWriter::new();
    zip.add("[Content_Types].xml", content_types().into_bytes())?;
    zip.add("_rels/.rels", root_rels().into_bytes())?;
    zip.add("xl/workbook.xml", workbook_xml().into_bytes())?;
    zip.add("xl/_rels/workbook.xml.rels", workbook_rels().into_bytes())?;
    zip.add("xl/worksheets/sheet1.xml", sheet_xml.into_bytes())?;
    Ok(zip.finish()?)
}

/// One cell as written to the sheet XML.
fn cell_xml(
    address: &str,
    input: &CellInput,
    result: Option<&CellResult>,
) -> Result<String, XlsxError> {
    Ok(match input {
        CellInput::Number { value } => {
            if !value.is_finite() {
                return Err(XlsxError(format!("'{address}' is not a finite number")));
            }
            format!("<c r=\"{address}\"><v>{value}</v></c>")
        }
        CellInput::Text { value } => format!(
            "<c r=\"{address}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            escape(value)?
        ),
        CellInput::Boolean { value } => {
            format!("<c r=\"{address}\" t=\"b\"><v>{}</v></c>", u8::from(*value))
        }
        CellInput::Formula { source } => {
            let formula = source.strip_prefix('=').unwrap_or(source);
            if formula.is_empty() {
                return Err(XlsxError(format!("'{address}' has an empty formula")));
            }
            let formula = escape(formula)?;
            match result {
                Some(CellResult::Number(n)) if n.is_finite() => {
                    format!("<c r=\"{address}\"><f>{formula}</f><v>{n}</v></c>")
                }
                Some(CellResult::Boolean(b)) => format!(
                    "<c r=\"{address}\" t=\"b\"><f>{formula}</f><v>{}</v></c>",
                    u8::from(*b)
                ),
                Some(CellResult::Text(t)) => format!(
                    "<c r=\"{address}\" t=\"str\"><f>{formula}</f><v>{}</v></c>",
                    escape(t)?
                ),
                _ => format!("<c r=\"{address}\"><f>{formula}</f></c>"),
            }
        }
    })
}

fn content_types() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
     <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
     <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
     <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
     <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
     <Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
     </Types>"
        .to_string()
}

fn root_rels() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{NS_REL}/officeDocument\" Target=\"xl/workbook.xml\"/>\
         </Relationships>"
    )
}

fn workbook_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">\
         <sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>"
    )
}

fn workbook_rels() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId1\" Type=\"{NS_REL}/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
         </Relationships>"
    )
}

/// Escapes text for XML content and attributes. Characters XML 1.0 forbids are an error.
fn escape(text: &str) -> Result<String, XlsxError> {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {
                return Err(XlsxError(format!(
                    "character U+{:04X} cannot be written to XML",
                    c as u32
                )));
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// `(column, row)`, both 1-based, from a canonical A1 address.
fn split_address(address: &str) -> Option<(usize, usize)> {
    if !is_valid_address(address) {
        return None;
    }
    let letters = address.bytes().take_while(u8::is_ascii_uppercase).count();
    let col = address[..letters]
        .bytes()
        .fold(0usize, |acc, b| acc * 26 + usize::from(b - b'A' + 1));
    Some((col, address[letters..].parse().ok()?))
}

fn read_text(archive: &Archive<'_>, name: &str) -> Result<String, XlsxError> {
    let bytes = archive.read(name)?;
    String::from_utf8(bytes).map_err(|_| XlsxError(format!("'{name}' is not UTF-8")))
}

/// Part names are relative to `xl/` in the relationships file, and may start with `/`.
fn resolve_target(target: &str) -> String {
    match target.strip_prefix('/') {
        Some(absolute) => absolute.to_string(),
        None => format!("xl/{target}"),
    }
}

fn attr(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<String>, XlsxError> {
    for a in e.attributes() {
        let a = a.map_err(xml_err)?;
        if a.key.as_ref() == key {
            let value = a.unescape_value().map_err(xml_err)?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

fn xml_err(e: impl fmt::Display) -> XlsxError {
    XlsxError(format!("malformed XML: {e}"))
}

struct Workbook {
    /// `(sheet name, relationship id)` in workbook order.
    sheets: Vec<(String, String)>,
    defined_names: usize,
}

fn parse_workbook(xml: &str) -> Result<Workbook, XlsxError> {
    let mut reader = Reader::from_str(xml);
    let mut sheets = Vec::new();
    let mut defined_names = 0;
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                b"sheet" => {
                    let name = attr(&e, b"name")?.unwrap_or_default();
                    let rid = attr(&e, b"r:id")?
                        .ok_or_else(|| XlsxError(format!("sheet '{name}' has no relationship")))?;
                    sheets.push((name, rid));
                }
                b"definedName" => defined_names += 1,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(Workbook {
        sheets,
        defined_names,
    })
}

fn parse_rels(xml: &str) -> Result<BTreeMap<String, String>, XlsxError> {
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

fn parse_shared_strings(xml: &str) -> Result<Vec<String>, XlsxError> {
    let mut reader = Reader::from_str(xml);
    let mut strings = Vec::new();
    let mut current: Option<String> = None;
    let mut in_text = false;
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"si" => current = Some(String::new()),
                b"t" => in_text = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                b"si" => strings.push(current.take().unwrap_or_default()),
                b"t" => in_text = false,
                _ => {}
            },
            Event::Text(t) if in_text => {
                let text = t.unescape().map_err(xml_err)?;
                if let Some(cur) = current.as_mut() {
                    cur.push_str(&text);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(strings)
}

#[derive(Debug)]
enum CellKind {
    Input(CellInput),
    /// A styled cell with no value. Nothing to keep, and nothing lost.
    Empty,
    /// Kept, but with a different meaning than in the file. The note says what changed.
    Approximated(CellInput, String),
    Dropped(String),
}

#[derive(Debug)]
struct ParsedCell {
    address: String,
    kind: CellKind,
}

struct ParsedSheet {
    cells: Vec<ParsedCell>,
    merged: usize,
}

/// One `<c>` element while it is being read.
#[derive(Default)]
struct RawCell {
    address: String,
    cell_type: Option<String>,
    value: Option<String>,
    inline_text: String,
    formula: Option<String>,
    formula_kind: Option<String>,
}

fn parse_sheet(xml: &str, shared: &[String]) -> Result<ParsedSheet, XlsxError> {
    let mut reader = Reader::from_str(xml);
    let mut cells = Vec::new();
    let mut merged = 0usize;
    let mut cur: Option<RawCell> = None;
    // Which element's text is being collected: `v`, `f` or `t` (inline string).
    let mut collecting: Option<&'static str> = None;

    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"c" => {
                    cur = Some(RawCell {
                        address: attr(&e, b"r")?.unwrap_or_default(),
                        cell_type: attr(&e, b"t")?,
                        ..RawCell::default()
                    });
                }
                b"v" => collecting = Some("v"),
                b"f" => {
                    if let Some(c) = cur.as_mut() {
                        c.formula = Some(String::new());
                        c.formula_kind = attr(&e, b"t")?;
                    }
                    collecting = Some("f");
                }
                b"t" => collecting = Some("t"),
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"c" => {
                    let raw = RawCell {
                        address: attr(&e, b"r")?.unwrap_or_default(),
                        cell_type: attr(&e, b"t")?,
                        ..RawCell::default()
                    };
                    cells.push(finish_cell(raw, shared));
                }
                b"f" => {
                    if let Some(c) = cur.as_mut() {
                        c.formula = Some(String::new());
                        c.formula_kind = attr(&e, b"t")?;
                    }
                }
                b"mergeCell" => merged += 1,
                _ => {}
            },
            Event::Text(t) => {
                let text = t.unescape().map_err(xml_err)?;
                if let (Some(c), Some(which)) = (cur.as_mut(), collecting) {
                    match which {
                        "v" => c.value.get_or_insert_with(String::new).push_str(&text),
                        "f" => {
                            if let Some(f) = c.formula.as_mut() {
                                f.push_str(&text);
                            }
                        }
                        _ => c.inline_text.push_str(&text),
                    }
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"c" => {
                    if let Some(raw) = cur.take() {
                        cells.push(finish_cell(raw, shared));
                    }
                }
                b"v" | b"f" | b"t" => collecting = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(ParsedSheet { cells, merged })
}

fn finish_cell(raw: RawCell, shared: &[String]) -> ParsedCell {
    let address = raw.address.clone();
    let kind = classify(&raw, shared);
    ParsedCell { address, kind }
}

fn classify(raw: &RawCell, shared: &[String]) -> CellKind {
    if !is_valid_address(&raw.address) {
        return CellKind::Dropped(format!("cell address '{}' is not supported", raw.address));
    }
    let value = raw.value.as_deref().unwrap_or("");

    if let Some(formula) = &raw.formula {
        match raw.formula_kind.as_deref() {
            None | Some("normal") if !formula.is_empty() => {
                return CellKind::Input(CellInput::Formula {
                    source: format!("={formula}"),
                });
            }
            _ => {
                // A shared or array formula. Only its computed value survives.
                let note = format!(
                    "{} formula kept as its computed value",
                    raw.formula_kind.as_deref().unwrap_or("shared")
                );
                return match constant(raw, value, shared) {
                    Some(input) => CellKind::Approximated(input, note),
                    None => CellKind::Dropped(note),
                };
            }
        }
    }
    match constant(raw, value, shared) {
        Some(input) => CellKind::Input(input),
        None => match raw.cell_type.as_deref() {
            Some("e") => CellKind::Dropped(format!("error value {value}")),
            _ if value.is_empty() && raw.inline_text.is_empty() => CellKind::Empty,
            _ => CellKind::Dropped("value could not be read".into()),
        },
    }
}

/// The plain value of a cell that has no formula, when it can be read.
fn constant(raw: &RawCell, value: &str, shared: &[String]) -> Option<CellInput> {
    match raw.cell_type.as_deref() {
        Some("s") => value
            .parse::<usize>()
            .ok()
            .and_then(|i| shared.get(i))
            .map(|s| CellInput::Text { value: s.clone() }),
        Some("str") => Some(CellInput::Text {
            value: value.to_string(),
        }),
        Some("inlineStr") => Some(CellInput::Text {
            value: raw.inline_text.clone(),
        }),
        Some("b") => match value {
            "1" => Some(CellInput::Boolean { value: true }),
            "0" => Some(CellInput::Boolean { value: false }),
            _ => None,
        },
        Some("e") => None,
        None | Some("n") => {
            if value.is_empty() {
                return None;
            }
            value
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .map(|n| CellInput::Number { value: n })
        }
        Some(_) => None,
    }
}
