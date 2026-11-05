// SPDX-License-Identifier: MIT OR Apache-2.0
//! Times live-grid evaluation: a full pass over a generated sheet, and an incremental pass
//! after one input changes. Run with:
//! `cargo run --release -p tpt-codex-grid --example bench_eval -- 1000`
//! The argument is the number of rows. Each row has an input, a formula that uses it, and a
//! formula that uses that formula, so a change at the top touches the whole chain.

use std::time::Instant;

use tpt_codex_core::{CellInput, Sheet};
use tpt_codex_grid::GridSession;

fn main() {
    let rows: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1_000);

    let mut sheet = Sheet::default();
    for r in 1..=rows {
        sheet
            .cells
            .insert(format!("A{r}"), CellInput::Number { value: r as f64 });
        sheet.cells.insert(
            format!("B{r}"),
            CellInput::Formula {
                source: format!("=A{r}*2+1"),
            },
        );
        sheet.cells.insert(
            format!("C{r}"),
            CellInput::Formula {
                source: format!("=B{r}+A{r}"),
            },
        );
    }

    let t = Instant::now();
    let mut session = GridSession::new(&sheet);
    let results = session.results();
    let full_time = t.elapsed();

    let t = Instant::now();
    session
        .set("A1", Some(CellInput::Number { value: 42.0 }))
        .expect("A1 is a valid address");
    let changed = session.results();
    let incremental_time = t.elapsed();

    println!(
        "cells={} full={:?} incremental_one_input={:?}",
        results.len(),
        full_time,
        incremental_time
    );
    println!(
        "check: B1={:?} C1={:?}",
        changed.get("B1"),
        changed.get("C1")
    );
}
