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

impl CellInput {
    /// Turns the text of a cell into an input, as a user types it.
    ///
    /// Empty text is `Ok(None)`, which clears the cell. A leading `=` is a formula, a finite
    /// number is a number, `TRUE` or `FALSE` is a boolean, and anything else is text.
    /// A lone `=` is an error.
    pub fn parse(text: &str) -> Result<Option<CellInput>, String> {
        if text.is_empty() {
            return Ok(None);
        }
        if text.starts_with('=') {
            if text.len() == 1 {
                return Err(String::from("a formula needs something after '='"));
            }
            return Ok(Some(CellInput::Formula {
                source: String::from(text),
            }));
        }
        if let Ok(value) = text.parse::<f64>() {
            if value.is_finite() {
                return Ok(Some(CellInput::Number { value }));
            }
        }
        Ok(Some(match text {
            "TRUE" => CellInput::Boolean { value: true },
            "FALSE" => CellInput::Boolean { value: false },
            _ => CellInput::Text {
                value: String::from(text),
            },
        }))
    }

    /// The text that [`CellInput::parse`] reads back as this input, for a value that
    /// round-trips. Numbers use the shortest form that reads back exactly.
    pub fn to_text(&self) -> String {
        match self {
            CellInput::Number { value } => alloc::format!("{value}"),
            CellInput::Text { value } => value.clone(),
            CellInput::Boolean { value } => String::from(if *value { "TRUE" } else { "FALSE" }),
            CellInput::Formula { source } => source.clone(),
        }
    }
}

/// The cells of one live grid, keyed by A1 address (for example `B4`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    /// Non-empty cells only. Empty cells are absent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cells: BTreeMap<String, CellInput>,
    /// Minimum number of rows the grid shows, so that a new table can have empty rows.
    /// Zero (the default, and absent from the JSON) means the rows that hold cells.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rows: u32,
    /// Minimum number of columns the grid shows. Zero means the columns that hold cells.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cols: u32,
    /// Number of leading rows that are headers. Pagination repeats them at the top of each page
    /// that the grid continues onto. Zero (the default, and absent from the JSON) means none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub header_rows: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Sheet {
    /// An empty sheet that shows `rows` by `cols` cells.
    pub fn with_size(rows: u32, cols: u32) -> Self {
        Self {
            cells: BTreeMap::new(),
            rows,
            cols,
            header_rows: 0,
        }
    }

    /// The number of rows and columns to show: the larger of the stored size and the cells.
    pub fn extent(&self) -> (u32, u32) {
        let (mut rows, mut cols) = (self.rows, self.cols);
        for address in self.cells.keys() {
            if let Some((col, row)) = address_position(address) {
                rows = rows.max(row);
                cols = cols.max(col);
            }
        }
        (rows, cols)
    }
}

/// Splits a canonical A1 address into `(column, row)`, both 1-based. `None` if it is not valid.
pub fn address_position(address: &str) -> Option<(u32, u32)> {
    if !is_valid_address(address) {
        return None;
    }
    let letters = address.bytes().take_while(u8::is_ascii_uppercase).count();
    let col = address.as_bytes()[..letters]
        .iter()
        .fold(0u32, |acc, b| acc * 26 + u32::from(b - b'A' + 1));
    let row: u32 = address[letters..].parse().ok()?;
    Some((col, row))
}

/// The canonical A1 address of a 1-based `(column, row)`, for example `(27, 4)` gives `AA4`.
pub fn address_of(col: u32, row: u32) -> alloc::string::String {
    let mut letters = alloc::vec::Vec::new();
    let mut c = col;
    while c > 0 {
        letters.push(b'A' + ((c - 1) % 26) as u8);
        c = (c - 1) / 26;
    }
    letters.reverse();
    let mut out = alloc::string::String::from_utf8(letters).expect("ASCII letters");
    out.push_str(&alloc::format!("{row}"));
    out
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
    fn positions_and_addresses_round_trip() {
        assert_eq!(address_position("A1"), Some((1, 1)));
        assert_eq!(address_position("AA4"), Some((27, 4)));
        assert_eq!(address_position("a1"), None);
        for (col, row) in [(1, 1), (26, 9), (27, 4), (702, 100), (16384, 1_048_576)] {
            let a = address_of(col, row);
            assert_eq!(address_position(&a), Some((col, row)), "{a}");
        }
    }

    #[test]
    fn extent_is_at_least_the_stored_size() {
        let mut s = Sheet::with_size(3, 2);
        assert_eq!(s.extent(), (3, 2));
        s.cells
            .insert("D5".into(), CellInput::Number { value: 1.0 });
        assert_eq!(s.extent(), (5, 4));
    }

    #[test]
    fn formulas() {
        assert!(is_formula_source("=A1"));
        assert!(!is_formula_source("="));
        assert!(!is_formula_source("A1"));
    }
}
