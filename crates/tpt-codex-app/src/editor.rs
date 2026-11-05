// SPDX-License-Identifier: MIT OR Apache-2.0
//! The editor's state and view, without a browser.
//!
//! [`Editor::update`] applies a [`Msg`] to the document, and [`Editor::view`] builds the
//! `tpt-appfront` UI tree for it. The tree renders to semantic HTML with `tpt-appfront-html`.
//! Keeping the logic here lets it run and be tested natively.

use tpt_appfront_core::UITree;
use tpt_appfront_core::ui_tree::ContainerBuilder;
use tpt_codex_core::{
    Block, BlockId, CellInput, Document, EditError, History, MediaSource, Movement, Op, Selection,
    Sheet, SheetId, delete_backward, delete_forward, insert_text, is_valid_address,
};
use tpt_codex_grid::{CellResult, evaluate_sheet};
use tpt_typeset::{document_entities, run_query};

/// Something the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// Put the caret at the end of this text block.
    Focus(BlockId),
    /// Type this text at the caret.
    Type(String),
    /// Delete the character before the caret, or the selection.
    Backspace,
    /// Delete the character after the caret, or the selection.
    Delete,
    /// Move the caret.
    Move(Movement),
    /// Set a grid cell from what the user typed. Empty text clears the cell.
    EditCell {
        /// The sheet holding the cell.
        sheet: SheetId,
        /// Canonical A1 address.
        address: String,
        /// The text typed into the cell.
        text: String,
    },
    /// Replace the source of a query block. A source that does not parse or run is rejected,
    /// and the document is unchanged.
    EditQuery {
        /// The query block to change.
        block: BlockId,
        /// The new query source (JSON).
        source: String,
    },
    /// Select a grid cell, so that arrow keys move from it. Ignored if the cell is outside
    /// the sheet's extent or the sheet does not exist.
    FocusCell {
        /// The sheet holding the cell.
        sheet: SheetId,
        /// Canonical A1 address.
        address: String,
    },
    /// Move the selected cell one step. At the edge of the table it stays put.
    MoveCell(Arrow),
    /// Add an empty table of `rows` by `cols` cells at the end of the document, as one undoable
    /// step. Each side must be between 1 and [`MAX_TABLE_SIDE`].
    InsertTable {
        /// Number of rows.
        rows: u32,
        /// Number of columns.
        cols: u32,
    },
    /// Undo the last step.
    Undo,
    /// Redo the last undone step.
    Redo,
}

/// A direction for [`Msg::MoveCell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Left one column.
    Left,
    /// Right one column.
    Right,
}

/// The selected grid cell: its sheet, column and row (both 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellFocus {
    /// The sheet holding the cell.
    pub sheet: SheetId,
    /// Column, 1 is `A`.
    pub col: u32,
    /// Row, 1 is the first row.
    pub row: u32,
}

/// A document open for editing.
#[derive(Debug)]
pub struct Editor {
    doc: Document,
    history: History,
    caret: Option<Selection>,
    cell: Option<CellFocus>,
    status: Option<String>,
}

impl Editor {
    /// Opens `doc` with no caret.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            history: History::new(),
            caret: None,
            cell: None,
            status: None,
        }
    }

    /// The document as it is now.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The caret, if one is in a text block.
    pub fn caret(&self) -> Option<Selection> {
        self.caret
    }

    /// The selected grid cell, if any.
    pub fn cell(&self) -> Option<CellFocus> {
        self.cell
    }

    /// The last message that could not be applied, if any.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Applies a message. A message that cannot be applied sets [`Editor::status`] and
    /// leaves the document unchanged.
    pub fn update(&mut self, msg: Msg) {
        self.status = None;
        let result = match msg {
            Msg::Focus(id) => {
                self.caret = match self.doc.get(id) {
                    Some(Block::Text { content, .. }) => {
                        Some(Selection::caret(id, content.chars().count()))
                    }
                    _ => None,
                };
                self.cell = None;
                Ok(())
            }
            Msg::FocusCell { sheet, address } => self.focus_cell(sheet, &address),
            Msg::MoveCell(arrow) => self.move_cell(arrow),
            Msg::Type(text) => self.edit(|doc, h, sel| insert_text(doc, h, sel, &text).map(|_| ())),
            Msg::Backspace => self.edit(|doc, h, sel| delete_backward(doc, h, sel).map(|_| ())),
            Msg::Delete => self.edit(|doc, h, sel| delete_forward(doc, h, sel).map(|_| ())),
            Msg::Move(movement) => match self.caret.as_mut() {
                Some(sel) => sel
                    .move_caret(&self.doc, movement, false)
                    .map_err(|e| e.to_string()),
                None => Err(NO_CARET.to_string()),
            },
            Msg::EditCell {
                sheet,
                address,
                text,
            } => self.edit_cell(sheet, &address, &text),
            Msg::EditQuery { block, source } => self.edit_query(block, source),
            Msg::InsertTable { rows, cols } => self.insert_table(rows, cols),
            Msg::Undo => {
                let r = self.history.undo(&mut self.doc).map(|_| ());
                self.clamp_caret();
                r.map_err(|e| e.to_string())
            }
            Msg::Redo => {
                let r = self.history.redo(&mut self.doc).map(|_| ());
                self.clamp_caret();
                r.map_err(|e| e.to_string())
            }
        };
        if let Err(message) = result {
            self.status = Some(message);
        }
    }

    fn focus_cell(&mut self, sheet: SheetId, address: &str) -> Result<(), String> {
        let (col, row) = tpt_codex_core::address_position(address)
            .ok_or_else(|| format!("'{address}' is not a valid cell address"))?;
        let data = self
            .doc
            .sheets
            .get(&sheet)
            .ok_or_else(|| format!("sheet {} does not exist", sheet.0))?;
        let (rows, cols) = data.extent();
        if row > rows || col > cols {
            return Err(format!("'{address}' is outside the table"));
        }
        self.caret = None;
        self.cell = Some(CellFocus { sheet, col, row });
        Ok(())
    }

    fn move_cell(&mut self, arrow: Arrow) -> Result<(), String> {
        let Some(focus) = self.cell else {
            return Err("no cell selected: click a grid cell first".to_string());
        };
        let (rows, cols) = self
            .doc
            .sheets
            .get(&focus.sheet)
            .map_or((0, 0), |sheet| sheet.extent());
        let (mut col, mut row) = (focus.col, focus.row);
        match arrow {
            Arrow::Up => row = row.saturating_sub(1).max(1),
            Arrow::Down => row = (row + 1).min(rows.max(1)),
            Arrow::Left => col = col.saturating_sub(1).max(1),
            Arrow::Right => col = (col + 1).min(cols.max(1)),
        }
        self.cell = Some(CellFocus { col, row, ..focus });
        Ok(())
    }

    fn edit(
        &mut self,
        step: impl FnOnce(&mut Document, &mut History, &mut Selection) -> Result<(), EditError>,
    ) -> Result<(), String> {
        let Some(mut sel) = self.caret else {
            return Err(NO_CARET.to_string());
        };
        step(&mut self.doc, &mut self.history, &mut sel).map_err(|e| e.to_string())?;
        self.caret = Some(sel);
        Ok(())
    }

    fn edit_cell(&mut self, sheet: SheetId, address: &str, text: &str) -> Result<(), String> {
        if !is_valid_address(address) {
            return Err(format!("'{address}' is not a valid cell address"));
        }
        let input = parse_cell_input(text)?;
        self.history
            .commit(
                &mut self.doc,
                vec![Op::SetCell {
                    sheet,
                    address: address.to_string(),
                    input,
                }],
            )
            .map_err(|e| e.to_string())
    }

    fn edit_query(&mut self, block: BlockId, source: String) -> Result<(), String> {
        if !matches!(self.doc.get(block), Some(Block::Query(_))) {
            return Err("that block is not a query".to_string());
        }
        let query = tpt_codex_core::Query {
            source: source.clone(),
        };
        let entities = document_entities(&self.doc);
        run_query(&query, &entities).map_err(|e| e.to_string())?;
        self.history
            .commit(
                &mut self.doc,
                vec![Op::SetBlock {
                    id: block,
                    block: Block::Query(query),
                }],
            )
            .map_err(|e| e.to_string())
    }

    fn insert_table(&mut self, rows: u32, cols: u32) -> Result<(), String> {
        if !(1..=MAX_TABLE_SIDE).contains(&rows) || !(1..=MAX_TABLE_SIDE).contains(&cols) {
            return Err(format!(
                "a table needs between 1 and {MAX_TABLE_SIDE} rows and columns"
            ));
        }
        let Some(Block::Container { children, .. }) = self.doc.get(self.doc.root) else {
            return Err("the document root cannot hold a table".to_string());
        };
        let index = children.len();
        let root = self.doc.root;
        let sheet_id = SheetId(
            self.doc
                .sheets
                .keys()
                .map(|s| s.0)
                .max()
                .map_or(1, |m| m + 1),
        );
        let block_id = BlockId(
            self.doc
                .blocks
                .keys()
                .map(|b| b.0)
                .max()
                .map_or(1, |m| m + 1),
        );
        let ops = vec![
            Op::SetSheet {
                id: sheet_id,
                sheet: Some(Sheet::with_size(rows, cols)),
            },
            Op::AddBlock {
                id: block_id,
                block: Block::Grid {
                    sheet_id,
                    view_bounds: None,
                },
            },
            Op::InsertChild {
                parent: root,
                index,
                child: block_id,
            },
        ];
        self.history
            .commit(&mut self.doc, ops)
            .map_err(|e| e.to_string())
    }

    /// After undo or redo, the caret may be past the end of its block, or its block may be gone.
    /// Move it back into range, or drop it.
    fn clamp_caret(&mut self) {
        let Some(sel) = self.caret else { return };
        self.caret = match self.doc.get(sel.block) {
            Some(Block::Text { content, .. }) => {
                let len = content.chars().count();
                Some(Selection::caret(sel.block, sel.focus.min(len)))
            }
            _ => None,
        };
    }

    /// Builds the UI tree for the current state.
    pub fn view(&self) -> UITree<Msg> {
        let children = match self.doc.get(self.doc.root) {
            Some(Block::Container { children, .. }) => children.clone(),
            _ => Vec::new(),
        };
        UITree::container(|c| {
            for id in children {
                self.view_block(c, id);
            }
            c.button("Undo").on_click(Msg::Undo);
            c.button("Redo").on_click(Msg::Redo);
            c.button("Insert table")
                .on_click(Msg::InsertTable { rows: 3, cols: 3 });
            if let Some(status) = &self.status {
                c.text(status.clone()).class("status");
            }
        })
    }

    fn view_block(&self, c: &mut ContainerBuilder<Msg>, id: BlockId) {
        let Some(block) = self.doc.get(id) else {
            return;
        };
        match block {
            Block::Text { content, style } => {
                let shown = match self.caret {
                    Some(sel) if sel.block == id && sel.is_collapsed() => {
                        let mut s: String = content.chars().take(sel.focus).collect();
                        s.push('|');
                        s.extend(content.chars().skip(sel.focus));
                        s
                    }
                    _ => content.clone(),
                };
                match style.heading_level {
                    Some(level) => c.heading(level, shown),
                    None => c.text(shown),
                }
                .on_click(Msg::Focus(id));
            }
            Block::List { items, ordered, .. } => {
                for (n, item) in items.iter().enumerate() {
                    if let Some(Block::Text { content, .. }) = self.doc.get(*item) {
                        let marker = if *ordered {
                            format!("{}. ", n + 1)
                        } else {
                            "• ".to_string()
                        };
                        c.text(format!("{marker}{content}"))
                            .on_click(Msg::Focus(*item));
                    }
                }
            }
            Block::Code { code, .. } => {
                c.text(code.clone()).class("code");
            }
            Block::ThematicBreak => {
                c.text("—————");
            }
            Block::Math { source } => {
                c.text(format!("$$ {source} $$")).class("math");
            }
            Block::Media { source, alt_text } => {
                if let MediaSource::Uri { uri } = source {
                    c.image(uri.clone(), alt_text.clone().unwrap_or_default());
                }
            }
            Block::Grid { sheet_id, .. } => match self.doc.sheets.get(sheet_id) {
                Some(sheet) => self.view_grid(c, *sheet_id, sheet),
                None => {
                    c.text("(missing sheet)").class("grid");
                }
            },
            Block::Query(query) => self.view_query(c, id, query),
            Block::Container { .. } => {}
        }
    }

    /// A query shows one line for each block it matches, or why it could not run. The query runs
    /// over a snapshot of the document, so it sees the current text.
    fn view_query(
        &self,
        c: &mut ContainerBuilder<Msg>,
        id: BlockId,
        query: &tpt_codex_core::Query,
    ) {
        let source = query.source.clone();
        c.input(source)
            .aria("label", "query source")
            .on_input(move |source| Msg::EditQuery { block: id, source })
            .class("query-source");
        let entities = document_entities(&self.doc);
        let json = match run_query(query, &entities) {
            Ok(json) => json,
            Err(e) => {
                c.text(e.to_string()).class("query query-error");
                return;
            }
        };
        let rows: Vec<serde_json::Value> = match serde_json::from_str(&json) {
            Ok(rows) => rows,
            Err(e) => {
                c.text(format!("query result is not a list of rows: {e}"))
                    .class("query query-error");
                return;
            }
        };
        if rows.is_empty() {
            c.text("(no matching blocks)").class("query");
            return;
        }
        for row in rows {
            let id = row.get("id").and_then(serde_json::Value::as_u64);
            let line = id
                .and_then(|id| self.doc.get(BlockId(id)))
                .map_or_else(|| "(unknown block)".to_string(), block_text);
            c.text(line).class("query query-row");
        }
    }

    /// A grid shows its computed values as a table, with an editable input for every cell in
    /// its extent, so that empty cells can be typed into.
    fn view_grid(
        &self,
        c: &mut ContainerBuilder<Msg>,
        sheet_id: SheetId,
        sheet: &tpt_codex_core::Sheet,
    ) {
        let (rows, cols) = sheet.extent();
        if rows == 0 || cols == 0 {
            c.text("(empty grid)").class("grid");
            return;
        }
        let results = evaluate_sheet(sheet);

        let mut columns = vec![String::new()];
        columns.extend((1..=cols).map(|col| column_letters(col as usize)));
        let table_rows: Vec<Vec<String>> = (1..=rows)
            .map(|row| {
                let mut line = vec![row.to_string()];
                for col in 1..=cols {
                    let address = tpt_codex_core::address_of(col, row);
                    line.push(results.get(&address).map(show).unwrap_or_default());
                }
                line
            })
            .collect();
        c.data_grid(columns, table_rows).class("grid-table");

        for row in 1..=rows {
            for col in 1..=cols {
                let address = tpt_codex_core::address_of(col, row);
                let raw = sheet.cells.get(&address).map(raw_text).unwrap_or_default();
                let address_for_msg = address.clone();
                let selected = self.cell
                    == Some(CellFocus {
                        sheet: sheet_id,
                        col,
                        row,
                    });
                let address_for_focus = address.clone();
                c.input(raw)
                    .aria("label", address.clone())
                    .on_input(move |text| Msg::EditCell {
                        sheet: sheet_id,
                        address: address_for_msg.clone(),
                        text,
                    })
                    .on_focus(Msg::FocusCell {
                        sheet: sheet_id,
                        address: address_for_focus,
                    })
                    .on_keydown(|key| arrow_of(&key.key).map(Msg::MoveCell))
                    .class(if selected {
                        "grid-cell selected"
                    } else {
                        "grid-cell"
                    });
            }
        }
    }
}

/// The largest number of rows or columns that [`Msg::InsertTable`] creates.
pub const MAX_TABLE_SIDE: u32 = 50;

const NO_CARET: &str = "no caret: click a paragraph first";

/// The arrow key a keydown reports (`ArrowUp` and so on), as a cell movement. Other keys are
/// left alone, so they still edit the cell.
pub fn arrow_of(key: &str) -> Option<Arrow> {
    match key {
        "ArrowUp" => Some(Arrow::Up),
        "ArrowDown" => Some(Arrow::Down),
        "ArrowLeft" => Some(Arrow::Left),
        "ArrowRight" => Some(Arrow::Right),
        _ => None,
    }
}

/// Turns what the user typed into a cell input.
///
/// Empty text clears the cell. A leading `=` is a formula, a finite number is a number,
/// `TRUE` or `FALSE` is a boolean, and anything else is text.
pub fn parse_cell_input(text: &str) -> Result<Option<CellInput>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    if text.starts_with('=') {
        if text.len() == 1 {
            return Err("a formula needs something after '='".to_string());
        }
        return Ok(Some(CellInput::Formula {
            source: text.to_string(),
        }));
    }
    if let Ok(value) = text.parse::<f64>() {
        if value.is_finite() {
            return Ok(Some(CellInput::Number { value }));
        }
    }
    match text {
        "TRUE" => Ok(Some(CellInput::Boolean { value: true })),
        "FALSE" => Ok(Some(CellInput::Boolean { value: false })),
        _ => Ok(Some(CellInput::Text {
            value: text.to_string(),
        })),
    }
}

/// What an input shows for a stored cell, so that editing it shows what was typed.
fn raw_text(input: &CellInput) -> String {
    match input {
        CellInput::Number { value } => format!("{value}"),
        CellInput::Text { value } => value.clone(),
        CellInput::Boolean { value } => if *value { "TRUE" } else { "FALSE" }.to_string(),
        CellInput::Formula { source } => source.clone(),
    }
}

/// The text a block shows when a query lists it.
fn block_text(block: &Block) -> String {
    match block {
        Block::Text { content, .. } => content.clone(),
        Block::Code { code, .. } => code.clone(),
        Block::Math { source } => format!("$$ {source} $$"),
        Block::List { .. } => "(list)".to_string(),
        Block::Grid { .. } => "(grid)".to_string(),
        Block::Media { .. } => "(media)".to_string(),
        Block::Query(_) => "(query)".to_string(),
        Block::ThematicBreak => "—————".to_string(),
        Block::Container { .. } => "(container)".to_string(),
    }
}

/// The column letters for a 1-based column number: 1 is `A`, 27 is `AA`.
fn column_letters(mut col: usize) -> String {
    let mut out = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        out.push(b'A' + rem as u8);
        col = (col - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII letters")
}

fn show(value: &CellResult) -> String {
    match value {
        CellResult::Empty => String::new(),
        CellResult::Number(n) => format!("{n}"),
        CellResult::Text(t) => t.clone(),
        CellResult::Boolean(b) => format!("{b}"),
        CellResult::Error(e) => format!("#ERR({e})"),
    }
}
