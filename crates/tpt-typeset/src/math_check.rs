// SPDX-License-Identifier: MIT OR Apache-2.0
//! Checks the math blocks of a document before layout.

use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document};

use crate::math::{MathError, parse_math};

/// A math block whose source does not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathBlockError {
    /// The `Block::Math` that failed.
    pub block: BlockId,
    /// The parse error. Its line and column count from the start of that block's source.
    pub error: MathError,
}

/// Parses the source of every `Block::Math` in the document and returns the failures.
///
/// Checks every block in the document, in id order, including blocks that no container
/// references. An empty result means every math block parses.
pub fn check_math_blocks(doc: &Document) -> Vec<MathBlockError> {
    doc.blocks
        .iter()
        .filter_map(|(&id, block)| match block {
            Block::Math { source } => parse_math(source)
                .err()
                .map(|error| MathBlockError { block: id, error }),
            _ => None,
        })
        .collect()
}
