// SPDX-License-Identifier: MIT OR Apache-2.0
//! The browser entry point. Build with `wasm32-unknown-unknown` and run `wasm-bindgen` on the
//! result; see `README.md` in this crate. On other targets the crate is empty.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// A mounted editor, as the page sees it. Keep it for as long as the editor is on the page.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct EditorHandle {
    controller: tpt_codex_app::web::Controller,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl EditorHandle {
    /// Marks the presenter stage as full screen (`true`) or not. The page makes the browser go
    /// full screen itself; this only changes what the editor draws.
    pub fn set_fullscreen(&self, on: bool) {
        self.controller
            .send(tpt_codex_app::Msg::SetFullscreen { on });
    }

    /// Shows the document as pages (`true`) or as the editable flow (`false`).
    pub fn set_pages(&self, on: bool) {
        self.controller
            .send(tpt_codex_app::Msg::SetPaginated { on });
    }
}

/// Mounts the sample editor in the element with id `container_id`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn mount_sample(container_id: &str) -> Result<EditorHandle, JsValue> {
    Ok(EditorHandle {
        controller: tpt_codex_app::web::mount_sample(container_id)?,
    })
}
