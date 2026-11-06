// SPDX-License-Identifier: MIT OR Apache-2.0
//! Grids, slides and figure attributes: exact round trips, reader errors with lines, and
//! writer refusals for documents the reader could not read back.

use tpt_codex_core::{
    Block, BlockId, Canvas, CellInput, Document, Figure, LayoutMode, Rect, Sheet, SheetId,
    address_of,
};
use tpt_codex_markdown::{from_markdown, to_markdown};

const GRID: &str =
    "::: grid { id=\"1\" }\n| Item | Qty |\n| --- | --- |\n| Bolts | 4 |\n| Total | =B2 * 2 |\n:::";

const SLIDE: &str =
    "::: canvas { ratio=\"16:9\" }\n\n# Title\n\n![Plan](plan.png) {#fig:plan width=80%}\n:::";

fn root_children(doc: &Document) -> Vec<BlockId> {
    match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children.clone(),
        other => panic!("root is not a container: {other:?}"),
    }
}

fn children_of(doc: &Document, id: BlockId) -> Vec<BlockId> {
    match doc.get(id) {
        Some(Block::Container { children, .. }) => children.clone(),
        other => panic!("not a container: {other:?}"),
    }
}

fn read_err(src: &str) -> (usize, String) {
    let err = from_markdown(src).expect_err("should be an error");
    (err.line, err.message)
}

// ---- round trips -----------------------------------------------------------

#[test]
fn grid_round_trips_exactly() {
    let doc = from_markdown(GRID).unwrap();
    let md = to_markdown(&doc).unwrap();
    assert_eq!(md, format!("{GRID}\n"));
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn grid_cells_hold_typed_values() {
    let doc = from_markdown(GRID).unwrap();
    let grid = root_children(&doc)[0];
    let Some(Block::Grid { sheet_id, .. }) = doc.get(grid) else {
        panic!("expected a grid block");
    };
    let sheet = &doc.sheets[sheet_id];
    assert_eq!(sheet.header_rows, 1);
    assert_eq!(sheet.cells["B2"], CellInput::Number { value: 4.0 });
    assert!(matches!(sheet.cells["B3"], CellInput::Formula { .. }));
}

#[test]
fn grid_escapes_pipes_and_backslashes() {
    let src = "::: grid { id=\"2\" }\n| a\\|b | c\\\\d |\n:::";
    let doc = from_markdown(src).unwrap();
    let sheet = &doc.sheets[&SheetId(2)];
    assert_eq!(
        sheet.cells["A1"],
        CellInput::Text {
            value: "a|b".into()
        }
    );
    assert_eq!(
        sheet.cells["B1"],
        CellInput::Text {
            value: "c\\d".into()
        }
    );
    let md = to_markdown(&doc).unwrap();
    assert_eq!(md, format!("{src}\n"));
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn slide_round_trips_exactly() {
    let doc = from_markdown(SLIDE).unwrap();
    let md = to_markdown(&doc).unwrap();
    assert_eq!(md, format!("{SLIDE}\n"));
    assert_eq!(from_markdown(&md).unwrap(), doc);
}

#[test]
fn slide_has_a_16_by_9_size_and_absolute_layout() {
    let doc = from_markdown(SLIDE).unwrap();
    let slide = root_children(&doc)[0];
    assert!(matches!(
        doc.get(slide),
        Some(Block::Container {
            layout_mode: LayoutMode::Absolute,
            ..
        })
    ));
    assert_eq!(doc.canvases[&slide], Canvas::WIDESCREEN);
    assert_eq!(children_of(&doc, slide).len(), 2);
}

#[test]
fn figure_attributes_round_trip() {
    for src in [
        "![Plan](plan.png) {#fig:plan width=80%}",
        "![Chart](chart.svg) {width=50%}",
        "![Logo](logo.png) {#fig:logo}",
    ] {
        let doc = from_markdown(src).unwrap();
        let md = to_markdown(&doc).unwrap();
        assert_eq!(md, format!("{src}\n"), "source {src:?}");
        assert_eq!(from_markdown(&md).unwrap(), doc);
    }
}

#[test]
fn figure_is_stored_on_the_image_block() {
    let doc = from_markdown("![Plan](plan.png) {#fig:plan width=80%}").unwrap();
    let image = root_children(&doc)[0];
    let figure = &doc.figures[&image];
    assert_eq!(figure.id.as_deref(), Some("fig:plan"));
    assert_eq!(figure.width_percent, Some(80));
}

#[test]
fn image_without_figure_attributes_has_no_figure_entry() {
    let doc = from_markdown("![Plan](plan.png)").unwrap();
    assert!(doc.figures.is_empty());
}

// ---- reader errors ---------------------------------------------------------

#[test]
fn reader_errors_name_the_line() {
    let cases: &[(&str, usize, &str)] = &[
        ("# T\n\n::: widget\n:::\n", 3, "unknown container 'widget'"),
        ("::: grid { id=\"1\" }\n| a |\n", 1, "grid is not closed"),
        (
            "::: canvas { ratio=\"1:1\" }\n# T\n",
            2,
            "canvas is not closed",
        ),
        (
            "::: canvas { ratio=\"1:1\" }\n::: canvas { ratio=\"1:1\" }\n:::\n:::\n",
            2,
            "cannot be placed inside another canvas",
        ),
        (
            "::: grid { id=\"1\" }\n| a | b |\n| c |\n:::\n",
            3,
            "row has 1 cells",
        ),
        (
            "::: grid { id=\"1\" }\n| a |\n| --- |\n| --- |\n:::\n",
            4,
            "only one separator row",
        ),
        (
            "::: grid { id=\"1\" }\n| --- |\n:::\n",
            2,
            "must come after a row of cells",
        ),
        (
            "::: grid { id=\"1\" }\nhello\n:::\n",
            2,
            "can only hold table rows",
        ),
        ("::: grid { id=\"1\" }\n:::\n", 1, "grid has no rows"),
        (
            "::: grid { id=\"1\" }\n| a |\n:::\n::: grid { id=\"1\" }\n| b |\n:::\n",
            4,
            "used by more than one grid",
        ),
        (
            "::: grid { id=\"1\" }\n| = |\n:::\n",
            2,
            "a formula needs something",
        ),
        (
            "![a](b.png) {#fig:x width=120%}\n",
            1,
            "width must be a whole percentage",
        ),
        ("![a](b.png) {color=red}\n", 1, "unknown figure attribute"),
        ("![a](b.png) {#fig:x #fig:y}\n", 1, "only one #label"),
        ("![a](b.png) foo\n", 1, "figure attributes must be written"),
    ];
    for (src, line, message) in cases {
        let (got_line, got_message) = read_err(src);
        assert_eq!(got_line, *line, "line for {src:?}");
        assert!(
            got_message.contains(message),
            "message for {src:?} was {got_message:?}, expected it to contain {message:?}"
        );
    }
}

#[test]
fn reader_errors_come_with_hints() {
    let err = from_markdown("::: grid { id=\"1\" }\nhello\n:::\n").unwrap_err();
    assert!(
        err.suggestion.is_some(),
        "a grid error should suggest a fix"
    );
}

// ---- writer refusals -------------------------------------------------------

fn write_err(doc: &Document) -> String {
    to_markdown(doc).expect_err("should be refused").reason
}

#[test]
fn writer_refuses_slide_sections() {
    let mut doc = from_markdown(SLIDE).unwrap();
    doc.sections.insert(root_children(&doc)[0], "Intro".into());
    assert!(write_err(&doc).contains("sections"));
}

#[test]
fn writer_refuses_slide_frames() {
    let mut doc = from_markdown(SLIDE).unwrap();
    let child = children_of(&doc, root_children(&doc)[0])[0];
    doc.frames.insert(
        child,
        Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    );
    assert!(write_err(&doc).contains("frames"));
}

#[test]
fn writer_refuses_a_grid_sheet_nothing_shows() {
    let mut doc = from_markdown(GRID).unwrap();
    doc.insert_sheet(SheetId(9), Sheet::with_size(1, 1));
    assert!(write_err(&doc).contains("not shown by a grid"));
}

#[test]
fn writer_refuses_grid_view_bounds() {
    let mut doc = from_markdown(GRID).unwrap();
    let grid = root_children(&doc)[0];
    if let Some(Block::Grid { view_bounds, .. }) = doc.blocks.get_mut(&grid) {
        *view_bounds = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
    }
    assert!(write_err(&doc).contains("view bounds"));
}

#[test]
fn writer_refuses_cells_that_would_read_back_differently() {
    let mut doc = from_markdown(GRID).unwrap();
    let sheet_id = SheetId(1);
    doc.sheets.get_mut(&sheet_id).unwrap().cells.insert(
        address_of(1, 3),
        CellInput::Text {
            value: "TRUE".into(),
        },
    );
    assert!(write_err(&doc).contains("different value"));
}

#[test]
fn writer_refuses_a_row_of_dashes() {
    let mut doc = from_markdown("::: grid { id=\"1\" }\n| a |\n:::\n").unwrap();
    doc.sheets.get_mut(&SheetId(1)).unwrap().cells.insert(
        address_of(1, 1),
        CellInput::Text {
            value: "---".into(),
        },
    );
    assert!(write_err(&doc).contains("header separator"));
}

#[test]
fn writer_refuses_more_header_rows_than_rows() {
    let mut doc = from_markdown("::: grid { id=\"1\" }\n| a |\n:::\n").unwrap();
    doc.sheets.get_mut(&SheetId(1)).unwrap().header_rows = 5;
    assert!(write_err(&doc).contains("header rows"));
}

#[test]
fn writer_refuses_a_slide_size_with_no_ratio() {
    let mut doc = from_markdown(SLIDE).unwrap();
    let slide = root_children(&doc)[0];
    doc.canvases.insert(
        slide,
        Canvas {
            width: 500.0,
            height: 300.0,
        },
    );
    assert!(write_err(&doc).contains("ratio"));
}

#[test]
fn writer_refuses_figures_on_text() {
    let mut doc = from_markdown("A paragraph.").unwrap();
    let para = root_children(&doc)[0];
    doc.figures.insert(
        para,
        Figure {
            id: Some("fig:x".into()),
            width_percent: None,
        },
    );
    assert!(write_err(&doc).contains("only be written on images"));
}

#[test]
fn writer_refuses_a_figure_label_that_cannot_be_read() {
    let mut doc = from_markdown("![a](b.png)").unwrap();
    let image = root_children(&doc)[0];
    doc.figures.insert(
        image,
        Figure {
            id: Some("has space".into()),
            width_percent: None,
        },
    );
    assert!(write_err(&doc).contains("figure label"));
}
