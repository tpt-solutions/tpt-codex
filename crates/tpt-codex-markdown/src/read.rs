// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{
    Block, BlockId, Canvas, CellInput, Document, Figure, LayoutMode, Metadata, NamedCell, Sheet,
    SheetId, TextStyle, address_of,
};

use crate::error::ParseError;
use crate::inline::parse_inline;
use crate::syntax::{
    ContainerOpen, ListMarker, MathOpen, canvas_size, container_closes, container_open,
    fence_closes, fence_open, figure_attributes, heading, image_line, is_separator, leading_spaces,
    list_item, math_close, math_open, quote_content, table_row, thematic_break, unsupported,
};

/// Reads the supported Markdown subset into a [`Document`].
///
/// The result always passes [`Document::validate`]. Errors carry a 1-based
/// line and column.
pub fn from_markdown(src: &str) -> Result<Document, ParseError> {
    let lines: Vec<&str> = src.lines().collect();
    let mut metadata = Metadata::default();
    let body_start = read_front_matter(&lines, &mut metadata)?;

    let mut b = Builder::default();
    read_body(&lines[body_start..], body_start + 1, &mut b)?;
    Ok(document_from(b, metadata))
}

/// Reads a run of body lines into `b`. `first_line` is the 1-based line number of `lines[0]`.
/// A block quote's lines are read by a call to this function, on the lines behind its `>`.
fn read_body(lines: &[&str], first_line: usize, b: &mut Builder) -> Result<(), ParseError> {
    let mut open = Open::default();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let lineno = first_line + index;
        index += 1;

        if let Some(source_lines) = open.math.as_mut() {
            if let Some(close) = math_close(line) {
                let label = close.map_err(|reason| {
                    error(
                        lineno,
                        reason,
                        "close the math with $$ alone, or with $$ {#eq:id}",
                    )
                })?;
                let source = source_lines.join(
                    "
",
                );
                open.math = None;
                if source.is_empty() {
                    return Err(error(
                        lineno,
                        "math block is empty",
                        "put the math source between the two $$ lines",
                    ));
                }
                let id = b.push_block(Block::Math { source });
                if let Some(label) = label {
                    b.equation_labels.insert(id, label);
                }
            } else {
                // Math source is kept exactly, including leading spaces.
                source_lines.push(String::from(line));
            }
            continue;
        }

        if let Some(code) = open.code.as_mut() {
            if fence_closes(line, code.ch, code.len) {
                if let Some(code) = open.code.take() {
                    b.push_block(Block::Code {
                        language: code.language,
                        code: code.lines.join("\n"),
                    });
                }
            } else {
                // Content loses up to as many leading spaces as the opening fence had.
                let n = line
                    .bytes()
                    .take(code.indent)
                    .take_while(|&c| c == b' ')
                    .count();
                code.lines.push(String::from(&line[n..]));
            }
            continue;
        }

        // A table ends at the first line that is not a pipe row.
        if open.table.is_some() && table_row(line).is_none() {
            finish_table(&mut open, b)?;
        }

        if open.grid.is_some() && container_closes(line) {
            finish_grid(&mut open, b)?;
            continue;
        }
        if let Some(grid) = open.grid.as_mut() {
            if line.trim().is_empty() {
                continue;
            }
            match table_row(line) {
                None => {
                    return Err(error(
                        lineno,
                        "a grid can only hold table rows",
                        "write each row as | cell | cell |, then close the grid with a line of :::",
                    ));
                }
                Some(Err(reason)) => {
                    return Err(error(lineno, reason, "write each row as | cell | cell |"));
                }
                Some(Ok(cells)) if is_separator(&cells) => {
                    if grid.rows.is_empty() {
                        return Err(error(
                            lineno,
                            "a grid's separator row must come after a row of cells",
                            "put the header row first, then the | --- | separator",
                        ));
                    }
                    if grid.header_rows.is_some() {
                        return Err(error(
                            lineno,
                            "a grid can have only one separator row",
                            "remove the extra | --- | row",
                        ));
                    }
                    grid.header_rows = Some(grid.rows.len());
                }
                Some(Ok(cells)) => {
                    if let Some((_, first)) = grid.rows.first() {
                        if first.len() != cells.len() {
                            return Err(error(
                                lineno,
                                &alloc::format!(
                                    "row has {} cells, but the grid's first row has {}",
                                    cells.len(),
                                    first.len()
                                ),
                                "give every row the same number of cells",
                            ));
                        }
                    }
                    grid.rows.push((lineno, cells));
                }
            }
            continue;
        }

        if container_closes(line) {
            if b.canvas.is_none() {
                return Err(error(
                    lineno,
                    "::: closes nothing",
                    "remove it, or open a container with ::: grid { id=\"1\" } or ::: canvas { ratio=\"16:9\" }",
                ));
            }
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            open.after_image = false;
            finish_canvas(b);
            continue;
        }

        if let Some(opener) = container_open(line) {
            let opener = opener.map_err(|reason| {
                error(
                    lineno,
                    reason,
                    "write the attributes as { key=\"value\" } after the container name",
                )
            })?;
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            open.after_image = false;
            match opener {
                ContainerOpen::Grid { id, label, names } => {
                    if b.sheets.contains_key(&SheetId(id)) {
                        return Err(error(
                            lineno,
                            &alloc::format!("grid id {id} is used by more than one grid"),
                            "give each grid its own id",
                        ));
                    }
                    open.grid = Some(OpenGrid {
                        id,
                        label,
                        names,
                        line: lineno,
                        rows: Vec::new(),
                        header_rows: None,
                    });
                }
                ContainerOpen::Canvas { ratio } => {
                    if b.canvas.is_some() {
                        return Err(error(
                            lineno,
                            "a canvas cannot be placed inside another canvas",
                            "close the open canvas with ::: first",
                        ));
                    }
                    b.canvas = Some(OpenCanvas {
                        ratio,
                        children: Vec::new(),
                    });
                }
                ContainerOpen::Unknown(name) => {
                    return Err(error(
                        lineno,
                        &alloc::format!("unknown container '{name}'"),
                        "supported containers are grid and canvas",
                    ));
                }
            }
            continue;
        }

        if line.trim().is_empty() {
            open.close_paragraph(b)?;
            open.after_image = false;
            if !open.lists.is_empty() {
                open.list_blank = true;
            }
            continue;
        }
        let after_image = core::mem::take(&mut open.after_image);

        // A block quote: its lines, behind their `>`, are read as a body of their own.
        if let Some(first_content) = quote_content(line) {
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            if b.canvas.is_some() {
                return Err(error(
                    lineno,
                    "block quotes cannot be placed inside a slide yet",
                    "move the quote outside the ::: canvas block",
                ));
            }
            let mut quoted: Vec<&str> = alloc::vec![first_content];
            while index < lines.len() {
                match quote_content(lines[index]) {
                    Some(content) => quoted.push(content),
                    None => break,
                }
                index += 1;
            }
            let outer = core::mem::take(&mut b.root);
            let read = read_body(&quoted, lineno, b);
            let children = core::mem::replace(&mut b.root, outer);
            read?;
            b.push_block(Block::Quote { children });
            continue;
        }

        if let Some(item) = list_item(line) {
            if item.text.is_empty() {
                return Err(error(
                    lineno,
                    "list item is empty",
                    "write the item's text after the marker",
                ));
            }
            if open.para.is_some() && item.ordered && item.number != 1 {
                return Err(error(
                    lineno,
                    "a numbered list that does not start at 1 cannot follow a paragraph",
                    "start the list at 1, or put a blank line before it",
                ));
            }
            open.close_paragraph(b)?;
            // An item at this depth belongs to the open list at this depth, if the kind matches.
            // Lists deeper than the item end; a list of another kind at this depth ends. Otherwise
            // a new list starts here, nested in the item above it when there is one.
            let indent = leading_spaces(line);
            let key = item.key();
            loop {
                open.close_lists_deeper_than(indent, b);
                let same_depth = open.lists.last().map(|top| top.indent == indent);
                match same_depth {
                    Some(true) if open.lists.last().is_some_and(|top| top.key == key) => {
                        if open.list_blank {
                            return Err(error(
                                lineno,
                                "loose lists (blank lines between items) are not supported yet",
                                "remove the blank lines between the items",
                            ));
                        }
                        let id = b.alloc(text_block(item.text, lineno)?);
                        if let Some(top) = open.lists.last_mut() {
                            top.items.push(id);
                        }
                        break;
                    }
                    Some(true) => open.close_top_list(b),
                    _ => {
                        open.lists.push(OpenList::start(b, item, lineno, indent)?);
                        break;
                    }
                }
            }
            open.list_blank = false;
            continue;
        }

        if let Some(fence) = fence_open(line) {
            if fence.info.contains(['\\', '&']) {
                return Err(error(
                    lineno,
                    "code block info string cannot contain backslashes or entities",
                    "use a plain language name such as rust",
                ));
            }
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            open.code = Some(OpenCode {
                ch: fence.ch,
                len: fence.len,
                indent: fence.indent,
                language: (!fence.info.is_empty()).then(|| String::from(fence.info)),
                lines: Vec::new(),
            });
            continue;
        }

        if let Some(opener) = math_open(line) {
            let opener = opener.map_err(|reason| {
                error(
                    lineno,
                    reason,
                    "write the math as $$, then the source lines, then $$",
                )
            })?;
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            match opener {
                MathOpen::Single { source, label } => {
                    let id = b.push_block(Block::Math {
                        source: String::from(source),
                    });
                    if let Some(label) = label {
                        b.equation_labels.insert(id, label);
                    }
                }
                MathOpen::Block => open.math = Some(Vec::new()),
            }
            continue;
        }

        if let Some(ch) = thematic_break(line) {
            if ch == b'-' && open.para.is_some() {
                return Err(error(
                    lineno,
                    "a line of dashes under text is a setext heading, which is not supported yet",
                    "put a blank line between the text and the dashes, or use *** for a rule",
                ));
            }
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            b.push_block(Block::ThematicBreak);
            continue;
        }

        if let Some((level, text)) = heading(line) {
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            if text.is_empty() {
                return Err(error(
                    lineno,
                    "heading has no text",
                    "add text after the hashes, or write a paragraph instead",
                ));
            }
            let block = text_block_styled(
                text,
                lineno,
                TextStyle {
                    bold: false,
                    italic: false,
                    heading_level: Some(level),
                    runs: Vec::new(),
                },
            )?;
            b.push_block(block);
            continue;
        }

        if let Some(image) = image_line(line) {
            if open.para.is_some() || (!open.lists.is_empty() && !open.list_blank) {
                return Err(error(
                    lineno,
                    "an image directly after text is not supported yet",
                    "put a blank line before the image",
                ));
            }
            open.close_all_lists(b);
            let (alt, uri, attributes) = image
                .map_err(|reason| error(lineno, reason, "use ![alt text](path) with no title"))?;
            let figure = figure_attributes(attributes).map_err(|reason| {
                error(
                    lineno,
                    reason,
                    "write figure attributes as {#fig:id width=80%}",
                )
            })?;
            let id = b.push_block(Block::Media {
                source: tpt_codex_core::MediaSource::Uri {
                    uri: String::from(uri),
                },
                alt_text: (!alt.is_empty()).then(|| String::from(alt)),
            });
            if !figure.is_empty() {
                b.figures.insert(id, figure);
            }
            open.after_image = true;
            continue;
        }

        // A pipe table outside a grid: its header row, then a separator row, then body rows.
        if let Some(row) = table_row(line) {
            let cells = match row {
                Ok(cells) => cells,
                Err(reason) => {
                    return Err(error(lineno, reason, "write each row as | cell | cell |"));
                }
            };
            open.close_paragraph(b)?;
            open.close_all_lists(b);
            let table = open.table.get_or_insert_with(|| OpenTable {
                line: lineno,
                rows: Vec::new(),
                separator: false,
            });
            if is_separator(&cells) {
                if table.rows.len() != 1 || table.separator {
                    return Err(error(
                        lineno,
                        "a table's separator row must come once, right after its header row",
                        "put one row of --- under the header row, and nowhere else",
                    ));
                }
                table.separator = true;
                continue;
            }
            if let Some(first) = table.rows.first() {
                if first.len() != cells.len() {
                    let message = alloc::format!(
                        "row has {} cells, but the table's header has {}",
                        cells.len(),
                        first.len()
                    );
                    return Err(error(
                        lineno,
                        &message,
                        "give every row of the table the same number of cells",
                    ));
                }
            }
            table.rows.push(cells);
            continue;
        }

        if let Some(reason) = unsupported(line) {
            return Err(error(
                lineno,
                &alloc::format!("{reason} is not supported yet"),
                "rewrite as plain paragraphs, headings, lists and code blocks",
            ));
        }

        if after_image {
            return Err(error(
                lineno,
                "text directly after an image is not supported yet",
                "put a blank line between the image and the text",
            ));
        }
        if !open.lists.is_empty() {
            if !open.list_blank {
                return Err(error(
                    lineno,
                    "continuation lines inside a list item are not supported yet",
                    "put each list item on one line",
                ));
            }
            open.close_all_lists(b);
        }
        match &mut open.para {
            Some(text) => {
                text.push('\n');
                text.push_str(line);
            }
            None => {
                open.para = Some(String::from(line));
                open.para_line = lineno;
            }
        }
    }

    let last_line = first_line + lines.len().saturating_sub(1);
    finish_table(&mut open, b)?;
    if open.code.is_some() {
        return Err(error(
            last_line,
            "code block is not closed",
            "close the block with a fence of the same character and at least the same length",
        ));
    }
    if open.math.is_some() {
        return Err(error(
            last_line,
            "math block is not closed",
            "close the block with a line containing only $$",
        ));
    }
    if let Some(grid) = open.grid.as_ref() {
        return Err(error(
            grid.line,
            "grid is not closed",
            "close the grid with a line containing only :::",
        ));
    }
    if b.canvas.is_some() {
        return Err(error(
            last_line,
            "canvas is not closed",
            "close the slide with a line containing only :::",
        ));
    }
    open.close_paragraph(b)?;
    open.close_all_lists(b);
    Ok(())
}

/// The document made of what was read: the root's children, and the side tables of the reader.
fn document_from(mut b: Builder, metadata: Metadata) -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: b.root,
        },
    );
    doc.metadata = metadata;
    let labels = core::mem::take(&mut b.labels);
    for (i, block) in b.blocks.into_iter().enumerate() {
        doc.insert(BlockId(i as u64 + 1), block);
    }
    for (id, label) in labels {
        doc.footnotes.insert(id, label);
    }
    for (id, sheet) in b.sheets {
        doc.insert_sheet(id, sheet);
    }
    doc.canvases = b.canvases;
    doc.figures = b.figures;
    doc.equation_labels = b.equation_labels;
    doc.table_labels = b.table_labels;
    doc.cell_names = b.cell_names;
    doc
}

fn error(line: usize, message: &str, suggestion: &'static str) -> ParseError {
    ParseError {
        line,
        column: 1,
        message: String::from(message),
        suggestion: Some(suggestion),
    }
}

/// Adds the table whose rows were read, as a text table block. A table needs its separator row.
fn finish_table(open: &mut Open, b: &mut Builder) -> Result<(), ParseError> {
    let Some(table) = open.table.take() else {
        return Ok(());
    };
    if !table.separator {
        return Err(error(
            table.line,
            "table has no separator row",
            "put one row of --- under the header row, such as | --- | --- |",
        ));
    }
    b.push_block(Block::Table { rows: table.rows });
    Ok(())
}

/// Builds the grid whose body was read. The grid holds one sheet with the cells that are not
/// empty; the grid's size is its rows and columns.
fn finish_grid(open: &mut Open, b: &mut Builder) -> Result<(), ParseError> {
    let Some(grid) = open.grid.take() else {
        return Ok(());
    };
    if grid.rows.is_empty() {
        return Err(error(
            grid.line,
            "grid has no rows",
            "add rows of cells between the grid line and the closing :::",
        ));
    }
    let cols = grid.rows[0].1.len();
    let mut sheet = Sheet::with_size(grid.rows.len() as u32, cols as u32);
    sheet.header_rows = grid.header_rows.unwrap_or(0) as u32;
    for (r, (line, cells)) in grid.rows.iter().enumerate() {
        for (c, text) in cells.iter().enumerate() {
            match CellInput::parse(text) {
                Ok(Some(input)) => {
                    sheet
                        .cells
                        .insert(address_of(c as u32 + 1, r as u32 + 1), input);
                }
                Ok(None) => {}
                Err(reason) => {
                    return Err(error(
                        *line,
                        &reason,
                        "write a formula as =expression, or put text or a number in the cell",
                    ));
                }
            }
        }
    }
    let sheet_id = SheetId(grid.id);
    if let Some(label) = grid.label {
        b.table_labels.insert(sheet_id, label);
    }
    for (name, address) in grid.names {
        if b.cell_names.contains_key(&name) {
            return Err(error(
                grid.line,
                &alloc::format!("cell name {name} is used by more than one grid"),
                "give each name to one cell, and each cell one name",
            ));
        }
        b.cell_names.insert(
            name,
            NamedCell {
                sheet: sheet_id,
                address,
            },
        );
    }
    b.sheets.insert(sheet_id, sheet);
    b.push_block(Block::Grid {
        sheet_id,
        view_bounds: None,
    });
    Ok(())
}

/// Closes the open slide: its children become an absolute container at the root.
fn finish_canvas(b: &mut Builder) {
    let Some(canvas) = b.canvas.take() else {
        return;
    };
    let id = b.alloc(Block::Container {
        layout_mode: LayoutMode::Absolute,
        children: canvas.children,
    });
    b.root.push(id);
    b.canvases.insert(id, canvas_size(canvas.ratio));
}

/// A text block read from the source line `line`. Inline marks are read here.
fn text_block(content: &str, line: usize) -> Result<Block, ParseError> {
    text_block_styled(content, line, TextStyle::default())
}

fn text_block_styled(content: &str, line: usize, style: TextStyle) -> Result<Block, ParseError> {
    let inline = parse_inline(content).map_err(|e| error(line, e.message, e.suggestion))?;
    Ok(Block::Text {
        content: inline.text,
        style: TextStyle {
            runs: inline.runs,
            ..style
        },
    })
}

/// Splits a footnote definition `[^label]: text` into its label and text. `None` for any other
/// paragraph, including one whose label is not a valid key.
fn footnote_definition(content: &str) -> Option<(String, String)> {
    let rest = content.strip_prefix("[^")?;
    let end = rest.find("]:")?;
    let label = &rest[..end];
    if !crate::inline::is_citation_key(label) {
        return None;
    }
    let text = rest[end + 2..]
        .strip_prefix(' ')
        .unwrap_or(&rest[end + 2..]);
    if text.trim().is_empty() {
        return None;
    }
    Some((String::from(label), String::from(text)))
}

/// Block ids are allocated in the order blocks are created. The root is `BlockId(0)`.
#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    root: Vec<BlockId>,
    /// Footnote definitions: the block that holds each text, and its label.
    labels: Vec<(BlockId, String)>,
    /// The slide being read. Blocks read while it is open are its children.
    canvas: Option<OpenCanvas>,
    /// Sheets of the grids read so far, keyed by the id each grid names.
    sheets: BTreeMap<SheetId, Sheet>,
    /// Sizes of the slides read so far.
    canvases: BTreeMap<BlockId, Canvas>,
    /// Figure attributes of the images read so far.
    figures: BTreeMap<BlockId, Figure>,
    /// Equation labels of the math blocks read so far.
    equation_labels: BTreeMap<BlockId, String>,
    /// Table labels of the grids read so far.
    table_labels: BTreeMap<SheetId, String>,
    /// Cell names of the grids read so far.
    cell_names: BTreeMap<String, NamedCell>,
}

impl Builder {
    fn alloc(&mut self, block: Block) -> BlockId {
        self.blocks.push(block);
        BlockId(self.blocks.len() as u64)
    }

    /// Adds a block to the open slide, or to the root when no slide is open.
    fn push_block(&mut self, block: Block) -> BlockId {
        let id = self.alloc(block);
        self.place(id);
        id
    }

    fn place(&mut self, id: BlockId) {
        match self.canvas.as_mut() {
            Some(canvas) => canvas.children.push(id),
            None => self.root.push(id),
        }
    }
}

/// A slide whose closing `:::` line has not been read yet.
struct OpenCanvas {
    ratio: (u32, u32),
    children: Vec<BlockId>,
}

/// The block that is still open while reading lines.
#[derive(Default)]
struct Open {
    para: Option<String>,
    /// The 1-based line where the open paragraph starts.
    para_line: usize,
    /// The open lists, outermost first. Each one after the first is a sub-list of the last item
    /// of the one before it.
    lists: Vec<OpenList>,
    /// A blank line has been seen since the list's last item.
    list_blank: bool,
    code: Option<OpenCode>,
    /// Source lines of a multi-line math block that is still open.
    math: Option<Vec<String>>,
    /// A grid whose closing `:::` line has not been read yet.
    grid: Option<OpenGrid>,
    /// A pipe table outside a grid whose rows are being read.
    table: Option<OpenTable>,
    /// The previous line was an image. Text on the next line would join it in CommonMark.
    after_image: bool,
}

impl Open {
    fn close_paragraph(&mut self, b: &mut Builder) -> Result<(), ParseError> {
        if let Some(content) = self.para.take() {
            match footnote_definition(&content) {
                Some((label, text)) => {
                    if b.canvas.is_some() {
                        return Err(error(
                            self.para_line,
                            "footnotes cannot be placed inside a slide yet",
                            "move the footnote definition outside the ::: canvas block",
                        ));
                    }
                    let block = text_block(&text, self.para_line)?;
                    let id = b.alloc(block);
                    b.root.push(id);
                    b.labels.push((id, label));
                }
                None => {
                    let block = text_block(&content, self.para_line)?;
                    b.push_block(block);
                }
            }
        }
        Ok(())
    }

    /// Closes the innermost open list. It becomes the last item of the list around it, or a block
    /// of its own when no list is around it.
    fn close_top_list(&mut self, b: &mut Builder) {
        let Some(list) = self.lists.pop() else {
            return;
        };
        let block = Block::List {
            ordered: list.key.0,
            start: list.start,
            items: list.items,
        };
        match self.lists.last_mut() {
            Some(parent) => {
                let id = b.alloc(block);
                parent.items.push(id);
            }
            None => {
                b.push_block(block);
            }
        }
    }

    /// Closes the open lists indented further than `indent`.
    fn close_lists_deeper_than(&mut self, indent: usize, b: &mut Builder) {
        while self.lists.last().is_some_and(|top| top.indent > indent) {
            self.close_top_list(b);
        }
    }

    /// Closes every open list.
    fn close_all_lists(&mut self, b: &mut Builder) {
        while !self.lists.is_empty() {
            self.close_top_list(b);
        }
        self.list_blank = false;
    }
}

struct OpenList {
    key: (bool, u8),
    start: u32,
    /// The number of spaces before the list's markers. A sub-list is indented further than its
    /// parent.
    indent: usize,
    items: Vec<BlockId>,
}

impl OpenList {
    fn start(
        b: &mut Builder,
        item: ListMarker<'_>,
        line: usize,
        indent: usize,
    ) -> Result<Self, ParseError> {
        let first = b.alloc(text_block(item.text, line)?);
        Ok(OpenList {
            key: item.key(),
            start: item.number,
            indent,
            items: alloc::vec![first],
        })
    }
}

/// A grid whose body is still being read: its pipe-table rows, and where its separator is.
struct OpenTable {
    /// The line of the table's header row.
    line: usize,
    /// The rows read so far: the header first, then the body rows.
    rows: Vec<Vec<String>>,
    /// Whether the separator row has been read.
    separator: bool,
}

struct OpenGrid {
    id: u64,
    /// The table label from the opening line, if any.
    label: Option<String>,
    /// The cell names from the opening line: `(name, address)`.
    names: Vec<(String, String)>,
    /// The line of the opening `:::` line.
    line: usize,
    /// Each row's line and trimmed cells.
    rows: Vec<(usize, Vec<String>)>,
    /// How many rows come before the `| --- |` separator, if there is one.
    header_rows: Option<usize>,
}

/// A fenced code block whose closing fence has not been read yet.
struct OpenCode {
    ch: u8,
    len: usize,
    indent: usize,
    language: Option<String>,
    lines: Vec<String>,
}

/// Reads an optional front-matter block. Returns the index of the first body line.
fn read_front_matter(lines: &[&str], metadata: &mut Metadata) -> Result<usize, ParseError> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Ok(0);
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        let lineno = i + 1;
        if line.trim_end() == "---" {
            return Ok(i + 1);
        }
        if line.trim().is_empty() {
            continue;
        }
        let (key, raw) = line.split_once(':').ok_or_else(|| ParseError {
            line: lineno,
            column: 1,
            message: String::from("front matter line is not 'key: value'"),
            suggestion: Some("write each entry as key: value"),
        })?;
        let value = parse_value(raw.trim()).map_err(|message| ParseError {
            line: lineno,
            column: raw.len() - raw.trim_start().len() + key.len() + 2,
            message,
            suggestion: Some("wrap the value in double quotes; escape \\\" and \\\\"),
        })?;
        let slot = match key.trim() {
            "title" => &mut metadata.title,
            "author" => &mut metadata.author,
            "date" => &mut metadata.date,
            "class" => &mut metadata.class,
            other => {
                return Err(ParseError {
                    line: lineno,
                    column: 1,
                    message: alloc::format!("unknown front matter key '{other}'"),
                    suggestion: Some("supported keys: title, author, date, class"),
                });
            }
        };
        *slot = Some(value);
    }
    Err(ParseError {
        line: lines.len(),
        column: 1,
        message: String::from("front matter is not closed"),
        suggestion: Some("add a '---' line after the last front matter entry"),
    })
}

/// Parses a bare or double-quoted value. Quoted values accept `\"` and `\\`.
fn parse_value(raw: &str) -> Result<String, String> {
    if !raw.starts_with('"') {
        return Ok(String::from(raw));
    }
    let inner = raw[1..]
        .strip_suffix('"')
        .ok_or_else(|| String::from("quoted value is missing its closing quote"))?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(e @ ('"' | '\\')) => out.push(e),
                _ => return Err(String::from("invalid escape in quoted value")),
            }
        } else if c == '"' {
            return Err(String::from("unescaped quote inside quoted value"));
        } else {
            out.push(c);
        }
    }
    Ok(out)
}
