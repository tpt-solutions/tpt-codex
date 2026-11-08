// SPDX-License-Identifier: MIT OR Apache-2.0
//! The structure tree of a tagged PDF: which marked content is a paragraph, a heading, a list item
//! or a figure, in reading order.
//!
//! Every tagged piece of text, or figure, has a marked-content identifier (MCID) on its page. The
//! tree names each identifier through an element, so a reader can say what each part of the page
//! is. Lists are an `L` element with an `LI` per item, and each item's text is an `LBody`.

use crate::text_layer::{Item, Role};

/// One element of the tree, before its object number is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The structure type: `P`, `H1` to `H6`, `L`, `LI`, `LBody` or `Figure`.
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

/// Builds the tree of a file whose pages hold these items.
///
/// Consecutive text with the same role and block is one element. A list is one `L` for its
/// consecutive items. Headers and footers are artifacts, which are not in the tree.
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
        // The open paragraph or heading: its role, its block, and its element.
        let mut run: Option<(Role, u64, usize)> = None;
        // The open list: its id and element.
        let mut list: Option<(u64, usize)> = None;
        // The open list item: its block, and the element its text is in.
        let mut item: Option<(u64, usize)> = None;
        for entry in items {
            match entry {
                Item::Figure { alt, .. } => {
                    let node = tree.push(Node {
                        kind: "Figure".into(),
                        alt: alt.clone(),
                        page,
                        mcids: vec![mcid],
                        children: Vec::new(),
                        number: 0,
                    });
                    tree.top.push(node);
                    owners.push(node);
                    marks.push(Mark::Tagged {
                        mcid,
                        tag: "Figure".into(),
                    });
                    mcid += 1;
                    run = None;
                    list = None;
                    item = None;
                }
                Item::Run(text) => match &text.role {
                    Role::Untagged => marks.push(Mark::None),
                    Role::Artifact => marks.push(Mark::Artifact),
                    Role::Paragraph | Role::Heading(_) => {
                        let kind = match text.role {
                            Role::Heading(level) => format!("H{}", level.clamp(1, 6)),
                            _ => "P".to_string(),
                        };
                        let node = match &run {
                            Some((role, block, node))
                                if *role == text.role && *block == text.block =>
                            {
                                *node
                            }
                            _ => {
                                let node = tree.push(Node {
                                    kind: kind.clone(),
                                    alt: None,
                                    page,
                                    mcids: Vec::new(),
                                    children: Vec::new(),
                                    number: 0,
                                });
                                tree.top.push(node);
                                run = Some((text.role.clone(), text.block, node));
                                node
                            }
                        };
                        list = None;
                        item = None;
                        tree.nodes[node].mcids.push(mcid);
                        owners.push(node);
                        marks.push(Mark::Tagged { mcid, tag: kind });
                        mcid += 1;
                    }
                    Role::ListItem { list: list_id } => {
                        run = None;
                        let list_node = match list {
                            Some((id, node)) if id == *list_id => node,
                            _ => {
                                let node = tree.push(Node {
                                    kind: "L".into(),
                                    alt: None,
                                    page,
                                    mcids: Vec::new(),
                                    children: Vec::new(),
                                    number: 0,
                                });
                                tree.top.push(node);
                                list = Some((*list_id, node));
                                node
                            }
                        };
                        let body = match item {
                            Some((block, body)) if block == text.block => body,
                            _ => {
                                let entry = tree.push(Node {
                                    kind: "LI".into(),
                                    alt: None,
                                    page,
                                    mcids: Vec::new(),
                                    children: Vec::new(),
                                    number: 0,
                                });
                                tree.nodes[list_node].children.push(entry);
                                let body = tree.push(Node {
                                    kind: "LBody".into(),
                                    alt: None,
                                    page,
                                    mcids: Vec::new(),
                                    children: Vec::new(),
                                    number: 0,
                                });
                                tree.nodes[entry].children.push(body);
                                item = Some((text.block, body));
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
                },
            }
        }
        tree.marks.push(marks);
        tree.owners.push(owners);
    }
    tree
}
