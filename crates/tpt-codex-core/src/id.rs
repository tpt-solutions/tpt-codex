// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::{Deserialize, Serialize};

/// Identifier of a [`Block`](crate::Block) inside a [`Document`](crate::Document).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub u64);

/// Identifier of a live grid (sheet) referenced by a `Block::Grid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SheetId(pub u64);
