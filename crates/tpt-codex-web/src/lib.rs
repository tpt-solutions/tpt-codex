// SPDX-License-Identifier: MIT OR Apache-2.0
//! The browser entry point. Build with `wasm32-unknown-unknown` and run `wasm-bindgen` on the
//! result; see `README.md` in this crate. On other targets the crate is empty.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Mounts the sample editor in the element with id `container_id`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn mount_sample(container_id: &str) -> Result<(), JsValue> {
    tpt_codex_app::web::mount_sample(container_id)
}
