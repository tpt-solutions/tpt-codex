// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cell storage for live grids. A `Block::Grid` displays one [`Sheet`].
//!
//! The core stores cell inputs as plain data. It does not evaluate formulas: `tpt-codex-grid`
//! does that with `tpt-lattice`, so the core stays `no_std` and free of the spreadsheet engine.

use alloc::collections::BTreeMap;
use alloc::string::String;
use serde::{Deserialize, Serialize};

/// What the user typed into one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellInput {
    /// A finite number.
    Number {
        /// The value.
        value: f64,
    },
    /// Text.
    Text {
        /// The text.
        value: String,
    },
    /// A boolean.
    Boolean {
        /// The value.
        value: bool,
    },
    /// A formula. The source includes the leading `=`.
    Formula {
        /// The formula text, for example `=A1 * 2`.
        source: String,
    },
}

/// The cells of one live grid, keyed by A1 address (for example `B4`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    /// Non-empty cells only. Empty cells are absent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cells: BTreeMap<String, CellInput>,
}

/// Returns `true` for a canonical A1 address: one to three upper-case letters, then a row
/// number from 1 to 1 048 576 with no leading zero (for example `A1`, `XFD1048576`).
pub fn is_valid_address(address: &str) -> bool {
    let letters = address.bytes().take_while(u8::is_ascii_uppercase).count();
    if !(1..=3).contains(&letters) {
        return false;
    }
    let digits = &address[letters..];
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    matches!(digits.parse::<u32>(), Ok(row) if (1..=1_048_576).contains(&row))
}

/// Returns `true` when `source` is a formula: `=` followed by at least one character.
pub fn is_formula_source(source: &str) -> bool {
    source.starts_with('=') && source.len() > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        for ok in ["A1", "Z9", "AA10", "XFD1048576", "B4"] {
            assert!(is_valid_address(ok), "{ok}");
        }
        for bad in [
            "", "a1", "A", "1", "A0", "A01", "AAAA1", "A1048577", "A-1", "A1 ",
        ] {
            assert!(!is_valid_address(bad), "{bad}");
        }
    }

    #[test]
    fn formulas() {
        assert!(is_formula_source("=A1"));
        assert!(!is_formula_source("="));
        assert!(!is_formula_source("A1"));
    }
}
