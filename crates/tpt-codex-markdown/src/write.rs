// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use tpt_codex_core::{
    Block, BlockId, Canvas, CellInput, Document, Figure, InlineRun, LayoutMode, MediaSource,
    Metadata, SheetId, TextStyle, address_of, address_position, is_equation_label, is_figure_label,
    is_table_label,
};

use crate::error::SerializeError;
use crate::inline::write_inline;
use crate::syntax::{
    MAX_LIST_NUMBER, canvas_size, figure_attributes, image_line, is_separator, list_item,
    math_closes, starts_block,
};

/// Writes a [`Document`] as Markdown in the supported subset.
///
/// The root must be a flow container. Its children may be headings, paragraphs,
/// lists, fenced code blocks, thematic breaks, images with a URI and display math. Anything
/// else returns a [`SerializeError`]. Block ids are not written, so ids in the
/// document are not preserved.
pub fn to_markdown(doc: &Document) -> Result<String, SerializeError> {
    let root = match doc.get(doc.root) {
        Some(Block::Container {
            layout_mode: LayoutMode::Flow,
            children,
        }) => children,
        _ => {
            return Err(SerializeError {
                index: None,
                reason: String::from("root must be a flow container"),
            });
        }
    };

    let mut out = String::new();
    write_front_matter(&doc.metadata, &mut out)?;

    check_layout(doc, root)?;

    let mut parts: Vec<String> = Vec::new();
    let mut prev: Option<&Block> = None;
    for (index, id) in root.iter().enumerate() {
        let block = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("child block {} is missing", id.0),
        })?;
        match block {
            Block::Container {
                layout_mode: LayoutMode::Absolute,
                children,
            } => parts.push(write_canvas(doc, index, *id, children)?),
            _ => {
                let label = doc.footnotes.get(id).map(String::as_str);
                parts.push(write_block(doc, index, *id, block, prev, label)?);
            }
        }
        prev = Some(block);
    }
    out.push_str(&parts.join("\n\n"));
    if !parts.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

fn write_front_matter(meta: &Metadata, out: &mut String) -> Result<(), SerializeError> {
    let entries = [
        ("title", &meta.title),
        ("author", &meta.author),
        ("date", &meta.date),
        ("class", &meta.class),
    ];
    if entries.iter().all(|(_, v)| v.is_none()) {
        return Ok(());
    }
    out.push_str("---\n");
    for (key, value) in entries {
        if let Some(value) = value {
            if value.contains('\n') || value.contains('\r') {
                return Err(SerializeError {
                    index: None,
                    reason: format!("front matter '{key}' contains a line break"),
                });
            }
            out.push_str(key);
            out.push_str(": \"");
            for c in value.chars() {
                if c == '"' || c == '\\' {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push_str("\"\n");
        }
    }
    out.push_str("---\n\n");
    Ok(())
}

/// Checks what the reader cannot express, before anything is written.
fn check_layout(doc: &Document, root: &[BlockId]) -> Result<(), SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: None,
        reason: String::from(reason),
    };
    if !doc.sections.is_empty() {
        return Err(fail("slide sections are not in the Markdown subset yet"));
    }
    if !doc.frames.is_empty() {
        return Err(fail(
            "slide frames (where blocks sit on a slide) are not in the Markdown subset yet",
        ));
    }
    // Every block that is written: the root's children and the children of its slides.
    let mut placed: Vec<BlockId> = root.to_vec();
    for id in root {
        if let Some(Block::Container {
            layout_mode: LayoutMode::Absolute,
            children,
        }) = doc.get(*id)
        {
            placed.extend(children.iter().copied());
        }
    }
    for &id in doc.footnotes.keys() {
        if !placed.contains(&id) {
            return Err(fail(&format!(
                "footnote block {} is not a top-level block",
                id.0
            )));
        }
    }
    for &id in doc.figures.keys() {
        if !placed.contains(&id) || !matches!(doc.get(id), Some(Block::Media { .. })) {
            return Err(fail(
                "figure attributes can only be written on images in the document",
            ));
        }
    }
    for &id in doc.equation_labels.keys() {
        if !placed.contains(&id) || !matches!(doc.get(id), Some(Block::Math { .. })) {
            return Err(fail(
                "equation labels can only be written on display math in the document",
            ));
        }
    }
    let mut shown: BTreeSet<SheetId> = BTreeSet::new();
    for id in &placed {
        if let Some(Block::Grid { sheet_id, .. }) = doc.get(*id) {
            shown.insert(*sheet_id);
        }
    }
    for sheet_id in doc.sheets.keys() {
        if !shown.contains(sheet_id) {
            return Err(fail(&format!(
                "sheet {} is not shown by a grid block, so it cannot be written",
                sheet_id.0
            )));
        }
    }
    Ok(())
}

/// Writes a slide: `::: canvas { ratio="W:H" }`, its blocks, then `:::`.
fn write_canvas(
    doc: &Document,
    index: usize,
    id: BlockId,
    children: &[BlockId],
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let canvas = doc.canvases.get(&id).ok_or_else(|| {
        fail("an absolute container must be a canvas (a slide with a size) to be written")
    })?;
    let (width, height) = ratio_of(canvas).ok_or_else(|| {
        fail("the slide size does not match a ratio from 1:1 to 32:32 at 960 points wide")
    })?;
    let mut lines = vec![format!("::: canvas {{ ratio=\"{width}:{height}\" }}")];
    let mut parts: Vec<String> = Vec::new();
    let mut prev: Option<&Block> = None;
    for child in children {
        let block = doc.get(*child).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("slide child block {} is missing", child.0),
        })?;
        let label = doc.footnotes.get(child).map(String::as_str);
        parts.push(write_block(doc, index, *child, block, prev, label)?);
        prev = Some(block);
    }
    if !parts.is_empty() {
        lines.push(String::new());
        lines.push(parts.join("\n\n"));
    }
    lines.push(String::from(":::"));
    Ok(lines.join("\n"))
}

/// The ratio whose slide size the reader would give this canvas. The reader sets the size from
/// the ratio, so this must find the same ratio again.
fn ratio_of(canvas: &Canvas) -> Option<(u32, u32)> {
    (1..=32u32)
        .flat_map(|w| (1..=32u32).map(move |h| (w, h)))
        .find(|&ratio| canvas_size(ratio) == *canvas)
}

/// Writes a live grid as a pipe table inside `::: grid { id="N" }`.
///
/// The first `header_rows` rows come before the `| --- |` separator. Cells are escaped with `\|`
/// and `\\`. Each cell must read back as the same value, or the write is refused.
fn write_grid(
    doc: &Document,
    index: usize,
    sheet_id: SheetId,
    has_view_bounds: bool,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if has_view_bounds {
        return Err(fail("grid view bounds are not in the Markdown subset yet"));
    }
    let sheet = doc
        .sheets
        .get(&sheet_id)
        .ok_or_else(|| fail(&format!("grid refers to missing sheet {}", sheet_id.0)))?;
    // A sheet that stores no size (an imported one, for example) is as big as its cells.
    let (rows, cols) = sheet.extent();
    if rows == 0 || cols == 0 {
        return Err(fail("a grid needs at least one row and one column"));
    }
    if sheet.header_rows > rows {
        return Err(fail("the grid has more header rows than rows"));
    }
    for address in sheet.cells.keys() {
        match address_position(address) {
            Some((col, row)) if col <= cols && row <= rows => {}
            _ => return Err(fail(&format!("cell {address} is outside the grid"))),
        }
    }

    let mut attributes = format!("id=\"{}\"", sheet_id.0);
    if let Some(label) = doc.table_labels.get(&sheet_id) {
        if !is_table_label(label) {
            return Err(fail(
                "table label must start with tab: and use letters, digits, and - _ : .",
            ));
        }
        attributes.push_str(&format!(" label=\"{label}\""));
    }
    let names: Vec<String> = doc
        .cell_names
        .iter()
        .filter(|(_, cell)| cell.sheet == sheet_id)
        .map(|(name, cell)| format!("{name}={}", cell.address))
        .collect();
    if !names.is_empty() {
        attributes.push_str(&format!(" names=\"{}\"", names.join(",")));
    }
    let open_line = format!("::: grid {{ {attributes} }}");
    let mut lines = vec![open_line];
    for row in 1..=rows {
        let mut cells: Vec<String> = Vec::with_capacity(cols as usize);
        for col in 1..=cols {
            let address = address_of(col, row);
            let text = match sheet.cells.get(&address) {
                None => String::new(),
                Some(input) => {
                    let text = input.to_text();
                    if text.contains(['\n', '\r']) || text.trim() != text {
                        return Err(fail(&format!(
                            "cell {address} has a line break or leading or trailing spaces"
                        )));
                    }
                    if CellInput::parse(&text) != Ok(Some(input.clone())) {
                        return Err(fail(&format!(
                            "cell {address} would read back as a different value"
                        )));
                    }
                    text
                }
            };
            cells.push(text);
        }
        if is_separator(&cells) {
            return Err(fail(
                "a row of only dashes would read back as the header separator",
            ));
        }
        let escaped: Vec<String> = cells
            .iter()
            .map(|c| c.replace('\\', "\\\\").replace('|', "\\|"))
            .collect();
        lines.push(format!("| {} |", escaped.join(" | ")));
        if sheet.header_rows > 0 && row == sheet.header_rows {
            let dashes = vec!["---"; cols as usize];
            lines.push(format!("| {} |", dashes.join(" | ")));
        }
    }
    lines.push(String::from(":::"));
    Ok(lines.join("\n"))
}

/// Writes a text table as a pipe table: the first row is the header, under a separator row.
fn write_table(index: usize, rows: &[Vec<String>]) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let Some(first) = rows.first() else {
        return Err(fail("a table has no rows"));
    };
    let cols = first.len();
    if cols == 0 {
        return Err(fail("a table has no cells"));
    }
    let mut lines = Vec::with_capacity(rows.len() + 1);
    for (r, row) in rows.iter().enumerate() {
        if row.len() != cols {
            return Err(fail("the rows of a table have different numbers of cells"));
        }
        for text in row {
            if text.contains(['\n', '\r']) || text.trim() != text {
                return Err(fail(
                    "a table cell has a line break or leading or trailing spaces",
                ));
            }
        }
        if is_separator(row) {
            return Err(fail(
                "a row of only dashes would read back as the header separator",
            ));
        }
        let escaped: Vec<String> = row
            .iter()
            .map(|c| c.replace('\\', "\\\\").replace('|', "\\|"))
            .collect();
        lines.push(format!("| {} |", escaped.join(" | ")));
        if r == 0 {
            let dashes = vec!["---"; cols];
            lines.push(format!("| {} |", dashes.join(" | ")));
        }
    }
    Ok(lines.join("\n"))
}

/// The figure attributes text for `{...}` after an image: `#label width=N%`, or `None` when empty.
fn figure_text(index: usize, figure: &Figure) -> Result<Option<String>, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let mut tokens: Vec<String> = Vec::new();
    if let Some(label) = &figure.id {
        if !is_figure_label(label) {
            return Err(fail("figure label must be letters, digits, and - _ : ."));
        }
        tokens.push(format!("#{label}"));
    }
    if let Some(width) = figure.width_percent {
        if !(1..=100).contains(&width) {
            return Err(fail("figure width must be from 1% to 100%"));
        }
        tokens.push(format!("width={width}%"));
    }
    if tokens.is_empty() {
        return Ok(None);
    }
    let text = tokens.join(" ");
    match figure_attributes(&text) {
        Ok(back) if back == *figure => Ok(Some(text)),
        _ => Err(fail("figure attributes would read back differently")),
    }
}

fn write_block(
    doc: &Document,
    index: usize,
    id: BlockId,
    block: &Block,
    prev: Option<&Block>,
    label: Option<&str>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    match block {
        Block::Text { content, style } => match label {
            Some(label) => write_footnote(index, label, content, style),
            None => write_text(index, content, style),
        },
        Block::List {
            ordered,
            start,
            items,
        } => write_list(doc, index, *ordered, *start, items, prev),
        Block::Code { language, code } => write_code(index, language.as_deref(), code),
        Block::Math { source } => write_math(
            index,
            source,
            doc.equation_labels.get(&id).map(String::as_str),
        ),
        Block::ThematicBreak => Ok(String::from("***")),
        Block::Media { source, alt_text } => write_image(
            index,
            source,
            alt_text.as_deref(),
            doc.figures.get(&id).filter(|f| !f.is_empty()),
        ),
        Block::Grid {
            sheet_id,
            view_bounds,
        } => write_grid(doc, index, *sheet_id, view_bounds.is_some()),
        Block::Query(_) => Err(fail("queries are not in the Markdown subset yet")),
        Block::Quote { children } => write_quote(doc, index, children),
        Block::Table { rows } => write_table(index, rows),
        Block::Container { .. } => Err(fail("nested containers are not supported yet")),
    }
}

/// The runs of a text block, with its bold and italic flags added as one run over the whole text.
/// Markdown has no block-level emphasis, so the flags are written as emphasis around the text and
/// read back as a run.
fn runs_with_flags(content: &str, style: &TextStyle) -> Vec<InlineRun> {
    let mut runs = style.runs.clone();
    if (style.bold || style.italic) && !content.is_empty() {
        runs.push(InlineRun {
            start: 0,
            end: content.chars().count(),
            bold: style.bold,
            italic: style.italic,
            link: None,
            math: None,
            link_title: None,
        });
    }
    runs
}

fn write_text(index: usize, content: &str, style: &TextStyle) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if content.is_empty() {
        return Err(fail("empty text cannot be written"));
    }
    if content.trim() != content {
        return Err(fail("leading or trailing spaces would be lost"));
    }
    let escaped = write_inline(content, &runs_with_flags(content, style)).map_err(fail)?;

    if let Some(level) = style.heading_level {
        if !(1..=6).contains(&level) {
            return Err(fail("heading level must be 1 to 6"));
        }
        if content.contains('\n') {
            return Err(fail("headings must be a single line"));
        }
        return Ok(format!("{} {}", "#".repeat(level as usize), escaped));
    }

    for line in escaped.split('\n') {
        if line.trim().is_empty() {
            return Err(fail("paragraphs cannot contain blank lines"));
        }
        if starts_block(line) {
            return Err(fail("paragraph line would read back as other syntax"));
        }
    }
    Ok(escaped)
}

/// A footnote definition: `[^label]: text`, in a plain paragraph.
fn write_footnote(
    index: usize,
    label: &str,
    content: &str,
    style: &TextStyle,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if style.heading_level.is_some() || style.bold || style.italic {
        return Err(fail("footnote text must be a plain paragraph"));
    }
    if !crate::inline::is_citation_key(label) {
        return Err(fail("footnote label must be letters, digits, and - _ : ."));
    }
    let text = write_text(index, content, style)?;
    Ok(format!("[^{label}]: {text}"))
}

fn write_list(
    doc: &Document,
    index: usize,
    ordered: bool,
    start: u32,
    items: &[BlockId],
    prev: Option<&Block>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if items.is_empty() {
        return Err(fail("an empty list cannot be written"));
    }
    if !ordered && start != 1 {
        return Err(fail("bulleted lists always start at 1"));
    }
    if ordered && (start > MAX_LIST_NUMBER || (items.len() as u32 - 1) > MAX_LIST_NUMBER - start) {
        return Err(fail("list numbers are out of range"));
    }
    if let Some(Block::List {
        ordered: prev_ordered,
        ..
    }) = prev
    {
        if *prev_ordered == ordered {
            return Err(fail(
                "two adjacent lists of the same kind would read back as one list; put a paragraph or other block between them",
            ));
        }
    }
    if ordered && start != 1 {
        if let Some(Block::Text { style, .. }) = prev {
            if style.heading_level.is_none() {
                return Err(fail(
                    "a numbered list that does not start at 1 cannot follow a paragraph",
                ));
            }
        }
    }

    let mut lines: Vec<String> = Vec::new();
    list_lines(doc, index, ordered, start, items, 0, &mut lines)?;
    Ok(lines.join("\n"))
}

/// The lines of a list at `depth`, and of the sub-lists in it, each one indented by two spaces
/// per level. A sub-list is an item that is a list; it must follow a text item.
fn list_lines(
    doc: &Document,
    index: usize,
    ordered: bool,
    start: u32,
    items: &[BlockId],
    depth: usize,
    lines: &mut Vec<String>,
) -> Result<(), SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if items.is_empty() {
        return Err(fail("an empty list cannot be written"));
    }
    if !ordered && start != 1 {
        return Err(fail("bulleted lists always start at 1"));
    }
    if ordered && (start > MAX_LIST_NUMBER || (items.len() as u32 - 1) > MAX_LIST_NUMBER - start) {
        return Err(fail("list numbers are out of range"));
    }
    let indent = "  ".repeat(depth);
    let mut previous_sub: Option<bool> = None;
    // Numbers count this list's own items; a sub-list has its own numbers.
    let mut number = start;
    for (i, id) in items.iter().enumerate() {
        let item = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("list item {} is missing", id.0),
        })?;
        let (text, style) = match item {
            Block::Text { content, style } => (content, style),
            Block::List {
                ordered: sub_ordered,
                start: sub_start,
                items: sub_items,
            } => {
                if i == 0 {
                    return Err(fail("a sub-list must follow a list item"));
                }
                if previous_sub == Some(*sub_ordered) {
                    return Err(fail(
                        "two adjacent sub-lists of the same kind would read back as one; put a list item between them",
                    ));
                }
                previous_sub = Some(*sub_ordered);
                list_lines(
                    doc,
                    index,
                    *sub_ordered,
                    *sub_start,
                    sub_items,
                    depth + 1,
                    lines,
                )?;
                continue;
            }
            _ => return Err(fail("list items must be text or lists")),
        };
        previous_sub = None;
        if style.heading_level.is_some() {
            return Err(fail("list items must be plain text"));
        }
        let content = text.as_str();
        if content.is_empty() || content.trim() != content {
            return Err(fail(
                "list items must be non-empty, without leading or trailing spaces",
            ));
        }
        if content.contains(['\n', '\r']) {
            return Err(fail("list items must be single lines"));
        }
        let escaped = write_inline(content, &runs_with_flags(content, style)).map_err(fail)?;
        // "- - -" would read back as a thematic break, not as an item.
        if escaped
            .bytes()
            .all(|b| matches!(b, b'-' | b'*' | b'_' | b' '))
        {
            return Err(fail(
                "list item text made only of rule characters would read back as a break",
            ));
        }
        let line = if ordered {
            format!("{indent}{number}. {escaped}")
        } else {
            format!("{indent}- {escaped}")
        };
        number += 1;
        match list_item(&line) {
            Some(marker) if marker.text == escaped.as_str() && marker.ordered == ordered => {}
            _ => return Err(fail("list item would read back as other syntax")),
        }
        lines.push(line);
    }
    Ok(())
}

/// A block quote: its children written as Markdown, each line behind `> `. A blank line inside
/// the quote is a bare `>`.
fn write_quote(
    doc: &Document,
    index: usize,
    children: &[BlockId],
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if children.is_empty() {
        return Err(fail("an empty block quote cannot be written"));
    }
    let mut parts: Vec<String> = Vec::with_capacity(children.len());
    let mut prev: Option<&Block> = None;
    for id in children {
        let block = doc.get(*id).ok_or_else(|| SerializeError {
            index: Some(index),
            reason: format!("quoted block {} is missing", id.0),
        })?;
        let label = doc.footnotes.get(id).map(String::as_str);
        parts.push(write_block(doc, index, *id, block, prev, label)?);
        prev = Some(block);
    }
    let inner = parts.join("\n\n");
    let quoted: Vec<String> = inner
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                String::from(">")
            } else {
                format!("> {line}")
            }
        })
        .collect();
    Ok(quoted.join("\n"))
}

fn write_code(index: usize, language: Option<&str>, code: &str) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if code.contains('\r') {
        return Err(fail("code cannot contain carriage returns"));
    }
    if let Some(lang) = language {
        if lang.is_empty() || lang.trim() != lang {
            return Err(fail(
                "code language must be non-empty, without leading or trailing spaces",
            ));
        }
        if lang.contains(['\n', '`', '\\', '&']) {
            return Err(fail(
                "code language cannot contain line breaks, backticks, backslashes or entities",
            ));
        }
    }
    // A fence longer than any backtick run in the code can never be closed by a code line.
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(core::cmp::max(3, longest + 1));
    let mut out = String::new();
    out.push_str(&fence);
    if let Some(lang) = language {
        out.push_str(lang);
    }
    out.push('\n');
    out.push_str(code);
    out.push('\n');
    out.push_str(&fence);
    Ok(out)
}

fn write_math(index: usize, source: &str, label: Option<&str>) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    if source.is_empty() {
        return Err(fail("math block is empty"));
    }
    if source.contains('\r') {
        return Err(fail("math source cannot contain carriage returns"));
    }
    // A source line that reads as the closing delimiter would end the block early.
    if source.split('\n').any(math_closes) {
        return Err(fail(
            "math source cannot contain a line with only $$, which would close the block",
        ));
    }
    let close = match label {
        None => String::from("$$"),
        Some(label) => {
            if !is_equation_label(label) {
                return Err(fail(
                    "equation label must start with eq: and use letters, digits, and - _ : ., such as eq:euler",
                ));
            }
            format!("$$ {{#{label}}}")
        }
    };
    Ok(format!(
        "$$
{source}
{close}"
    ))
}

fn write_image(
    index: usize,
    source: &MediaSource,
    alt: Option<&str>,
    figure: Option<&Figure>,
) -> Result<String, SerializeError> {
    let fail = |reason: &str| SerializeError {
        index: Some(index),
        reason: String::from(reason),
    };
    let uri = match source {
        MediaSource::Uri { uri } => uri,
        MediaSource::Svg { .. } => {
            return Err(fail("inline SVG is not in the Markdown subset yet"));
        }
    };
    if uri.is_empty()
        || uri.contains(|c: char| c.is_whitespace() || c.is_control())
        || uri.contains(['(', ')', '<', '>', '\\', '&'])
    {
        return Err(fail(
            "image uri cannot contain spaces, parentheses, angle brackets, backslashes or entities",
        ));
    }
    if alt == Some("") {
        return Err(fail("empty alt text is read back as no alt text; use None"));
    }
    let alt = alt.unwrap_or("");
    if alt.contains(['[', ']', '\\', '&', '\n', '\r']) {
        return Err(fail(
            "image alt text cannot contain brackets, backslashes, entities or line breaks",
        ));
    }
    let attributes = match figure {
        Some(figure) => figure_text(index, figure)?,
        None => None,
    };
    let line = match &attributes {
        Some(text) => format!("![{alt}]({uri}) {{{text}}}"),
        None => format!("![{alt}]({uri})"),
    };
    let expected = attributes.unwrap_or_default();
    match image_line(&line) {
        Some(Ok((a, u, attr))) if a == alt && u == uri && attr == expected => Ok(line),
        _ => Err(fail("image would read back differently")),
    }
}
