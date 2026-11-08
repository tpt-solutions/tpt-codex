# Migrating from Word, Excel and PowerPoint

Every import gives a **fidelity report**: a list of what was kept exactly, approximated or dropped.
`tpt convert` prints a one-line summary, and `--report` prints every item.

```sh
tpt convert thesis.docx -o thesis.cdx --report
```

Nothing is dropped silently. Read the report before you rely on the result.

## Word (`.docx`)

| Comes across | Notes |
| --- | --- |
| Paragraphs, `Title` and `Heading 1`–`6` styles | Headings keep their level |
| Bold and italic | A whole paragraph keeps its flag. Parts of a paragraph become runs. Spaces at the edge of a run are moved out of it |
| Line breaks and tabs | |
| Title and author | Into the front matter |
| Tables | Become grids: each cell is a cell, and numbers stay numbers |

| Approximated or lost | What happens |
| --- | --- |
| Merged table cells | Kept as separate cells |
| List items | Kept as plain paragraphs without markers |
| Images | Dropped |
| Footnotes, comments, tracked changes, styles beyond headings | Not read |

Known limit: `.cdx` has no nested emphasis. A bold paragraph with an italic word inside it cannot be
written as `.cdx`, so `tpt convert x.docx -o x.cdx` stops and names the paragraph. Convert to `.json`,
`.pdf` or `.docx` instead.

Writing `.docx` keeps headings, bold, italic and runs. Grids are written as Word tables, with each
formula cell's computed value. Links are written as plain text, lists lose their markers, and images
and queries are not written. The report says which.

## Excel (`.xlsx`)

Each worksheet becomes a heading with its name and a live grid. Formulas are kept and recalculated.
Dates and number formats come in as plain numbers, and merged cells keep only their top-left value.

Writing `.xlsx` writes the first grid, with formulas and their computed values.

## PowerPoint (`.pptx`)

Each slide becomes a level-2 heading (the slide's title, or `Slide N`) followed by its text.
Pictures, tables, speaker notes, animations and formatting are reported and not imported.

## PDF

PDF is an output format. Reading PDFs is not supported yet.

## What you gain

- Text files that work with version control: `tpt diff` shows word-level changes.
- Spreadsheet cells inside a document, recalculated when the PDF is built.
- One source for a printed report and a slide deck.
- `tpt check` in CI to catch broken references before a document goes out.
