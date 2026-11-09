# Markdown grammar (supported subset)

This is the grammar that `tpt-codex-markdown` reads and writes today. Anything outside it is a
structured error when read, and a refusal when written. The spec's Codex Flavored Markdown
extensions are partly in this subset: grids and slides (see Containers), figure attributes (see
Images), display and inline math, and citations.

The reader and the writer share the same line predicates (`src/syntax.rs`). A document the writer
accepts reads back as the same document. The tests check this on a fixed fixture and on random input.

The subset follows CommonMark for the constructs it reads. Its inline syntax is smaller than CommonMark's,
and any construct whose reading would differ from CommonMark is an error rather than a silent change.

## Notation

EBNF-style: `*` zero or more, `+` one or more, `?` optional, `|` alternative, `"…"` literal text.
Line endings are `\n` or `\r\n`.

```ebnf
document     = front-matter? body ;

front-matter = "---" NL { fm-entry | blank } "---" NL ;
fm-entry     = fm-key ":" SP* fm-value NL ;
fm-key       = "title" | "author" | "date" | "class" ;
fm-value     = bare-value | quoted-value ;
bare-value   = any text without a line break ;
quoted-value = '"' { fm-char } '"' ;
fm-char      = any char except '"' and '\' | '\"' | '\\' ;

body         = { blank | block } ;
blank        = SP* NL ;
block        = heading | fence | rule | image | math | list | quote | paragraph ;

heading      = hashes ( SP text )? NL ;   (* text is trimmed; empty text is an error *)
hashes       = "#" | "##" | "###" | "####" | "#####" | "######" ;
              (* a 7+ hash run, or a hash run not followed by a space, is not a heading *)

fence        = fence-open { line } fence-close ;
fence-open   = indent{0,3} run info NL ;      (* run is 3+ backticks, or 3+ tildes *)
fence-close  = indent{0,3} run{same char, at least as long as the opener} SP* NL ;
info         = text without backticks (backtick fences), no backslash or '&' ;

rule         = indent{0,3} ( "***" | "---" | "___" with SP between marks ) SP* NL ;
              (* "---" directly under a paragraph line is a setext heading, which is an error *)

math         = indent{0,3} "$$" SP* NL { math-line } indent{0,3} "$$" SP* NL
              | indent{0,3} "$$" SP+ source SP+ "$$" SP* NL ;   (* one-line form; source has no "$$" *)
math-line    = any line that is not a closing "$$" line ;
image        = indent{0,3} "![" alt "](" uri ")" SP* NL ;
alt          = text without brackets, backslashes or '&' ;
uri          = text without spaces, parentheses, angle brackets, backslashes or '&' ;

list         = list-item { NL list-item } ;
list-item    = indent marker SP+ text NL ;
              (* an item indented further than the item above it is a sub-list of that item *)
marker       = "-" | "*" | "+" | digits{1,9} ( "." | ")" ) ;
              (* a list of one kind is a run of items with the same marker character;
                 changing the marker starts a new list *)

quote        = quote-line { NL quote-line } ;
quote-line   = indent{0,3} ">" SP? line NL ;
              (* the lines behind the ">" are read as a body of blocks, so a quote can hold
                 paragraphs, lists, code and other quotes; a bare ">" line separates blocks *)

paragraph    = para-line { NL para-line } ;   (* ends at a blank line or any other block *)
para-line    = text-line ;                     (* must not start any block syntax above *)
```

Lines in a paragraph and list items are kept exactly as written, apart from the trimming
described below. Inline Markdown is read in paragraphs, headings and list items (see Inline below).
Other text is kept as written.

### Inline

```ebnf
inline       = { plain | escape | emphasis | strong | both | link } ;
escape       = "\" ASCII-punctuation ;      (* the punctuation character, literally *)
emphasis     = "*" text-no-edge-space "*" ;  (* closes with one star *)
strong       = "**" text-no-edge-space "**" ; (* closes with two stars *)
both         = "***" text-no-edge-space "***" ; (* bold and italic; closes with three stars *)
link         = "[" link-text "](" address [ " " title ] ")" ;
title        = '"' { character - '"' - ")" - NL } '"' ;  (* not empty; plain text, no escapes *)
citation     = "[@" key "]" ;               (* shows the key until its text is replaced *)
footnote-ref = "[^" label "]" ;            (* likewise, for a footnote label *)
binding      = "[[" name "]]" ;            (* a value the host supplies, shown by name until then *)
key          = letters, digits, and - _ : . ;
name         = letters, digits, and - _ : . ;  (* the same characters as a key *)
footnote-def = "[^" label "]: " text ;     (* a paragraph; its text becomes the footnote *)
link-text    = { plain | escape } ;          (* not empty; no unescaped * [ ] *)
address      = text without spaces, parentheses, angle brackets or backslashes ;
```

- A `*` or `**` opens emphasis when the next character is not a space. It closes when the text
  before it is not a space. An opening `**` is strong, and a `*` run of three after strong text
  closes strong and opens emphasis, so `**a***b*` reads as strong `a` then emphasis `b`.
- Emphasis does not nest, and a link cannot start inside emphasis. Three stars open bold and italic
  together, and close with three stars. A run of stars that is longer than the marker closes it and
  opens the next, so `***a****b*` reads as bold italic `a` then emphasis `b`.
- A backslash before ASCII punctuation is an escape. Any other backslash is literal.
- A link whose address starts with `@` is a citation of that key, and one that starts with `^` is a
  footnote reference. `[@key]` and `[^label]` are the bare forms, with the key or label as their
  text. A link to `#anchor` is a cross-reference to a heading.
- `[[name]]` is a binding. It reads as a run whose text is the name, and the typesetter replaces
  it with the value the host supplies for that name. A grid cell is named `sheet<id>.<address>`,
  such as `[[sheet1.B2]]`. PDF and Word export, and the paginated view, show a grid cell's computed
  value in its place (`tpt-codex-grid::resolve_document`). The writer writes it back as `[[name]]` only while its text is still the
  name. A binding inside emphasis or a link is an error, and so is an unclosed `[[`.
- A paragraph that starts `[^label]: ` is a footnote definition. Its text becomes the footnote's
  block, and it is written back in the same form. A plain paragraph that starts that way is escaped,
  so it reads back as text.
- A lone `*` or `[` that the subset cannot close is an error, with the fix "escape it with a
  backslash". It is never silently literal.
- The writer escapes `*`, `[`, `]` and `\` where they would start syntax. It refuses what the
  subset cannot express, such as a link inside emphasis or two addresses on one character.

### Lists

- A bulleted list item and an ordered list item starting at 1 can interrupt a paragraph. An ordered
  item with any other number cannot, so reading it after a paragraph is an error.
- Each item is one line. Its text is trimmed. An empty item is an error.
- A blank line between items makes the list loose in CommonMark. Loose lists are an error here.
- Only the number of the first item is kept. Later numbers are written from it, so `1. 1. 1.`
  reads as items 1, 2, 3. This matches how CommonMark renders the list.
- An item indented further than the item above it is a sub-list of that item. Sub-lists nest to any
  depth, and a sub-list may be bulleted under a numbered item or the other way round. The writer
  indents two spaces a level. A list's numbers count its own items, so a sub-list does not break the
  numbering of its parent.
- A sub-list cannot be the first thing in a list, since it has no item to belong to.

### Block quotes

- A line that starts with `>` (up to three spaces before it) is part of a block quote. The `>` and
  one space after it are removed, and the rest is read as a body of blocks.
- A bare `>` line separates two blocks inside the quote. A line without `>` ends the quote.
- A quote can hold any blocks, including lists and other quotes. It cannot be placed inside a slide.
- `>` at the start of a line inside a code fence is code, not a quote.

### Math blocks

- The source is the lines between two `$$` lines, kept exactly, including blank and indented lines.
  Math source is TeX-style text. The reader does not parse it. `tpt-typeset::check_math_blocks` does.
- A closing line is `$$` alone, with up to three leading spaces. A line with more indentation is
  source.
- `$$ source $$` on one line is also a math block. The source is trimmed. The writer always writes
  the multi-line form, so the one-line form reads back as the same block.
- A math block can follow text or a list directly, as a fence can. Text after the closing `$$` line
  starts a new paragraph.
- An empty source is an error, and so is a block that is still open at the end of the document.
- A display equation can be labelled. The closing line is `$$ {#eq:id}`, or the one-line form is
  `$$ source $$ {#eq:id}`. A label starts with `eq:` and then letters, digits, and `- _ : .`. A
  labelled equation is numbered from 1 among the equations, in reading order, and `[text](#eq:id)`
  is a cross-reference to it. Only one label is allowed.

### Code blocks

- The content is stored exactly, with no trimming, and is not escaped.
- A fence closes only with the same character and at least the same length as the opener.
  Up to three leading spaces are removed from each content line, to match the opener's indent.
- A block that is still open at the end of the document is an error.

### Images

- A line that starts with `![` must be a complete image: `![alt](uri)` with nothing after it but
  spaces. An image directly under text, or text directly under an image, is an error. Put a blank
  line between them.
- An image is a block. It is not an inline image inside a paragraph.
- Figure attributes can follow the image on the same line, in braces: `![alt](uri) {#fig:id width=80%}`.
  `#label` names the figure. A label is letters, digits, and `- _ : .`. `width=N%` sets the width from
  1% to 100%. Each attribute can appear once, and any other word is an error.
- A labelled image is a figure. Figures are numbered from 1 in reading order, and `[text](#fig:id)`
  is a cross-reference to the figure. An image with only a width has no label, so it is not numbered
  and cannot be referred to.

### Containers

- `::: grid { id="N" }` opens a live grid, where N is the sheet id. Each line inside is a table
  row, `| cell | cell |`, and every row has the same number of cells as the first. A `| --- |`
  row after some rows makes those rows header rows. There can be only one such row, and it must
  come after a row of cells. A cell reads like sheet input: `=` starts a formula, a number is a
  number, `TRUE` and `FALSE` are booleans, and anything else is text. Write `\|` for a pipe and
  `\\` for a backslash inside a cell. A line of `:::` alone closes the grid. A grid needs at least
  one row, and each grid needs its own id.
- `::: canvas { ratio="W:H" }` opens a slide, where W and H are whole numbers from 1 to 32. Its body
  is ordinary blocks: headings, paragraphs, lists, code, math, images and grids. A line of `:::`
  alone closes it. A slide cannot be inside another slide. The slide is 960 points wide, and its
  height follows the ratio.
- A grid can carry a table label: `::: grid { id="N" label="tab:id" }`. A label starts with `tab:`
  and then letters, digits, and `- _ : .`. Labelled tables are numbered from 1 among the tables, in
  reading order, and `[text](#tab:id)` is a cross-reference to the table.
- A grid can name cells: `::: grid { id="N" names="Total=B4,Rate=C2" }`. A name starts with a letter or an underscore, uses only letters, digits and underscores, and is not a cell address. `[[Total]]` shows that cell's value. A name is unique in the document, and the writer sorts names.
- Any other `:::` name is an error, and a grid or slide left open at the end of the document is an error.
- Attributes are `key="value"` pairs inside braces. Values cannot contain quotes.
- A pipe table outside a grid is a text table (`Block::Table`). Its first row is the header, under a line of `---` cells, and its body rows follow. Its cells are text: a cell that starts with `=` is not a formula. Every row has the same number of cells, and a row of only dashes is refused, since it would read as the separator.

## Unsupported syntax (read errors)

A line that starts with any of these is an error. The error names the construct and suggests a fix.

| Syntax | Examples |
| --- | --- |
| Indented code block | a line starting with 4 spaces or a tab |
| Other container block | `::: name` for a name other than grid or canvas |
| Raw HTML | a line starting with `<` |
| Setext heading | a line of `=`, or `---` under a paragraph line |
| Loose list | a blank line between two items of the same list |
| List continuation | a line under an item that is not indented into it |

Front matter problems are also errors: an unknown key, a line that is not `key: value`, a
quoted value with a bad escape, or a missing closing `---`.

## Writer refusals

`to_markdown` refuses rather than changing meaning. It refuses:

- a root that is not a flow container,
- queries, nested containers, sections, and slide frames (where blocks sit on a slide),
- grids with view bounds, a cell that would not read back as the same value or has a line break,
  or a row of only dashes (it would read back as the header separator),
- a sheet that no grid in the document shows,
- a slide size that matches no ratio from 1:1 to 32:32 at 960 points wide,
- figure attributes on anything but an image, or a label or width the reader would not read back,
- an equation label on anything but a display math block in the document, or a label without `eq:`,
- a table label without `tab:`,
- empty text, or text with leading or trailing spaces,
- a heading level outside 1 to 6, or a heading with a line break,
- a paragraph with a blank line, or a line that would read back as another block,
- a list with no items, a bulleted list not starting at 1, or a number above 999 999 999,
- a list item that is empty, has leading or trailing spaces, has a line break, or is made only of
  `-`, `*`, `_` and spaces (it would read back as a rule),
- two adjacent lists of the same kind, which would read back as one list,
- a numbered list that does not start at 1 directly after a paragraph,
- a list item that is neither a plain text block nor a sub-list,
- a sub-list as the first item of a list, or two adjacent sub-lists of the same kind,
- an empty block quote,
- a code block whose language has a line break, backtick, backslash or entity, or whose content has a
  carriage return,
- an image that is not a URI source, has an empty URI, a URI with spaces, parentheses, angle brackets,
  backslashes or entities, an empty alt text (write `None` instead), or alt text with brackets,
  backslashes, entities or line breaks,
- math source that is empty, contains a carriage return, or has a line that would close the block
  (`$$` alone, with up to three leading spaces),
- paragraph text that starts with `$$`, which would read back as a math block,
- front matter values with line breaks.

The writer writes a thematic break as `***`, not `---`. A `---` line as the first line of a
document reads as front matter.

## Bold and italic flags

A text block's own `bold` and `italic` flags have no Markdown form, so the writer writes them as
emphasis around the whole text (`**text**`, `*text*` or `***text***`), in paragraphs, headings and
list items. Reading that back gives one run over the whole text, not the flags. The writer refuses
a flagged block that also has runs which would nest inside the emphasis.

## Block ids

Markdown does not store block ids. A document read from Markdown has the root as `BlockId(0)`.
Every other block gets the next id in the order the reader creates it. A list's items get ids
before the list does. Writing a document back and reading it again keeps the same ids only if the
document already used that numbering.

## Not yet in the grammar

Loose lists, setext headings, tables outside a grid, `[#ref]`
cross-references (use `[text](#anchor)` for headings and `[text](#fig:id)`, `[text](#eq:id)` or `[text](#tab:id)`
for figures, equations and tables), emphasis nested
in links, bold combined with italic, grid view bounds, and slide sections and frames.
