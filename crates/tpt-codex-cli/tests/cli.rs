// SPDX-License-Identifier: MIT OR Apache-2.0
use std::fs;
use std::path::{Path, PathBuf};

use tpt_codex_cli::{TEMPLATES, Watcher, run};

/// A scratch directory for one test, under cargo's per-target temp directory.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/office")
        .join(name)
}

struct Outcome {
    status: i32,
    out: String,
    err: String,
}

fn tpt(args: &[&str]) -> Outcome {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &mut out, &mut err);
    Outcome {
        status,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn every_template_checks_clean_and_exports_to_pdf_and_markdown() {
    let dir = scratch("templates");
    for (name, _, _) in TEMPLATES {
        let file = dir.join(format!("{name}.cdx"));
        let made = tpt(&["new", name, "-o", s(&file)]);
        assert_eq!(made.status, 0, "{name}: {}", made.err);

        let checked = tpt(&["check", s(&file), "--strict"]);
        assert_eq!(checked.status, 0, "{name}: {}{}", checked.out, checked.err);

        let pdf = dir.join(format!("{name}.pdf"));
        let converted = tpt(&["convert", s(&file), "-o", s(&pdf)]);
        assert_eq!(converted.status, 0, "{name}: {}", converted.err);
        assert!(fs::read(&pdf).unwrap().starts_with(b"%PDF-"), "{name}");

        let md = dir.join(format!("{name}-again.cdx"));
        assert_eq!(
            tpt(&["convert", s(&file), "-o", s(&md)]).status,
            0,
            "{name}"
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap().replace("\r\n", "\n"),
            fs::read_to_string(&md).unwrap().replace("\r\n", "\n"),
            "{name}: a template is already in the writer's canonical form"
        );
    }
}

#[test]
fn pdf_output_is_deterministic() {
    let dir = scratch("determinism");
    let file = dir.join("a.cdx");
    tpt(&["new", "article", "-o", s(&file)]);
    let (one, two) = (dir.join("one.pdf"), dir.join("two.pdf"));
    tpt(&["convert", s(&file), "-o", s(&one)]);
    tpt(&["convert", s(&file), "-o", s(&two)]);
    assert_eq!(fs::read(one).unwrap(), fs::read(two).unwrap());
}

#[test]
fn office_files_convert_and_report_what_was_lost() {
    let dir = scratch("office");

    let md = dir.join("basic.cdx");
    let done = tpt(&[
        "convert",
        s(&fixture("basic.docx")),
        "-o",
        s(&md),
        "--report",
    ]);
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(done.out.contains("import:"), "{}", done.out);
    assert!(!fs::read_to_string(&md).unwrap().trim().is_empty());

    let pdf = dir.join("basic.pdf");
    assert_eq!(
        tpt(&["convert", s(&fixture("basic.docx")), "-o", s(&pdf)]).status,
        0
    );
    assert!(fs::read(pdf).unwrap().starts_with(b"%PDF-"));

    let slides = dir.join("slides.cdx");
    let done = tpt(&["convert", s(&fixture("basic.pptx")), "-o", s(&slides)]);
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(fs::read_to_string(slides).unwrap().contains("## "));

    let book = dir.join("book.cdx");
    let done = tpt(&["convert", s(&fixture("basic.xlsx")), "-o", s(&book)]);
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(fs::read_to_string(&book).unwrap().contains("::: grid"));

    let back = dir.join("back.xlsx");
    assert_eq!(tpt(&["convert", s(&book), "-o", s(&back)]).status, 0);
    assert!(fs::read(back).unwrap().starts_with(b"PK"));

    let docx = dir.join("out.docx");
    assert_eq!(tpt(&["convert", s(&md), "-o", s(&docx)]).status, 0);
    assert!(fs::read(docx).unwrap().starts_with(b"PK"));
}

#[test]
fn a_markdown_error_is_reported_with_file_line_and_column() {
    let dir = scratch("parse-error");
    let file = dir.join("bad.md");
    fs::write(&file, "# Title\n\n> a quote\n").unwrap();
    let out = tpt(&["check", s(&file)]);
    assert_eq!(out.status, 1);
    assert!(out.err.contains("bad.md:3:"), "{}", out.err);
    assert!(out.err.contains("error:"), "{}", out.err);
}

#[test]
fn check_finds_unresolved_references_and_bad_math() {
    let dir = scratch("check");
    let file = dir.join("doc.md");
    fs::write(
        &file,
        "# Title\n\nSee [nothing](#missing) and [a figure](#fig:none).\n\n$$ \\frac{1 $$\n\nA note.[^gone]\n",
    )
    .unwrap();
    let out = tpt(&["check", s(&file)]);
    assert_eq!(out.status, 1, "{}{}", out.out, out.err);
    assert!(
        out.out.contains("no heading has that anchor"),
        "{}",
        out.out
    );
    assert!(out.out.contains("no figure has that label"), "{}", out.out);
    assert!(out.out.contains("math"), "{}", out.out);
    assert!(out.out.contains("[^gone] has no definition"), "{}", out.out);
    assert!(out.out.contains("error(s)"), "{}", out.out);
}

#[test]
fn citations_are_checked_only_with_a_bibliography() {
    let dir = scratch("bib");
    let file = dir.join("doc.md");
    fs::write(&file, "# T\n\nAs shown in [@known] and [@unknown].\n").unwrap();
    let bib = dir.join("refs.json");
    fs::write(
        &bib,
        r#"[{"id":"known","title":"A Title","author":[{"family":"Smith","given":"A"}],"issued":{"date-parts":[[2020]]}}]"#,
    )
    .unwrap();

    let without = tpt(&["check", s(&file)]);
    assert_eq!(without.status, 0, "{}", without.out);
    assert!(
        without.err.contains("2 citation(s) were not checked"),
        "{}",
        without.err
    );

    let with = tpt(&["check", s(&file), "--bib", s(&bib)]);
    assert_eq!(with.status, 1);
    assert!(
        with.out.contains("[@unknown] is not in the bibliography"),
        "{}",
        with.out
    );
    assert!(!with.out.contains("[@known]"), "{}", with.out);
}

#[test]
fn strict_makes_warnings_fail() {
    let dir = scratch("strict");
    let file = dir.join("doc.md");
    fs::write(&file, "# T\n\nText.\n\n[^unused]: Never referenced.\n").unwrap();
    let plain = tpt(&["check", s(&file)]);
    assert_eq!(plain.status, 0, "{}{}", plain.out, plain.err);
    assert!(plain.out.contains("warning"), "{}", plain.out);
    assert_eq!(tpt(&["check", s(&file), "--strict"]).status, 1);
}

#[test]
fn diff_shows_word_changes_and_sets_the_status() {
    let dir = scratch("diff");
    let (a, b) = (dir.join("a.md"), dir.join("b.md"));
    fs::write(&a, "# T\n\nThe quick brown fox.\n").unwrap();
    fs::write(&b, "# T\n\nThe slow brown fox.\n").unwrap();
    let out = tpt(&["diff", s(&a), s(&b)]);
    assert_eq!(out.status, 1);
    assert!(out.out.contains("[-quick-]"), "{}", out.out);
    assert!(out.out.contains("[+slow+]"), "{}", out.out);
    assert!(out.out.contains("1 block(s) differ"), "{}", out.out);

    let same = tpt(&["diff", s(&a), s(&a)]);
    assert_eq!(same.status, 0);
    assert!(same.out.contains("0 block(s) differ"));
}

#[test]
fn new_refuses_to_overwrite_unless_forced() {
    let dir = scratch("new");
    let file = dir.join("mine.cdx");
    assert_eq!(tpt(&["new", "slides", "-o", s(&file)]).status, 0);
    fs::write(&file, "edited").unwrap();
    let again = tpt(&["new", "slides", "-o", s(&file)]);
    assert_eq!(again.status, 1);
    assert!(again.err.contains("--force"), "{}", again.err);
    assert_eq!(fs::read_to_string(&file).unwrap(), "edited");
    assert_eq!(tpt(&["new", "slides", "-o", s(&file), "--force"]).status, 0);
    assert_ne!(fs::read_to_string(&file).unwrap(), "edited");

    let list = tpt(&["new", "--list"]);
    assert_eq!(list.status, 0);
    for (name, _, _) in TEMPLATES {
        assert!(list.out.contains(name), "{}", list.out);
    }
}

#[test]
fn bad_usage_exits_with_2() {
    assert_eq!(tpt(&[]).status, 2);
    assert_eq!(tpt(&["frobnicate"]).status, 2);
    assert_eq!(tpt(&["convert"]).status, 2);
    assert_eq!(tpt(&["convert", "a.md"]).status, 2, "no output given");
    assert_eq!(tpt(&["convert", "a.md", "--wat"]).status, 2);
    assert_eq!(tpt(&["new", "nonesuch"]).status, 2);
    assert_eq!(
        tpt(&["watch", "a.md", "-o", "a.pdf", "--poll-ms", "0"]).status,
        2
    );
    assert_eq!(tpt(&["check", "a.md", "--bib"]).status, 2);
    let help = tpt(&["--help"]);
    assert_eq!(help.status, 0);
    assert!(help.out.contains("USAGE"));
}

#[test]
fn unreadable_and_unknown_files_fail_with_a_message() {
    let dir = scratch("failures");
    let missing = tpt(&["check", s(&dir.join("nope.md"))]);
    assert_eq!(missing.status, 1);
    assert!(missing.err.contains("cannot read"), "{}", missing.err);

    let odd = dir.join("file.txt");
    fs::write(&odd, "x").unwrap();
    let out = tpt(&["convert", s(&odd), "-o", s(&dir.join("o.pdf"))]);
    assert_eq!(out.status, 1);
    assert!(out.err.contains("unknown file type"), "{}", out.err);

    let md = dir.join("a.md");
    fs::write(&md, "# T\n").unwrap();
    let to_pptx = tpt(&["convert", s(&md), "-o", s(&dir.join("o.pptx"))]);
    assert_eq!(to_pptx.status, 1);

    let no_grid = tpt(&["convert", s(&md), "-o", s(&dir.join("o.xlsx"))]);
    assert_eq!(no_grid.status, 1);
    assert!(no_grid.err.contains("no grid"), "{}", no_grid.err);

    let not_pdf_in = tpt(&["check", s(&dir.join("x.pdf"))]);
    assert_eq!(not_pdf_in.status, 1);
}

#[test]
fn json_round_trips_the_model() {
    let dir = scratch("json");
    let md = dir.join("a.cdx");
    tpt(&["new", "budget", "-o", s(&md)]);
    let json = dir.join("a.json");
    assert_eq!(tpt(&["convert", s(&md), "-o", s(&json)]).status, 0);
    let back = dir.join("b.cdx");
    assert_eq!(tpt(&["convert", s(&json), "-o", s(&back)]).status, 0);
    assert_eq!(fs::read(&md).unwrap(), fs::read(&back).unwrap());
}

#[test]
fn watcher_reports_the_first_look_and_each_change() {
    let dir = scratch("watch");
    let file = dir.join("w.md");
    fs::write(&file, "one").unwrap();
    let mut watcher = Watcher::default();
    assert!(watcher.changed(&file), "the first look counts as a change");
    assert!(!watcher.changed(&file));
    fs::write(&file, "two, longer").unwrap();
    assert!(watcher.changed(&file));
    assert!(!watcher.changed(&file));
    fs::remove_file(&file).unwrap();
    assert!(!watcher.changed(&file), "a missing file is not a change");
}
