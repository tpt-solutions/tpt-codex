# Quickstart

tpt-codex works on documents written as plain text (`.cdx`, which is Markdown plus grids, slides and
math) and converts to and from Word, Excel and PowerPoint files. Everything is done with the `tpt`
command.

## Install

```sh
cargo install --path crates/tpt-codex-cli      # puts `tpt` on your PATH
# or, without installing:
cargo run -p tpt-codex-cli -- --help
```

Prebuilt binaries are attached to tagged releases once those are published.

## Your first document

```sh
tpt new article            # writes article.cdx
tpt check article.cdx      # finds broken references, bad math, missing footnotes
tpt convert article.cdx -o article.pdf
```

`tpt new --list` shows the templates: `article`, `report`, `slides` and `budget`. Open the `.cdx`
file in any text editor. [The cheat-sheet](cdx-cheatsheet.md) lists the syntax.

## Rebuild on every save

```sh
tpt watch article.cdx -o article.pdf
```

The PDF is rewritten whenever the file changes. If the file has an error, the error is printed
(`article.cdx:12:1: error: …`) and the last good PDF is left alone.

## Bring in Office files

```sh
tpt convert report.docx -o report.cdx --report     # Word to text, with a list of what was lost
tpt convert budget.xlsx -o budget.cdx              # each worksheet becomes a live grid
tpt convert deck.pptx   -o deck.cdx                # each slide becomes a heading and its text
```

See [migrating from Office](migrating-from-office.md) for what comes across.

## Send Office files out

```sh
tpt convert article.cdx -o article.docx
tpt convert budget.cdx  -o budget.xlsx             # the first grid, with formulas
```

## Check in CI

`tpt check` exits with status 1 when it finds an error, so it can gate a build:

```sh
tpt check paper.cdx --bib refs.json --strict       # --strict also fails on warnings
```

`--bib` takes CSL-JSON, or BibTeX when the file ends in `.bib`. Without it, citations are not
checked and the command says how many were skipped.

## See what changed between two files

```sh
tpt diff old.cdx new.cdx
```

Changed paragraphs are shown word by word as `[-removed-]` and `[+added+]`. The exit status is 1
when the files differ.

## Merge two edited copies

```sh
tpt merge base.cdx mine.cdx theirs.cdx -o merged.cdx
```

Two people edited copies of `base.cdx`. Their changes are combined: different paragraphs, different
words in the same paragraph, added and removed paragraphs, list items, footnotes and grid cells.
Paragraphs are matched between the files by what they say, not by position, so a paragraph added near
the top does not confuse the merge. When both people changed the same words, `merged.cdx` keeps
yours, the conflict is listed, and the exit status is 1. Run again with `--prefer ours` or
`--prefer theirs` to settle every conflict one way.

## Exit status

| Status | Meaning |
| --- | --- |
| 0 | Success |
| 1 | The command found problems (for `merge`, conflicts left unresolved), or could not read or write a file |
| 2 | Bad usage (unknown command or option, missing argument) |

## The browser editor

`crates/tpt-codex-web` builds the editor to WebAssembly (`just serve`, or `scripts/build-web.sh`).
The page opens a `.cdx`, `.md`, `.docx`, `.xlsx`, `.pptx` or `.json` file (the Open button, or drop a
file on the page), shows what an import could not keep, saves as `.cdx`, `.docx`, `.xlsx`, `.pdf` or
`.json`, and keeps your document in the browser's local storage so it is there next time (Start over
clears it). See its [README](../crates/tpt-codex-web/README.md).

## Collaboration

`tpt-codex-relay` is a small WebSocket relay that lets several editors share a document. See its
[README](../crates/tpt-codex-sync/README.md). It has no authentication, so run it on a trusted
network.
