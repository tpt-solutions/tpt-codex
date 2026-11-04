# Markdown grammar (supported subset)

This is the grammar that `tpt-codex-markdown` reads and writes today. Anything outside it is a
structured error when read, and a refusal when written. The spec's Codex Flavored Markdown
extensions (grids, canvases, math, citations) are not in this subset yet.

The reader and the writer share the same line predicates (`src/syntax.rs`). A document the writer
accepts reads back as the same document. The tests check this on a fixed fixture and on random input.

The subset follows CommonMark for the constructs it reads, with two deliberate differences: inline
Markdown is kept as literal text, and any construct whose reading would differ from CommonMark is an
error rather than a silent change.

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
block        = heading | fence | rule | image | list | paragraph ;

heading      = hashes ( SP text )? NL ;   (* text is trimmed; empty text is an error *)
hashes       = "#" | "##" | "###" | "####" | "#####" | "######" ;
              (* a 7+ hash run, or a hash run not followed by a space, is not a heading *)

fence        = fence-open { line } fence-close ;
fence-open   = indent{0,3} run info NL ;      (* run is 3+ backticks, or 3+ tildes *)
fence-close  = indent{0,3} run{same char, at least as long as the opener} SP* NL ;
info         = text without backticks (backtick fences), no backslash or '&' ;

rule         = indent{0,3} ( "***" | "---" | "___" with SP between marks ) SP* NL ;
              (* "---" directly under a paragraph line is a setext heading, which is an error *)

image        = indent{0,3} "![" alt "](" uri ")" SP* NL ;
alt          = text without brackets, backslashes or '&' ;
uri          = text without spaces, parentheses, angle brackets, backslashes or '&' ;

list         = list-item { NL list-item } ;
list-item    = indent{0,3} marker SP+ text NL ;
marker       = "-" | "*" | "+" | digits{1,9} ( "." | ")" ) ;
              (* a list of one kind is a run of items with the same marker character;
                 changing the marker starts a new list *)

paragraph    = para-line { NL para-line } ;   (* ends at a blank line or any other block *)
para-line    = text-line ;                     (* must not start any block syntax above *)
```

Lines in a paragraph and list items are kept exactly as written, apart from the trimming
described below. Inline Markdown (`*emphasis*`, `[links](…)`, backticks) is kept as literal text.
It is not parsed.

### Lists

- A bulleted list item and an ordered list item starting at 1 can interrupt a paragraph. An ordered
  item with any other number cannot, so reading it after a paragraph is an error.
- Each item is one line. Its text is trimmed. An empty item is an error.
- A blank line between items makes the list loose in CommonMark. Loose lists are an error here.
- Only the number of the first item is kept. Later numbers are written from it, so `1. 1. 1.`
  reads as items 1, 2, 3. This matches how CommonMark renders the list.

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

## Unsupported syntax (read errors)

A line that starts with any of these is an error. The error names the construct and suggests a fix.

| Syntax | Examples |
| --- | --- |
| Indented code block | a line starting with 4 spaces or a tab |
| Block quote | `> ` or `>` |
| Table | a line starting with `|` |
| Container block | `:::` |
| Math block | `$$` |
| Raw HTML | a line starting with `<` |
| Footnote definition | a line starting with `[^` |
| Setext heading | a line of `=`, or `---` under a paragraph line |
| Loose list | a blank line between two items of the same list |
| List continuation | a line under an item that is not indented into it |
| Nested list | an item indented four or more spaces |

Front matter problems are also errors: an unknown key, a line that is not `key: value`, a
quoted value with a bad escape, or a missing closing `---`.

## Writer refusals

`to_markdown` refuses rather than changing meaning. It refuses:

- a root that is not a flow container,
- children that are grids, queries, or nested containers,
- text with bold or italic flags (not yet written),
- empty text, or text with leading or trailing spaces,
- a heading level outside 1 to 6, or a heading with a line break,
- a paragraph with a blank line, or a line that would read back as another block,
- a list with no items, a bulleted list not starting at 1, or a number above 999 999 999,
- a list item that is empty, has leading or trailing spaces, has a line break, or is made only of
  `-`, `*`, `_` and spaces (it would read back as a rule),
- two adjacent lists of the same kind, which would read back as one list,
- a numbered list that does not start at 1 directly after a paragraph,
- a list item that is not a plain text block,
- a code block whose language has a line break, backtick, backslash or entity, or whose content has a
  carriage return,
- an image that is not a URI source, has an empty URI, a URI with spaces, parentheses, angle brackets,
  backslashes or entities, an empty alt text (write `None` instead), or alt text with brackets,
  backslashes, entities or line breaks,
- front matter values with line breaks.

The writer writes a thematic break as `***`, not `---`. A `---` line as the first line of a
document reads as front matter.

## Block ids

Markdown does not store block ids. A document read from Markdown has the root as `BlockId(0)`.
Every other block gets the next id in the order the reader creates it. A list's items get ids
before the list does. Writing a document back and reading it again keeps the same ids only if the
document already used that numbering.

## Not yet in the grammar

Links, emphasis runs, block quotes, nested lists, loose lists, setext headings, tables,
`::: grid`, `::: canvas`, math, citations, `[#ref]` cross-references, `{#fig:id}` attributes and
footnotes. Emphasis and links need inline run support in the model first (see `todo.md`, Phase 2).
