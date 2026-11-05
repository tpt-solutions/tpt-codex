// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{Block, BlockId, Document, History, LayoutMode, Op};
use tpt_typeset::slide_sections;

fn deck() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3)],
        },
    );
    for id in [1, 2, 3] {
        d.insert(
            BlockId(id),
            Block::Container {
                layout_mode: LayoutMode::Absolute,
                children: vec![],
            },
        );
    }
    d
}

#[test]
fn consecutive_slides_with_the_same_section_are_grouped() {
    let mut d = deck();
    let mut h = History::new();
    h.commit(
        &mut d,
        vec![
            Op::SetSection {
                id: BlockId(1),
                name: Some("Intro".into()),
            },
            Op::SetSection {
                id: BlockId(2),
                name: Some("Intro".into()),
            },
        ],
    )
    .unwrap();
    let groups = slide_sections(&d);
    assert_eq!(groups.len(), 2, "{groups:?}");
    assert_eq!(groups[0].name.as_deref(), Some("Intro"));
    assert_eq!(groups[0].slides, vec![BlockId(1), BlockId(2)]);
    assert_eq!(groups[1].name, None);
    d.validate().unwrap();
}

#[test]
fn a_section_can_be_cleared_and_only_slides_hold_one() {
    let mut d = deck();
    let mut h = History::new();
    h.commit(
        &mut d,
        vec![Op::SetSection {
            id: BlockId(1),
            name: Some("A".into()),
        }],
    )
    .unwrap();
    h.commit(
        &mut d,
        vec![Op::SetSection {
            id: BlockId(1),
            name: None,
        }],
    )
    .unwrap();
    assert!(d.sections.is_empty());
    // The root is a flow container, not a slide.
    assert!(
        tpt_codex_core::apply(
            &mut d,
            &Op::SetSection {
                id: BlockId(0),
                name: Some("x".into())
            }
        )
        .is_err()
    );
    // An empty name is refused.
    assert!(
        tpt_codex_core::apply(
            &mut d,
            &Op::SetSection {
                id: BlockId(1),
                name: Some(String::new())
            }
        )
        .is_err()
    );
}

#[test]
fn a_section_on_a_non_slide_fails_validation() {
    let mut d = deck();
    d.sections.insert(BlockId(0), "Oops".into());
    assert!(d.validate().is_err());
}
