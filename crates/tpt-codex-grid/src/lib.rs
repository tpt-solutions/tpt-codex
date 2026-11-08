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
//! [`resolve_document`] fills `[[sheet1.B4]]` in prose with that cell's computed value, for output
//! that is not live.
//!
//! Errors stay in their cell. A formula that does not parse, a cycle, or a bad reference shows
//! as [`CellResult::Error`] in that cell, and the other cells still compute.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

use tpt_codex_core::{CellInput, Document, Sheet, is_valid_address};
use tpt_lattice_core::{CellId, CellValue};
use tpt_lattice_evaluator::Evaluator;
use tpt_typeset::{Bindings, check_bindings, grid_cell_name, resolve_bindings};

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
                if let Err(e) = self.evaluator.set_formula(id, &translate_ranges(source)) {
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

/// Rewrites spreadsheet ranges to the evaluator's own syntax: `B2:B9` becomes `RANGE(B2, B9)`, so
/// `=SUM(B2:B9)` works as it does in Excel. Dollar signs in a reference (`$B$2:$B$9`) are dropped,
/// and references are upper-cased. Text inside double quotes is left alone.
pub fn translate_ranges(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 16);
    let mut i = 0;
    let mut in_text = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            in_text = !in_text;
        }
        let at_boundary = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
        if !in_text && at_boundary {
            if let Some((first, next)) = cell_reference(&chars, i) {
                if chars.get(next) == Some(&':') {
                    if let Some((second, end)) = cell_reference(&chars, next + 1) {
                        let boundary = chars
                            .get(end)
                            .is_none_or(|c| !(c.is_alphanumeric() || *c == '_' || *c == '('));
                        if boundary {
                            out.push_str(&format!("RANGE({first}, {second})"));
                            i = end;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// A cell reference at `at`: optional `$`, one to three letters, optional `$`, then digits.
/// Returns it upper-cased without dollar signs, and the index after it.
fn cell_reference(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut i = at;
    let mut text = String::new();
    if chars.get(i) == Some(&'$') {
        i += 1;
    }
    let letters = chars[i..]
        .iter()
        .take_while(|c| c.is_ascii_alphabetic())
        .count();
    if !(1..=3).contains(&letters) {
        return None;
    }
    text.extend(chars[i..i + letters].iter().map(|c| c.to_ascii_uppercase()));
    i += letters;
    if chars.get(i) == Some(&'$') {
        i += 1;
    }
    let digits = chars[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    text.extend(&chars[i..i + digits]);
    Some((text, i + digits))
}

/// A number as it reads in prose: whole numbers without a decimal point, and other numbers to at
/// most ten decimals with no trailing zeros, so `0.1 + 0.2` shows as `0.3`.
pub fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return format!("{n}");
    }
    let text = format!("{n:.10}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        return String::from("0");
    }
    text.to_string()
}

/// The values of every grid cell in `doc`, bound under `sheet<id>.<address>` (for example
/// `sheet1.B4`), so that `[[sheet1.B4]]` in a text block can show them. Formulas are evaluated.
/// Empty cells and cells with an error are not bound.
pub fn grid_bindings(doc: &Document) -> Bindings {
    let mut bindings = Bindings::new();
    for (&id, sheet) in &doc.sheets {
        for (address, result) in evaluate_sheet(sheet) {
            let value = match result {
                CellResult::Number(n) => format_number(n),
                CellResult::Text(s) => s,
                CellResult::Boolean(true) => String::from("TRUE"),
                CellResult::Boolean(false) => String::from("FALSE"),
                CellResult::Empty | CellResult::Error(_) => continue,
            };
            bindings.set(&grid_cell_name(id, &address), &value);
        }
    }
    bindings
}

/// `doc` with each `[[sheet<id>.<address>]]` replaced by that cell's computed value, for output
/// that is not live (a PDF, a Word file). `None` when the document has no such binding, so
/// nothing is evaluated or copied in the common case.
pub fn resolve_document(doc: &Document) -> Option<Document> {
    if check_bindings(doc, &Bindings::new()).is_empty() {
        return None;
    }
    resolve_bindings(doc, &grid_bindings(doc))
}

/// A copy of `doc` in which every formula cell holds its computed value, as text, number or
/// boolean. A formula that failed holds its error message as text, and an empty result holds
/// empty text. For formats with no formulas, such as Word tables.
pub fn with_computed_cells(doc: &Document) -> Document {
    let mut out = doc.clone();
    for sheet in out.sheets.values_mut() {
        let results = evaluate_sheet(sheet);
        for (address, input) in sheet.cells.iter_mut() {
            if !matches!(input, CellInput::Formula { .. }) {
                continue;
            }
            *input = match results.get(address) {
                Some(CellResult::Number(n)) => CellInput::Number { value: *n },
                Some(CellResult::Boolean(b)) => CellInput::Boolean { value: *b },
                Some(CellResult::Text(s)) => CellInput::Text { value: s.clone() },
                Some(CellResult::Error(message)) => CellInput::Text {
                    value: format!("error: {message}"),
                },
                Some(CellResult::Empty) | None => CellInput::Text {
                    value: String::new(),
                },
            };
        }
    }
    out
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
