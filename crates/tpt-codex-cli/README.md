# tpt-codex-cli

The `tpt` command line: convert documents between formats, check them, rebuild on save, diff and
merge text files, and start from a template.

Status: pre-release, version 0.1.0-spec, `publish = false`. Not yet on crates.io.

## What it does

`tpt` has six commands. `tpt --help` prints the usage text.

| Command | What it does |
| --- | --- |
| `convert` | Reads a document and writes it in another format, printing the fidelity report of the import and export when anything was lost. |
| `check` | Runs the typesetter's checks (math, cross-references, figures, labels, footnotes, bindings, grid formulas and, with `--bib`, citations) and lists the problems. |
| `watch` | Rebuilds the output whenever the input file changes. Runs until interrupted. |
| `diff` | Compares two documents block by block, and shows changed paragraphs word by word. |
| `merge` | Three-way merge of two edited copies of a text file against their common base. |
| `new` | Writes a starter document from one of four templates, or lists them with `--list`. |

File formats are recognised by extension, through `tpt-codex-files`:

- read: `.cdx`, `.md`, `.markdown`, `.docx`, `.xlsx`, `.pptx`, `.pdf` (text only), `.json`
- write: `.cdx`, `.md`, `.docx`, `.xlsx` (the first grid), `.json`, `.pdf`

`.pptx` can be read but not written. A PDF is read as its text: paragraphs and headings.

The crate is a library and a binary. The library entry point, `tpt_codex_cli::run`, takes the
arguments and two writers, so the commands are tested without starting a process. The binary
(`src/main.rs`) passes the process arguments and the standard streams to it, and exits with the
status it returns.

The crate deliberately does not:

- Watch directories or several files at once. `watch` follows one input file.
- Write `.pptx`, or any format not listed above.
- Check citations without a bibliography. Without `--bib`, citations are skipped and a note says how
  many.
- Change its inputs. `merge` writes a new file.

## Where it sits in the workspace

Depends on:

- `tpt-codex-core` for the document model, the block diff and the three-way merge.
- `tpt-codex-files` for opening and saving files by extension (also used by the browser demo).
- `tpt-codex-grid` for evaluating grid formulas during `check`.
- `tpt-typeset` for the typesetter's checks and the bibliography readers.

Nothing in the workspace depends on this crate. The acceptance crate
([`tpt-codex-acceptance`](../tpt-codex-acceptance/README.md)) does not use it either.

## Command reference

Help is recognised only as the first argument: `tpt -h`, `tpt --help` and `tpt help` print the usage
text. `tpt convert --help` is an unknown-option error (exit status 2).

### `tpt convert <INPUT> -o <OUTPUT> [--report]`

Reads `<INPUT>` and writes `<OUTPUT>`. The output format is taken from `<OUTPUT>`'s extension.

- `-o, --output FILE` is required.
- `--report` prints the full fidelity report of the import and of the export. Without it, a lossy
  step prints one summary line (`import: N approximated, M dropped (use --report for details)`), and
  a lossless step prints nothing.
- On success it prints `<INPUT> -> <OUTPUT>`.
- Relative picture addresses in the document are read from the folder of `<INPUT>`.
- Writing a `.pptx` output is an error (exit status 1).

### `tpt check <INPUT> [--bib FILE] [--strict]`

Lists each problem as `<file>: error|warning: block N "excerpt": message`, then a count line,
`N error(s), M warning(s)`.

- Errors: math that does not parse (with line, column and hint); inline math errors; a
  cross-reference to a heading anchor that no heading has; a reference to a figure or label that
  nothing has; a grid formula error; a `[[name]]` binding with no value; a footnote reference with no
  definition; a citation missing from the bibliography.
- Warnings: a footnote that is never referenced.
- `--bib FILE` gives the bibliography. A file ending in `.bib` is read as BibTeX; any other file as
  CSL-JSON.
- `--strict` makes warnings fail the command.
- Exit status is 1 when there are errors (or warnings, with `--strict`), otherwise 0.
- Without `--bib`, a note on standard error says how many citations were not checked.

### `tpt watch <INPUT> -o <OUTPUT> [--poll-ms N]`

Prints `watching <INPUT> (ctrl-c to stop)`, writes `<OUTPUT>` once at the start, then rewrites it
whenever `<INPUT>`'s modification time or length changes. After each rebuild it prints
`wrote <OUTPUT> in N ms`.

- `-o, --output FILE` is required.
- `--poll-ms N` sets how often the input is checked; the default is 500. `N` must be a positive whole
  number.
- If the input has an error, the error goes to standard error and the last good output is left as it
  was.
- The command never exits on its own. Stop it with ctrl-c.
- Changes are detected by polling file metadata, not by OS file-notification APIs.

### `tpt diff <OLD> <NEW>`

Prints one line per block that differs, as `added`, `removed` or `changed`, with a short excerpt of
the block. A changed paragraph is followed by its words, with removed words as `[-removed-]` and
added words as `[+added+]`. The last line is `N block(s) differ`.

Exit status is 1 when the files differ, 0 when they do not.

### `tpt merge <BASE> <OURS> <THEIRS> -o <OUTPUT> [--prefer ours|theirs]`

Merges two edited copies of `<BASE>`. Before merging, each side's blocks are matched with the base
by kind and word overlap, so a paragraph added near the top does not shift every later block. The
core's three-way merge then combines the edits.

- `-o, --output FILE` is required.
- `--prefer ours` or `--prefer theirs` settles every conflict in that side's favour. Without it,
  conflicts are left for you. In `<OUTPUT>`, an unresolved conflict keeps the "ours" text.
- Each conflict is printed as `conflict: <place>: <outcome>`, then
  `merged into <OUTPUT> (N conflict(s), M unresolved)`.
- Exit status is 1 when any conflict is unresolved, otherwise 0. A set of files that cannot be merged
  at all is an error, also exit status 1.

### `tpt new <TEMPLATE> [-o FILE] [--force]` and `tpt new --list`

Writes a starter document. The templates are:

| Name | Description |
| --- | --- |
| `article` | An article with math, a footnote and a live table. |
| `report` | A report with sections and a figures table. |
| `slides` | A three-slide 16:9 presentation. |
| `budget` | A budget grid with formulas. |

- Without `-o`, the file is `<TEMPLATE>.cdx` in the current folder.
- An existing file is not overwritten unless `--force` is given.
- `--list` prints each template's name and description, and exits 0.

### Options and exit status

| Option | Used by | Meaning |
| --- | --- | --- |
| `-o, --output FILE` | `convert`, `watch`, `merge`, `new` | File to write. |
| `--report` | `convert` | Print the full fidelity report. |
| `--bib FILE` | `check` | Bibliography for citations (CSL-JSON, or BibTeX for `.bib`). |
| `--strict` | `check` | Treat warnings as errors. |
| `--poll-ms N` | `watch` | Polling interval in milliseconds (default 500). |
| `--prefer SIDE` | `merge` | `ours` or `theirs`. |
| `--force` | `new` | Overwrite an existing file. |
| `--list` | `new` | List the templates. |
| `-h, --help` | (first argument only) | Show the usage text. |

Long options also accept `--option=value`. Options are not checked against the command: an option a
command does not use is accepted and ignored.

Exit status:

- `0`: success.
- `1`: the command found problems or failed. That covers check errors, differing files, unresolved
  merge conflicts, a file that cannot be read or written, and a format that cannot be written.
- `2`: bad usage. That covers an unknown command or option, a missing or extra argument, and a
  missing `-o`. The usage text follows the message.

Error messages for a file begin with its name. A Markdown error reads `line:column: error: message
(hint: ...)`.

## Library API

The library exposes the same code the binary runs:

- `run(args, out, err) -> i32`: runs one command. `args` is the argument list without the program
  name.
- `USAGE`: the usage text.
- `check_document(doc, library) -> Vec<Finding>`, with `Finding { severity, block, message }`,
  `Severity::{Error, Warning}`, and `excerpt(doc, block)`.
- `load(path)`, `save(doc, path)`, `save_from(doc, path, base)`, `format_of(path)` and `Format`.
  `load` returns a `Loaded` with the `document` and an optional import `report`.
- `TEMPLATES` and `template(name)`: the starter documents.
- `Watcher::changed(path) -> bool`: the change detection that `watch` uses.

The library exists for the crate's tests and for embedding the commands. It is pre-release and makes
no stability promise.

```rust,ignore
use tpt_codex_cli::run;

fn main() {
    let args: Vec<String> = ["convert", "report.docx", "-o", "report.cdx", "--report"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let status = run(&args, &mut std::io::stdout(), &mut std::io::stderr());
    std::process::exit(status);
}
```

## Pictures and files

- A picture in a document is drawn into PDF output. A relative picture address is resolved against
  the folder of the input file. A picture file over 64 MB is not read, and the picture stays a
  placeholder.
- An absolute picture address is read from wherever it points, and a relative one can climb out of
  the input's folder with `..`. Export only documents from sources you trust, because exporting reads
  the picture files they name.

## Determinism

The same input gives byte-for-byte the same PDF on every run (`pdf_output_is_deterministic` in
`tests/cli.rs`). `watch` rebuilds from the file's current contents.

## Testing

```sh
cargo test -p tpt-codex-cli
```

- `tests/cli.rs` runs the commands on the Office fixtures in `fixtures/office` and on temporary
  files. It covers: every template checks clean and exports to PDF and Markdown; PDF output is
  deterministic; `.docx` and `.xlsx` conversion with the fidelity report; a Word file with bold and
  italic text converts to `.cdx` and back; a Markdown error is reported with file, line and column;
  `check` finds unresolved references and bad math; citations are checked only with a bibliography;
  `--strict`; `diff` output and exit status; `new` refuses to overwrite without `--force`; bad usage
  exits with 2; unreadable and unknown files fail with a message; JSON round-trips the model; the
  watcher reports the first look and each change; live values reach the Word file; and `check` reports
  formula errors and unresolved values.
- `tests/merge.rs` covers `tpt merge`: edits combine when a side adds a paragraph; a real conflict is
  left to the user unless a side is named; a removed and an added paragraph both happen; paragraphs
  added on both sides are both kept; merging a file with itself changes nothing; and lists, footnotes
  and grids line up.

## Documentation

- [Quickstart](../../docs/quickstart.md): install, first document, watch, Office conversion, CI use.
- [`.cdx` cheat-sheet](../../docs/cdx-cheatsheet.md): the document syntax.
- [Markdown grammar](../../docs/markdown-grammar.md): the `.cdx` grammar in full.
- [Migrating from Office](../../docs/migrating-from-office.md): what comes across from Word, Excel and
  PowerPoint.

## Install

```sh
cargo install --path crates/tpt-codex-cli      # puts `tpt` on your PATH
cargo run -p tpt-codex-cli -- --help           # without installing
```

## License

Dual-licensed under MIT OR Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and
[LICENSE-APACHE](../../LICENSE-APACHE).
