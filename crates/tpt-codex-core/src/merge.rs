// SPDX-License-Identifier: MIT OR Apache-2.0
//! Three-way merge of two diverged versions of a document, from their common ancestor.
//!
//! Each side is compared with the base, and changes that touch different parts of the document
//! combine:
//!
//! - A block changed on one side only takes that side's value. A block deleted on one side
//!   and unchanged on the other is deleted.
//! - Inside a text block changed on both sides, word-level edits combine when they do not touch.
//!   Inline runs move with the text, as in [`edit_text`](crate::edit_text).
//! - A container or list whose children both sides changed merges the two lists: a child added
//!   on either side is kept, and a child removed on either side is removed. Each added child is
//!   placed after the child it followed on its own side.
//! - Anything else that both sides changed differently is a [`Conflict`]. The caller's choice
//!   decides it: keep ours, keep theirs, or leave it for manual resolution. A manual conflict
//!   keeps the value from `ours` in the result, and the caller can take `theirs` from the inputs.
//!
//! The result is checked with [`Document::validate`]. A merge that cannot be made valid, for
//! example when both sides build a cycle, returns [`MergeError::Invalid`] rather than a document
//! that fails later.
//!
//! Limits: a block that one side removed from its container, and the other side edited, keeps
//! the edited value as an unreferenced block when that edit is chosen. It is not put back into
//! the container. Attribute entries (frames, footnotes, sections, figures, canvases) of a block
//! that does not survive the merge are dropped without a conflict.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::block::Block;
use crate::document::{Document, Metadata};
use crate::error::ValidationError;
use crate::id::{BlockId, SheetId};
use crate::primitives::TextStyle;
use crate::text::{remap_runs, replace_chars};
use crate::versions::tokens;

/// Which side's value a conflict resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The value from the `ours` document.
    Ours,
    /// The value from the `theirs` document.
    Theirs,
}

/// Why a value could not be merged automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Both sides changed the value, in ways that do not combine.
    BothChanged,
    /// One side deleted the value and the other side changed it.
    DeletedAndEdited,
}

/// Where a conflict is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A document-level setting, such as `"root"` or `"metadata.title"`.
    Setting(&'static str),
    /// A block.
    Block(BlockId),
    /// A sheet.
    Sheet(SheetId),
    /// An entry of a per-block map, such as `"frames"` or `"figures"`, for that block.
    Attribute {
        /// The map's name.
        name: &'static str,
        /// The block the entry belongs to.
        block: BlockId,
    },
}

/// A value both sides changed in ways that cannot be combined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// What the conflict is about. The values are in the three input documents.
    pub target: Target,
    /// Why it could not be merged.
    pub kind: ConflictKind,
    /// The side the conflict was resolved to, or `None` if it is left for manual resolution.
    /// A manual conflict keeps the `ours` value in the merged document.
    pub resolution: Option<Side>,
}

/// A policy for every conflict, for [`three_way_merge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Keep the `ours` value of every conflict.
    KeepOurs,
    /// Keep the `theirs` value of every conflict.
    KeepTheirs,
    /// Leave every conflict for the caller. The result keeps the `ours` values.
    Manual,
}

/// The outcome of a merge.
#[derive(Debug, Clone, PartialEq)]
pub struct Merge {
    /// The merged document. It has passed [`Document::validate`].
    pub document: Document,
    /// Every conflict, in a fixed order: settings, then blocks in id order, then sheets, then
    /// attributes.
    pub conflicts: Vec<Conflict>,
}

/// Why a merge could not produce a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// The merged document is not valid. The validation error says what is wrong.
    Invalid(ValidationError),
}

/// Merges `ours` and `theirs`, which both changed from `base`, with `policy` for conflicts.
///
/// See the module documentation for the rules.
pub fn three_way_merge(
    base: &Document,
    ours: &Document,
    theirs: &Document,
    policy: Policy,
) -> Result<Merge, MergeError> {
    three_way_merge_with(base, ours, theirs, |_| match policy {
        Policy::KeepOurs => Some(Side::Ours),
        Policy::KeepTheirs => Some(Side::Theirs),
        Policy::Manual => None,
    })
}

/// Merges `ours` and `theirs` from `base`. `choose` is called once for each conflict, in order,
/// with the conflict's resolution unset. It returns the side to keep, or `None` to leave the
/// conflict for manual resolution.
pub fn three_way_merge_with<F>(
    base: &Document,
    ours: &Document,
    theirs: &Document,
    choose: F,
) -> Result<Merge, MergeError>
where
    F: FnMut(&Conflict) -> Option<Side>,
{
    let mut m = Merger {
        conflicts: Vec::new(),
        choose,
    };
    let mut doc = ours.clone();
    doc.schema_version = m.setting(
        "schema_version",
        &base.schema_version,
        &ours.schema_version,
        &theirs.schema_version,
    );
    doc.root = m.setting("root", &base.root, &ours.root, &theirs.root);
    doc.metadata = merge_metadata(&mut m, &base.metadata, &ours.metadata, &theirs.metadata);
    doc.blocks = m.blocks(base, ours, theirs);
    doc.sheets = m.entries(&base.sheets, &ours.sheets, &theirs.sheets, Target::Sheet);
    doc.frames = m.entries(&base.frames, &ours.frames, &theirs.frames, |block| {
        Target::Attribute {
            name: "frames",
            block,
        }
    });
    doc.canvases = m.entries(&base.canvases, &ours.canvases, &theirs.canvases, |block| {
        Target::Attribute {
            name: "canvases",
            block,
        }
    });
    doc.footnotes = m.entries(
        &base.footnotes,
        &ours.footnotes,
        &theirs.footnotes,
        |block| Target::Attribute {
            name: "footnotes",
            block,
        },
    );
    doc.sections = m.entries(&base.sections, &ours.sections, &theirs.sections, |block| {
        Target::Attribute {
            name: "sections",
            block,
        }
    });
    doc.figures = m.entries(&base.figures, &ours.figures, &theirs.figures, |block| {
        Target::Attribute {
            name: "figures",
            block,
        }
    });
    doc.equation_labels = m.entries(
        &base.equation_labels,
        &ours.equation_labels,
        &theirs.equation_labels,
        |block| Target::Attribute {
            name: "equation labels",
            block,
        },
    );
    doc.table_labels = m.entries(
        &base.table_labels,
        &ours.table_labels,
        &theirs.table_labels,
        Target::Sheet,
    );
    prune(&mut doc);
    doc.validate().map_err(MergeError::Invalid)?;
    Ok(Merge {
        document: doc,
        conflicts: m.conflicts,
    })
}

struct Merger<F> {
    conflicts: Vec<Conflict>,
    choose: F,
}

impl<F: FnMut(&Conflict) -> Option<Side>> Merger<F> {
    /// Records a conflict and returns the side it was resolved to.
    fn resolve(&mut self, target: Target, kind: ConflictKind) -> Option<Side> {
        let mut conflict = Conflict {
            target,
            kind,
            resolution: None,
        };
        conflict.resolution = (self.choose)(&conflict);
        let side = conflict.resolution;
        self.conflicts.push(conflict);
        side
    }

    /// Merges one value. Returns the merged value, or `None` for a deletion.
    fn value<T: Clone + PartialEq>(
        &mut self,
        target: impl FnOnce() -> Target,
        base: &Option<T>,
        ours: &Option<T>,
        theirs: &Option<T>,
    ) -> Option<T> {
        if let Some(v) = three_way(base, ours, theirs) {
            return v.clone();
        }
        let kind = if ours.is_none() || theirs.is_none() {
            ConflictKind::DeletedAndEdited
        } else {
            ConflictKind::BothChanged
        };
        let side = self.resolve(target(), kind);
        pick(side, ours, theirs).clone()
    }

    /// Merges a setting that always has a value.
    fn setting<T: Clone + PartialEq>(
        &mut self,
        name: &'static str,
        base: &T,
        ours: &T,
        theirs: &T,
    ) -> T {
        if let Some(v) = three_way(base, ours, theirs) {
            return v.clone();
        }
        let side = self.resolve(Target::Setting(name), ConflictKind::BothChanged);
        pick(side, ours, theirs).clone()
    }

    /// Merges a keyed map whose entries are compared as whole values.
    fn entries<K, V>(
        &mut self,
        base: &BTreeMap<K, V>,
        ours: &BTreeMap<K, V>,
        theirs: &BTreeMap<K, V>,
        target: impl Fn(K) -> Target,
    ) -> BTreeMap<K, V>
    where
        K: Ord + Copy,
        V: Clone + PartialEq,
    {
        let mut out = BTreeMap::new();
        for key in union_keys([base, ours, theirs]) {
            let merged = self.value(
                || target(key),
                &base.get(&key).cloned(),
                &ours.get(&key).cloned(),
                &theirs.get(&key).cloned(),
            );
            if let Some(v) = merged {
                out.insert(key, v);
            }
        }
        out
    }

    /// Merges the block tree. A block changed on both sides is merged by [`merge_block`] when
    /// it can be, and is a conflict otherwise.
    fn blocks(
        &mut self,
        base: &Document,
        ours: &Document,
        theirs: &Document,
    ) -> BTreeMap<BlockId, Block> {
        let mut out = BTreeMap::new();
        for id in union_keys([&base.blocks, &ours.blocks, &theirs.blocks]) {
            let (b, o, t) = (
                base.blocks.get(&id),
                ours.blocks.get(&id),
                theirs.blocks.get(&id),
            );
            let (bc, oc, tc) = (b.cloned(), o.cloned(), t.cloned());
            let merged = match three_way(&bc, &oc, &tc) {
                Some(v) => v.clone(),
                None => {
                    let attempt = match (o, t) {
                        (Some(x), Some(y)) => merge_block(b, x, y),
                        _ => Err(ConflictKind::DeletedAndEdited),
                    };
                    match attempt {
                        Ok(block) => Some(block),
                        Err(kind) => {
                            let side = self.resolve(Target::Block(id), kind);
                            pick(side, &oc, &tc).clone()
                        }
                    }
                }
            };
            if let Some(block) = merged {
                out.insert(id, block);
            }
        }
        out
    }
}

/// Returns the value every side agrees on, if one side changed it or none did. `None` if both
/// sides changed it and disagree.
fn three_way<'a, T: PartialEq>(base: &'a T, ours: &'a T, theirs: &'a T) -> Option<&'a T> {
    if ours == theirs {
        Some(ours)
    } else if ours == base {
        Some(theirs)
    } else if theirs == base {
        Some(ours)
    } else {
        None
    }
}

fn pick<'a, T>(side: Option<Side>, ours: &'a T, theirs: &'a T) -> &'a T {
    match side {
        Some(Side::Theirs) => theirs,
        _ => ours,
    }
}

fn union_keys<K: Ord + Copy, V, const N: usize>(maps: [&BTreeMap<K, V>; N]) -> Vec<K> {
    let set: BTreeSet<K> = maps.iter().flat_map(|m| m.keys().copied()).collect();
    set.into_iter().collect()
}

fn merge_metadata<F>(
    m: &mut Merger<F>,
    base: &Metadata,
    ours: &Metadata,
    theirs: &Metadata,
) -> Metadata
where
    F: FnMut(&Conflict) -> Option<Side>,
{
    Metadata {
        title: m.setting("metadata.title", &base.title, &ours.title, &theirs.title),
        author: m.setting(
            "metadata.author",
            &base.author,
            &ours.author,
            &theirs.author,
        ),
        date: m.setting("metadata.date", &base.date, &ours.date, &theirs.date),
        class: m.setting("metadata.class", &base.class, &ours.class, &theirs.class),
    }
}

/// Merges two sides of a block that both changed from `base`. `None` for `base` means both
/// sides created the block.
fn merge_block(base: Option<&Block>, ours: &Block, theirs: &Block) -> Result<Block, ConflictKind> {
    let Some(base) = base else {
        return Err(ConflictKind::BothChanged);
    };
    match (base, ours, theirs) {
        (
            Block::Text {
                content: bc,
                style: bs,
            },
            Block::Text {
                content: oc,
                style: os,
            },
            Block::Text {
                content: tc,
                style: ts,
            },
        ) => {
            let (content, style) = merge_text((bc, bs), (oc, os), (tc, ts))?;
            Ok(Block::Text { content, style })
        }
        (
            Block::Container {
                layout_mode: bl,
                children: bch,
            },
            Block::Container {
                layout_mode: ol,
                children: och,
            },
            Block::Container {
                layout_mode: tl,
                children: tch,
            },
        ) => {
            let layout_mode = *three_way(bl, ol, tl).ok_or(ConflictKind::BothChanged)?;
            Ok(Block::Container {
                layout_mode,
                children: merge_children(bch, och, tch),
            })
        }
        (
            Block::List {
                ordered: bo,
                start: bs,
                items: bi,
            },
            Block::List {
                ordered: oo,
                start: os,
                items: oi,
            },
            Block::List {
                ordered: to,
                start: ts,
                items: ti,
            },
        ) => {
            let ordered = *three_way(bo, oo, to).ok_or(ConflictKind::BothChanged)?;
            let start = *three_way(bs, os, ts).ok_or(ConflictKind::BothChanged)?;
            Ok(Block::List {
                ordered,
                start,
                items: merge_children(bi, oi, ti),
            })
        }
        (
            Block::Quote { children: bch },
            Block::Quote { children: och },
            Block::Quote { children: tch },
        ) => Ok(Block::Quote {
            children: merge_children(bch, och, tch),
        }),
        _ => Err(ConflictKind::BothChanged),
    }
}

/// Merges the children of a container or the items of a list.
fn merge_children(base: &[BlockId], ours: &[BlockId], theirs: &[BlockId]) -> Vec<BlockId> {
    if ours == theirs || theirs == base {
        return ours.to_vec();
    }
    if ours == base {
        return theirs.to_vec();
    }
    let mut out = ours.to_vec();
    // A child that the base had and theirs no longer has was removed by theirs.
    out.retain(|id| theirs.contains(id) || !base.contains(id));
    // A child that theirs added goes after the child it followed on theirs side.
    for (idx, id) in theirs.iter().enumerate() {
        if base.contains(id) || out.contains(id) {
            continue;
        }
        let at = match idx.checked_sub(1) {
            None => 0,
            Some(prev) => match out.iter().position(|x| *x == theirs[prev]) {
                Some(p) => p + 1,
                None => out.len(),
            },
        };
        out.insert(at, *id);
    }
    out
}

/// Merges two sides of a text block. Returns the content and style.
fn merge_text(
    (bc, bs): (&String, &TextStyle),
    (oc, os): (&String, &TextStyle),
    (tc, ts): (&String, &TextStyle),
) -> Result<(String, TextStyle), ConflictKind> {
    let bold = *three_way(&bs.bold, &os.bold, &ts.bold).ok_or(ConflictKind::BothChanged)?;
    let italic = *three_way(&bs.italic, &os.italic, &ts.italic).ok_or(ConflictKind::BothChanged)?;
    let heading_level = *three_way(&bs.heading_level, &os.heading_level, &ts.heading_level)
        .ok_or(ConflictKind::BothChanged)?;

    let runs_o = os.runs != bs.runs;
    let runs_t = ts.runs != bs.runs;
    let text_o = runs_o || oc != bc;
    let text_t = runs_t || tc != bc;
    let style = |runs| TextStyle {
        bold,
        italic,
        heading_level,
        runs,
    };
    if !text_o {
        return Ok((tc.clone(), style(ts.runs.clone())));
    }
    if !text_t {
        return Ok((oc.clone(), style(os.runs.clone())));
    }

    // Both sides changed the text. Runs are in the coordinates of the content they were made
    // for, so a side that changed its runs and its content cannot be combined with the other.
    if (runs_o && oc != bc) || (runs_t && tc != bc) {
        return Err(ConflictKind::BothChanged);
    }
    if runs_o && runs_t && os.runs != ts.runs {
        return Err(ConflictKind::BothChanged);
    }
    let start_runs = if runs_o {
        &os.runs
    } else if runs_t {
        &ts.runs
    } else {
        &bs.runs
    };
    let edits =
        merge_edits(token_edits(bc, oc), token_edits(bc, tc)).ok_or(ConflictKind::BothChanged)?;

    let mut content = bc.clone();
    let mut runs = start_runs.clone();
    // Edits are in base coordinates and do not touch, so the last one is applied first and the
    // others keep their positions.
    for edit in &edits {
        content = replace_chars(&content, edit.start, edit.end, &edit.insert)
            .ok_or(ConflictKind::BothChanged)?;
        runs = remap_runs(&runs, edit.start, edit.end, edit.insert.chars().count());
    }
    Ok((content, style(runs)))
}

/// One change to the base text, in character positions of the base.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
    start: usize,
    end: usize,
    insert: String,
}

/// The word-level changes that turn `base` into `side`. Edits are in order and do not touch.
fn token_edits(base: &str, side: &str) -> Vec<Edit> {
    let x = tokens(base);
    let y = tokens(side);
    let (n, m) = (x.len(), y.len());
    // lcs[i][j]: length of the common subsequence of x[i..] and y[j..].
    let mut lcs = alloc::vec![alloc::vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if x[i] == y[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Character offset of each base token.
    let mut offset = alloc::vec![0usize; n + 1];
    for k in 0..n {
        offset[k + 1] = offset[k] + x[k].chars().count();
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && x[i] == y[j] {
            i += 1;
            j += 1;
            continue;
        }
        let (i0, j0) = (i, j);
        while i < n || j < m {
            if i < n && j < m && x[i] == y[j] {
                break;
            }
            if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
                i += 1;
            } else {
                j += 1;
            }
        }
        out.push(Edit {
            start: offset[i0],
            end: offset[i],
            insert: y[j0..j].concat(),
        });
    }
    out
}

/// Combines the edits of both sides, or `None` if any two of them touch. An edit both sides made
/// identically is kept once. The result is in descending order of position.
fn merge_edits(ours: Vec<Edit>, theirs: Vec<Edit>) -> Option<Vec<Edit>> {
    let mut out = ours.clone();
    for edit in theirs {
        if ours.contains(&edit) {
            continue;
        }
        if ours
            .iter()
            .any(|o| o.start <= edit.end && edit.start <= o.end)
        {
            return None;
        }
        out.push(edit);
    }
    out.sort_by(|a, b| b.start.cmp(&a.start).then(b.end.cmp(&a.end)));
    Some(out)
}

/// Removes references that the merge left dangling: children of containers and lists, and
/// per-block attributes of blocks that are gone.
fn prune(doc: &mut Document) {
    let ids: BTreeSet<BlockId> = doc.blocks.keys().copied().collect();
    for block in doc.blocks.values_mut() {
        match block {
            Block::Container { children, .. } | Block::Quote { children } => {
                children.retain(|c| ids.contains(c));
            }
            Block::List { items, .. } => items.retain(|c| ids.contains(c)),
            _ => {}
        }
    }
    doc.frames.retain(|block, _| ids.contains(block));
    doc.canvases.retain(|block, _| ids.contains(block));
    doc.footnotes.retain(|block, _| ids.contains(block));
    doc.sections.retain(|block, _| ids.contains(block));
    doc.figures.retain(|block, _| ids.contains(block));
    doc.equation_labels.retain(|block, _| ids.contains(block));
    let sheets: BTreeSet<_> = doc.sheets.keys().copied().collect();
    doc.table_labels.retain(|sheet, _| sheets.contains(sheet));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_edits_and_merge_edits_agree_with_the_text() {
        let base = "one two three";
        let ours = token_edits(base, "ONE two three");
        assert_eq!(ours.len(), 1);
        let theirs = token_edits(base, "one two THREE");
        let merged = merge_edits(ours, theirs).expect("disjoint");
        let mut text = String::from(base);
        for e in &merged {
            text = replace_chars(&text, e.start, e.end, &e.insert).unwrap();
        }
        assert_eq!(text, "ONE two THREE");
    }

    #[test]
    fn children_merge_keeps_both_additions() {
        let base = [BlockId(1)];
        let ours = [BlockId(1), BlockId(2)];
        let theirs = [BlockId(1), BlockId(3)];
        // Theirs' addition follows its predecessor, block 1, so it comes before ours' block 2.
        assert_eq!(
            merge_children(&base, &ours, &theirs),
            [BlockId(1), BlockId(3), BlockId(2)]
        );
    }

    #[test]
    fn three_way_takes_the_side_that_changed() {
        assert_eq!(three_way(&1, &2, &1), Some(&2));
        assert_eq!(three_way(&1, &1, &3), Some(&3));
        assert_eq!(three_way(&1, &2, &3), None);
        assert_eq!(three_way(&1, &2, &2), Some(&2));
    }
}
