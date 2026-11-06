// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `tpt` command line. `run` takes the arguments and two writers, so the commands can be
//! tested without starting a process.
//!
//! Commands: `convert`, `check`, `watch`, `diff` and `new`. Run `tpt --help` for the usage text.

mod check;
mod formats;
mod templates;
mod watch;

use std::io::Write;
use std::path::{Path, PathBuf};

use tpt_codex_core::{Block, ChangeKind, Document, TextChange, diff_documents, diff_text};
use tpt_typeset::Library;

pub use check::{Finding, Severity, check_document, excerpt};
pub use formats::{Format, Loaded, format_of, load, save};
pub use templates::{TEMPLATES, template};
pub use watch::Watcher;

/// The usage text.
pub const USAGE: &str = "\
tpt - convert, check and typeset tpt-codex documents

USAGE:
    tpt convert <INPUT> -o <OUTPUT> [--report]
    tpt check <INPUT> [--bib FILE] [--strict]
    tpt watch <INPUT> -o <OUTPUT> [--poll-ms N]
    tpt diff <OLD> <NEW>
    tpt new <TEMPLATE> [-o FILE] [--force]
    tpt new --list

Files are recognised by extension:
    read:   .cdx .md .docx .xlsx .pptx .json
    write:  .cdx .md .docx .xlsx .json .pdf

OPTIONS:
    -o, --output FILE   File to write.
    --report            Print the full fidelity report of an import or export.
    --bib FILE          Bibliography for checking citations (CSL-JSON, or BibTeX for .bib).
    --strict            Treat warnings as errors.
    --poll-ms N         How often `watch` looks for a change (default 500).
    --force             Overwrite an existing file.
    -h, --help          Show this text.

Exit status: 0 success, 1 the command found problems or failed, 2 bad usage.";

#[derive(Debug, Default)]
struct Opts {
    positional: Vec<String>,
    output: Option<String>,
    bib: Option<String>,
    report: bool,
    strict: bool,
    force: bool,
    list: bool,
    poll_ms: Option<u64>,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut opts = Opts::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |label: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{label} needs a value"))
        };
        match name.as_str() {
            "-o" | "--output" => opts.output = Some(value("--output")?),
            "--bib" => opts.bib = Some(value("--bib")?),
            "--poll-ms" => {
                let text = value("--poll-ms")?;
                let n: u64 = text.parse().ok().filter(|n| *n > 0).ok_or_else(|| {
                    format!("--poll-ms must be a positive whole number, got '{text}'")
                })?;
                opts.poll_ms = Some(n);
            }
            "--report" => opts.report = true,
            "--strict" => opts.strict = true,
            "--force" => opts.force = true,
            "--list" => opts.list = true,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown option '{flag}'"));
            }
            _ => opts.positional.push(arg.clone()),
        }
        i += 1;
    }
    Ok(opts)
}

/// Runs the command in `args` (without the program name). Returns the exit status.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(command) = args.first() else {
        let _ = writeln!(err, "{USAGE}");
        return 2;
    };
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        let _ = writeln!(out, "{USAGE}");
        return 0;
    }
    let opts = match parse_opts(&args[1..]) {
        Ok(opts) => opts,
        Err(message) => {
            let _ = writeln!(err, "tpt: {message}\n\n{USAGE}");
            return 2;
        }
    };
    let result = match command.as_str() {
        "convert" => convert(&opts, out, err),
        "check" => check_command(&opts, out, err),
        "watch" => watch_command(&opts, out, err),
        "diff" => diff_command(&opts, out),
        "new" => new_command(&opts, out),
        other => Err(Failure::Usage(format!("unknown command '{other}'"))),
    };
    match result {
        Ok(status) => status,
        Err(Failure::Usage(message)) => {
            let _ = writeln!(err, "tpt: {message}\n\n{USAGE}");
            2
        }
        Err(Failure::Run(message)) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

enum Failure {
    Usage(String),
    Run(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Run(message)
    }
}

fn one_input(opts: &Opts) -> Result<&Path, Failure> {
    match opts.positional.as_slice() {
        [one] => Ok(Path::new(one)),
        [] => Err(Failure::Usage("an input file is required".into())),
        _ => Err(Failure::Usage("only one input file is accepted".into())),
    }
}

fn output_of(opts: &Opts) -> Result<&Path, Failure> {
    opts.output
        .as_deref()
        .map(Path::new)
        .ok_or_else(|| Failure::Usage("-o <OUTPUT> is required".into()))
}

fn print_report(
    label: &str,
    report: &tpt_codex_interop::FidelityReport,
    full: bool,
    out: &mut dyn Write,
) {
    use tpt_codex_interop::Fidelity;
    if full {
        let _ = writeln!(out, "{label}:\n{report}");
    } else if !report.is_lossless() {
        let _ = writeln!(
            out,
            "{label}: {} approximated, {} dropped (use --report for details)",
            report.count(Fidelity::Approximated),
            report.count(Fidelity::Dropped)
        );
    }
}

fn convert(opts: &Opts, out: &mut dyn Write, _err: &mut dyn Write) -> Result<i32, Failure> {
    let input = one_input(opts)?;
    let output = output_of(opts)?;
    let loaded = load(input)?;
    if let Some(report) = &loaded.report {
        print_report("import", report, opts.report, out);
    }
    if let Some(report) = save(&loaded.document, output)? {
        print_report("export", &report, opts.report, out);
    }
    let _ = writeln!(out, "{} -> {}", input.display(), output.display());
    Ok(0)
}

fn load_library(path: &str) -> Result<Library, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: cannot read: {e}"))?;
    let bibtex = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("bib"));
    let library = if bibtex {
        Library::from_bibtex(&text)
    } else {
        Library::from_csl_json(&text)
    };
    library.map_err(|e| format!("{path}: {e}"))
}

fn check_command(opts: &Opts, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32, Failure> {
    let input = one_input(opts)?;
    let doc = load(input)?.document;
    let library = opts.bib.as_deref().map(load_library).transpose()?;
    let findings = check_document(&doc, library.as_ref());
    let mut errors = 0;
    let mut warnings = 0;
    for finding in &findings {
        let label = match finding.severity {
            Severity::Error => {
                errors += 1;
                "error"
            }
            Severity::Warning => {
                warnings += 1;
                "warning"
            }
        };
        let _ = writeln!(
            out,
            "{}: {label}: {}: {}",
            input.display(),
            excerpt(&doc, finding.block),
            finding.message
        );
    }
    if library.is_none() {
        let citations = check::citation_count(&doc);
        if citations > 0 {
            let _ = writeln!(
                err,
                "note: {citations} citation(s) were not checked; pass --bib to check them"
            );
        }
    }
    let _ = writeln!(out, "{errors} error(s), {warnings} warning(s)");
    Ok(if errors > 0 || (opts.strict && warnings > 0) {
        1
    } else {
        0
    })
}

fn watch_command(opts: &Opts, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32, Failure> {
    let input = one_input(opts)?;
    let output = output_of(opts)?;
    let poll = std::time::Duration::from_millis(opts.poll_ms.unwrap_or(500));
    let mut watcher = Watcher::default();
    let _ = writeln!(out, "watching {} (ctrl-c to stop)", input.display());
    loop {
        if watcher.changed(input) {
            let started = std::time::Instant::now();
            match load(input).and_then(|loaded| save(&loaded.document, output)) {
                Ok(_) => {
                    let _ = writeln!(
                        out,
                        "wrote {} in {} ms",
                        output.display(),
                        started.elapsed().as_millis()
                    );
                }
                Err(message) => {
                    let _ = writeln!(err, "{message}");
                }
            }
        }
        std::thread::sleep(poll);
    }
}

fn text_of(doc: &Document, id: tpt_codex_core::BlockId) -> Option<&str> {
    match doc.get(id) {
        Some(Block::Text { content, .. }) => Some(content),
        _ => None,
    }
}

fn diff_command(opts: &Opts, out: &mut dyn Write) -> Result<i32, Failure> {
    let [old, new] = opts.positional.as_slice() else {
        return Err(Failure::Usage("diff needs two files: <OLD> <NEW>".into()));
    };
    let a = load(Path::new(old))?.document;
    let b = load(Path::new(new))?.document;
    let changes = diff_documents(&a, &b);
    for change in &changes {
        let (kind, doc) = match change.kind {
            ChangeKind::Added => ("added  ", &b),
            ChangeKind::Removed => ("removed", &a),
            ChangeKind::Changed => ("changed", &b),
        };
        let _ = writeln!(out, "{kind} {}", excerpt(doc, change.block));
        if change.kind == ChangeKind::Changed
            && let (Some(x), Some(y)) = (text_of(&a, change.block), text_of(&b, change.block))
        {
            let mut line = String::new();
            for piece in diff_text(x, y) {
                match piece {
                    TextChange::Same(s) => line.push_str(&s),
                    TextChange::Added(s) => line.push_str(&format!("[+{s}+]")),
                    TextChange::Removed(s) => line.push_str(&format!("[-{s}-]")),
                }
            }
            let _ = writeln!(out, "        {line}");
        }
    }
    let _ = writeln!(out, "{} block(s) differ", changes.len());
    Ok(if changes.is_empty() { 0 } else { 1 })
}

fn new_command(opts: &Opts, out: &mut dyn Write) -> Result<i32, Failure> {
    if opts.list {
        for (name, description, _) in TEMPLATES {
            let _ = writeln!(out, "{name:<8} {description}");
        }
        return Ok(0);
    }
    let [name] = opts.positional.as_slice() else {
        return Err(Failure::Usage(
            "new needs a template name (see `tpt new --list`)".into(),
        ));
    };
    let Some(text) = template(name) else {
        return Err(Failure::Usage(format!(
            "no template named '{name}' (see `tpt new --list`)"
        )));
    };
    let path: PathBuf = opts
        .output
        .clone()
        .map_or_else(|| PathBuf::from(format!("{name}.cdx")), PathBuf::from);
    if path.exists() && !opts.force {
        return Err(Failure::Run(format!(
            "{} already exists (use --force to overwrite)",
            path.display()
        )));
    }
    std::fs::write(&path, text).map_err(|e| format!("{}: cannot write: {e}", path.display()))?;
    let _ = writeln!(out, "created {}", path.display());
    Ok(0)
}
