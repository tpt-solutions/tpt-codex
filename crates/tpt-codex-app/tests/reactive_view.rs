// SPDX-License-Identifier: MIT OR Apache-2.0
//! The editor's view is cached per top-level block: an edit re-renders only the blocks it
//! changed, and the cached view matches a full render.

use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Sheet, SheetId, TextStyle};

/// Root with paragraphs 1 and 2, a list (3) holding item 5, and a grid (4) of sheet 1.
fn document() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)],
        },
    );
    for (id, text) in [(1, "first"), (2, "second")] {
        doc.insert(
            BlockId(id),
            Block::Text {
                content: text.into(),
                style: TextStyle::default(),
            },
        );
    }
    doc.insert(
        BlockId(5),
        Block::Text {
            content: "item".into(),
            style: TextStyle::default(),
        },
    );
    doc.insert(
        BlockId(3),
        Block::List {
            items: vec![BlockId(5)],
            ordered: false,
            start: 1,
        },
    );
    doc.insert(
        BlockId(4),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.insert_sheet(SheetId(1), Sheet::with_size(2, 2));
    doc
}

/// Renders the view and returns how many top-level blocks that render drew.
fn rendered_by(editor: &Editor) -> usize {
    let before = editor.blocks_rendered();
    let _ = editor.view();
    editor.blocks_rendered() - before
}

#[test]
fn first_view_renders_every_top_level_block() {
    let editor = Editor::new(document());
    assert_eq!(rendered_by(&editor), 4);
    assert_eq!(rendered_by(&editor), 0, "an unchanged view renders nothing");
}

#[test]
fn typing_renders_only_the_paragraph_being_typed_in() {
    let mut editor = Editor::new(document());
    rendered_by(&editor);

    editor.update(Msg::Focus(BlockId(2)));
    assert_eq!(
        rendered_by(&editor),
        1,
        "moving the caret re-renders its paragraph"
    );

    editor.update(Msg::Type("!".into()));
    assert_eq!(rendered_by(&editor), 1);
    assert_eq!(rendered_by(&editor), 0);
}

#[test]
fn a_change_inside_a_list_renders_the_list_only() {
    let mut editor = Editor::new(document());
    rendered_by(&editor);

    editor.update(Msg::Focus(BlockId(5)));
    assert_eq!(rendered_by(&editor), 1, "the list owns item 5");
}

#[test]
fn a_cell_edit_renders_the_grid_that_shows_the_sheet() {
    let mut editor = Editor::new(document());
    rendered_by(&editor);

    editor.update(Msg::EditCell {
        sheet: SheetId(1),
        address: "A1".into(),
        text: "7".into(),
    });
    assert_eq!(rendered_by(&editor), 1);
}

/// A quote (6) that holds a grid (7) of sheet 1, and nothing else.
fn quoted_grid() -> Document {
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(6)],
        },
    );
    doc.insert(
        BlockId(6),
        Block::Quote {
            children: vec![BlockId(7)],
        },
    );
    doc.insert(
        BlockId(7),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    doc.insert_sheet(SheetId(1), Sheet::with_size(2, 2));
    doc
}

#[test]
fn a_cell_edit_renders_the_quote_that_holds_the_grid() {
    let mut editor = Editor::new(quoted_grid());
    rendered_by(&editor);
    editor.update(Msg::EditCell {
        sheet: SheetId(1),
        address: "A1".into(),
        text: "7".into(),
    });
    assert_eq!(rendered_by(&editor), 1, "the quote owns the grid");
}

#[test]
fn cached_view_matches_a_fresh_render_after_edits() {
    let mut editor = Editor::new(document());
    editor.update(Msg::Focus(BlockId(1)));
    editor.update(Msg::Type("X".into()));
    editor.update(Msg::EditCell {
        sheet: SheetId(1),
        address: "B2".into(),
        text: "=A1*2".into(),
    });
    let cached = format!("{:?}", editor.view());

    // The same editor state, with every cached block dropped, renders the same tree. A fresh
    // editor would not do: it starts with the caret elsewhere, so its state differs.
    editor.invalidate_view();
    assert_eq!(format!("{:?}", editor.view()), cached);
}

#[test]
fn undo_renders_the_changed_block_again() {
    let mut editor = Editor::new(document());
    editor.update(Msg::Focus(BlockId(1)));
    editor.update(Msg::Type("X".into()));
    let with_x = format!("{:?}", editor.view());
    editor.update(Msg::Undo);
    let undone = format!("{:?}", editor.view());
    assert_ne!(with_x, undone, "undo must show the earlier text");
    assert!(!undone.contains("firstX"));
}
