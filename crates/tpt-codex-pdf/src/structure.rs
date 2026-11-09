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
    /// The lists open on this page, outermost first: each one's id and element. A sub-list is
    /// open inside the item above it.
    lists: Vec<(u64, usize)>,
    /// The list item: its block, the element its text is in, and its own element.
    item: Option<(u64, usize, usize)>,
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
                        open.lists.clear();
                        open.item = None;
                        open.close_table();
                        tree.nodes[node].mcids.push(mcid);
                        owners.push(node);
                        marks.push(Mark::Tagged { mcid, tag: kind });
                        mcid += 1;
                    }
                    Role::ListItem {
                        list: list_id,
                        parent,
                    } => {
                        open.run = None;
                        open.close_table();
                        // The item is in its list, which is open already or starts now. Lists
                        // nested deeper than it are closed. A new list that is an item of an open
                        // list goes in that list's item, as a sub-list.
                        let list_node = match open.lists.iter().position(|(id, _)| id == list_id) {
                            Some(depth) => {
                                open.lists.truncate(depth + 1);
                                open.lists[depth].1
                            }
                            None => {
                                let node = tree.new_node("L", page);
                                let parent_item = match (parent, open.item) {
                                    (Some(p), Some((_, _, entry)))
                                        if open.lists.iter().any(|(id, _)| id == p) =>
                                    {
                                        Some(entry)
                                    }
                                    _ => None,
                                };
                                match parent_item {
                                    Some(entry) => tree.nodes[entry].children.push(node),
                                    None => tree.top.push(node),
                                }
                                open.lists.push((*list_id, node));
                                node
                            }
                        };
                        let body = match open.item {
                            Some((block, body, _)) if block == text.block => body,
                            _ => {
                                let entry = tree.new_node("LI", page);
                                tree.nodes[list_node].children.push(entry);
                                let body = tree.new_node("LBody", page);
                                tree.nodes[entry].children.push(body);
                                open.item = Some((text.block, body, entry));
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
                        open.lists.clear();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_layer::TextRun;

    fn item(list: u64, parent: Option<u64>, block: u64) -> Item {
        Item::Run(TextRun {
            face: 0,
            size: 10.0,
            glyphs: Vec::new(),
            role: Role::ListItem { list, parent },
            block,
        })
    }

    fn kinds(tree: &Tree, ids: &[usize]) -> Vec<String> {
        ids.iter().map(|&i| tree.nodes[i].kind.clone()).collect()
    }

    #[test]
    fn a_sub_list_is_inside_the_item_above_it() {
        // Outer list 10 has items 1 and 2; item 1 has sub-list 20 with item 3.
        let pages = vec![vec![
            item(10, None, 1),
            item(20, Some(10), 3),
            item(10, None, 2),
        ]];
        let tree = build(&pages);
        assert_eq!(tree.top.len(), 1, "only the outer list is at the top");
        let outer = tree.top[0];
        assert_eq!(tree.nodes[outer].kind, "L");
        let items = tree.nodes[outer].children.clone();
        assert_eq!(
            kinds(&tree, &items),
            ["LI", "LI"],
            "the outer list has two items"
        );
        // The first item holds its body and then the sub-list.
        let first = items[0];
        assert_eq!(kinds(&tree, &tree.nodes[first].children), ["LBody", "L"]);
        let sub = tree.nodes[first].children[1];
        assert_eq!(kinds(&tree, &tree.nodes[sub].children), ["LI"]);
    }

    #[test]
    fn a_sub_list_closes_when_the_outer_item_after_it_starts() {
        let pages = vec![vec![
            item(10, None, 1),
            item(20, Some(10), 3),
            item(10, None, 2),
        ]];
        let tree = build(&pages);
        // The second outer item is in the same outer list, not in the sub-list.
        let outer = tree.top[0];
        assert_eq!(tree.nodes[outer].children.len(), 2);
        let second = tree.nodes[outer].children[1];
        assert_eq!(kinds(&tree, &tree.nodes[second].children), ["LBody"]);
    }
}
