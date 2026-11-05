// SPDX-License-Identifier: MIT OR Apache-2.0
//! Times flow layout and pagination on a large generated document.
//! Run with: cargo run --release -p tpt-codex-views --example bench_layout -- 20000

use std::time::Instant;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, TextStyle};
use tpt_codex_views::{FlowOptions, MonospaceMeasure, PageGeometry, layout_flow, paginate};

fn main() {
    let paragraphs: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);

    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: (1..=paragraphs as u64).map(BlockId).collect(),
        },
    );
    let sentence = "the quick brown fox jumps over the lazy dog and keeps running far away";
    for i in 0..paragraphs {
        let content = format!("{i}: {sentence} {sentence}");
        doc.insert(
            BlockId(i as u64 + 1),
            Block::Text {
                content,
                style: TextStyle::default(),
            },
        );
    }

    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 14.0,
    };
    let options = FlowOptions {
        width: 468.0,
        block_spacing: 6.0,
        box_height: 40.0,
        row_height: 20.0,
        header_rows: 0,
    };
    let geometry = PageGeometry {
        height: 792.0,
        margin_top: 72.0,
        margin_bottom: 72.0,
        min_lines: 2,
    };

    let t = Instant::now();
    let flow = layout_flow(&doc, &measure, options).expect("layout");
    let layout_time = t.elapsed();

    let t = Instant::now();
    let pages = paginate(&flow, geometry).expect("paginate");
    let paginate_time = t.elapsed();

    println!(
        "paragraphs={paragraphs} lines={} pages={}",
        flow.items.len(),
        pages.len()
    );
    println!("layout:   {layout_time:?}");
    println!("paginate: {paginate_time:?}");
}
