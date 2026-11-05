// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_markdown::from_markdown;
use tpt_typeset::{check_footnotes, footnote_numbers, unreferenced_footnotes};

#[test]
fn references_are_numbered_in_reading_order_and_point_at_their_definitions() {
    let md =
        "First[^b] then second[^a] and first again[^b].\n\n[^a]: Text of a.\n\n[^b]: Text of b.\n";
    let doc = from_markdown(md).unwrap();
    let numbers = footnote_numbers(&doc);
    assert_eq!(numbers.get("b"), Some(&1));
    assert_eq!(numbers.get("a"), Some(&2));

    let refs = check_footnotes(&doc);
    assert_eq!(refs.len(), 3);
    assert!(refs.iter().all(|r| r.definition.is_some()), "{refs:?}");
    assert_eq!(refs[0].number, Some(1));
    assert_eq!(refs[1].number, Some(2));
    assert_eq!(refs[2].number, Some(1));
    assert!(unreferenced_footnotes(&doc).is_empty());
}

#[test]
fn a_reference_without_a_definition_and_a_definition_without_a_reference_are_reported() {
    let md = "Missing[^nothere] and used[^used].\n\n[^used]: Used.\n\n[^orphan]: Never cited.\n";
    let doc = from_markdown(md).unwrap();
    let refs = check_footnotes(&doc);
    let missing = refs.iter().find(|r| r.label == "nothere").unwrap();
    assert_eq!(missing.definition, None);
    assert_eq!(missing.number, None);
    let orphans = unreferenced_footnotes(&doc);
    assert_eq!(orphans.len(), 1);
    assert_eq!(
        doc.footnotes.get(&orphans[0]).map(String::as_str),
        Some("orphan")
    );
}
