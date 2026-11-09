// SPDX-License-Identifier: MIT OR Apache-2.0
//! Runs a `Block::Query` with `tpt-query`.
//!
//! The query source is the JSON form of a `tpt-query` query (see `tpt_query::json`). The host
//! supplies the entities, because the document does not store them. Results come back as the
//! JSON that `tpt-query` writes for the matching rows.

use alloc::format;
use alloc::string::String;
use core::fmt;

use tpt_codex_core::Query as QueryBlock;
use tpt_query::EntitySource;
use tpt_query::json::{execute_json, query_from_json};

/// Why a query block could not produce rows.
#[derive(Debug)]
pub enum QueryRunError {
    /// The source is not a valid JSON query. The message says what is wrong.
    Parse {
        /// What is wrong with the query.
        message: String,
        /// A suggested fix.
        hint: &'static str,
    },
    /// The query is valid but could not run against the source.
    Execute(tpt_query::QueryError),
}

impl QueryRunError {
    /// A suggested fix for this error.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Parse { hint, .. } => hint,
            Self::Execute(_) => {
                "check that the fields and the source named in the query exist in the entities"
            }
        }
    }
}

impl fmt::Display for QueryRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse { message, hint } => {
                write!(f, "query is not valid JSON: {message} (hint: {hint})")
            }
            Self::Execute(error) => {
                write!(f, "query could not run: {error:?} (hint: {})", self.hint())
            }
        }
    }
}

/// Parses the query block's JSON source and runs it against `source`.
///
/// Returns the result rows as JSON text.
pub fn run_query<S: EntitySource>(block: &QueryBlock, source: &S) -> Result<String, QueryRunError> {
    let query = query_from_json(&block.source).map_err(|e| QueryRunError::Parse {
        message: format!("{e:?}"),
        hint: "write the query as tpt-query JSON, with a source and the fields it filters on",
    })?;
    execute_json(source, &query).map_err(QueryRunError::Execute)
}

/// Entity type ids used by [`document_entities`].
pub mod entity_type {
    /// A paragraph (`Block::Text` without a heading level).
    pub const PARAGRAPH: u32 = 1;
    /// A heading (`Block::Text` with a level).
    pub const HEADING: u32 = 2;
    /// One item of a list.
    pub const LIST_ITEM: u32 = 3;
    /// A fenced code block.
    pub const CODE: u32 = 4;
    /// A display math block.
    pub const MATH: u32 = 5;
    /// An image or other media block.
    pub const MEDIA: u32 = 6;
    /// A live grid.
    pub const GRID: u32 = 7;
    /// A query block.
    pub const QUERY: u32 = 8;
    /// A horizontal rule.
    pub const THEMATIC_BREAK: u32 = 9;
    /// A list.
    pub const LIST: u32 = 10;
    /// A table of plain text outside a grid.
    pub const TABLE: u32 = 11;
}

/// Exposes the document's blocks as `tpt-query` entities, so a query can run over the document.
///
/// Each block is an entity whose id is its block id. Fields:
/// - `kind`: the block kind, as text (`"paragraph"`, `"heading"`, ...),
/// - `content`: the text of a paragraph, heading, list item, code block or math source,
/// - `level`: the heading level (headings only), `language`: the code language (code only),
/// - `position`: the index among the root's children (top-level blocks only),
/// - `list`: the id of the containing list (list items only).
///
/// The result is a snapshot. Build it again after the document changes.
pub fn document_entities(doc: &tpt_codex_core::Document) -> tpt_query::MemorySource {
    use tpt_codex_core::Block;
    use tpt_query::{Entity, EntityId, EntityTypeId};

    let mut src = tpt_query::MemorySource::new();
    let root_children: alloc::vec::Vec<tpt_codex_core::BlockId> = match doc.get(doc.root) {
        Some(Block::Container { children, .. }) => children.clone(),
        _ => alloc::vec::Vec::new(),
    };
    let entity =
        |id: tpt_codex_core::BlockId, ty: u32| Entity::new(EntityId(id.0), EntityTypeId(ty));
    for (&id, block) in &doc.blocks {
        let mut e = match block {
            Block::Text { content, style } => match style.heading_level {
                Some(level) => entity(id, entity_type::HEADING)
                    .with_field("kind", "heading")
                    .with_field("content", content.as_str())
                    .with_field("level", i64::from(level)),
                None => entity(id, entity_type::PARAGRAPH)
                    .with_field("kind", "paragraph")
                    .with_field("content", content.as_str()),
            },
            Block::List { .. } => entity(id, entity_type::LIST).with_field("kind", "list"),
            Block::Code { language, code } => {
                let e = entity(id, entity_type::CODE)
                    .with_field("kind", "code")
                    .with_field("content", code.as_str());
                match language {
                    Some(l) => e.with_field("language", l.as_str()),
                    None => e,
                }
            }
            Block::Math { source } => entity(id, entity_type::MATH)
                .with_field("kind", "math")
                .with_field("content", source.as_str()),
            Block::Media { .. } => entity(id, entity_type::MEDIA).with_field("kind", "media"),
            Block::Grid { .. } => entity(id, entity_type::GRID).with_field("kind", "grid"),
            Block::Query(_) => entity(id, entity_type::QUERY).with_field("kind", "query"),
            Block::Table { .. } => entity(id, entity_type::TABLE).with_field("kind", "table"),
            Block::ThematicBreak => {
                entity(id, entity_type::THEMATIC_BREAK).with_field("kind", "thematic_break")
            }
            // Containers and quotes group blocks; their children are entities of their own.
            Block::Container { .. } | Block::Quote { .. } => continue,
        };
        if let Some(position) = root_children.iter().position(|c| *c == id) {
            e = e.with_field("position", position as i64);
        }
        src.insert(e);
    }
    // List items are not children of the root, so record their list here.
    for (&id, block) in &doc.blocks {
        if let Block::List { items, .. } = block {
            for item in items {
                if let Some(Block::Text { content, .. }) = doc.get(*item) {
                    src.insert(
                        entity(*item, entity_type::LIST_ITEM)
                            .with_field("kind", "list_item")
                            .with_field("content", content.as_str())
                            .with_field("list", id.0 as i64),
                    );
                }
            }
        }
    }
    src
}

/// The name a query block's result is bound under: `q<id>`, such as `q1` for the block `BlockId(1)`.
pub fn query_name(block: tpt_codex_core::BlockId) -> alloc::string::String {
    format!("q{}", block.0)
}

/// Runs every `Block::Query` in `doc` against the document's own blocks and binds each result to
/// [`query_name`] as its row count: `"3 rows"`, or `"1 row"`. A query that does not parse or run is
/// not bound, so a `[[q1]]` that uses it is reported as having no value.
pub fn query_bindings(doc: &tpt_codex_core::Document) -> crate::Bindings {
    let mut bindings = crate::Bindings::new();
    let entities = document_entities(doc);
    for (&id, block) in &doc.blocks {
        let tpt_codex_core::Block::Query(query) = block else {
            continue;
        };
        let Ok(json) = run_query(query, &entities) else {
            continue;
        };
        let Ok(serde_json::Value::Array(rows)) = serde_json::from_str::<serde_json::Value>(&json)
        else {
            continue;
        };
        let value = if rows.len() == 1 {
            format!("{} row", rows.len())
        } else {
            format!("{} rows", rows.len())
        };
        bindings.set(&query_name(id), &value);
    }
    bindings
}
