// SPDX-License-Identifier: MIT OR Apache-2.0
//! `tpt merge`: three text files in, one merged file out.

use std::fs;
use std::path::{Path, PathBuf};

use tpt_codex_cli::run;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("merge-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

struct Outcome {
    status: i32,
    out: String,
}

fn tpt(args: &[&str]) -> Outcome {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &mut out, &mut err);
    let mut text = String::from_utf8(out).unwrap();
    text.push_str(&String::from_utf8(err).unwrap());
    Outcome { status, out: text }
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn write_three(dir: &Path, base: &str, ours: &str, theirs: &str) -> [PathBuf; 3] {
    let paths = [
        dir.join("base.md"),
        dir.join("ours.md"),
        dir.join("theirs.md"),
    ];
    for (path, text) in paths.iter().zip([base, ours, theirs]) {
        fs::write(path, text).unwrap();
    }
    paths
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

const BASE: &str =
    "# Title\n\nThe quick brown fox jumps over it.\n\nSecond paragraph here.\n\nThird one.\n";

#[test]
fn edits_combine_even_when_a_side_adds_a_paragraph() {
    let dir = scratch("combine");
    let [base, ours, theirs] = write_three(
        &dir,
        BASE,
        // Ours adds a paragraph at the top, which moves every block after it, and changes a word.
        "# Title\n\nAn added introduction.\n\nThe slow brown fox jumps over it.\n\nSecond paragraph here.\n\nThird one.\n",
        // Theirs changes another word of the same paragraph, and a later paragraph.
        "# Title\n\nThe quick brown fox leaps over it.\n\nSecond paragraph here.\n\nThird one, edited.\n",
    );
    let merged = dir.join("merged.md");
    let out = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(out.status, 0, "{}", out.out);
    assert!(out.out.contains("0 conflict(s)"), "{}", out.out);
    assert_eq!(
        read(&merged),
        "# Title\n\nAn added introduction.\n\nThe slow brown fox leaps over it.\n\nSecond paragraph here.\n\nThird one, edited.\n"
    );
}

#[test]
fn a_real_conflict_is_left_to_the_user_unless_told_which_side() {
    let dir = scratch("conflict");
    let [base, ours, theirs] = write_three(
        &dir,
        BASE,
        &BASE.replace("quick", "slow"),
        &BASE.replace("quick", "fast"),
    );
    let merged = dir.join("merged.md");
    let manual = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(manual.status, 1, "{}", manual.out);
    assert!(manual.out.contains("conflict: block 2"), "{}", manual.out);
    assert!(manual.out.contains("1 unresolved"), "{}", manual.out);
    assert!(read(&merged).contains("slow"), "ours is kept");

    let theirs_wins = tpt(&[
        "merge",
        s(&base),
        s(&ours),
        s(&theirs),
        "-o",
        s(&merged),
        "--prefer",
        "theirs",
    ]);
    assert_eq!(theirs_wins.status, 0, "{}", theirs_wins.out);
    assert!(read(&merged).contains("fast"));

    let bad = tpt(&[
        "merge",
        s(&base),
        s(&ours),
        s(&theirs),
        "-o",
        s(&merged),
        "--prefer",
        "both",
    ]);
    assert_eq!(bad.status, 2);
}

#[test]
fn a_paragraph_removed_on_one_side_and_one_added_on_the_other_both_happen() {
    let dir = scratch("structure");
    let [base, ours, theirs] = write_three(
        &dir,
        BASE,
        // Ours removes the second paragraph.
        "# Title\n\nThe quick brown fox jumps over it.\n\nThird one.\n",
        // Theirs adds one at the end.
        "# Title\n\nThe quick brown fox jumps over it.\n\nSecond paragraph here.\n\nThird one.\n\nA closing note.\n",
    );
    let merged = dir.join("merged.md");
    let out = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(out.status, 0, "{}", out.out);
    assert_eq!(
        read(&merged),
        "# Title\n\nThe quick brown fox jumps over it.\n\nThird one.\n\nA closing note.\n"
    );
}

#[test]
fn both_sides_adding_different_paragraphs_keeps_both() {
    let dir = scratch("both-add");
    let [base, ours, theirs] = write_three(
        &dir,
        BASE,
        "# Title\n\nOurs opens here.\n\nThe quick brown fox jumps over it.\n\nSecond paragraph here.\n\nThird one.\n",
        "# Title\n\nThe quick brown fox jumps over it.\n\nSecond paragraph here.\n\nThird one.\n\nTheirs closes here.\n",
    );
    let merged = dir.join("merged.md");
    let out = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(out.status, 0, "{}", out.out);
    let text = read(&merged);
    assert!(text.contains("Ours opens here."), "{text}");
    assert!(text.contains("Theirs closes here."), "{text}");
    assert_eq!(text.matches("Second paragraph here.").count(), 1, "{text}");
}

#[test]
fn merging_a_file_with_itself_changes_nothing() {
    let dir = scratch("same");
    let [base, ours, theirs] = write_three(&dir, BASE, BASE, BASE);
    let merged = dir.join("merged.md");
    let out = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(out.status, 0, "{}", out.out);
    assert_eq!(read(&merged), BASE);
}

#[test]
fn lists_footnotes_and_grids_line_up_too() {
    let dir = scratch("rich");
    let base_text = "# T\n\n- one\n- two\n\nNote[^a].\n\n[^a]: The note.\n\n::: grid { id=\"1\" }\n| a | b |\n| --- | --- |\n| 1 | 2 |\n:::\n";
    let [base, ours, theirs] = write_three(
        &dir,
        base_text,
        // Ours adds a paragraph before the list, shifting everything, and edits the note.
        &format!(
            "# T\n\nA new first paragraph.\n{}",
            &base_text["# T\n".len()..]
        )
        .replace("The note.", "The note, improved."),
        // Theirs edits a grid cell and a list item.
        &base_text
            .replace("| 1 | 2 |", "| 1 | 3 |")
            .replace("- two", "- twelve"),
    );
    let merged = dir.join("merged.md");
    let out = tpt(&["merge", s(&base), s(&ours), s(&theirs), "-o", s(&merged)]);
    assert_eq!(out.status, 0, "{}", out.out);
    let text = read(&merged);
    for expected in [
        "A new first paragraph.",
        "- twelve",
        "The note, improved.",
        "| 1 | 3 |",
    ] {
        assert!(text.contains(expected), "{expected} missing from:\n{text}");
    }
    assert_eq!(text.matches("[^a]:").count(), 1, "{text}");
}
