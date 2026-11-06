// SPDX-License-Identifier: MIT OR Apache-2.0
//! Change detection for `tpt watch`: the file's modification time and length, polled.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Remembers the last state of a file and says when it differs.
#[derive(Debug, Default)]
pub struct Watcher {
    last: Option<(SystemTime, u64)>,
    seen: bool,
}

impl Watcher {
    /// `true` the first time it is called, and afterwards whenever the file's modification time
    /// or length differs from the last call. A missing file is not a change.
    pub fn changed(&mut self, path: &Path) -> bool {
        let state = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok().map(|t| (t, m.len())));
        if !self.seen {
            self.seen = true;
            self.last = state;
            return true;
        }
        if state.is_some() && state != self.last {
            self.last = state;
            return true;
        }
        false
    }
}
