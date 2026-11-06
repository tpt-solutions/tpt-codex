// SPDX-License-Identifier: MIT OR Apache-2.0
//! In-house hyphenation for English words, used when a line is broken.
//!
//! This is our own rule set, not a dictionary or a pattern table from another project. It works on
//! ASCII letters only; any other word is never hyphenated. The rules:
//!
//! - A word is split into vowel groups (`a e i o u`, and `y` after the first letter) and the
//!   consonant runs between them. A consonant digraph (`ch`, `sh`, `th`, `ph`, `wh`, `gh`, `ck`,
//!   `ng`) counts as one consonant.
//! - Between two vowel groups, a single consonant starts the second syllable (`ta-ble`).
//! - In a longer run, the break goes before the longest run ending that is a valid onset, a
//!   consonant cluster that can begin a syllable (`ex-tra`, `com-pu-ter`, `con-struct`). A run
//!   with no valid onset longer than one consonant breaks before its last consonant.
//! - A break leaves at least [`LEFT_MIN`] letters before it and [`RIGHT_MIN`] after it.
//! - A short list of exceptions gives the points for words that these rules get wrong.
//!
//! This is an approximation. It does not know affixes or stress, so it will sometimes break a
//! word where a dictionary would not. Improve it by adding exceptions or onsets, not by guessing.

use alloc::string::String;
use alloc::vec::Vec;

/// Letters that must stay before a break.
pub const LEFT_MIN: usize = 2;
/// Letters that must follow a break.
pub const RIGHT_MIN: usize = 3;

/// Consonant clusters that may begin a syllable, as their letters.
const ONSETS: &[&str] = &[
    "bl", "br", "cl", "cr", "dr", "dw", "fl", "fr", "gl", "gr", "kl", "kr", "pl", "pr", "tr", "tw",
    "sc", "sk", "sl", "sm", "sn", "sp", "st", "sw", "chr", "thr", "shr", "spl", "spr", "scr",
    "str",
];

/// Words that the rules get wrong, with the byte offsets of their break points.
const EXCEPTIONS: &[(&str, &[usize])] = &[
    ("hyphenation", &[2, 6]),
    ("algorithm", &[2, 4]),
    ("associate", &[2, 4, 6]),
];

/// Where a word may be broken, as byte offsets from its start, in order. Empty for words that are
/// too short, not plain ASCII letters, or have no break that keeps both sides long enough.
pub fn break_points(word: &str) -> Vec<usize> {
    if word.len() < LEFT_MIN + RIGHT_MIN || !word.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Vec::new();
    }
    let lower: String = word.to_ascii_lowercase();
    if let Some((_, points)) = EXCEPTIONS.iter().find(|(w, _)| *w == lower) {
        return points
            .iter()
            .copied()
            .filter(|&p| p >= LEFT_MIN && word.len() - p >= RIGHT_MIN)
            .collect();
    }
    let units = units(&lower);
    let mut points = Vec::new();
    let mut i = 0;
    while i < units.len() {
        // A vowel group, then the consonant run after it.
        if !units[i].vowel {
            i += 1;
            continue;
        }
        while i < units.len() && units[i].vowel {
            i += 1;
        }
        let run_start = i;
        while i < units.len() && !units[i].vowel {
            i += 1;
        }
        // A break needs a vowel after the run, and a run that is not at the start of the word.
        if i >= units.len() || run_start == i {
            continue;
        }
        let run = &units[run_start..i];
        let split = (0..run.len())
            .find(|&j| is_onset(&run[j..]))
            .unwrap_or(run.len() - 1);
        let at = run[split].start;
        if at >= LEFT_MIN && word.len() - at >= RIGHT_MIN {
            points.push(at);
        }
    }
    points
}

/// A letter, or a consonant digraph, with its byte range in the word.
#[derive(Debug, Clone, Copy)]
struct Unit<'a> {
    text: &'a str,
    start: usize,
    vowel: bool,
}

/// Splits a lowercase word into vowel letters and consonant units. `y` is a vowel after the first
/// letter, and a consonant digraph is one unit.
fn units(word: &str) -> Vec<Unit<'_>> {
    const DIGRAPHS: &[&str] = &["ch", "sh", "th", "ph", "wh", "gh", "ck", "ng"];
    let bytes = word.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let pair = bytes
            .get(i..i + 2)
            .and_then(|p| core::str::from_utf8(p).ok());
        let (len, vowel) = match pair {
            Some(p) if DIGRAPHS.contains(&p) => (2, false),
            _ => {
                let c = bytes[i] as char;
                let vowel = matches!(c, 'a' | 'e' | 'i' | 'o' | 'u') || (c == 'y' && i > 0);
                (1, vowel)
            }
        };
        out.push(Unit {
            text: &word[i..i + len],
            start: i,
            vowel,
        });
        i += len;
    }
    out
}

/// Whether the consonant units form a valid onset: one unit, or a listed cluster.
fn is_onset(run: &[Unit<'_>]) -> bool {
    if run.len() == 1 {
        return true;
    }
    let joined: String = run.iter().map(|u| u.text).collect();
    ONSETS.contains(&joined.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn hyphenated(word: &str) -> String {
        let points = break_points(word);
        let mut out = String::new();
        let mut last = 0;
        for p in points {
            out.push_str(&word[last..p]);
            out.push('-');
            last = p;
        }
        out.push_str(&word[last..]);
        out
    }

    #[test]
    fn vowel_consonant_vowel_breaks_before_the_consonant() {
        assert_eq!(hyphenated("table"), "ta-ble");
        assert_eq!(hyphenated("paper"), "pa-per");
        assert_eq!(hyphenated("hotel"), "ho-tel");
    }

    #[test]
    fn clusters_break_at_the_longest_onset() {
        assert_eq!(hyphenated("computer"), "com-pu-ter");
        assert_eq!(hyphenated("extra"), "ex-tra");
        assert_eq!(hyphenated("construct"), "con-struct");
        assert_eq!(hyphenated("object"), "ob-ject");
    }

    #[test]
    fn digraphs_are_not_split() {
        assert_eq!(hyphenated("machine"), "ma-chine");
        assert_eq!(hyphenated("lather"), "la-ther");
        assert_eq!(break_points("machine"), vec![2], "ch is not split");
    }

    #[test]
    fn exceptions_give_their_points() {
        assert_eq!(hyphenated("hyphenation"), "hy-phen-ation");
        assert_eq!(hyphenated("algorithm"), "al-go-rithm");
        assert_eq!(hyphenated("associate"), "as-so-ci-ate");
    }

    #[test]
    fn short_and_non_letter_words_are_never_broken() {
        assert!(break_points("the").is_empty());
        assert!(break_points("hello").len() <= 1);
        assert!(break_points("e-mail-address").is_empty());
        assert!(break_points("Ärger").is_empty());
        assert!(break_points("c3po-units").is_empty());
    }

    #[test]
    fn every_break_keeps_the_minimum_on_both_sides() {
        for word in ["extraordinary", "encyclopedia", "a", "ab", "abcdefgh"] {
            for p in break_points(word) {
                assert!(p >= LEFT_MIN, "{word} breaks at {p}");
                assert!(word.len() - p >= RIGHT_MIN, "{word} breaks at {p}");
            }
        }
    }
}
