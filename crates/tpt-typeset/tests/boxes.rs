// SPDX-License-Identifier: MIT OR Apache-2.0
//! Section boxes: each section's extent on a laid-out column covers its heading, its body and the
//! sections nested below it, and sibling sections do not overlap.

use tpt_codex_markdown::from_markdown;
use tpt_codex_views::{FlowOptions, MonospaceMeasure, layout_flow};
use tpt_typeset::{SectionBox, section_boxes, semantic_tree};

const SAMPLE: &str = "Preface.

# Introduction

Body one, long enough that it wraps across several lines of the narrow column.

## Background

Body two.

# Method

Body three.
";

fn boxes() -> (Vec<SectionBox>, f64) {
    let doc = from_markdown(SAMPLE).unwrap();
    let tree = semantic_tree(&doc);
    let layout = layout_flow(
        &doc,
        &MonospaceMeasure {
            char_width: 6.0,
            line_height: 12.0,
        },
        FlowOptions {
            width: 120.0,
            block_spacing: 4.0,
            box_height: 10.0,
            row_height: 10.0,
        },
    )
    .unwrap();
    (section_boxes(&tree, &layout), layout.height)
}

#[test]
fn sections_carry_their_numbers_and_nest_like_the_tree() {
    let (boxes, _) = boxes();
    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[0].number, "1");
    assert_eq!(boxes[0].children.len(), 1);
    assert_eq!(boxes[0].children[0].number, "1.1");
    assert_eq!(boxes[1].number, "2");
    assert_eq!(boxes[1].title, "Method");
}

#[test]
fn a_parent_extent_covers_its_children() {
    let (boxes, _) = boxes();
    let (top, bottom) = boxes[0].extent.expect("introduction is on the layout");
    let (child_top, child_bottom) = boxes[0].children[0]
        .extent
        .expect("background is on the layout");
    assert!(top <= child_top, "{top} <= {child_top}");
    assert!(child_bottom <= bottom, "{child_bottom} <= {bottom}");
}

#[test]
fn sibling_sections_do_not_overlap_and_stay_on_the_column() {
    let (boxes, height) = boxes();
    let (_, intro_bottom) = boxes[0].extent.unwrap();
    let (method_top, method_bottom) = boxes[1].extent.unwrap();
    assert!(intro_bottom <= method_top, "{intro_bottom} <= {method_top}");
    assert!(method_bottom <= height, "{method_bottom} <= {height}");
}
