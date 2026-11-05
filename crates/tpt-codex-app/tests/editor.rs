// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_appfront_core::{KeyEvent, NodeKind, UITree};
use tpt_appfront_html::render;
use tpt_codex_app::{Arrow, Editor, Msg};
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
                runs: Vec::new(),
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
    assert!(html.contains("Insert table"), "{html}");
}

fn table_count(e: &Editor) -> usize {
    e.document()
        .blocks
        .values()
        .filter(|b| matches!(b, Block::Grid { .. }))
        .count()
}

#[test]
fn insert_table_adds_an_empty_grid_and_undo_removes_it() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 2, cols: 3 });
    assert!(e.status().is_none(), "{:?}", e.status());
    assert_eq!(table_count(&e), 1);
    let html = render(&e.view());
    assert_eq!(html.matches("class=\"grid-cell\"").count(), 6, "{html}");
    assert!(!html.contains("(empty grid)"), "{html}");

    e.update(Msg::Undo);
    assert_eq!(table_count(&e), 0);
    assert!(e.document().sheets.is_empty());
    assert!(e.document().validate().is_ok());
    e.update(Msg::Redo);
    assert_eq!(table_count(&e), 1);
}

#[test]
fn typing_into_an_empty_table_cell_stores_it() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 2, cols: 2 });
    let sheet = *e.document().sheets.keys().next().expect("a sheet");
    e.update(Msg::EditCell {
        sheet,
        address: "B2".into(),
        text: "=2*21".into(),
    });
    assert!(e.status().is_none(), "{:?}", e.status());
    assert_eq!(
        e.document().sheets[&sheet].cells["B2"],
        CellInput::Formula {
            source: "=2*21".into()
        }
    );
    assert!(render(&e.view()).contains("42"));
}

#[test]
fn insert_table_rejects_bad_sizes_and_changes_nothing() {
    let mut e = Editor::new(doc());
    for (rows, cols) in [(0, 2), (2, 0), (51, 1), (1, 51)] {
        e.update(Msg::InsertTable { rows, cols });
        assert!(e.status().unwrap().contains("between 1 and 50"));
    }
    assert_eq!(table_count(&e), 0);
    assert_eq!(e.document().blocks.len(), 4);
}

#[test]
fn sheet_size_is_kept_in_the_saved_form() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 4, cols: 1 });
    let json = serde_json::to_string(e.document()).expect("serialize");
    assert!(json.contains("\"rows\":4"), "{json}");
    let back: Document = serde_json::from_str(&json).expect("parse");
    assert_eq!(&back, e.document());
}

#[test]
fn arrow_keys_move_the_selected_cell_within_the_table() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 2, cols: 3 });
    let sheet = *e.document().sheets.keys().next().expect("a sheet");

    e.update(Msg::MoveCell(Arrow::Right));
    assert!(e.status().unwrap().contains("no cell selected"));

    e.update(Msg::FocusCell {
        sheet,
        address: "A1".into(),
    });
    e.update(Msg::MoveCell(Arrow::Left));
    assert_eq!(
        e.cell().map(|c| (c.col, c.row)),
        Some((1, 1)),
        "stays at the edge"
    );
    e.update(Msg::MoveCell(Arrow::Right));
    e.update(Msg::MoveCell(Arrow::Right));
    e.update(Msg::MoveCell(Arrow::Right));
    e.update(Msg::MoveCell(Arrow::Down));
    e.update(Msg::MoveCell(Arrow::Down));
    assert_eq!(e.cell().map(|c| (c.col, c.row)), Some((3, 2)));
    e.update(Msg::MoveCell(Arrow::Up));
    assert_eq!(e.cell().map(|c| (c.col, c.row)), Some((3, 1)));
    assert!(e.status().is_none());
    assert!(render(&e.view()).contains("grid-cell selected"));
}

#[test]
fn focusing_a_cell_outside_the_table_is_reported() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 2, cols: 2 });
    let sheet = *e.document().sheets.keys().next().expect("a sheet");
    e.update(Msg::FocusCell {
        sheet,
        address: "C1".into(),
    });
    assert!(e.status().unwrap().contains("outside the table"));
    assert!(e.cell().is_none());
    e.update(Msg::FocusCell {
        sheet,
        address: "a1".into(),
    });
    assert!(e.status().unwrap().contains("not a valid cell address"));
}

fn all_nodes<'a>(ui: &'a UITree<Msg>, out: &mut Vec<&'a UITree<Msg>>) {
    out.push(ui);
    if let NodeKind::Container { children } = &ui.kind {
        for child in children {
            all_nodes(child, out);
        }
    }
}

fn key(name: &str) -> KeyEvent {
    KeyEvent {
        key: name.into(),
        ..KeyEvent::default()
    }
}

#[test]
fn grid_cells_select_themselves_and_arrows_move_the_selection() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertTable { rows: 2, cols: 2 });
    let sheet = *e.document().sheets.keys().next().expect("a sheet");

    // The input for B2 is the one whose focus message names B2.
    let view = e.view();
    let mut nodes = Vec::new();
    all_nodes(&view, &mut nodes);
    let b2 = nodes
        .iter()
        .find(|n| {
            n.meta.on_focus
                == Some(Msg::FocusCell {
                    sheet,
                    address: "B2".into(),
                })
        })
        .expect("an input for B2 with a focus message");

    // Focusing it dispatches that message, which selects B2.
    let focus = b2.meta.on_focus.clone().expect("focus");
    e.update(focus);
    assert_eq!(e.cell().map(|c| (c.col, c.row)), Some((2, 2)));

    // Arrow keys become movement messages; other keys are left to the browser.
    let on_keydown = b2.meta.on_keydown.as_ref().expect("keydown handler");
    assert_eq!(on_keydown(key("ArrowUp")), Some(Msg::MoveCell(Arrow::Up)));
    assert_eq!(
        on_keydown(key("ArrowLeft")),
        Some(Msg::MoveCell(Arrow::Left))
    );
    assert_eq!(on_keydown(key("a")), None);

    e.update(Msg::MoveCell(Arrow::Up));
    assert_eq!(e.cell().map(|c| (c.col, c.row)), Some((2, 1)));
}

fn query_doc(source: &str) -> Document {
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
            content: "Alpha".into(),
            style: TextStyle::default(),
        },
    );
    d.insert(
        BlockId(2),
        Block::Text {
            content: "Title".into(),
            style: TextStyle {
                bold: false,
                italic: false,
                heading_level: Some(1),
                runs: Vec::new(),
            },
        },
    );
    d.insert(
        BlockId(3),
        Block::Query(tpt_codex_core::Query {
            source: source.into(),
        }),
    );
    d
}

#[test]
fn query_blocks_show_the_blocks_they_match() {
    // Entity type 1 is a paragraph, so only "Alpha" matches.
    let e = Editor::new(query_doc(
        r#"{"source":1,"filters":[{"Eq":{"field":"kind","value":"paragraph"}}]}"#,
    ));
    let html = render(&e.view());
    assert!(html.contains("query-row"), "{html}");
    assert!(html.contains("Alpha"), "{html}");
    // "Title" is in the document as a heading, but the query must not list it.
    assert!(!html.contains(r#"query-row">Title"#), "{html}");
}

#[test]
fn query_blocks_show_why_they_cannot_run() {
    let e = Editor::new(query_doc("not a query"));
    let html = render(&e.view());
    assert!(html.contains("query-error"), "{html}");
    assert!(html.contains("not valid JSON"), "{html}");
}

#[test]
fn query_with_no_matches_says_so() {
    let e = Editor::new(query_doc(
        r#"{"source":6,"filters":[{"Eq":{"field":"kind","value":"paragraph"}}]}"#,
    ));
    let html = render(&e.view());
    assert!(html.contains("no matching blocks"), "{html}");
}

const PARAGRAPHS_ONLY: &str =
    r#"{"source":1,"filters":[{"Eq":{"field":"kind","value":"paragraph"}}]}"#;

#[test]
fn a_valid_query_edit_is_applied_and_undoable() {
    let mut e = Editor::new(query_doc("not a query"));
    e.update(Msg::EditQuery {
        block: BlockId(3),
        source: PARAGRAPHS_ONLY.into(),
    });
    assert_eq!(e.status(), None);
    let html = render(&e.view());
    assert!(html.contains("query-row"), "{html}");
    assert!(html.contains("Alpha"), "{html}");

    e.update(Msg::Undo);
    let html = render(&e.view());
    assert!(html.contains("query-error"), "{html}");
}

#[test]
fn a_bad_query_edit_is_rejected_and_changes_nothing() {
    let mut e = Editor::new(query_doc(PARAGRAPHS_ONLY));
    e.update(Msg::EditQuery {
        block: BlockId(3),
        source: "{".into(),
    });
    assert!(e.status().is_some());
    let html = render(&e.view());
    assert!(html.contains("Alpha"), "{html}");
    assert!(!html.contains("query-error"), "{html}");
}

#[test]
fn query_edits_only_target_query_blocks() {
    let mut e = Editor::new(query_doc(PARAGRAPHS_ONLY));
    e.update(Msg::EditQuery {
        block: BlockId(1),
        source: PARAGRAPHS_ONLY.into(),
    });
    assert_eq!(e.status(), Some("that block is not a query"));
}

#[test]
fn query_source_is_shown_in_an_editable_input() {
    let e = Editor::new(query_doc(PARAGRAPHS_ONLY));
    let html = render(&e.view());
    assert!(html.contains("query-source"), "{html}");
}
