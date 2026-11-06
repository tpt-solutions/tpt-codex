// SPDX-License-Identifier: MIT OR Apache-2.0
//! The browser entry point. Build with `wasm32-unknown-unknown` and run `wasm-bindgen` on the
//! result; see `README.md` in this crate. On other targets the crate is empty.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// A mounted editor, as the page sees it. Keep it for as long as the editor is on the page.
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
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

    /// Takes in one frame from the relay, as text. Presence frames move the other people's
    /// carets. Frames that are not messages, such as the relay's join reply, are an error the
    /// page can ignore.
    pub fn receive_sync(&self, text: &str) -> Result<(), JsValue> {
        self.controller
            .receive_sync(text)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The frames to send to the relay, one per line, or an empty string when there are none.
    pub fn pending_sync(&self) -> String {
        self.controller.pending_sync()
    }

    /// Asks this editor to announce its caret again. A page calls it once it has joined.
    pub fn announce_again(&self) {
        self.controller.announce_again();
    }

    /// This editor's replica number.
    pub fn replica(&self) -> u32 {
        self.controller.replica() as u32
    }

    /// Opens a file the user chose: `name` is its file name, which says the format (`.cdx`, `.md`,
    /// `.docx`, `.xlsx`, `.pptx` or `.json`), and `bytes` its contents. The document being edited
    /// is replaced. Returns what the import dropped or approximated, as text, or an empty string
    /// when nothing was lost. On an error the document is left as it was.
    pub fn open_file(&self, name: &str, bytes: &[u8]) -> Result<String, JsValue> {
        let opened = tpt_codex_files::open(name, bytes).map_err(|e| JsValue::from_str(&e))?;
        let report = opened
            .report
            .as_ref()
            .filter(|r| !r.is_lossless())
            .map(ToString::to_string)
            .unwrap_or_default();
        self.controller.replace_document(opened.document);
        Ok(report)
    }

    /// The document as the bytes of a file called `name`, whose extension says the format
    /// (`.cdx`, `.md`, `.docx`, `.xlsx`, `.json` or `.pdf`). The page turns them into a download.
    pub fn save_file(&self, name: &str) -> Result<Vec<u8>, JsValue> {
        self.controller
            .with_document(|doc| tpt_codex_files::save(doc, name))
            .map(|saved| saved.bytes)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The document as PDF file bytes. The page turns them into a download.
    pub fn export_pdf(&self) -> Result<Vec<u8>, JsValue> {
        self.controller
            .export_pdf()
            .map_err(|e| JsValue::from_str(&e))
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

/// Mounts the sample editor in the element with id `container_id`, as replica `replica`. Pages
/// that share a relay document each need a different replica number.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn mount_sample_as(container_id: &str, replica: u32) -> Result<EditorHandle, JsValue> {
    Ok(EditorHandle {
        controller: tpt_codex_app::web::mount_sample_as(container_id, u64::from(replica))?,
    })
}
