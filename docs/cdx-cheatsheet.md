# `.cdx` cheat-sheet

A `.cdx` file is Markdown with front matter and three extensions: live grids, slides and math. This
page is the short version. [The grammar](markdown-grammar.md) is the full one. Anything outside the
supported syntax is an error with a line and column, never silently changed.

## Front matter

```
---
title: "Quarterly report"
author: "Your name"
date: "2026-01-01"
class: "report"
---
```

Only `title`, `author`, `date` and `class` (`article` or `report`) are allowed.

## Text

```
# Heading 1
## Heading 2

A paragraph with *emphasis*, **strong text**, a [link](https://example.com)
and an escaped \* star.

- Bullet
- Another

1. First
2. Second

***
```

Not supported yet: bold and italic on the same text, nested lists, block quotes, link titles, tables
outside a grid.

## Code

````
```rust
fn main() {}
```
````

## Images and figures

```
![Alt text](plan.png) {#fig:plan width=80%}

See [the plan](#fig:plan).
```

## Math

```
Inline math is $x^2$, and display math is:

$$ E = mc^2 $$ {#eq:energy}

See [the equation](#eq:energy).
```

## Live grids

```
::: grid { id="1" label="tab:costs" }
| Item   | Cost         |
| --- |
| Design | 1200         |
| Build  | 3400         |
| Total  | =SUM(B2:B3)  |
:::

See [the costs](#tab:costs).
```

Cells starting with `=` are formulas. The `| --- |` line marks the rows above it as header rows,
which repeat on each page. Each grid needs its own `id`.

## Slides

```
::: canvas { ratio="16:9" }
## Slide title

- Point one
- Point two
:::
```

## References

| You write | You get |
| --- | --- |
| `[text](#anchor)` | A cross-reference to a heading |
| `[text](#fig:id)`, `[text](#eq:id)`, `[text](#tab:id)` | A reference to a figure, equation or table |
| `[@key]` | A citation (check with `tpt check --bib`) |
| `[^label]` and `[^label]: Note text.` | A footnote, placed at the foot of the page in the PDF |
| `[[name]]` | A value the host supplies, shown by its name until then |
