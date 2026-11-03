# Markdown grammar (supported subset)

This is the grammar that `tpt-codex-markdown` reads and writes today. Anything outside it is a
structured error when read, and a refusal when written. The spec's Codex Flavored Markdown
extensions (grids, canvases, math, citations) are not in this subset yet.

The reader and the writer share the same line predicates (`src/syntax.rs`). A document the writer
accepts reads back as the same document. The tests check this on a fixed fixture and on random input.

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

body         = { blank | heading | paragraph } ;
blank        = SP* NL ;

heading      = hashes ( SP text )? NL ;   (* text is trimmed; empty text is an error *)
hashes       = "#" | "##" | "###" | "####" | "#####" | "######" ;
              (* a 7+ hash run, or a hash run not followed by a space, is not a heading *)

paragraph    = para-line { NL para-line } ;   (* ends at a blank line or a heading *)
para-line    = text-line ;                     (* must not match any unsupported syntax below *)
```

Lines in a paragraph are kept exactly as written, including leading spaces. Inline Markdown
(`*emphasis*`, `[links](…)`, backticks) is kept as literal text. It is not parsed.

## Unsupported syntax (read errors)

A line that starts with any of these is an error. The error names the construct and suggests a fix.

| Syntax | Examples |
| --- | --- |
| Indented code block | a line starting with 4 spaces or a tab |
| Bullet list | `- `, `* `, `+ `, or a bare `-`, `*`, `+` |
| Ordered list | `1. `, `2) ` |
| Block quote | `> ` or `>` |
| Fenced code | ` ``` ` or `~~~` |
| Table | a line starting with `|` |
| Container block | `:::` |
| Math block | `$$` |
| Raw HTML | a line starting with `<` |
| Image | a line starting with `![` |
| Footnote definition | a line starting with `[^` |
| Thematic break or setext underline | a line made only of `-`, `*`, `_` or `=` (with optional spaces) |

Front matter problems are also errors: an unknown key, a line that is not `key: value`, a
quoted value with a bad escape, or a missing closing `---`.

## Writer refusals

`to_markdown` refuses rather than changing meaning. It refuses:

- a root that is not a flow container,
- children that are not text: grids, media, queries, nested containers,
- text with bold or italic flags (not yet written),
- empty text, or text with leading or trailing spaces,
- a heading level outside 1 to 6, or a heading with a line break,
- a paragraph with a blank line, or a line that would read back as another construct,
- front matter values with line breaks.

## Block ids

Markdown does not store block ids. A document read from Markdown has the root as `BlockId(0)` and
the children as `BlockId(1)..` in order. Writing a document back and reading it again keeps the
same ids only if the document already used that numbering.

## Not yet in the grammar

Lists, code blocks, links, images, emphasis runs, tables, `::: grid`, `::: canvas`, math, citations,
`[#ref]` cross-references and `{#fig:id}` attributes. Each needs a model variant or inline run
support first (see `todo.md`, Phase 2).
