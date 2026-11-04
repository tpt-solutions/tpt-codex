// SPDX-License-Identifier: MIT OR Apache-2.0
//! Live-grid evaluation for tpt-codex.
//!
//! [`GridSession`] keeps a `tpt-lattice` evaluator for one sheet alive. Each change is applied
//! with [`GridSession::set`], and [`GridSession::results`] recomputes only the cells that are
//! affected. [`evaluate_sheet`] is the one-shot form.
//!
//! The core stores the inputs; this crate computes the results. It is `std` because
//! `tpt-lattice-evaluator` is.
//!
//! Errors stay in their cell. A formula that does not parse, a cycle, or a bad reference shows
//! as [`CellResult::Error`] in that cell, and the other cells still compute.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

use tpt_codex_core::{CellInput, Sheet, is_valid_address};
use tpt_lattice_core::{CellId, CellValue};
use tpt_lattice_evaluator::Evaluator;

pub use tpt_codex_core as core;

/// The computed value of one non-empty cell.
#[derive(Debug, Clone, PartialEq)]
pub enum CellResult {
    /// The cell was cleared, or it holds a value the grid cannot show.
    Empty,
    /// A number. Dates are shown as their serial number.
    Number(f64),
    /// Text.
    Text(String),
    /// A boolean.
    Boolean(bool),
    /// An error, with a message. Covers parse errors, cycles, division by zero and other evaluation errors.
    Error(String),
}

/// A change that could not be applied to a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridError {
    /// The address is not canonical A1 notation.
    InvalidAddress(String),
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::InvalidAddress(a) => write!(f, "'{a}' is not a valid cell address"),
        }
    }
}

impl std::error::Error for GridError {}

/// A sheet being edited: the inputs plus an evaluator that recomputes only what changed.
pub struct GridSession {
    evaluator: Evaluator,
    inputs: BTreeMap<String, CellInput>,
    /// Formula cells whose source did not parse. They are reported as errors and not evaluated.
    parse_errors: BTreeMap<String, String>,
}

impl std::fmt::Debug for GridSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridSession")
            .field("inputs", &self.inputs)
            .field("parse_errors", &self.parse_errors)
            .finish_non_exhaustive()
    }
}

impl GridSession {
    /// Starts a session with the cells of `sheet`.
    pub fn new(sheet: &Sheet) -> Self {
        let mut session = Self {
            evaluator: Evaluator::new(),
            inputs: BTreeMap::new(),
            parse_errors: BTreeMap::new(),
        };
        for (address, input) in &sheet.cells {
            // Addresses from a validated sheet are valid, so a failure here is ignored and
            // reported by `results` as an error cell.
            let _ = session.set(address, Some(input.clone()));
        }
        session
    }

    /// Sets or clears one cell. `None` clears it. Nothing is recomputed until [`results`](Self::results).
    pub fn set(&mut self, address: &str, input: Option<CellInput>) -> Result<(), GridError> {
        // The core's rule is canonical upper-case A1 notation, which is stricter than the evaluator.
        if !is_valid_address(address) {
            return Err(GridError::InvalidAddress(address.to_string()));
        }
        let id = CellId::try_from_a1(address)
            .map_err(|_| GridError::InvalidAddress(address.to_string()))?;
        self.parse_errors.remove(address);
        self.inputs.remove(address);
        let Some(input) = input else {
            self.evaluator.set_value(id, CellValue::Empty);
            return Ok(());
        };
        match &input {
            CellInput::Number { value } => self.evaluator.set_value(id, CellValue::Number(*value)),
            CellInput::Text { value } => {
                self.evaluator.set_value(id, CellValue::Text(value.clone()))
            }
            CellInput::Boolean { value } => {
                self.evaluator.set_value(id, CellValue::Boolean(*value))
            }
            CellInput::Formula { source } => {
                if let Err(e) = self.evaluator.set_formula(id, source) {
                    // Drop any previous value or formula in the cell, and remember the error.
                    self.evaluator.set_value(id, CellValue::Empty);
                    self.parse_errors
                        .insert(address.to_string(), format!("{e:?}"));
                }
            }
        }
        self.inputs.insert(address.to_string(), input);
        Ok(())
    }

    /// Recomputes the cells affected by changes since the last call, and returns every
    /// non-empty cell keyed by A1 address.
    pub fn results(&mut self) -> BTreeMap<String, CellResult> {
        // The evaluator reports cycles and evaluation errors per cell, so this cannot fail.
        let _ = self.evaluator.evaluate();
        let mut out = BTreeMap::new();
        for (address, input) in &self.inputs {
            let result = match input {
                CellInput::Number { value } => CellResult::Number(*value),
                CellInput::Text { value } => CellResult::Text(value.clone()),
                CellInput::Boolean { value } => CellResult::Boolean(*value),
                CellInput::Formula { .. } => {
                    let id = CellId::try_from_a1(address).expect("address was validated by set");
                    to_result(self.evaluator.get_value(id))
                }
            };
            out.insert(address.clone(), result);
        }
        for (address, message) in &self.parse_errors {
            out.insert(address.clone(), CellResult::Error(message.clone()));
        }
        out
    }
}

/// Computes every non-empty cell of `sheet`, keyed by A1 address.
///
/// Addresses that are not valid A1 notation are reported as errors rather than skipped.
/// Cells that hold a value the evaluator cannot show as a scalar (lists) are `Empty`.
pub fn evaluate_sheet(sheet: &Sheet) -> BTreeMap<String, CellResult> {
    let mut results = GridSession::new(sheet).results();
    for address in sheet.cells.keys() {
        if CellId::try_from_a1(address).is_err() {
            results.insert(
                address.clone(),
                CellResult::Error(format!("'{address}' is not a valid cell address")),
            );
        }
    }
    results
}

fn to_result(value: CellValue) -> CellResult {
    match value {
        CellValue::Empty | CellValue::List(_) => CellResult::Empty,
        CellValue::Number(n) | CellValue::Date(n) => CellResult::Number(n),
        CellValue::Text(s) => CellResult::Text(s),
        CellValue::Boolean(b) => CellResult::Boolean(b),
        CellValue::Error(e) => CellResult::Error(format!("{e:?}")),
    }
}
