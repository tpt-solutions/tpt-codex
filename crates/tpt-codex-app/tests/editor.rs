// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_appfront_html::render;
use tpt_codex_app::{Editor, Msg};
use tpt_codex_core::{
    Block, BlockId, CellInput, Document, LayoutMode, Movement, Sheet, SheetId, TextStyle,
};

fn doc() -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3)],
        },
    );
    d.insert(
        BlockId(1),
        Block::Text {
            content: "Title".into(),
            style: TextStyle {
                bold: false,
                italic: false,
                heading_level: Some(1),
            },
        },
    );
    d.insert(
        BlockId(2),
        Block::Text {
            content: "Body".into(),
            style: TextStyle::default(),
        },
    );
    d.insert(
        BlockId(3),
        Block::Math {
            source: "x^2".into(),
        },
    );
    d
}

fn text(e: &Editor, id: u64) -> String {
    match e.document().get(BlockId(id)) {
        Some(Block::Text { content, .. }) => content.clone(),
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn typing_moving_and_undo_go_through_messages() {
    let mut e = Editor::new(doc());
    e.update(Msg::Focus(BlockId(2)));
    e.update(Msg::Type("!".into()));
    assert_eq!(text(&e, 2), "Body!");
    e.update(Msg::Backspace);
    e.update(Msg::Move(Movement::Left));
    e.update(Msg::Type("X".into()));
    assert_eq!(text(&e, 2), "BodXy");

    e.update(Msg::Undo);
    assert_eq!(text(&e, 2), "Body");
    e.update(Msg::Undo);
    assert_eq!(text(&e, 2), "Body!");
    e.update(Msg::Redo);
    assert_eq!(text(&e, 2), "Body");
    assert_eq!(e.status(), None);
}

#[test]
fn caret_is_shown_in_the_rendered_view() {
    let mut e = Editor::new(doc());
    e.update(Msg::Focus(BlockId(2)));
    let html = render(&e.view());
    assert!(html.contains("Body|"), "{html}");
    assert!(html.contains("<h1>Title</h1>"), "{html}");

    e.update(Msg::Focus(BlockId(1)));
    let html = render(&e.view());
    assert!(!html.contains("Body|"), "{html}");
    assert!(html.contains("Title|"), "{html}");
}

#[test]
fn editing_without_a_caret_reports_a_status_and_changes_nothing() {
    let mut e = Editor::new(doc());
    let before = e.document().clone();
    e.update(Msg::Type("x".into()));
    assert!(e.status().unwrap().contains("no caret"));
    assert_eq!(e.document(), &before);

    // A math block cannot take a caret, so focusing it leaves none.
    e.update(Msg::Focus(BlockId(3)));
    assert_eq!(e.caret(), None);
    let html = render(&e.view());
    assert!(html.contains("$$ x^2 $$"), "{html}");
}

#[test]
fn undo_clamps_the_caret_into_its_block() {
    let mut e = Editor::new(doc());
    e.update(Msg::Focus(BlockId(2)));
    e.update(Msg::Type("abc".into()));
    e.update(Msg::Undo);
    // The caret was after "abc" (position 7); after undo the text is "Body" (length 4).
    assert_eq!(e.caret().unwrap().focus, 4);
}

#[test]
fn grid_blocks_show_their_evaluated_cells() {
    let mut d = doc();
    d.insert(
        BlockId(4),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(4));
    }
    let mut sheet = Sheet::default();
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 21.0 });
    sheet.cells.insert(
        "B1".into(),
        CellInput::Formula {
            source: "=A1 * 2".into(),
        },
    );
    d.insert_sheet(SheetId(1), sheet);
    d.validate().unwrap();

    let html = render(&Editor::new(d).view());
    assert!(html.contains(r#"<td role="gridcell">21</td>"#), "{html}");
    assert!(html.contains(r#"<td role="gridcell">42</td>"#), "{html}");
    assert!(html.contains(r#"value="=A1 * 2""#), "{html}");
}

fn grid_doc() -> Document {
    let mut d = doc();
    d.insert(
        BlockId(4),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(4));
    }
    let mut sheet = Sheet::default();
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 2.0 });
    d.insert_sheet(SheetId(1), sheet);
    d
}

fn cell(e: &Editor, address: &str) -> Option<CellInput> {
    e.document().sheets[&SheetId(1)].cells.get(address).cloned()
}

fn edit(e: &mut Editor, address: &str, text: &str) {
    e.update(Msg::EditCell {
        sheet: SheetId(1),
        address: address.into(),
        text: text.into(),
    });
}

#[test]
fn typed_cell_text_becomes_numbers_booleans_formulas_and_text() {
    let mut e = Editor::new(grid_doc());
    edit(&mut e, "B1", "3.5");
    assert_eq!(cell(&e, "B1"), Some(CellInput::Number { value: 3.5 }));
    edit(&mut e, "B2", "TRUE");
    assert_eq!(cell(&e, "B2"), Some(CellInput::Boolean { value: true }));
    edit(&mut e, "B3", "=A1 * B1");
    assert_eq!(
        cell(&e, "B3"),
        Some(CellInput::Formula {
            source: "=A1 * B1".into()
        })
    );
    edit(&mut e, "B4", "hello");
    assert_eq!(
        cell(&e, "B4"),
        Some(CellInput::Text {
            value: "hello".into()
        })
    );
    assert_eq!(e.status(), None);

    let html = render(&e.view());
    assert!(html.contains(r#"<td role="gridcell">7</td>"#), "{html}");
}

#[test]
fn clearing_a_cell_and_undoing_restores_it() {
    let mut e = Editor::new(grid_doc());
    edit(&mut e, "A1", "");
    assert_eq!(cell(&e, "A1"), None);
    e.update(Msg::Undo);
    assert_eq!(cell(&e, "A1"), Some(CellInput::Number { value: 2.0 }));
}

#[test]
fn bad_cell_edits_report_a_status_and_change_nothing() {
    let mut e = Editor::new(grid_doc());
    let before = e.document().clone();
    edit(&mut e, "a1", "5");
    assert!(e.status().unwrap().contains("not a valid cell address"));
    edit(&mut e, "A2", "=");
    assert!(e.status().unwrap().contains("formula"));
    assert_eq!(e.document(), &before);
}

#[test]
fn cell_edits_on_a_missing_sheet_are_reported() {
    let mut e = Editor::new(grid_doc());
    e.update(Msg::EditCell {
        sheet: SheetId(9),
        address: "A1".into(),
        text: "1".into(),
    });
    assert!(e.status().unwrap().contains("does not exist"));
}

#[test]
fn parse_cell_input_rules() {
    use tpt_codex_app::editor::parse_cell_input;
    assert_eq!(parse_cell_input(""), Ok(None));
    assert_eq!(
        parse_cell_input("FALSE"),
        Ok(Some(CellInput::Boolean { value: false }))
    );
    assert_eq!(
        parse_cell_input("inf"),
        Ok(Some(CellInput::Text {
            value: "inf".into()
        }))
    );
    assert!(parse_cell_input("=").is_err());
}

#[test]
fn view_renders_to_html_for_the_whole_document() {
    let html = render(&Editor::new(doc()).view());
    assert!(html.contains("Body"), "{html}");
    assert!(html.contains("Undo"), "{html}");
}
