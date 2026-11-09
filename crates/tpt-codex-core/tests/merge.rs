// SPDX-License-Identifier: MIT OR Apache-2.0
use tpt_codex_core::{
    Block, BlockId, Conflict, ConflictKind, Document, InlineRun, LayoutMode, MergeError, Policy,
    Side, Target, TextStyle, three_way_merge, three_way_merge_with,
};

fn text(content: &str) -> Block {
    Block::Text {
        content: content.into(),
        style: TextStyle::default(),
    }
}

/// A flow root (id 0) holding the given text blocks, in order.
fn doc(texts: &[(u64, &str)]) -> Document {
    let mut d = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: texts.iter().map(|(id, _)| BlockId(*id)).collect(),
        },
    );
    for (id, content) in texts {
        d.insert(BlockId(*id), text(content));
    }
    d
}

fn content(d: &Document, id: u64) -> Option<String> {
    match d.get(BlockId(id)) {
        Some(Block::Text { content, .. }) => Some(content.clone()),
        _ => None,
    }
}

#[test]
fn disjoint_block_edits_merge_cleanly() {
    let base = doc(&[(1, "alpha beta"), (2, "gamma delta")]);
    let mut ours = base.clone();
    ours.insert(BlockId(1), text("alpha BETA"));
    let mut theirs = base.clone();
    theirs.insert(BlockId(2), text("GAMMA delta"));

    let merged = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert!(merged.conflicts.is_empty());
    assert_eq!(content(&merged.document, 1).as_deref(), Some("alpha BETA"));
    assert_eq!(content(&merged.document, 2).as_deref(), Some("GAMMA delta"));
    merged.document.validate().unwrap();
}

#[test]
fn word_edits_in_one_block_combine_and_move_inline_runs() {
    let base_text = "one two three four";
    let mut base = doc(&[(1, base_text)]);
    // Bold on "three", at characters 8..13.
    base.insert(
        BlockId(1),
        Block::Text {
            content: base_text.into(),
            style: TextStyle {
                runs: vec![InlineRun {
                    start: 8,
                    end: 13,
                    bold: true,
                    italic: false,
                    link: None,
                    math: None,
                    link_title: None,
                }],
                ..TextStyle::default()
            },
        },
    );
    let mut ours = base.clone();
    ours.insert(
        BlockId(1),
        Block::Text {
            content: "ONE two three four".into(),
            style: match base.get(BlockId(1)) {
                Some(Block::Text { style, .. }) => style.clone(),
                _ => unreachable!(),
            },
        },
    );
    let mut theirs = base.clone();
    theirs.insert(
        BlockId(1),
        Block::Text {
            content: "one two three 4".into(),
            style: match base.get(BlockId(1)) {
                Some(Block::Text { style, .. }) => style.clone(),
                _ => unreachable!(),
            },
        },
    );

    let merged = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert!(merged.conflicts.is_empty());
    match merged.document.get(BlockId(1)) {
        Some(Block::Text { content, style }) => {
            assert_eq!(content, "ONE two three 4");
            assert_eq!(style.runs.len(), 1);
            let run = &style.runs[0];
            let covered: String = content
                .chars()
                .skip(run.start)
                .take(run.end - run.start)
                .collect();
            assert_eq!(covered, "three", "the bold run still covers its word");
        }
        other => panic!("expected a text block, got {other:?}"),
    }
    merged.document.validate().unwrap();
}

#[test]
fn conflicting_edits_are_reported_and_resolved_by_the_policy() {
    let base = doc(&[(1, "alpha beta")]);
    let mut ours = base.clone();
    ours.insert(BlockId(1), text("alpha X"));
    let mut theirs = base.clone();
    theirs.insert(BlockId(1), text("alpha Y"));

    let manual = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert_eq!(
        manual.conflicts,
        vec![Conflict {
            target: Target::Block(BlockId(1)),
            kind: ConflictKind::BothChanged,
            resolution: None,
        }]
    );
    assert_eq!(content(&manual.document, 1).as_deref(), Some("alpha X"));

    let theirs_wins = three_way_merge(&base, &ours, &theirs, Policy::KeepTheirs).unwrap();
    assert_eq!(theirs_wins.conflicts[0].resolution, Some(Side::Theirs));
    assert_eq!(
        content(&theirs_wins.document, 1).as_deref(),
        Some("alpha Y")
    );

    let ours_wins = three_way_merge(&base, &ours, &theirs, Policy::KeepOurs).unwrap();
    assert_eq!(ours_wins.conflicts[0].resolution, Some(Side::Ours));
    assert_eq!(content(&ours_wins.document, 1).as_deref(), Some("alpha X"));
}

#[test]
fn a_caller_chooser_sees_each_conflict_in_order() {
    let base = doc(&[(1, "a b"), (2, "c d")]);
    let mut ours = base.clone();
    ours.insert(BlockId(1), text("a X"));
    ours.insert(BlockId(2), text("c X"));
    let mut theirs = base.clone();
    theirs.insert(BlockId(1), text("a Y"));
    theirs.insert(BlockId(2), text("c Y"));

    let mut seen = Vec::new();
    let merged = three_way_merge_with(&base, &ours, &theirs, |c: &Conflict| {
        seen.push(c.target.clone());
        Some(Side::Theirs)
    })
    .unwrap();
    assert_eq!(
        seen,
        vec![Target::Block(BlockId(1)), Target::Block(BlockId(2))]
    );
    assert_eq!(content(&merged.document, 2).as_deref(), Some("c Y"));
}

#[test]
fn deleted_versus_edited_is_a_conflict() {
    let base = doc(&[(1, "keep"), (2, "gamma")]);
    // Ours deletes block 2 and removes it from the root.
    let mut ours = doc(&[(1, "keep")]);
    ours.blocks.remove(&BlockId(2));
    // Theirs edits block 2.
    let mut theirs = base.clone();
    theirs.insert(BlockId(2), text("gamma!"));

    let manual = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert_eq!(
        manual.conflicts,
        vec![Conflict {
            target: Target::Block(BlockId(2)),
            kind: ConflictKind::DeletedAndEdited,
            resolution: None,
        }]
    );
    assert!(
        manual.document.get(BlockId(2)).is_none(),
        "manual keeps ours: deleted"
    );
    manual.document.validate().unwrap();

    let theirs_wins = three_way_merge(&base, &ours, &theirs, Policy::KeepTheirs).unwrap();
    assert_eq!(content(&theirs_wins.document, 2).as_deref(), Some("gamma!"));
    theirs_wins.document.validate().unwrap();
}

#[test]
fn children_added_on_both_sides_are_kept_in_order() {
    let base = doc(&[(1, "one")]);
    let mut ours = base.clone();
    ours.insert(BlockId(2), text("two"));
    ours.insert(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    let mut theirs = base.clone();
    theirs.insert(BlockId(3), text("three"));
    theirs.insert(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(3)],
        },
    );

    let merged = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
    assert!(merged.conflicts.is_empty());
    assert_eq!(
        merged.document.get(BlockId(0)),
        Some(&Block::Container {
            layout_mode: LayoutMode::Flow,
            // Block 3 followed block 1 on theirs' side, so it goes straight after block 1.
            children: vec![BlockId(1), BlockId(3), BlockId(2)],
        })
    );
    merged.document.validate().unwrap();
}

#[test]
fn a_cycle_built_by_both_sides_is_an_error_not_a_bad_document() {
    // Base: root 0 holds block 1 (a container) and block 2 (text).
    let mut base = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1), BlockId(2)],
        },
    );
    base.insert(
        BlockId(1),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![],
        },
    );
    base.insert(BlockId(2), text("x"));
    // Ours puts block 2 inside block 1. Theirs puts block 1 inside block 2.
    let mut ours = base.clone();
    ours.insert(
        BlockId(1),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(2)],
        },
    );
    ours.insert(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    let mut theirs = base.clone();
    theirs.insert(
        BlockId(2),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(1)],
        },
    );
    theirs.insert(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: vec![BlockId(2)],
        },
    );
    let result = three_way_merge(&base, &ours, &theirs, Policy::Manual);
    assert!(matches!(result, Err(MergeError::Invalid(_))), "{result:?}");
}

#[test]
fn merging_with_an_unchanged_side_gives_the_other_side() {
    let base = doc(&[(1, "one"), (2, "two")]);
    let mut changed = base.clone();
    changed.insert(BlockId(2), text("TWO"));
    let a = three_way_merge(&base, &base, &changed, Policy::Manual).unwrap();
    assert_eq!(a.document, changed);
    let b = three_way_merge(&base, &changed, &base, Policy::Manual).unwrap();
    assert_eq!(b.document, changed);
}

/// A small deterministic generator, so the property test is repeatable.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const WORDS: [&str; 6] = ["alpha", "beta", "gamma", "delta", "eps", "zeta"];

fn random_text(rng: &mut Lcg) -> String {
    let n = 2 + rng.below(4) as usize;
    (0..n)
        .map(|_| WORDS[rng.below(WORDS.len() as u64) as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Applies a few random edits to `d`: word replacement, a new block, or a removed block.
fn random_edits(d: &mut Document, rng: &mut Lcg, next_id: &mut u64) {
    for _ in 0..rng.below(4) {
        let ids: Vec<u64> = d
            .blocks
            .keys()
            .filter(|id| id.0 != 0)
            .map(|id| id.0)
            .collect();
        match rng.below(3) {
            0 if !ids.is_empty() => {
                let id = ids[rng.below(ids.len() as u64) as usize];
                let words = random_text(rng);
                d.insert(BlockId(id), text(&words));
            }
            1 => {
                let id = *next_id;
                *next_id += 1;
                d.insert(BlockId(id), text(&random_text(rng)));
                if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
                    children.push(BlockId(id));
                }
            }
            2 if ids.len() > 1 => {
                let id = ids[rng.below(ids.len() as u64) as usize];
                d.blocks.remove(&BlockId(id));
                if let Some(Block::Container { children, .. }) = d.blocks.get_mut(&BlockId(0)) {
                    children.retain(|c| c.0 != id);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn random_edit_pairs_always_merge_to_a_valid_document() {
    let mut rng = Lcg(0x5eed);
    for round in 0..400 {
        let mut next_id = 100;
        let base_texts: Vec<(u64, String)> = (1..=3).map(|i| (i, random_text(&mut rng))).collect();
        let base = {
            let refs: Vec<(u64, &str)> = base_texts.iter().map(|(i, s)| (*i, s.as_str())).collect();
            doc(&refs)
        };
        let mut ours = base.clone();
        random_edits(&mut ours, &mut rng, &mut next_id);
        let mut theirs = base.clone();
        random_edits(&mut theirs, &mut rng, &mut next_id);

        let manual = three_way_merge(&base, &ours, &theirs, Policy::Manual)
            .unwrap_or_else(|e| panic!("round {round}: {e:?}"));
        manual.document.validate().unwrap();
        let again = three_way_merge(&base, &ours, &theirs, Policy::Manual).unwrap();
        assert_eq!(manual, again, "round {round}: merging is deterministic");

        let kept = three_way_merge(&base, &ours, &theirs, Policy::KeepTheirs).unwrap();
        kept.document.validate().unwrap();
        assert_eq!(kept.conflicts.len(), manual.conflicts.len());

        if ours == theirs {
            assert_eq!(manual.document, ours, "round {round}");
            assert!(manual.conflicts.is_empty());
        }
        let one_sided = three_way_merge(&base, &base, &theirs, Policy::Manual).unwrap();
        assert_eq!(one_sided.document, theirs, "round {round}");
    }
}
