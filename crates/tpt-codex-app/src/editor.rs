// SPDX-License-Identifier: MIT OR Apache-2.0
//! The editor's state and view, without a browser.
//!
//! [`Editor::update`] applies a [`Msg`] to the document, and [`Editor::view`] builds the
//! `tpt-appfront` UI tree for it. The tree renders to semantic HTML with `tpt-appfront-html`.
//! Keeping the logic here lets it run and be tested natively.

use tpt_appfront_core::UITree;
use tpt_appfront_core::ui_tree::ContainerBuilder;
use tpt_codex_core::{
    Block, BlockId, Canvas, CellInput, Document, EditError, History, InlineRun, LayoutMode,
    MediaSource, Movement, Op, Rect, Selection, Sheet, SheetId, TextStyle, delete_backward,
    delete_forward, edit_text, insert_text, is_valid_address,
};
use tpt_codex_grid::{CellResult, evaluate_sheet};
use tpt_codex_views::{
    FlowKind, FlowOptions, PageChrome, PageGeometry, TextMeasure, layout_flow, paginate,
    render_chrome,
};
use tpt_typeset::{document_entities, run_query};

/// Something the user did.
#[derive(Debug, Clone, PartialEq)]
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
    /// Show slide `index` (0-based) in presenter mode. Out-of-range indices are clamped.
    StartPresenting {
        /// The slide to start on.
        index: usize,
    },
    /// Leave presenter mode and show the whole document.
    StopPresenting,
    /// Show the document as pages (`true`) or as the editable flow (`false`).
    SetPaginated {
        /// Whether to show pages.
        on: bool,
    },
    /// Mark the presenter stage as full screen. The host makes the browser go full screen when
    /// this changes; the editor only records it, and draws the stage with a `fullscreen` class.
    SetFullscreen {
        /// Whether the stage is full screen.
        on: bool,
    },
    /// Show the next slide in presenter mode. The last slide stays on screen.
    NextSlide,
    /// Show the previous slide in presenter mode. The first slide stays on screen.
    PreviousSlide,
    /// Choose the size of the next table. Each side must be between 1 and [`MAX_TABLE_SIDE`].
    SetTableSize {
        /// Number of rows.
        rows: u32,
        /// Number of columns.
        cols: u32,
    },
    /// Replace the whole text of paragraph `block` with `text`, as typed into an input element.
    /// Only the characters that differ are changed, as one undoable step, and the caret goes to
    /// the end of the change.
    ReplaceText {
        /// The text block.
        block: BlockId,
        /// Its new text.
        text: String,
    },
    /// Add an empty slide (16:9) at the end of the document, as one undoable step.
    InsertSlide,
    /// Place `block` on the slide that holds it, at `frame`. `None` removes its position.
    PlaceBlock {
        /// The block to place. It must be a child of a slide.
        block: BlockId,
        /// Where it goes, in points.
        frame: Option<Rect>,
    },
    /// Move a block on its slide by (`dx`, `dy`) points. It stays inside the slide.
    MoveBlock {
        /// The block to move. It must have a position on a slide.
        block: BlockId,
        /// Horizontal distance, in points.
        dx: f64,
        /// Vertical distance, in points.
        dy: f64,
    },
    /// Give a block on its slide a new size, keeping its top-left corner. It stays inside the slide.
    ResizeBlock {
        /// The block to resize. It must have a position on a slide.
        block: BlockId,
        /// New width, in points.
        width: f64,
        /// New height, in points.
        height: f64,
    },
    /// Make a query block show the blocks whose `field` equals `value`. The fields are `kind`,
    /// `content` and `language`. The query is checked like any other source edit.
    FilterQuery {
        /// The query block to set.
        block: BlockId,
        /// The field to test: `kind`, `content` or `language`.
        field: String,
        /// The text the field must equal.
        value: String,
    },
    /// Move slide `from` (0-based, in slide order) so that it becomes slide `to`. As one undoable
    /// step.
    MoveSlide {
        /// The slide to move.
        from: usize,
        /// Its new position among the slides.
        to: usize,
    },
    /// Put slide `slide` in section `name`. An empty name takes it out of its section.
    SetSection {
        /// The slide.
        slide: BlockId,
        /// The section name, or empty for none.
        name: String,
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
    /// Rows and columns of the next table inserted.
    table_size: (u32, u32),
    /// The slide being shown in presenter mode, as an index into the slides. `None` shows the
    /// whole document.
    presenting: Option<usize>,
    /// Whether the document is shown as pages rather than as the editable flow.
    paginated: bool,
    /// Whether the presenter stage is full screen.
    fullscreen: bool,
    /// Other people's carets, drawn in their paragraphs.
    remote_cursors: Vec<RemoteCursor>,
}

/// Another replica's caret, to draw in a paragraph. The label names the replica's author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCursor {
    /// Shown next to the caret.
    pub label: String,
    /// The paragraph it is in.
    pub block: BlockId,
    /// Character offset in the paragraph.
    pub offset: usize,
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
            table_size: (3, 3),
            presenting: None,
            paginated: false,
            fullscreen: false,
            remote_cursors: Vec::new(),
        }
    }

    /// The rows and columns the next inserted table will have.
    pub fn table_size(&self) -> (u32, u32) {
        self.table_size
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
            Msg::SetTableSize { rows, cols } => self.set_table_size(rows, cols),
            Msg::InsertSlide => self.insert_slide(),
            Msg::StartPresenting { index } => {
                let count = self.slide_ids().len();
                if count == 0 {
                    Err("there are no slides to present".to_string())
                } else {
                    self.presenting = Some(index.min(count - 1));
                    Ok(())
                }
            }
            Msg::SetPaginated { on } => {
                self.paginated = on;
                Ok(())
            }
            Msg::StopPresenting => {
                self.presenting = None;
                self.fullscreen = false;
                Ok(())
            }
            Msg::SetFullscreen { on } => {
                if self.presenting.is_none() && on {
                    Err("start presenting before going full screen".to_string())
                } else {
                    self.fullscreen = on;
                    Ok(())
                }
            }
            Msg::NextSlide => self.move_slide(1),
            Msg::PreviousSlide => self.move_slide(-1),
            Msg::ReplaceText { block, text } => self.replace_text(block, &text),
            Msg::PlaceBlock { block, frame } => self.place_block(block, frame),
            Msg::MoveBlock { block, dx, dy } => self.move_block(block, dx, dy),
            Msg::MoveSlide { from, to } => self.move_slide_to(from, to),
            Msg::SetSection { slide, name } => self.set_section(slide, &name),
            Msg::FilterQuery {
                block,
                field,
                value,
            } => self.filter_query(block, &field, &value),
            Msg::ResizeBlock {
                block,
                width,
                height,
            } => self.resize_block(block, width, height),
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
        // The paragraph caret is kept. Clearing it would re-draw the paragraph, and the browser
        // would lose focus from the cell being selected.
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

    fn replace_text(&mut self, block: BlockId, text: &str) -> Result<(), String> {
        let content = match self.doc.get(block) {
            Some(Block::Text { content, .. }) => content.clone(),
            Some(_) => return Err("that block is not text".to_string()),
            None => return Err(format!("block {} does not exist", block.0)),
        };
        let old: Vec<char> = content.chars().collect();
        let new: Vec<char> = text.chars().collect();
        // The change is the span between the common prefix and the common suffix.
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let max_suffix = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(max_suffix)
            .take_while(|(a, b)| a == b)
            .count();
        if prefix == old.len() && prefix == new.len() {
            return Ok(());
        }
        let insert: String = new[prefix..new.len() - suffix].iter().collect();
        let end = old.len() - suffix;
        edit_text(
            &mut self.doc,
            &mut self.history,
            block,
            prefix,
            end,
            &insert,
        )
        .map_err(|e| e.to_string())?;
        self.caret = Some(Selection::caret(block, prefix + insert.chars().count()));
        self.cell = None;
        Ok(())
    }

    /// Replaces the other people's carets shown in the document.
    pub fn set_remote_cursors(&mut self, cursors: Vec<RemoteCursor>) {
        self.remote_cursors = cursors;
    }

    /// The slides of the document, in order.
    pub fn slide_ids(&self) -> Vec<BlockId> {
        tpt_codex_views::slide_ids(&self.doc)
    }

    /// The slide being presented, if presenter mode is on.
    pub fn presenting(&self) -> Option<usize> {
        self.presenting
    }

    fn set_section(&mut self, slide: BlockId, name: &str) -> Result<(), String> {
        let name = (!name.is_empty()).then(|| name.to_string());
        self.history
            .commit(&mut self.doc, vec![Op::SetSection { id: slide, name }])
            .map_err(|e| e.to_string())
    }

    fn move_slide_to(&mut self, from: usize, to: usize) -> Result<(), String> {
        let slides = self.slide_ids();
        if from >= slides.len() || to >= slides.len() {
            return Err(format!("there are {} slides", slides.len()));
        }
        let root = self.doc.root;
        let Some(Block::Container { children, .. }) = self.doc.get(root) else {
            return Err("the document root cannot hold slides".to_string());
        };
        let moved = slides[from];
        let from_index = children
            .iter()
            .position(|c| *c == moved)
            .ok_or_else(|| "slide is not in the document root".to_string())?;
        // Without the moved slide, insert it straight after the slide that should precede it.
        let mut reduced = children.clone();
        reduced.remove(from_index);
        let slide_positions: Vec<usize> = reduced
            .iter()
            .enumerate()
            .filter(|(_, id)| slides.contains(id) && **id != moved)
            .map(|(i, _)| i)
            .collect();
        let to_index = if to == 0 {
            0
        } else {
            slide_positions[to - 1] + 1
        };
        if from_index == to_index || (from == to) {
            return Ok(());
        }
        self.history
            .commit(
                &mut self.doc,
                vec![Op::MoveChild {
                    from_parent: root,
                    from_index,
                    to_parent: root,
                    to_index,
                }],
            )
            .map_err(|e| e.to_string())
    }

    fn move_slide(&mut self, step: isize) -> Result<(), String> {
        let count = self.slide_ids().len();
        let Some(current) = self.presenting else {
            return Err("presenter mode is off".to_string());
        };
        if count == 0 {
            return Err("there are no slides to present".to_string());
        }
        let next = (current as isize + step).clamp(0, count as isize - 1);
        self.presenting = Some(next as usize);
        Ok(())
    }

    fn insert_slide(&mut self) -> Result<(), String> {
        let root = self.doc.root;
        let Some(Block::Container { children, .. }) = self.doc.get(root) else {
            return Err("the document root cannot hold a slide".to_string());
        };
        let index = children.len();
        let slide = BlockId(
            self.doc
                .blocks
                .keys()
                .map(|b| b.0)
                .max()
                .map_or(1, |m| m + 1),
        );
        let ops = vec![
            Op::AddBlock {
                id: slide,
                block: Block::Container {
                    layout_mode: LayoutMode::Absolute,
                    children: Vec::new(),
                },
            },
            Op::InsertChild {
                parent: root,
                index,
                child: slide,
            },
            Op::SetCanvas {
                id: slide,
                canvas: Some(Canvas::WIDESCREEN),
            },
        ];
        self.history
            .commit(&mut self.doc, ops)
            .map_err(|e| e.to_string())
    }

    /// The slide that holds `block`, if it is a child of an absolute container.
    fn slide_of(&self, block: BlockId) -> Option<BlockId> {
        self.doc.blocks.iter().find_map(|(id, b)| match b {
            Block::Container {
                layout_mode: LayoutMode::Absolute,
                children,
            } if children.contains(&block) => Some(*id),
            _ => None,
        })
    }

    /// The current frame of `block`, and the size of the slide it is on.
    fn placed(&self, block: BlockId) -> Result<(Rect, Canvas), String> {
        let frame = self
            .doc
            .frames
            .get(&block)
            .copied()
            .ok_or_else(|| format!("block {} has no position on a slide", block.0))?;
        let slide = self
            .slide_of(block)
            .ok_or_else(|| format!("block {} is not on a slide", block.0))?;
        let canvas = self
            .doc
            .canvases
            .get(&slide)
            .copied()
            .unwrap_or(Canvas::WIDESCREEN);
        Ok((frame, canvas))
    }

    fn filter_query(&mut self, block: BlockId, field: &str, value: &str) -> Result<(), String> {
        if !matches!(field, "kind" | "content" | "language") {
            return Err(format!("'{field}' cannot be filtered on"));
        }
        let source = serde_json::json!({
            "source": 1,
            "filters": [{ "Eq": { "field": field, "value": value } }],
        })
        .to_string();
        self.edit_query(block, source)
    }

    fn move_block(&mut self, block: BlockId, dx: f64, dy: f64) -> Result<(), String> {
        let (frame, canvas) = self.placed(block)?;
        if !dx.is_finite() || !dy.is_finite() {
            return Err("a move needs finite distances".to_string());
        }
        // Keep the whole block on the slide, even if it is larger than the slide.
        let x = clamp_to(frame.x + dx, 0.0, canvas.width - frame.width);
        let y = clamp_to(frame.y + dy, 0.0, canvas.height - frame.height);
        self.place_block(block, Some(Rect { x, y, ..frame }))
    }

    fn resize_block(&mut self, block: BlockId, width: f64, height: f64) -> Result<(), String> {
        let (frame, canvas) = self.placed(block)?;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err("a size needs positive, finite width and height".to_string());
        }
        // Keep the block's top-left corner, and stop at the slide's edges.
        let width = width.min(canvas.width - frame.x).max(1.0);
        let height = height.min(canvas.height - frame.y).max(1.0);
        self.place_block(
            block,
            Some(Rect {
                width,
                height,
                ..frame
            }),
        )
    }

    fn place_block(&mut self, block: BlockId, frame: Option<Rect>) -> Result<(), String> {
        self.history
            .commit(&mut self.doc, vec![Op::SetFrame { id: block, frame }])
            .map_err(|e| e.to_string())
    }

    fn set_table_size(&mut self, rows: u32, cols: u32) -> Result<(), String> {
        if !(1..=MAX_TABLE_SIDE).contains(&rows) || !(1..=MAX_TABLE_SIDE).contains(&cols) {
            return Err(format!(
                "a table needs between 1 and {MAX_TABLE_SIDE} rows and columns"
            ));
        }
        self.table_size = (rows, cols);
        Ok(())
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
    /// The document as pages, cut to the page size and margins of its document class (A4 when it
    /// has none).
    pub fn paginated_view_for_class(
        &self,
        measure: &dyn TextMeasure,
        flow: FlowOptions,
    ) -> Result<UITree<Msg>, String> {
        let spec = tpt_typeset::class_of(&self.doc)
            .unwrap_or(tpt_typeset::DocumentClass::Article)
            .spec();
        let geometry = PageGeometry {
            height: spec.page_height,
            margin_top: spec.margin,
            margin_bottom: spec.margin,
            min_lines: spec.min_lines,
        };
        self.paginated_view(measure, flow, geometry)
    }

    /// The document as pages. Each page is a container, with its lines, boxes and grid rows in
    /// order. Layout uses `measure` and `flow`, and pages are cut to `geometry`.
    pub fn paginated_view(
        &self,
        measure: &dyn TextMeasure,
        flow: FlowOptions,
        geometry: PageGeometry,
    ) -> Result<UITree<Msg>, String> {
        let chrome = PageChrome {
            header: None,
            footer: Some("Page {page} of {pages}".to_string()),
        };
        self.paginated_view_with(measure, flow, geometry, &chrome)
    }

    /// The paginated view with headers and footers. `{page}` and `{pages}` in them are the page
    /// number and the page count.
    pub fn paginated_view_with(
        &self,
        measure: &dyn TextMeasure,
        flow: FlowOptions,
        geometry: PageGeometry,
        chrome: &PageChrome,
    ) -> Result<UITree<Msg>, String> {
        let layout = layout_flow(&self.doc, measure, flow).map_err(|e| e.to_string())?;
        let pages = paginate(&layout, geometry).map_err(|e| e.to_string())?;
        let rendered = render_chrome(&pages, chrome);
        Ok(UITree::container(|c| {
            for (page, marks) in pages.iter().zip(&rendered) {
                c.container(|p| {
                    if let Some(header) = &marks.header {
                        styled_text(p, header.clone(), "page-header");
                    }
                    p.heading(3, format!("Page {}", page.number))
                        .class("page-number");
                    for item in &page.items {
                        match &item.kind {
                            FlowKind::Line { text, .. } => {
                                p.container(|s| {
                                    s.text(text.clone());
                                })
                                .class("page-line")
                                .on_click(Msg::Focus(item.block));
                            }
                            FlowKind::Box => {
                                styled_text(p, "(block)", "page-box");
                            }
                            FlowKind::GridRow { row, header, .. } => {
                                let class = if *header { "grid-header" } else { "grid-row" };
                                styled_text(p, format!("row {row}"), class);
                            }
                        }
                    }
                    if let Some(footer) = &marks.footer {
                        styled_text(p, footer.clone(), "page-footer");
                    }
                })
                .class("page");
            }
        }))
    }

    /// The whole document as the editor shows it: the root's blocks, with a status line and the
    /// editing controls. In presenter mode, the current slide only.
    pub fn view(&self) -> UITree<Msg> {
        if let Some(index) = self.presenting {
            return self.view_presenter(index);
        }
        if self.paginated {
            // A4-like page of 450 points of text width, with 20-point rows and a footer.
            let measure = tpt_codex_views::MonospaceMeasure {
                char_width: 6.0,
                line_height: 14.0,
            };
            let flow = FlowOptions {
                width: 450.0,
                block_spacing: 8.0,
                box_height: 40.0,
                row_height: 20.0,
            };
            return match self.paginated_view_for_class(&measure, flow) {
                Ok(pages) => UITree::container(|c| {
                    c.button("Back to editing")
                        .on_click(Msg::SetPaginated { on: false });
                    c.with(pages);
                }),
                Err(message) => UITree::container(|c| {
                    c.button("Back to editing")
                        .on_click(Msg::SetPaginated { on: false });
                    styled_text(c, message, "status");
                }),
            };
        }
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
            let (rows, cols) = self.table_size;
            c.input(rows.to_string())
                .aria("label", "table rows")
                .on_input(move |text| Msg::SetTableSize {
                    rows: text.trim().parse().unwrap_or(0),
                    cols,
                })
                .class("table-rows");
            c.input(cols.to_string())
                .aria("label", "table columns")
                .on_input(move |text| Msg::SetTableSize {
                    rows,
                    cols: text.trim().parse().unwrap_or(0),
                })
                .class("table-cols");
            c.button("Insert table")
                .on_click(Msg::InsertTable { rows, cols });
            c.button("Insert slide").on_click(Msg::InsertSlide);
            c.button("Pages").on_click(Msg::SetPaginated { on: true });
            c.button("Present slides")
                .on_click(Msg::StartPresenting { index: 0 });
            if let Some(status) = &self.status {
                styled_text(c, status.clone(), "status");
            }
        })
    }

    /// One paragraph or heading: a text area while the caret is in it, otherwise its text with
    /// marks and remote carets.
    fn view_paragraph(
        &self,
        c: &mut ContainerBuilder<Msg>,
        id: BlockId,
        content: &str,
        style: &TextStyle,
    ) {
        if self.has_caret_in(id) {
            // The paragraph being typed in is a native text area, so the browser handles
            // typing, selection and input methods. Each change becomes one undoable edit.
            // Keyed, so that the browser keeps this element (and its focus) across renders.
            c.textarea(content.to_string())
                .key(format!("paragraph-{}", id.0))
                .aria("label", "paragraph")
                .on_input(move |text| Msg::ReplaceText { block: id, text })
                .class("paragraph-input");
            return;
        }

        let mut markers = Vec::new();
        if let Some(sel) = self.caret {
            if sel.block == id && sel.is_collapsed() {
                markers.push(Marker {
                    at: sel.focus,
                    text: "|".to_string(),
                    kind: MarkerKind::Caret,
                });
            }
        }
        for remote in self.remote_cursors.iter().filter(|r| r.block == id) {
            markers.push(Marker {
                at: remote.offset,
                text: format!("[{}]", remote.label),
                kind: MarkerKind::Remote,
            });
        }
        // Headings are shown as plain text: their marks are not drawn.
        // A paragraph with marks, or with another person's caret, is drawn as containers.
        if style.heading_level.is_none() && (!style.runs.is_empty() || !markers.is_empty()) {
            c.container(|p| {
                for seg in inline_segments(content, &style.runs, &markers) {
                    segment_node(p, seg);
                }
            })
            .class("para")
            .on_click(Msg::Focus(id));
            return;
        }
        let shown = plain_with_markers(content, &markers);
        match style.heading_level {
            Some(level) => c.heading(level, shown),
            None => c.text(shown),
        }
        .on_click(Msg::Focus(id));
    }

    /// Whether the caret is collapsed in block `id`.
    fn has_caret_in(&self, id: BlockId) -> bool {
        self.caret
            .is_some_and(|sel| sel.block == id && sel.is_collapsed())
    }

    fn view_block(&self, c: &mut ContainerBuilder<Msg>, id: BlockId) {
        let Some(block) = self.doc.get(id) else {
            return;
        };
        match block {
            Block::Text { content, style } => {
                // Every paragraph is a container with a fixed type. Its contents change with the
                // caret (text area, text, marks), so the rest of the document keeps its place.
                c.container(|inner| self.view_paragraph(inner, id, content, style))
                    .class("text-block");
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
                styled_text(c, code.clone(), "code");
            }
            Block::ThematicBreak => {
                c.text("—————");
            }
            Block::Math { source } => {
                styled_text(c, format!("$$ {source} $$"), "math");
            }
            Block::Media { source, alt_text } => {
                if let MediaSource::Uri { uri } = source {
                    c.image(uri.clone(), alt_text.clone().unwrap_or_default());
                }
            }
            Block::Grid {
                sheet_id,
                view_bounds,
            } => match self.doc.sheets.get(sheet_id) {
                Some(sheet) => self.view_grid(c, *sheet_id, sheet, *view_bounds),
                None => {
                    styled_text(c, "(missing sheet)", "grid");
                }
            },
            Block::Query(query) => self.view_query(c, id, query),
            Block::Container {
                layout_mode: LayoutMode::Absolute,
                children,
            } => self.view_slide(c, id, children),
            Block::Container {
                layout_mode,
                children,
            } => {
                // Flow and grid containers stack their children in order. The class says which
                // layout the host should give them.
                let class = match layout_mode {
                    LayoutMode::Grid => "container-grid",
                    _ => "container-flow",
                };
                c.container(|inner| {
                    for &child in children {
                        self.view_block(inner, child);
                    }
                })
                .class(class);
            }
        }
    }

    /// Presenter mode: one slide, with controls. Shows a status line if there are no slides.
    fn view_presenter(&self, index: usize) -> UITree<Msg> {
        let slides = self.slide_ids();
        UITree::container(|c| {
            match slides.get(index) {
                Some(&slide) => {
                    c.container(|stage| self.view_block(stage, slide))
                        .class(if self.fullscreen {
                            "presenter fullscreen"
                        } else {
                            "presenter"
                        });
                    let section = self
                        .doc
                        .sections
                        .get(&slide)
                        .map_or(String::new(), |name| format!("{name} · "));
                    styled_text(
                        c,
                        format!("{section}slide {} of {}", index + 1, slides.len()),
                        "presenter-count",
                    );
                }
                None => {
                    styled_text(c, "there are no slides to present", "status");
                }
            }
            c.button("Previous").on_click(Msg::PreviousSlide);
            c.button("Next").on_click(Msg::NextSlide);
            c.button("Stop presenting").on_click(Msg::StopPresenting);
        })
    }

    /// A slide: its size is kept as the aspect ratio, and each child is placed at its frame.
    /// Children without a frame are listed below the slide.
    fn view_slide(&self, c: &mut ContainerBuilder<Msg>, id: BlockId, children: &[BlockId]) {
        let canvas = self
            .doc
            .canvases
            .get(&id)
            .copied()
            .unwrap_or(Canvas::WIDESCREEN);
        let aspect = format!("aspect-ratio:{} / {};", canvas.width, canvas.height);
        c.container(|slide| {
            for &child in children {
                match self.doc.frames.get(&child) {
                    Some(frame) => {
                        let style = format!(
                            "{aspect} position:absolute; left:{}px; top:{}px; width:{}px; height:{}px;",
                            frame.x, frame.y, frame.width, frame.height
                        );
                        slide.container(|wrap| self.view_block(wrap, child)).attr("style", style);
                    }
                    None => self.view_block(slide, child),
                }
            }
        })
        .class("slide")
        .attr("style", format!("{aspect} position:relative; width:100%;"));
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
                styled_text(c, e.to_string(), "query query-error");
                return;
            }
        };
        let rows: Vec<serde_json::Value> = match serde_json::from_str(&json) {
            Ok(rows) => rows,
            Err(e) => {
                styled_text(
                    c,
                    format!("query result is not a list of rows: {e}"),
                    "query query-error",
                );
                return;
            }
        };
        if rows.is_empty() {
            styled_text(c, "(no matching blocks)", "query");
            return;
        }
        for row in rows {
            let id = row.get("id").and_then(serde_json::Value::as_u64);
            let line = id
                .and_then(|id| self.doc.get(BlockId(id)))
                .map_or_else(|| "(unknown block)".to_string(), block_text);
            styled_text(c, line, "query query-row");
        }
    }

    /// A grid shows its computed values as a table, with an editable input for every cell in
    /// its extent, so that empty cells can be typed into.
    fn view_grid(
        &self,
        c: &mut ContainerBuilder<Msg>,
        sheet_id: SheetId,
        sheet: &tpt_codex_core::Sheet,
        view_bounds: Option<tpt_codex_core::Rect>,
    ) {
        let (rows, cols) = sheet.extent();
        if rows == 0 || cols == 0 {
            styled_text(c, "(empty grid)", "grid");
            return;
        }
        let results = evaluate_sheet(sheet);
        // The rows shown are the same as in the flow: the header rows, and the window's rows.
        let shown =
            tpt_codex_views::grid_rows_shown(rows, sheet.header_rows, view_bounds, GRID_ROW_POINTS);

        let mut columns = vec![String::new()];
        columns.extend((1..=cols).map(|col| column_letters(col as usize)));
        let table_rows: Vec<Vec<String>> = shown
            .iter()
            .map(|&row| {
                let mut line = vec![row.to_string()];
                for col in 1..=cols {
                    let address = tpt_codex_core::address_of(col, row);
                    line.push(results.get(&address).map(show).unwrap_or_default());
                }
                line
            })
            .collect();
        c.data_grid(columns, table_rows).class("grid-table");

        for &row in &shown {
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
                // Keyed by address, so that focus stays on a cell when the table re-renders.
                c.input(raw)
                    .key(format!("cell-{}-{}", sheet_id.0, address))
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

/// Something drawn inside a paragraph at a character offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Marker {
    /// Character offset it is drawn before.
    pub(crate) at: usize,
    /// What is drawn.
    pub(crate) text: String,
    /// What it stands for.
    pub(crate) kind: MarkerKind,
}

/// The kinds of marker a paragraph can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkerKind {
    /// This editor's caret.
    Caret,
    /// Another person's caret.
    Remote,
}

/// A stretch of a paragraph that shares its marks.
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    text: String,
    strong: bool,
    italic: bool,
    link: Option<String>,
    /// The marker this segment is, if it is one.
    marker: Option<MarkerKind>,
    /// Inline math: the text is the formula's source.
    math: bool,
}

/// A text line with a class. The browser applies classes to containers, not to text nodes, so the
/// text is wrapped in a container that carries the class.
fn styled_text(c: &mut ContainerBuilder<Msg>, text: impl Into<String>, class: &str) {
    let text = text.into();
    c.container(|s| {
        s.text(text);
    })
    .class(class);
}

/// The height of one grid row in the flow and the paginated view, in points.
const GRID_ROW_POINTS: f64 = 20.0;

/// `value` limited to `low..=high`. If the range is empty (`high < low`), the result is `low`.
fn clamp_to(value: f64, low: f64, high: f64) -> f64 {
    if high < low {
        low
    } else {
        value.clamp(low, high)
    }
}

/// The paragraph as plain text, with markers drawn in. Markers at the same offset keep their
/// order.
fn plain_with_markers(content: &str, markers: &[Marker]) -> String {
    let mut out = String::new();
    let mut sorted: Vec<&Marker> = markers.iter().collect();
    sorted.sort_by_key(|m| m.at);
    let mut next = sorted.iter().peekable();
    for (i, ch) in content.chars().enumerate() {
        while let Some(m) = next.next_if(|m| m.at <= i) {
            out.push_str(&m.text);
        }
        out.push(ch);
    }
    for m in next {
        out.push_str(&m.text);
    }
    out
}

/// Splits `content` into segments with the same marks, with markers as segments of their own.
fn inline_segments(content: &str, runs: &[InlineRun], markers: &[Marker]) -> Vec<Segment> {
    type Char<'a> = (char, bool, bool, Option<&'a str>, Option<MarkerKind>, bool);
    let mut chars: Vec<Char<'_>> = content
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let (mut strong, mut italic, mut link, mut math) = (false, false, None, false);
            for run in runs.iter().filter(|r| i >= r.start && i < r.end) {
                strong |= run.bold;
                italic |= run.italic;
                math |= run.math.is_some();
                if run.link.is_some() {
                    link = run.link.as_deref();
                }
            }
            (ch, strong, italic, link, None, math)
        })
        .collect();
    // Insert from the end, so earlier offsets stay valid.
    let mut sorted: Vec<&Marker> = markers.iter().collect();
    sorted.sort_by_key(|m| m.at);
    for marker in sorted.iter().rev() {
        let at = marker.at.min(chars.len());
        for (k, ch) in marker.text.chars().enumerate() {
            chars.insert(at + k, (ch, false, false, None, Some(marker.kind), false));
        }
    }
    let mut out: Vec<Segment> = Vec::new();
    for (ch, strong, italic, link, marker, math) in chars {
        let link = link.map(String::from);
        match out.last_mut() {
            Some(seg)
                if seg.strong == strong
                    && seg.italic == italic
                    && seg.link == link
                    && seg.marker == marker
                    && seg.math == math =>
            {
                seg.text.push(ch);
            }
            _ => out.push(Segment {
                text: ch.to_string(),
                strong,
                italic,
                link,
                marker,
                math,
            }),
        }
    }
    out
}

/// Draws one segment. Links show their address as a tooltip and have no `href`: the address is
/// text the user typed, and a `javascript:` address must not become a live link.
fn segment_node(p: &mut ContainerBuilder<Msg>, seg: Segment) {
    let class = if seg.marker == Some(MarkerKind::Caret) {
        "caret"
    } else if seg.marker == Some(MarkerKind::Remote) {
        "remote-cursor"
    } else if seg.math {
        "math-inline"
    } else {
        match (seg.strong, seg.italic) {
            (true, true) => "strong em",
            (true, false) => "strong",
            (false, true) => "em",
            (false, false) => "plain",
        }
    };
    // The browser renders classes on containers, not on text nodes, so each stretch is a small
    // container. The host's stylesheet makes these inline (see the demo page).
    let text = seg.text;
    match seg.link {
        Some(address) => {
            p.container(|s| {
                s.text(text);
            })
            .class(format!("{class} link"))
            .attr("title", address);
        }
        None => {
            p.container(|s| {
                s.text(text);
            })
            .class(class);
        }
    }
}

#[cfg(test)]
mod inline_view_tests {
    use super::*;

    fn run(start: usize, end: usize, bold: bool, italic: bool, link: Option<&str>) -> InlineRun {
        InlineRun {
            start,
            end,
            bold,
            italic,
            link: link.map(String::from),
            math: None,
        }
    }

    #[test]
    fn segments_follow_the_runs() {
        let segs = inline_segments(
            "ab cd",
            &[
                run(0, 2, true, false, None),
                run(3, 5, false, false, Some("u")),
            ],
            &[],
        );
        let texts: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["ab", " ", "cd"]);
        assert!(segs[0].strong);
        assert_eq!(segs[2].link.as_deref(), Some("u"));
    }

    #[test]
    fn the_caret_is_its_own_segment() {
        let caret = [Marker {
            at: 1,
            text: "|".into(),
            kind: MarkerKind::Caret,
        }];
        let segs = inline_segments("abc", &[run(0, 3, true, false, None)], &caret);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["a", "|", "bc"]);
        assert_eq!(segs[1].marker, Some(MarkerKind::Caret));
        assert!(segs[0].strong && segs[2].strong && !segs[1].strong);
    }
}
