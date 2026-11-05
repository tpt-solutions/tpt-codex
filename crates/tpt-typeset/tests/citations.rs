// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{Library, author_year, bibliography, check_citations};

const LIBRARY: &str = r#"[
  {"id":"smith2020","type":"article-journal","title":"Field notes",
   "author":[{"family":"Smith","given":"Ann"}],
   "issued":{"date-parts":[[2020]]}},
  {"id":"jones2018","title":"Two authors",
   "author":[{"family":"Jones","given":"Bo"},{"family":"Lee","given":"Cy"}],
   "issued":{"date-parts":[[2018, 5]]}},
  {"id":"anon","title":"No author and no date"}
]"#;

#[test]
fn the_library_reads_csl_json() {
    let lib = Library::from_csl_json(LIBRARY).unwrap();
    assert_eq!(lib.len(), 3);
    let smith = lib.get("smith2020").unwrap();
    assert_eq!(smith.authors[0].family, "Smith");
    assert_eq!(smith.year, Some(2020));
}

#[test]
fn author_year_forms_follow_the_number_of_authors() {
    let lib = Library::from_csl_json(LIBRARY).unwrap();
    assert_eq!(author_year(lib.get("smith2020").unwrap()), "Smith 2020");
    assert_eq!(
        author_year(lib.get("jones2018").unwrap()),
        "Jones and Lee 2018"
    );
    assert_eq!(
        author_year(lib.get("anon").unwrap()),
        "No author and no date n.d."
    );
}

#[test]
fn the_bibliography_is_sorted_and_skips_unknown_keys() {
    let lib = Library::from_csl_json(LIBRARY).unwrap();
    let list = bibliography(&lib, &["smith2020", "missing", "jones2018", "smith2020"]);
    assert_eq!(list.len(), 2);
    assert!(list[0].starts_with("Jones, Bo; Lee, Cy. 2018."), "{list:?}");
    assert!(
        list[1].starts_with("Smith, Ann. 2020. Field notes"),
        "{list:?}"
    );
}

#[test]
fn bad_libraries_are_refused() {
    assert!(Library::from_csl_json("{}").is_err());
    assert!(Library::from_csl_json(r#"[{"title":"no id"}]"#).is_err());
    assert!(Library::from_csl_json(r#"[{"id":"a"},{"id":"a"}]"#).is_err());
}

#[test]
fn citations_are_checked_against_the_library() {
    let lib = Library::from_csl_json(LIBRARY).unwrap();
    let doc = from_markdown("As [Smith](@smith2020) showed, and [@jones2018] agreed [@nobody].\n")
        .unwrap();
    let found = check_citations(&doc, &lib);
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(found[0].text.as_deref(), Some("Smith 2020"));
    assert_eq!(found[1].key, "jones2018");
    assert_eq!(found[2].key, "nobody");
    assert_eq!(found[2].text, None);
}
