// SPDX-License-Identifier: MIT OR Apache-2.0
//! Sections of a slide deck: consecutive slides that share a section name form one group.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode};

/// A run of consecutive slides with the same section. `name` is `None` for slides with no section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionGroup {
    /// The section's name, or `None`.
    pub name: Option<String>,
    /// The slides in the group, in deck order.
    pub slides: Vec<BlockId>,
}

/// The slides of the deck, in order, grouped by section.
pub fn slide_sections(doc: &Document) -> Vec<SectionGroup> {
    let slides: Vec<BlockId> = match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children
            .iter()
            .copied()
            .filter(|id| {
                matches!(
                    doc.get(*id),
                    Some(Block::Container {
                        layout_mode: LayoutMode::Absolute,
                        ..
                    })
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut groups: Vec<SectionGroup> = Vec::new();
    for slide in slides {
        let name = doc.sections.get(&slide).cloned();
        match groups.last_mut() {
            Some(group) if group.name == name => group.slides.push(slide),
            _ => groups.push(SectionGroup {
                name,
                slides: alloc::vec![slide],
            }),
        }
    }
    groups
}
