# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Heading outline: section numbers, unique anchors, table of contents and anchor lookup.
- `article` and `report` document classes as a typed schema.
- A LaTeX-style math parser with line, column and hint errors, and box layout of parsed formulas
  from injected metrics. The bundled STIX Two Math font supplies the layout constants.
- SVG output for laid-out formulas, and checks for display and inline math in a document.
- Semantic tree of chapters and sections, with footnotes, citations and a table of contents, and
  section boxes on a laid-out flow column.
- `[[name]]` data binding for text blocks, with a check for names that have no value.
- Query blocks: `run_query` runs a query against a host-supplied entity source, `document_entities`
  maps the document's blocks to entities, and `query_builder` builds the query JSON from a
  structured description.
- Query results as bindings: each query block is bound as `q<block id>` to its row count.
- Citations: a CSL-JSON library, author-year text, bibliography and a check for unknown keys.
- BibTeX reader, which converts common TeX accents and letter commands.
- Footnote numbering in reading order, footnote notes for pages, and checks for unmatched
  references and unused definitions.
- Equation and table labels, numbered in reading order, with reference checks. Figure labels and
  their reference checks.
- Cross-reference checks for links to heading anchors.
- Slide sections: `slide_sections` groups slides by the document's sections.
- Entity type for a table of plain text outside a grid.
