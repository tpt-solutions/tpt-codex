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

The text of a PDF comes through as paragraphs. Lines that sit together become one paragraph, a line
that starts a list item is a paragraph of its own, and the largest type sizes become headings (the
largest is level 1). A PDF this project writes reads back as the text it shows.

| Comes across | Notes |
| --- | --- |
| Text, paragraphs and headings | Read from the page, in reading order |
| Title and author | From the file's information |

| Left out or approximated | What happens |
| --- | --- |
| Images and drawings | Not imported; counted in the report |
| Bold and italic | Read as plain text; the report says how many characters |
| Page numbers in a footer | Left out; counted in the report |
| Text without a character map | A guess from the character codes, reported as approximated |
| Tables, columns, nested list structure | Read line by line; the layout is not rebuilt |
| Encrypted files | Refused, with the reason |

A scanned PDF has no text to read, so it imports as an empty document with the report saying why.

## What you gain

- Text files that work with version control: `tpt diff` shows word-level changes.
- Spreadsheet cells inside a document, recalculated when the PDF is built.
- One source for a printed report and a slide deck.
- `tpt check` in CI to catch broken references before a document goes out.
