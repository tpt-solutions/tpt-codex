// SPDX-License-Identifier: MIT OR Apache-2.0
//! The structure tree of a tagged PDF: which marked content is a paragraph, a heading, a list item,
//! a table cell or a figure, in reading order.
//!
//! Every tagged piece of text, or figure, has a marked-content identifier (MCID) on its page. The
//! tree names each identifier through an element, so a reader can say what each part of the page
//! is. Lists are an `L` element with an `LI` per item, and each item's text is an `LBody`. Grids
//! are a `Table` with a `TR` per row and a `TD` (or `TH` for a header row) per cell.

use crate::text_layer::{Item, Role};

/// One element of the tree, before its object number is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The structure type: `P`, `H1` to `H6`, `L`, `LI`, `LBody`, `Table`, `TR`, `TD`, `TH` or
    /// `Figure`.
    pub kind: String,
    /// Alternative text for a figure.
    pub alt: Option<String>,
    /// The page the element's marked content is on.
    pub page: usize,
    /// The marked-content identifiers the element holds, on its page.
    pub mcids: Vec<u32>,
    /// The elements inside this one, in order.
    pub children: Vec<usize>,
    /// The object number the element is written as. Set by the writer.
    pub number: u64,
}

/// How one item of a page is marked in its content stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// Not marked: the text is drawn with no marks.
    None,
    /// An artifact: page furniture a reader skips.
    Artifact,
    /// Tagged content with this identifier, under this marked-content tag.
    Tagged {
        /// The identifier, unique on its page.
        mcid: u32,
        /// The tag the content is marked with.
        tag: String,
    },
}

/// The structure of a whole file.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// Every element. Indices refer to this list.
    pub nodes: Vec<Node>,
    /// The elements directly under the document, in reading order.
    pub top: Vec<usize>,
    /// The mark of each item of each page, in the page's order.
    pub marks: Vec<Vec<Mark>>,
    /// For each page, the element that holds each of its identifiers, by identifier.
    pub owners: Vec<Vec<usize>>,
}

impl Tree {
    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn new_node(&mut self, kind: &str, page: usize) -> usize {
        self.push(Node {
            kind: kind.into(),
            alt: None,
            page,
            mcids: Vec::new(),
            children: Vec::new(),
            number: 0,
        })
    }

    /// The element each element sits in: `None` for the top level.
    pub fn parents(&self) -> Vec<Option<usize>> {
        let mut parents = vec![None; self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            for &child in &node.children {
                parents[child] = Some(index);
            }
        }
        parents
    }
}

/// The open elements of a page, which the next item may join.
#[derive(Default)]
struct Open {
    /// The paragraph or heading: its role, its block, and its element.
    run: Option<(Role, u64, usize)>,
    /// The list: its id and element.
    list: Option<(u64, usize)>,
    /// The list item: its block, and the element its text is in.
    item: Option<(u64, usize)>,
    /// The table: its block, and its element.
    table: Option<(u64, usize)>,
    /// The table row: its number, and its element.
    row: Option<(u32, usize)>,
    /// The table cell: its block, row and column, and its element.
    cell: Option<((u64, u32, u32), usize)>,
}

impl Open {
    /// Closes everything: an item of another kind starts afresh.
    fn close_all(&mut self) {
        *self = Self::default();
    }

    /// Closes the table, its row and its cell: text outside a table ends it.
    fn close_table(&mut self) {
        self.table = None;
        self.row = None;
        self.cell = None;
    }
}

/// Builds the tree of a file whose pages hold these items.
///
/// Consecutive text with the same role and block is one element. A list is one `L` for its
/// consecutive items. A grid is one `Table`, its rows and cells. Headers and footers are artifacts,
/// which are not in the tree.
pub fn build(pages: &[Vec<Item>]) -> Tree {
    let mut tree = Tree {
        nodes: Vec::new(),
        top: Vec::new(),
        marks: Vec::with_capacity(pages.len()),
        owners: Vec::with_capacity(pages.len()),
    };
    for (page, items) in pages.iter().enumerate() {
        let mut marks = Vec::with_capacity(items.len());
        let mut owners: Vec<usize> = Vec::new();
        let mut mcid = 0u32;
        let mut open = Open::default();
        for entry in items {
            match entry {
                Item::Figure { alt, .. } => {
                    let node = tree.new_node("Figure", page);
                    tree.nodes[node].alt = alt.clone();
                    tree.nodes[node].mcids.push(mcid);
                    tree.top.push(node);
                    owners.push(node);
                    marks.push(Mark::Tagged {
                        mcid,
                        tag: "Figure".into(),
                    });
                    mcid += 1;
                    open.close_all();
                }
                Item::Run(text) => match &text.role {
                    Role::Untagged => marks.push(Mark::None),
                    Role::Artifact => marks.push(Mark::Artifact),
                    Role::Paragraph | Role::Heading(_) => {
                        let kind = match text.role {
                            Role::Heading(level) => format!("H{}", level.clamp(1, 6)),
                            _ => "P".to_string(),
                        };
                        let node = match &open.run {
                            Some((role, block, node))
                                if *role == text.role && *block == text.block =>
                            {
                                *node
                            }
                            _ => {
                                let node = tree.new_node(&kind, page);
                                tree.top.push(node);
                                open.run = Some((text.role.clone(), text.block, node));
                                node
                            }
                        };
                        open.list = None;
                        open.item = None;
                        open.close_table();
                        tree.nodes[node].mcids.push(mcid);
                        owners.push(node);
                        marks.push(Mark::Tagged { mcid, tag: kind });
                        mcid += 1;
                    }
                    Role::ListItem { list: list_id } => {
                        open.run = None;
                        open.close_table();
                        let list_node = match open.list {
                            Some((id, node)) if id == *list_id => node,
                            _ => {
                                let node = tree.new_node("L", page);
                                tree.top.push(node);
                                open.list = Some((*list_id, node));
                                node
                            }
                        };
                        let body = match open.item {
                            Some((block, body)) if block == text.block => body,
                            _ => {
                                let entry = tree.new_node("LI", page);
                                tree.nodes[list_node].children.push(entry);
                                let body = tree.new_node("LBody", page);
                                tree.nodes[entry].children.push(body);
                                open.item = Some((text.block, body));
                                body
                            }
                        };
                        tree.nodes[body].mcids.push(mcid);
                        owners.push(body);
                        marks.push(Mark::Tagged {
                            mcid,
                            tag: "LBody".into(),
                        });
                        mcid += 1;
                    }
                    Role::Cell {
                        table,
                        row,
                        col,
                        header,
                    } => {
                        open.run = None;
                        open.list = None;
                        open.item = None;
                        let table_node = match open.table {
                            Some((id, node)) if id == *table => node,
                            _ => {
                                let node = tree.new_node("Table", page);
                                tree.top.push(node);
                                open.table = Some((*table, node));
                                open.row = None;
                                open.cell = None;
                                node
                            }
                        };
                        let row_node = match open.row {
                            Some((number, node)) if number == *row => node,
                            _ => {
                                let node = tree.new_node("TR", page);
                                tree.nodes[table_node].children.push(node);
                                open.row = Some((*row, node));
                                open.cell = None;
                                node
                            }
                        };
                        let kind = if *header { "TH" } else { "TD" };
                        let key = (*table, *row, *col);
                        let cell = match open.cell {
                            Some((k, node)) if k == key => node,
                            _ => {
                                let node = tree.new_node(kind, page);
                                tree.nodes[row_node].children.push(node);
                                open.cell = Some((key, node));
                                node
                            }
                        };
                        tree.nodes[cell].mcids.push(mcid);
                        owners.push(cell);
                        marks.push(Mark::Tagged {
                            mcid,
                            tag: kind.into(),
                        });
                        mcid += 1;
                    }
                },
            }
        }
        tree.marks.push(marks);
        tree.owners.push(owners);
    }
    tree
}
