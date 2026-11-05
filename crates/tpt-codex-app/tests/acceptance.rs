// SPDX-License-Identifier: MIT OR Apache-2.0
//! End-to-end path through the headless stack: Markdown in, saved, outlined, laid out,
//! paginated, edited with undo, and written back to Markdown.

use std::fs;
use std::path::PathBuf;

use tpt_codex_app::{DocumentStore, FileStorage};
use tpt_codex_core::{Block, BlockId, History, edit_text};
use tpt_codex_markdown::{from_markdown, to_markdown};
use tpt_codex_views::{
    FlowKind, FlowOptions, MonospaceMeasure, PageChrome, PageGeometry, layout_flow, paginate,
    render_chrome,
};
use tpt_typeset::Outline;

const FIXTURE: &str = include_str!("../../../fixtures/notes.md");

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tpt-codex-accept-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn markdown_to_pages_and_back() {
    let dir = scratch("pipeline");

    // 1. Read Markdown and save it through the file store.
    let doc = from_markdown(FIXTURE).unwrap();
    let mut store = DocumentStore::new(FileStorage::new(&dir));
    store.save("notes", &doc).unwrap();
    let reloaded = store.load("notes").unwrap();
    assert_eq!(reloaded, doc);
    fs::remove_dir_all(&dir).ok();

    // 2. Outline, table of contents and cross-reference lookup.
    let outline = Outline::of(&reloaded);
    let toc = outline.toc(3);
    assert_eq!(toc.len(), 2);
    assert_eq!(toc[1].number, "1.1");
    assert_eq!(outline.resolve("details").unwrap().title, "Details");

    // 3. Lay out and paginate onto a small page with a footer.
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 14.0,
    };
    let flow = layout_flow(
        &reloaded,
        &measure,
        FlowOptions {
            width: 240.0,
            block_spacing: 8.0,
            box_height: 40.0,
            row_height: 20.0,
            header_rows: 0,
        },
    )
    .unwrap();
    let pages = paginate(
        &flow,
        PageGeometry {
            height: 120.0,
            margin_top: 10.0,
            margin_bottom: 10.0,
            min_lines: 2,
        },
    )
    .unwrap();
    assert!(!pages.is_empty());
    let text_items = pages
        .iter()
        .flat_map(|p| p.items.iter())
        .filter(|i| matches!(i.kind, FlowKind::Line { .. }))
        .count();
    assert_eq!(
        text_items,
        flow.items
            .iter()
            .filter(|i| matches!(i.kind, FlowKind::Line { .. }))
            .count()
    );
    let chrome = render_chrome(
        &pages,
        &PageChrome {
            header: None,
            footer: Some("{page}/{pages}".into()),
        },
    );
    assert_eq!(
        chrome.last().unwrap().footer.as_deref(),
        Some(format!("{}/{}", pages.len(), pages.len()).as_str())
    );

    // 4. Edit the first paragraph with undo, then write back to Markdown.
    let mut edited = reloaded.clone();
    let mut history = History::new();
    let para_id = match edited.get(BlockId(2)) {
        Some(Block::Text { .. }) => BlockId(2),
        other => panic!("expected the first paragraph at block 2, got {other:?}"),
    };
    let len = match edited.get(para_id) {
        Some(Block::Text { content, .. }) => content.chars().count(),
        _ => unreachable!(),
    };
    edit_text(&mut edited, &mut history, para_id, len, len, " Edited.").unwrap();
    assert_eq!(edited.validate(), Ok(()));
    let md = to_markdown(&edited).unwrap();
    assert!(md.contains("Edited."), "{md}");

    assert!(history.undo(&mut edited).unwrap());
    assert_eq!(edited, reloaded);
    assert_eq!(to_markdown(&edited).unwrap(), FIXTURE);
}
