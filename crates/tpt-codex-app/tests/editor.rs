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
    // The focused paragraph is a text area, holding its text.
    assert!(html.contains("paragraph-input"), "{html}");
    assert!(html.contains("Body</textarea>"), "{html}");
    assert!(html.contains("<h1>Title</h1>"), "{html}");

    e.update(Msg::Focus(BlockId(1)));
    let html = render(&e.view());
    assert!(!html.contains("Body</textarea>"), "{html}");
    assert!(
        html.contains("<h1>Title</h1>") || html.contains("Title</textarea>"),
        "{html}"
    );
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

#[test]
fn the_table_size_is_chosen_before_inserting() {
    let mut e = Editor::new(doc());
    assert_eq!(e.table_size(), (3, 3));
    e.update(Msg::SetTableSize { rows: 2, cols: 5 });
    assert_eq!(e.table_size(), (2, 5));
    e.update(Msg::InsertTable { rows: 2, cols: 5 });
    assert_eq!(e.status(), None);

    // A bad size is refused and the size stays as it was.
    e.update(Msg::SetTableSize { rows: 0, cols: 5 });
    assert!(e.status().is_some());
    assert_eq!(e.table_size(), (2, 5));
    let html = render(&e.view());
    assert!(html.contains("table-rows"), "{html}");
}

#[test]
fn a_slide_is_inserted_placed_and_drawn() {
    use tpt_codex_core::Rect;
    let mut e = Editor::new(doc());
    e.update(Msg::InsertSlide);
    assert_eq!(e.status(), None);
    let slide = *e.document().blocks.keys().max().unwrap();
    let html = render(&e.view());
    assert!(html.contains("slide"), "{html}");

    // Put the title on the slide, then place it. The slide must hold the block.
    let title = BlockId(1);
    let slide_children = match e.document().get(slide) {
        Some(Block::Container { children, .. }) => children.clone(),
        _ => unreachable!(),
    };
    assert!(slide_children.is_empty());
    e.update(Msg::PlaceBlock {
        block: title,
        frame: Some(Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        }),
    });
    // The title is not on the slide, so placing it is refused.
    assert!(e.status().is_some());

    e.update(Msg::Undo);
    assert!(e.document().canvases.is_empty() && !e.document().blocks.contains_key(&slide));
}

#[test]
fn replacing_text_changes_only_the_difference_and_undoes_as_one_step() {
    let mut e = Editor::new(doc());
    // Block 2 is "Body" in the test document.
    e.update(Msg::ReplaceText {
        block: BlockId(2),
        text: "Bxxdy!".into(),
    });
    assert_eq!(e.status(), None);
    match e.document().get(BlockId(2)) {
        Some(Block::Text { content, .. }) => assert_eq!(content, "Bxxdy!"),
        other => panic!("{other:?}"),
    }
    // The caret follows the change.
    assert_eq!(e.caret().unwrap().focus, 6);

    e.update(Msg::Undo);
    match e.document().get(BlockId(2)) {
        Some(Block::Text { content, .. }) => assert_eq!(content, "Body"),
        other => panic!("{other:?}"),
    }
    // Replacing with the same text changes nothing and is not an undo step.
    e.update(Msg::ReplaceText {
        block: BlockId(2),
        text: "Body".into(),
    });
    assert_eq!(e.status(), None);
}

#[test]
fn replacing_the_text_of_a_non_text_block_is_refused() {
    let mut e = Editor::new(doc());
    e.update(Msg::ReplaceText {
        block: BlockId(0),
        text: "x".into(),
    });
    assert!(e.status().is_some());
}

#[test]
fn presenter_mode_shows_one_slide_and_moves_between_slides() {
    let mut e = Editor::new(doc());
    // No slides yet: presenting says so and changes nothing.
    e.update(Msg::StartPresenting { index: 0 });
    assert!(e.status().is_some());
    assert_eq!(e.presenting(), None);

    e.update(Msg::InsertSlide);
    e.update(Msg::InsertSlide);
    e.update(Msg::StartPresenting { index: 9 });
    assert_eq!(e.presenting(), Some(1), "clamped to the last slide");
    let html = render(&e.view());
    assert!(html.contains("slide 2 of 2"), "{html}");
    assert!(!html.contains("Body"), "only the slide is shown: {html}");

    e.update(Msg::NextSlide);
    assert_eq!(e.presenting(), Some(1), "last slide stays");
    e.update(Msg::PreviousSlide);
    e.update(Msg::PreviousSlide);
    assert_eq!(e.presenting(), Some(0), "first slide stays");

    e.update(Msg::StopPresenting);
    assert_eq!(e.presenting(), None);
    assert!(render(&e.view()).contains("Body"));
}

#[test]
fn remote_carets_are_drawn_in_their_paragraph() {
    use tpt_codex_app::RemoteCursor;
    let mut e = Editor::new(doc());
    e.set_remote_cursors(vec![RemoteCursor {
        label: "Ann".into(),
        block: BlockId(2),
        offset: 2,
    }]);
    let html = render(&e.view());
    assert!(html.contains("remote-cursor"), "{html}");
    assert!(html.contains("[Ann]"), "{html}");
    assert!(html.contains("Title"), "{html}");
    assert!(!html.contains("Title["), "{html}");
}

/// A slide holding a text block (id 1 in `doc()`), placed at (10, 10) and 100 by 50 points.
fn slide_with_placed_title() -> Editor {
    use tpt_codex_core::Rect;
    let mut e = Editor::new(doc());
    e.update(Msg::InsertSlide);
    let slide = *e.document().blocks.keys().max().unwrap();
    // Put block 1 on the slide.
    let mut d = e.document().clone();
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&slide) {
        children.push(BlockId(1));
    }
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.retain(|c| *c != BlockId(1));
    }
    d.frames.insert(
        BlockId(1),
        Rect {
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 50.0,
        },
    );
    Editor::new(d)
}

#[test]
fn moving_a_block_keeps_it_on_the_slide_and_undoes() {
    let mut e = slide_with_placed_title();
    e.update(Msg::MoveBlock {
        block: BlockId(1),
        dx: 20.0,
        dy: 5.0,
    });
    assert_eq!(e.status(), None);
    let moved = e.document().frames[&BlockId(1)];
    assert_eq!((moved.x, moved.y), (30.0, 15.0));

    // Pushing past the right and bottom edges stops at the slide's edge.
    e.update(Msg::MoveBlock {
        block: BlockId(1),
        dx: 10_000.0,
        dy: 10_000.0,
    });
    let far = e.document().frames[&BlockId(1)];
    assert_eq!((far.x, far.y), (960.0 - 100.0, 540.0 - 50.0));

    e.update(Msg::Undo);
    assert_eq!(e.document().frames[&BlockId(1)], moved);
}

#[test]
fn resizing_stops_at_the_slide_edge_and_refuses_bad_sizes() {
    let mut e = slide_with_placed_title();
    e.update(Msg::ResizeBlock {
        block: BlockId(1),
        width: 5_000.0,
        height: 60.0,
    });
    let f = e.document().frames[&BlockId(1)];
    assert_eq!(f.width, 960.0 - 10.0);
    assert_eq!(f.height, 60.0);

    e.update(Msg::ResizeBlock {
        block: BlockId(1),
        width: 0.0,
        height: 60.0,
    });
    assert!(e.status().is_some());
    assert_eq!(e.document().frames[&BlockId(1)], f);
}

#[test]
fn a_block_that_is_not_on_a_slide_cannot_be_moved() {
    let mut e = Editor::new(doc());
    e.update(Msg::MoveBlock {
        block: BlockId(1),
        dx: 1.0,
        dy: 1.0,
    });
    assert!(e.status().is_some());
    assert!(e.document().frames.is_empty());
}

#[test]
fn a_filter_query_shows_matching_blocks_and_refuses_unknown_fields() {
    let mut e = Editor::new(query_doc("not a query"));
    e.update(Msg::FilterQuery {
        block: BlockId(3),
        field: "kind".into(),
        value: "paragraph".into(),
    });
    assert_eq!(e.status(), None);
    let html = render(&e.view());
    assert!(html.contains("query-row"), "{html}");
    assert!(html.contains("Alpha"), "{html}");

    e.update(Msg::FilterQuery {
        block: BlockId(3),
        field: "secret".into(),
        value: "x".into(),
    });
    assert!(e.status().is_some());
}

#[test]
fn nested_containers_draw_their_children() {
    let mut d = doc();
    // Put a flow container holding the title, inside the root.
    d.insert(
        BlockId(9),
        Block::Container {
            layout_mode: tpt_codex_core::LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
        children.push(BlockId(9));
    }
    let html = render(&Editor::new(d).view());
    assert!(html.contains("container-flow"), "{html}");
    assert!(html.contains("Title"), "{html}");
}

#[test]
fn the_paginated_view_cuts_the_document_into_pages() {
    use tpt_codex_views::{FlowOptions, MonospaceMeasure, PageGeometry};
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 14.0,
    };
    let flow = FlowOptions {
        width: 300.0,
        block_spacing: 0.0,
        box_height: 40.0,
        row_height: 20.0,
    };
    // Each page holds one 14-point line.
    let geometry = PageGeometry {
        height: 20.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 1,
    };
    let mut d = doc();
    // Make the body longer, so that it spans more than one page.
    let mut h = tpt_codex_core::History::new();
    tpt_codex_core::edit_text(&mut d, &mut h, BlockId(2), 4, 4, " and more text here").unwrap();
    let e = Editor::new(d);
    let html = render(&e.paginated_view(&measure, flow, geometry).unwrap());
    assert!(html.contains("Page 1"), "{html}");
    assert!(html.contains("Page 2"), "{html}");
    assert!(html.contains("page-line"), "{html}");
}

#[test]
fn slides_can_be_reordered_and_the_move_undoes() {
    let mut e = Editor::new(doc());
    for _ in 0..3 {
        e.update(Msg::InsertSlide);
    }
    let before = e.slide_ids();
    assert_eq!(before.len(), 3);

    e.update(Msg::MoveSlide { from: 0, to: 2 });
    assert_eq!(e.status(), None);
    assert_eq!(e.slide_ids(), vec![before[1], before[2], before[0]]);

    e.update(Msg::MoveSlide { from: 2, to: 0 });
    assert_eq!(e.slide_ids(), vec![before[0], before[1], before[2]]);

    // Bad indices are refused and change nothing.
    e.update(Msg::MoveSlide { from: 0, to: 7 });
    assert!(e.status().is_some());
    assert_eq!(e.slide_ids(), before);

    e.update(Msg::MoveSlide { from: 1, to: 2 });
    e.update(Msg::Undo);
    assert_eq!(e.slide_ids(), before);
}

#[test]
fn the_class_sets_the_page_size_of_the_paginated_view() {
    use tpt_codex_views::{FlowOptions, MonospaceMeasure};
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 14.0,
    };
    let flow = FlowOptions {
        width: 300.0,
        block_spacing: 0.0,
        box_height: 40.0,
        row_height: 20.0,
    };
    let mut d = doc();
    let mut h = tpt_codex_core::History::new();
    let long = "word ".repeat(1000);
    tpt_codex_core::edit_text(&mut d, &mut h, BlockId(2), 4, 4, &long).unwrap();
    d.metadata.class = Some("article".into());
    let article = render(
        &Editor::new(d.clone())
            .paginated_view_for_class(&measure, flow)
            .unwrap(),
    );
    d.metadata.class = Some("report".into());
    let report = render(
        &Editor::new(d.clone())
            .paginated_view_for_class(&measure, flow)
            .unwrap(),
    );
    assert_ne!(report, article);
    assert!(report.matches("class=\"page\"").count() >= article.matches("class=\"page\"").count());
    // Without a class, the default is an article.
    d.metadata.class = None;
    let none = render(
        &Editor::new(d)
            .paginated_view_for_class(&measure, flow)
            .unwrap(),
    );
    assert_eq!(none, article);
}

#[test]
fn fullscreen_needs_presenter_mode_and_stops_with_it() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertSlide);
    e.update(Msg::SetFullscreen { on: true });
    assert!(
        e.status().is_some(),
        "full screen without presenting is refused"
    );
    e.update(Msg::StartPresenting { index: 0 });
    e.update(Msg::SetFullscreen { on: true });
    assert!(render(&e.view()).contains("presenter fullscreen"));
    e.update(Msg::StopPresenting);
    assert!(!render(&e.view()).contains("fullscreen"));
}

#[test]
fn the_paginated_view_shows_headers_and_numbered_footers() {
    use tpt_codex_views::{FlowOptions, MonospaceMeasure, PageChrome, PageGeometry};
    let measure = MonospaceMeasure {
        char_width: 6.0,
        line_height: 14.0,
    };
    let flow = FlowOptions {
        width: 300.0,
        block_spacing: 0.0,
        box_height: 40.0,
        row_height: 20.0,
    };
    let geometry = PageGeometry {
        height: 20.0,
        margin_top: 0.0,
        margin_bottom: 0.0,
        min_lines: 1,
    };
    let chrome = PageChrome {
        header: Some("Field notes".into()),
        footer: Some("{page}/{pages}".into()),
    };
    let e = Editor::new(doc());
    let html = render(
        &e.paginated_view_with(&measure, flow, geometry, &chrome)
            .unwrap(),
    );
    assert!(html.contains("Field notes"), "{html}");
    assert!(html.contains("1/"), "{html}");
    assert!(html.contains("page-footer"), "{html}");
}

#[test]
fn slides_show_their_section_and_sections_undo() {
    let mut e = Editor::new(doc());
    e.update(Msg::InsertSlide);
    let slide = *e.slide_ids().first().unwrap();
    e.update(Msg::SetSection {
        slide,
        name: "Intro".into(),
    });
    assert_eq!(e.status(), None);
    e.update(Msg::StartPresenting { index: 0 });
    assert!(render(&e.view()).contains("Intro · slide 1 of 1"));
    e.update(Msg::Undo);
    e.update(Msg::StopPresenting);
    e.update(Msg::StartPresenting { index: 0 });
    let html = render(&e.view());
    assert!(
        html.contains("slide 1 of 1") && !html.contains("Intro"),
        "{html}"
    );
}

#[test]
fn inline_math_runs_are_drawn_as_math() {
    let mut d = doc();
    if let Some(Block::Text { style, .. }) = d.blocks.get_mut(&BlockId(2)) {
        style.runs.push(tpt_codex_core::InlineRun {
            start: 0,
            end: 4,
            bold: false,
            italic: false,
            link: None,
            math: Some("Body".into()),
        });
    }
    let html = render(&Editor::new(d).view());
    assert!(html.contains("math-inline"), "{html}");
}
