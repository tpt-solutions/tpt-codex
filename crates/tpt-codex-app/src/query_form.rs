// SPDX-License-Identifier: MIT OR Apache-2.0
//! The draft behind a query block's form: the controls' values, kept until the user applies
//! them. A draft is editor state, not part of the document, so it is not undoable.

use tpt_typeset::entity_type;
use tpt_typeset::query_builder::{Condition, QuerySpec};

/// The entity types the form can choose, as `(id, label)` pairs.
pub const SOURCES: &[(u32, &str)] = &[
    (entity_type::PARAGRAPH, "paragraph"),
    (entity_type::HEADING, "heading"),
    (entity_type::LIST_ITEM, "list item"),
    (entity_type::CODE, "code"),
    (entity_type::MATH, "math"),
    (entity_type::MEDIA, "media"),
    (entity_type::GRID, "grid"),
    (entity_type::QUERY, "query"),
];

/// The fields the form can test and sort by.
pub const FIELDS: &[&str] = &["kind", "content", "language"];

/// The tests the form offers, as `(id, label)` pairs. The id is the select's option value.
pub const TESTS: &[(&str, &str)] = &[
    ("equals", "equals"),
    ("not-equals", "not equals"),
    ("one-of", "one of"),
];

/// How the form's field is tested against its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Test {
    /// The field equals the value.
    Equals,
    /// The field does not equal the value.
    NotEquals,
    /// The field equals one of the comma-separated values.
    OneOf,
}

impl Test {
    /// The select option that stands for this test; one of the ids in [`TESTS`].
    pub fn id(self) -> &'static str {
        match self {
            Test::Equals => "equals",
            Test::NotEquals => "not-equals",
            Test::OneOf => "one-of",
        }
    }

    /// The test an option id stands for. Anything unknown is "equals".
    pub fn from_id(id: &str) -> Test {
        match id {
            "not-equals" => Test::NotEquals,
            "one-of" => Test::OneOf,
            _ => Test::Equals,
        }
    }
}

/// The values of a query block's form controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDraft {
    /// The entity type to query, one of [`SOURCES`].
    pub source: u32,
    /// The field to test, one of [`FIELDS`].
    pub field: String,
    /// How the field is tested.
    pub test: Test,
    /// The text the field is compared with. For "one of", a comma-separated list.
    pub value: String,
    /// The field to sort by, or empty for no sort.
    pub sort: String,
    /// Sort descending instead of ascending.
    pub descending: bool,
    /// The largest number of rows as typed, or empty for no limit.
    pub limit: String,
}

impl Default for QueryDraft {
    fn default() -> Self {
        QueryDraft {
            source: entity_type::PARAGRAPH,
            field: String::from("kind"),
            test: Test::Equals,
            value: String::new(),
            sort: String::new(),
            descending: false,
            limit: String::new(),
        }
    }
}

/// One control's new value. A control sends only its own change, and the editor applies it to
/// the draft it holds, so no handler needs to remember the rest of the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftChange {
    /// The source type, as the number of a [`SOURCES`] entry.
    Source(String),
    /// The field to test.
    Field(String),
    /// A test id from [`TESTS`]. Anything else is "equals".
    Test(String),
    /// The text to compare with.
    Value(String),
    /// The sort field, or empty for none.
    Sort(String),
    /// `desc` for descending, anything else for ascending.
    Direction(String),
    /// The limit as typed.
    Limit(String),
}

impl QueryDraft {
    /// Applies one control's change. A source that is not a number is ignored, as the form
    /// only offers numbers.
    pub fn apply(&mut self, change: DraftChange) {
        match change {
            DraftChange::Source(text) => {
                if let Ok(source) = text.parse() {
                    self.source = source;
                }
            }
            DraftChange::Field(field) => self.field = field,
            DraftChange::Test(test) => self.test = Test::from_id(&test),
            DraftChange::Value(value) => self.value = value,
            DraftChange::Sort(sort) => self.sort = sort,
            DraftChange::Direction(direction) => self.descending = direction == "desc",
            DraftChange::Limit(limit) => self.limit = limit,
        }
    }

    /// The query the controls describe. Fails when the limit is not a whole number, or the
    /// field or sort field is not one of [`FIELDS`]. The editor checks the rest when it stores
    /// the query.
    pub fn to_spec(&self) -> Result<QuerySpec, String> {
        if !FIELDS.contains(&self.field.as_str()) {
            return Err(format!("'{}' cannot be filtered on", self.field));
        }
        let condition = match self.test {
            Test::Equals => Condition::Equals {
                field: self.field.clone(),
                value: self.value.clone(),
            },
            Test::NotEquals => Condition::NotEquals {
                field: self.field.clone(),
                value: self.value.clone(),
            },
            Test::OneOf => {
                let values: Vec<String> = self
                    .value
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(String::from)
                    .collect();
                if values.is_empty() {
                    return Err(String::from(
                        "'one of' needs at least one value, separated by commas",
                    ));
                }
                Condition::OneOf {
                    field: self.field.clone(),
                    values,
                }
            }
        };
        let mut spec = QuerySpec::new(self.source).with(condition);
        if !self.sort.is_empty() {
            spec = spec.sorted_by(&self.sort, self.descending);
        }
        let limit = self.limit.trim();
        if !limit.is_empty() {
            let n: usize = limit
                .parse()
                .map_err(|_| format!("the limit must be a whole number, not '{limit}'"))?;
            spec = spec.limited_to(n);
        }
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_typeset::query_builder::build_source;

    #[test]
    fn the_default_draft_is_a_paragraph_filter_with_no_sort_or_limit() {
        let spec = QueryDraft::default().to_spec().unwrap();
        assert_eq!(spec.source, entity_type::PARAGRAPH);
        assert_eq!(spec.conditions.len(), 1);
        assert!(spec.order_by.is_none());
        assert!(spec.limit.is_none());
    }

    #[test]
    fn every_control_reaches_the_spec() {
        let draft = QueryDraft {
            source: entity_type::HEADING,
            field: String::from("content"),
            test: Test::NotEquals,
            value: String::from("Draft"),
            sort: String::from("language"),
            descending: true,
            limit: String::from(" 5 "),
        };
        let spec = draft.to_spec().unwrap();
        assert_eq!(spec.source, entity_type::HEADING);
        assert_eq!(
            spec.conditions,
            vec![Condition::NotEquals {
                field: String::from("content"),
                value: String::from("Draft"),
            }]
        );
        let order = spec.order_by.unwrap();
        assert_eq!(order.field, "language");
        assert!(order.descending);
        assert_eq!(spec.limit, Some(5));
    }

    #[test]
    fn a_limit_that_is_not_a_number_is_refused() {
        let draft = QueryDraft {
            limit: String::from("lots"),
            ..QueryDraft::default()
        };
        assert!(draft.to_spec().unwrap_err().contains("whole number"));
    }

    #[test]
    fn a_field_outside_the_form_is_refused() {
        let draft = QueryDraft {
            field: String::from("secret"),
            ..QueryDraft::default()
        };
        assert!(draft.to_spec().is_err());
    }

    #[test]
    fn the_spec_builds_valid_query_json() {
        let spec = QueryDraft {
            value: String::from("paragraph"),
            ..QueryDraft::default()
        }
        .to_spec()
        .unwrap();
        let source = build_source(&spec);
        assert!(source.contains("\"kind\""), "{source}");
    }

    #[test]
    fn one_of_splits_the_value_on_commas_and_drops_blanks() {
        let draft = QueryDraft {
            test: Test::OneOf,
            value: String::from(" paragraph, list item ,, "),
            ..QueryDraft::default()
        };
        let spec = draft.to_spec().unwrap();
        assert_eq!(
            spec.conditions,
            vec![Condition::OneOf {
                field: String::from("kind"),
                values: vec![String::from("paragraph"), String::from("list item")],
            }]
        );
        let source = build_source(&spec);
        assert!(
            source.contains(r#""In":{"field":"kind","values":["paragraph","list item"]}"#),
            "{source}"
        );
    }

    #[test]
    fn one_of_with_no_values_is_refused() {
        let draft = QueryDraft {
            test: Test::OneOf,
            value: String::from(" , "),
            ..QueryDraft::default()
        };
        assert!(draft.to_spec().unwrap_err().contains("at least one value"));
    }

    #[test]
    fn the_test_change_selects_each_test_and_unknown_ids_mean_equals() {
        let mut draft = QueryDraft::default();
        draft.apply(DraftChange::Test("one-of".into()));
        assert_eq!(draft.test, Test::OneOf);
        draft.apply(DraftChange::Test("not-equals".into()));
        assert_eq!(draft.test, Test::NotEquals);
        draft.apply(DraftChange::Test("bogus".into()));
        assert_eq!(draft.test, Test::Equals);
        for (id, _) in TESTS {
            assert_eq!(Test::from_id(id).id(), *id);
        }
    }
}
