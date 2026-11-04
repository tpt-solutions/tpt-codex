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
pub fn mount(container: &Element, doc: Document) -> Result<EffectHandle, JsValue> {
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
    let dispatch = {
        let editor = Rc::clone(&editor);
        let version = version.clone();
        Rc::new(move |msg: Msg| {
            editor.borrow_mut().update(msg);
            version.set(version.get() + 1);
        })
    };
    render(container, view, dispatch)
}
