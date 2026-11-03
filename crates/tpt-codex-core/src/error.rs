// SPDX-License-Identifier: MIT OR Apache-2.0
use core::fmt;

use crate::id::BlockId;

/// A structural problem found by [`Document::validate`](crate::Document::validate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The document was written with a schema version this build does not read.
    UnsupportedSchemaVersion {
        /// Version found in the document.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },
    /// The root id does not refer to a block in the document.
    MissingRoot(BlockId),
    /// A container references a child id that does not exist.
    DanglingChild {
        /// The container holding the reference.
        parent: BlockId,
        /// The missing child id.
        child: BlockId,
    },
    /// The block graph contains a cycle through this block.
    Cycle(BlockId),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "unsupported schema version {found} (this build supports {supported})"
            ),
            Self::MissingRoot(id) => write!(f, "root block {} does not exist", id.0),
            Self::DanglingChild { parent, child } => write!(
                f,
                "block {} references missing child block {}",
                parent.0, child.0
            ),
            Self::Cycle(id) => write!(f, "block {} is part of a cycle", id.0),
        }
    }
}

impl core::error::Error for ValidationError {}
