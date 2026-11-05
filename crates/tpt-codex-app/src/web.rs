// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mounts the [`Editor`](crate::Editor) in a browser page (`wasm32` only).
//!
//! The document state lives in a [`Signal`]-tracked editor. A message updates the editor, and
//! the signal change re-renders the view through `tpt-appfront-dom`, which updates the DOM in place.

use std::cell::RefCell;
use std::rc::Rc;

use tpt_appfront_core::{EffectHandle, Signal};
use tpt_appfront_dom::render;
use tpt_codex_core::Document;
use web_sys::Element;
use web_sys::wasm_bindgen::JsValue;

use crate::editor::{Editor, Msg};

/// Mounts `doc` as an editor inside `container`.
///
/// Keep the returned handle alive for as long as the editor is on the page.
/// A mounted editor that the page can send messages to. Keep it alive while the editor is shown.
pub struct Controller {
    dispatch: Rc<dyn Fn(Msg)>,
    _effect: EffectHandle,
}

impl std::fmt::Debug for Controller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Controller")
    }
}

impl Controller {
    /// Sends `msg` to the editor, as if the user had done it.
    pub fn send(&self, msg: Msg) {
        (self.dispatch)(msg);
    }
}

/// Mounts `doc` as an editor inside `container`, and returns the handle to send it messages.
pub fn mount_controlled(container: &Element, doc: Document) -> Result<Controller, JsValue> {
    let editor = Rc::new(RefCell::new(Editor::new(doc)));
    // Bumped after each message, so the view depends on it and re-runs.
    let version = Signal::new(0u64);

    let view = {
        let editor = Rc::clone(&editor);
        let version = version.clone();
        Rc::new(move || {
            let _ = version.get();
            editor.borrow().view()
        })
    };
    let dispatch: Rc<dyn Fn(Msg)> = {
        let editor = Rc::clone(&editor);
        let version = version.clone();
        Rc::new(move |msg: Msg| {
            editor.borrow_mut().update(msg);
            version.set(version.get() + 1);
        })
    };
    let effect = render(container, view, Rc::clone(&dispatch))?;
    Ok(Controller {
        dispatch,
        _effect: effect,
    })
}

/// A small sample document: a heading, a paragraph with emphasis and a link, and a grid with a
/// formula. Used by the browser demo page.
pub fn sample_document() -> Document {
    use tpt_codex_core::{
        Block, BlockId, CellInput, InlineRun, LayoutMode, Sheet, SheetId, TextStyle,
    };
    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2), BlockId(3)],
        },
    );
    doc.insert(
        BlockId(1),
        Block::Text {
            content: "Field notes".into(),
            style: TextStyle {
                heading_level: Some(1),
                ..TextStyle::default()
            },
        },
    );
    doc.insert(
        BlockId(2),
        Block::Text {
            content: "Plain, bold, and a link.".into(),
            style: TextStyle {
                runs: vec![
                    InlineRun {
                        start: 7,
                        end: 11,
                        bold: true,
                        italic: false,
                        link: None,
                        math: None,
                    },
                    InlineRun {
                        start: 19,
                        end: 23,
                        bold: false,
                        italic: false,
                        link: Some("https://example.test".into()),
                        math: None,
                    },
                ],
                ..TextStyle::default()
            },
        },
    );
    doc.insert(
        BlockId(3),
        Block::Grid {
            sheet_id: SheetId(1),
            view_bounds: None,
        },
    );
    let mut sheet = Sheet::with_size(2, 2);
    sheet
        .cells
        .insert("A1".into(), CellInput::Number { value: 2.0 });
    sheet.cells.insert(
        "B1".into(),
        CellInput::Formula {
            source: "=A1*21".into(),
        },
    );
    doc.insert_sheet(SheetId(1), sheet);
    doc
}

/// Mounts the sample document in the element with id `container_id`. Called from the demo page.

pub fn mount_sample(container_id: &str) -> Result<Controller, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let container = document
        .get_element_by_id(container_id)
        .ok_or_else(|| JsValue::from_str("no container element"))?;
    mount_controlled(&container, sample_document())
}
