// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_typeset::{Library, author_year, bibliography};

const BIB: &str = r#"
@comment{This block is ignored, even with {nested} braces.}
@string{press = "Field Press"}
@preamble{"\newcommand{\x}{y}"}

@article{smith2020,
  author  = {Smith, Ann and Bo Jones},
  title   = {{Field} notes on {DNA}: a study},
  journal = press,
  year    = 2020,
  month   = jan,
}

@book(jones2018,
  author = "Jones, Bo and Lee, Cy and Kim, Dee",
  title  = "Two " # "authors",
  date   = {2018-05-01}
)

@misc{who,
  author = {{World Health Organization}},
  title  = {Guidance},
}

@article{van2001,
  author = {Ludwig van Beethoven and Ann de Smith},
  title  = {Scores},
  year   = {2001}
}

@misc{anon, title = {No author and no date}}
"#;

#[test]
fn the_library_reads_bibtex_entries() {
    let lib = Library::from_bibtex(BIB).unwrap();
    assert_eq!(lib.len(), 5);
    let smith = lib.get("smith2020").unwrap();
    assert_eq!(smith.title, "Field notes on DNA: a study");
    assert_eq!(smith.year, Some(2020));
    assert_eq!(smith.authors.len(), 2);
    assert_eq!(smith.authors[0].family, "Smith");
    assert_eq!(smith.authors[0].given, "Ann");
    assert_eq!(smith.authors[1].family, "Jones");
    assert_eq!(smith.authors[1].given, "Bo");
}

#[test]
fn concatenation_and_the_date_field_are_read() {
    let lib = Library::from_bibtex(BIB).unwrap();
    let jones = lib.get("jones2018").unwrap();
    assert_eq!(jones.title, "Two authors");
    assert_eq!(jones.year, Some(2018), "the year comes from date");
    assert_eq!(jones.authors.len(), 3);
    assert_eq!(author_year(jones), "Jones et al. 2018");
}

#[test]
fn corporate_names_and_particles_are_kept_whole() {
    let lib = Library::from_bibtex(BIB).unwrap();
    let who = lib.get("who").unwrap();
    assert_eq!(who.authors[0].family, "World Health Organization");
    assert!(who.authors[0].given.is_empty());

    let van = lib.get("van2001").unwrap();
    assert_eq!(van.authors[0].family, "van Beethoven");
    assert_eq!(van.authors[0].given, "Ludwig");
    assert_eq!(van.authors[1].family, "de Smith");
}

#[test]
fn an_entry_without_authors_or_year_still_cites() {
    let lib = Library::from_bibtex(BIB).unwrap();
    assert_eq!(
        author_year(lib.get("anon").unwrap()),
        "No author and no date n.d."
    );
}

#[test]
fn bibtex_and_csl_json_entries_feed_the_same_bibliography() {
    let lib = Library::from_bibtex(BIB).unwrap();
    let list = bibliography(&lib, &["smith2020", "jones2018", "nobody"]);
    assert_eq!(list.len(), 2, "unknown keys are left out");
    assert!(list[0].starts_with("Jones, Bo; Lee, Cy; Kim, Dee."));
    assert!(list[1].starts_with("Smith, Ann; Jones, Bo. 2020."));
}

#[test]
fn comments_and_preambles_define_nothing() {
    let lib =
        Library::from_bibtex("@comment{@article{ghost, title={x}}} @preamble{\"p\"}").unwrap();
    assert!(lib.is_empty());
}

#[test]
fn a_missing_key_is_an_error_with_its_position() {
    let err = Library::from_bibtex("@article{\n  title = {x}\n}").unwrap_err();
    assert_eq!(err.message, "expected a comma after the key");
    assert_eq!(err.line, 2);

    let err = Library::from_bibtex("@article{k, title {x}}").unwrap_err();
    assert_eq!(err.message, "expected = after the field name");

    let err = Library::from_bibtex("\n@article{, title = {x}}").unwrap_err();
    assert_eq!(err.message, "an entry has no key");
    assert_eq!((err.line, err.column), (2, 10));
}

#[test]
fn an_unclosed_value_is_an_error() {
    let err = Library::from_bibtex("@article{k, title = {never closed").unwrap_err();
    assert_eq!(err.message, "a braced value is not closed");
}

#[test]
fn an_unknown_macro_is_an_error() {
    let err = Library::from_bibtex("@article{k, journal = nowhere}").unwrap_err();
    assert_eq!(err.message, "unknown macro");
}

#[test]
fn duplicate_keys_are_an_error_at_the_second_entry() {
    let err = Library::from_bibtex("@misc{a, title={x}}\n@misc{a, title={y}}").unwrap_err();
    assert_eq!(err.message, "two entries share an id");
    assert_eq!(err.line, 2);
}

#[test]
fn tex_accents_and_letter_commands_become_characters() {
    let src = r#"@misc{k,
  author = {{\"O}zt\"urk, Ay{\c s}e and Jos{\'e} M{\"u}ller and Str{\o}m, Bj{\o}rn and \v{S}imon Novak},
  title  = {Caf\'e {\&} na{\"i}ve \emph{text}}
}"#;
    let lib = Library::from_bibtex(src).unwrap();
    let entry = lib.get("k").unwrap();
    assert_eq!(entry.title, "Café & naïve text");
    let names: Vec<(&str, &str)> = entry
        .authors
        .iter()
        .map(|n| (n.family.as_str(), n.given.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Öztürk", "Ayşe"),
            ("Müller", "José"),
            ("Strøm", "Bjørn"),
            ("Novak", "Šimon"),
        ]
    );
}

#[test]
fn an_accent_without_a_precomposed_letter_keeps_the_letter() {
    let lib = Library::from_bibtex(r#"@misc{k, title = {\'{q}\v{x}}}"#).unwrap();
    assert_eq!(lib.get("k").unwrap().title, "qx");
}
