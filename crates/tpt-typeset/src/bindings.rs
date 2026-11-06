// SPDX-License-Identifier: MIT OR Apache-2.0
//! Data binding: `[[name]]` in a text block stands for a value the caller supplies.
//!
//! The caller decides what a name means. A query result is bound under the block's own name
//! (for example `q1`, after [`run_query`](crate::run_query)). A grid cell is bound under
//! [`grid_cell_name`], so the cell at `B2` of sheet 1 is `sheet1.B2`. The values are text, so
//! the caller formats numbers and runs formulas before binding. This crate does not evaluate
//! formulas.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, SheetId};

/// The values bound to names, for [`bound_text`] and [`check_bindings`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    values: BTreeMap<String, String>,
}

impl Bindings {
    /// An empty set of bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `name` to `value`, replacing any earlier value. Returns the earlier value.
    pub fn set(&mut self, name: &str, value: &str) -> Option<String> {
        self.values.insert(String::from(name), String::from(value))
    }

    /// The value bound to `name`, if any.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Number of bound names.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no name is bound.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// The name a grid cell is bound under: `sheet<id>.<address>`, such as `sheet1.B2`.
pub fn grid_cell_name(sheet: SheetId, address: &str) -> String {
    alloc::format!("sheet{}.{address}", sheet.0)
}

/// One binding found in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingCheck {
    /// The text block that holds the binding.
    pub block: BlockId,
    /// The name it refers to.
    pub name: String,
    /// The bound value, or `None` if nothing is bound to the name.
    pub value: Option<String>,
}

/// Every binding in the document, in block id order, then in text order.
pub fn check_bindings(doc: &Document, bindings: &Bindings) -> Vec<BindingCheck> {
    let mut out = Vec::new();
    for (&id, block) in &doc.blocks {
        let Block::Text { style, .. } = block else {
            continue;
        };
        for name in style.runs.iter().filter_map(binding_name) {
            out.push(BindingCheck {
                block: id,
                name: String::from(name),
                value: bindings.get(name).map(String::from),
            });
        }
    }
    out
}

/// The text of a text block with each bound `[[name]]` replaced by its value. A binding with no
/// value keeps its name as its text. Returns `None` if `block` is not a text block.
pub fn bound_text(doc: &Document, block: BlockId, bindings: &Bindings) -> Option<String> {
    let Some(Block::Text { content, style }) = doc.get(block) else {
        return None;
    };
    let chars: Vec<char> = content.chars().collect();
    let mut bound: Vec<(usize, usize, &str)> = style
        .runs
        .iter()
        .filter_map(|run| {
            let name = binding_name(run)?;
            let value = bindings.get(name)?;
            Some((run.start, run.end, value))
        })
        .collect();
    bound.sort_by_key(|&(start, _, _)| start);

    let mut out = String::new();
    let mut cursor = 0;
    for (start, end, value) in bound {
        if start < cursor || end > chars.len() {
            continue;
        }
        out.extend(&chars[cursor..start]);
        out.push_str(value);
        cursor = end;
    }
    out.extend(&chars[cursor..]);
    Some(out)
}

/// The name of a binding run, from its `var:` link. `None` for any other run.
fn binding_name(run: &tpt_codex_core::InlineRun) -> Option<&str> {
    run.link.as_deref()?.strip_prefix(BINDING_PREFIX)
}

/// The link prefix of a binding run. Kept in step with the Markdown reader.
const BINDING_PREFIX: &str = "var:";
