// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mounts the [`Editor`](crate::Editor) in a browser page (`wasm32` only).
//!
//! The document state lives in a [`Signal`]-tracked editor. A message updates the editor, and
//! the signal change re-renders the view through `tpt-appfront-dom`, which updates the DOM in place.

use std::cell::RefCell;
use std::rc::Rc;

use tpt_appfront_core::{EffectHandle, Signal};
use tpt_appfront_dom::render;
use tpt_codex_core::{Document, Message, ReplicaId};
use web_sys::Element;
use web_sys::wasm_bindgen::JsValue;

use crate::editor::{Editor, Msg};
use crate::presence::Presence;

/// A mounted editor that the page can send messages to, and that exchanges presence with a relay
/// through the page. Keep it alive while the editor is shown.
pub struct Controller {
    dispatch: Rc<dyn Fn(Msg)>,
    presence: Rc<RefCell<Presence>>,
    editor: Rc<RefCell<Editor>>,
    version: Signal<u64>,
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

    /// Takes in one frame from the relay. Presence updates move the other people's carets; other
    /// frames are not presence and are ignored. Fails only on a frame that is not a message.
    pub fn receive_sync(&self, text: &str) -> Result<(), String> {
        let message: Message = serde_json::from_str(text).map_err(|e| e.to_string())?;
        self.presence.borrow_mut().receive(message);
        let cursors = self.presence.borrow().remote_cursors();
        self.editor.borrow_mut().set_remote_cursors(cursors);
        // The borrows above have ended, so the view can read the editor when it re-renders.
        self.version.set(self.version.get() + 1);
        Ok(())
    }

    /// The frames to send to the relay, one per line: this editor's caret, if it has moved since
    /// the last frame sent. The frames are compact JSON, so a newline cannot be inside one.
    pub fn pending_sync(&self) -> String {
        let caret = self.editor.borrow().caret();
        match self.presence.borrow_mut().outbound_for(caret) {
            Some(message) => serde_json::to_string(&message).unwrap_or_default(),
            None => String::new(),
        }
    }

    /// Asks the next frames to announce this editor's caret again. A page calls this when it
    /// joins a document, because the relay does not replay presence.
    pub fn announce_again(&self) {
        self.presence.borrow_mut().announce_again();
    }

    /// The replica this editor is, as the relay sees it.
    pub fn replica(&self) -> ReplicaId {
        self.presence.borrow().id()
    }

    /// Replaces the document being edited, for example with one the user opened from a file. The
    /// undo history and the remote carets start again.
    pub fn replace_document(&self, doc: Document) {
        *self.editor.borrow_mut() = Editor::new(doc);
        self.version.set(self.version.get() + 1);
    }

    /// Runs `read` on the document being edited.
    pub fn with_document<R>(&self, read: impl FnOnce(&Document) -> R) -> R {
        read(self.editor.borrow().document())
    }

    /// The document as PDF file bytes, on the page size of its document class.
    pub fn export_pdf(&self) -> Result<Vec<u8>, String> {
        crate::export::export_pdf(self.editor.borrow().document())
    }
}

/// Mounts `doc` as an editor inside `container`, as replica 1, and returns the handle to send it
/// messages.
pub fn mount_controlled(container: &Element, doc: Document) -> Result<Controller, JsValue> {
    mount_controlled_as(container, doc, 1)
}

/// Mounts `doc` as an editor inside `container`, as replica `replica`. Each page on a shared
/// document needs its own replica number.
pub fn mount_controlled_as(
    container: &Element,
    doc: Document,
    replica: ReplicaId,
) -> Result<Controller, JsValue> {
    let editor = Rc::new(RefCell::new(Editor::new(doc)));
    let presence = Rc::new(RefCell::new(Presence::new(replica)));
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
        presence,
        editor,
        version,
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
                        link_title: None,
                    },
                    InlineRun {
                        start: 19,
                        end: 23,
                        bold: false,
                        italic: false,
                        link: Some("https://example.test".into()),
                        math: None,
                        link_title: None,
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

/// Mounts the sample document in the element with id `container_id`, as replica 1. Called from
/// the demo page.
pub fn mount_sample(container_id: &str) -> Result<Controller, JsValue> {
    mount_sample_as(container_id, 1)
}

/// Mounts the sample document in the element with id `container_id`, as replica `replica`.
pub fn mount_sample_as(container_id: &str, replica: ReplicaId) -> Result<Controller, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let container = document
        .get_element_by_id(container_id)
        .ok_or_else(|| JsValue::from_str("no container element"))?;
    mount_controlled_as(&container, sample_document(), replica)
}
