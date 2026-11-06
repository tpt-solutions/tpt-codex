// SPDX-License-Identifier: MIT OR Apache-2.0
//! Builds the JSON source of a `Block::Query` from a structured description, so a host can
//! offer filters, ordering and a limit without asking the user to write JSON.
//!
//! A [`QuerySpec`] names an entity type, a list of conditions that must all hold, an optional
//! sort order and an optional row limit. [`build_source`] turns it into the JSON form that
//! `tpt-query` reads (see `tpt_query::json`). Every condition uses the JSON form of a
//! `tpt-query` condition, so no query language is added here.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{Value as Json, json};

/// One test a row must pass. Conditions in a [`QuerySpec`] are combined with AND.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// The field equals `value`.
    Equals {
        /// Field to test, for example `content`.
        field: String,
        /// The text the field must equal.
        value: String,
    },
    /// The field exists and differs from `value`. A missing field does not match.
    NotEquals {
        /// Field to test.
        field: String,
        /// The text the field must differ from.
        value: String,
    },
    /// The field equals one of `values`.
    OneOf {
        /// Field to test.
        field: String,
        /// The accepted texts.
        values: Vec<String>,
    },
}

/// Sort order for the rows of a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// Field to sort by.
    pub field: String,
    /// Sort descending instead of ascending.
    pub descending: bool,
}

/// A query described without JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySpec {
    /// The entity type to query, for example [`crate::query::entity_type::PARAGRAPH`].
    pub source: u32,
    /// Conditions that must all hold. An empty list matches every entity of the type.
    pub conditions: Vec<Condition>,
    /// Sort order, if any.
    pub order_by: Option<Order>,
    /// Largest number of rows, if any.
    pub limit: Option<usize>,
}

impl QuerySpec {
    /// A query over `source` with no conditions, sorting or limit.
    #[must_use]
    pub fn new(source: u32) -> Self {
        QuerySpec {
            source,
            conditions: Vec::new(),
            order_by: None,
            limit: None,
        }
    }

    /// Adds a condition.
    #[must_use]
    pub fn with(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Sorts the rows by `field`.
    #[must_use]
    pub fn sorted_by(mut self, field: &str, descending: bool) -> Self {
        self.order_by = Some(Order {
            field: String::from(field),
            descending,
        });
        self
    }

    /// Keeps at most `limit` rows.
    #[must_use]
    pub fn limited_to(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// The JSON source for `spec`, in the form `tpt-query` reads.
#[must_use]
pub fn build_source(spec: &QuerySpec) -> String {
    let filters: Vec<Json> = spec.conditions.iter().map(condition_json).collect();
    let mut query = json!({
        "source": spec.source,
        "filters": filters,
    });
    if let Some(order) = &spec.order_by {
        query["order_by"] = json!({ "field": order.field, "descending": order.descending });
    }
    if let Some(limit) = spec.limit {
        query["limit"] = json!(limit);
    }
    query.to_string()
}

fn condition_json(condition: &Condition) -> Json {
    match condition {
        Condition::Equals { field, value } => {
            json!({ "Eq": { "field": field, "value": value } })
        }
        Condition::NotEquals { field, value } => {
            json!({ "Ne": { "field": field, "value": value } })
        }
        Condition::OneOf { field, values } => {
            json!({ "In": { "field": field, "values": values } })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn an_empty_spec_is_a_bare_source() {
        assert_eq!(
            build_source(&QuerySpec::new(1)),
            r#"{"filters":[],"source":1}"#
        );
    }

    #[test]
    fn conditions_order_and_limit_are_written() {
        let spec = QuerySpec::new(2)
            .with(Condition::Equals {
                field: "kind".into(),
                value: "heading".into(),
            })
            .sorted_by("content", true)
            .limited_to(3);
        let text = build_source(&spec);
        assert!(
            text.contains(r#""Eq":{"field":"kind","value":"heading"}"#),
            "{text}"
        );
        assert!(
            text.contains(r#""order_by":{"descending":true,"field":"content"}"#),
            "{text}"
        );
        assert!(text.contains(r#""limit":3"#), "{text}");
    }

    #[test]
    fn not_equals_and_one_of_use_the_tpt_query_names() {
        let spec = QuerySpec::new(1)
            .with(Condition::NotEquals {
                field: "language".into(),
                value: "rust".into(),
            })
            .with(Condition::OneOf {
                field: "kind".into(),
                values: vec!["paragraph".into(), "list item".into()],
            });
        let text = build_source(&spec);
        assert!(
            text.contains(r#""Ne":{"field":"language","value":"rust"}"#),
            "{text}"
        );
        assert!(
            text.contains(r#""In":{"field":"kind","values":["paragraph","list item"]}"#),
            "{text}"
        );
    }
}
