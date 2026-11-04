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
    SheetId, delete_backward, delete_forward, insert_text, is_valid_address,
};
use tpt_codex_grid::{CellResult, evaluate_sheet};

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
    /// Undo the last step.
    Undo,
    /// Redo the last undone step.
    Redo,
}

/// A document open for editing.
#[derive(Debug)]
pub struct Editor {
    doc: Document,
    history: History,
    caret: Option<Selection>,
    status: Option<String>,
}

impl Editor {
    /// Opens `doc` with no caret.
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            history: History::new(),
            caret: None,
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
                Ok(())
            }
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
            Block::Query(_) => {
                c.text("(query)").class("query");
            }
            Block::Container { .. } => {}
        }
    }

    /// A grid shows its computed values as a table, and one editable input per non-empty cell.
    fn view_grid(
        &self,
        c: &mut ContainerBuilder<Msg>,
        sheet_id: SheetId,
        sheet: &tpt_codex_core::Sheet,
    ) {
        if sheet.cells.is_empty() {
            c.text("(empty grid)").class("grid");
            return;
        }
        let results = evaluate_sheet(sheet);
        let (max_col, max_row) = sheet
            .cells
            .keys()
            .filter_map(|a| split_address(a))
            .fold((0, 0), |(c, r), (col, row)| (c.max(col), r.max(row)));

        let mut columns = vec![String::new()];
        columns.extend((1..=max_col).map(column_letters));
        let rows: Vec<Vec<String>> = (1..=max_row)
            .map(|row| {
                let mut line = vec![row.to_string()];
                for col in 1..=max_col {
                    let address = format!("{}{row}", column_letters(col));
                    line.push(results.get(&address).map(show).unwrap_or_default());
                }
                line
            })
            .collect();
        c.data_grid(columns, rows).class("grid-table");

        for (address, input) in &sheet.cells {
            let raw = raw_text(input);
            let address_for_msg = address.clone();
            c.input(raw)
                .aria("label", address.clone())
                .on_input(move |text| Msg::EditCell {
                    sheet: sheet_id,
                    address: address_for_msg.clone(),
                    text,
                })
                .class("grid-cell");
        }
    }
}

const NO_CARET: &str = "no caret: click a paragraph first";

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

/// Splits a canonical A1 address into `(column, row)`, both 1-based.
fn split_address(address: &str) -> Option<(usize, usize)> {
    if !is_valid_address(address) {
        return None;
    }
    let letters = address.bytes().take_while(u8::is_ascii_uppercase).count();
    let col = address[..letters]
        .bytes()
        .fold(0usize, |acc, b| acc * 26 + usize::from(b - b'A' + 1));
    let row: usize = address[letters..].parse().ok()?;
    Some((col, row))
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
