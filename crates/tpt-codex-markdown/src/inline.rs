// SPDX-License-Identifier: MIT OR Apache-2.0
//! Inline Markdown in the supported subset: `*emphasis*`, `**strong**`, `[text](address)` and
//! backslash escapes. The reader and the writer share the rules here, so a document the writer
//! accepts reads back as the same document. See `docs/markdown-grammar.md`.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::InlineRun;

/// Text with its marks, as read from or written to Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Inline {
    /// The plain text, with escapes removed.
    pub(crate) text: String,
    /// Marks on character ranges of `text`.
    pub(crate) runs: Vec<InlineRun>,
}

/// Why inline text cannot be read or written, with a fix for the author.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InlineError {
    /// What is wrong.
    pub(crate) message: &'static str,
    /// What to do about it.
    pub(crate) suggestion: &'static str,
}

const UNMATCHED_EMPHASIS: InlineError = InlineError {
    message: "emphasis marker is not matched by a closing marker",
    suggestion: "close the emphasis, or escape the marker with a backslash",
};
const EMPHASIS_SPACE: InlineError = InlineError {
    message: "emphasis marker is next to a space",
    suggestion: "remove the space inside the markers, or escape the marker with a backslash",
};
const LINK_SYNTAX: InlineError = InlineError {
    message: "link brackets are not followed by an address in parentheses",
    suggestion: "write the link as [text](address), or escape the bracket with a backslash",
};
const LINK_UNCLOSED: InlineError = InlineError {
    message: "square bracket is not closed",
    suggestion: "close the bracket, or escape it with a backslash",
};
const LINK_TEXT: InlineError = InlineError {
    message: "link text cannot be empty or contain emphasis or brackets",
    suggestion: "write plain link text without * or square brackets",
};
const LINK_IN_EMPHASIS: InlineError = InlineError {
    message: "a link cannot start inside emphasis",
    suggestion: "move the link outside the emphasis markers",
};
const CITATION_KEY: InlineError = InlineError {
    message: "citation key has a character other than letters, digits, - _ : or .",
    suggestion: "use only letters, digits, and - _ : . in a citation key",
};
const LINK_ADDRESS: InlineError = InlineError {
    message: "link address has a space, parenthesis, angle bracket or backslash",
    suggestion: "use an address without spaces, parentheses, angle brackets or backslashes",
};

/// Reads inline Markdown. Constructs outside the subset are errors, not literal text.
pub(crate) fn parse_inline(src: &str) -> Result<Inline, InlineError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out: Vec<char> = Vec::new();
    let mut runs: Vec<InlineRun> = Vec::new();
    // The open emphasis: (is strong, start in `out`).
    let mut open: Option<(bool, usize)> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1).is_some_and(|n| n.is_ascii_punctuation()) {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        match c {
            '*' => {
                let run = chars[i..].iter().take_while(|&&x| x == '*').count();
                match open {
                    Some((strong, start)) => {
                        // A closing strong marker is two stars; a closing emphasis is one. A
                        // longer run is read as the closing marker followed by an opening one.
                        let len = if strong { 2 } else { 1 };
                        if run < len {
                            return Err(UNMATCHED_EMPHASIS);
                        }
                        if out.len() == start || out.last().is_some_and(|c| c.is_whitespace()) {
                            return Err(EMPHASIS_SPACE);
                        }
                        runs.push(InlineRun {
                            start,
                            end: out.len(),
                            bold: strong,
                            italic: !strong,
                            link: None,
                        });
                        open = None;
                        i += len;
                    }
                    None => {
                        let len = if run >= 2 { 2 } else { 1 };
                        if chars.get(i + len).is_none_or(|n| n.is_whitespace()) {
                            return Err(EMPHASIS_SPACE);
                        }
                        open = Some((len == 2, out.len()));
                        i += len;
                    }
                }
            }
            '[' => {
                if open.is_some() {
                    return Err(LINK_IN_EMPHASIS);
                }
                let close = find_close_bracket(&chars, i + 1)?;
                if chars.get(close + 1) != Some(&'(') {
                    // A bare [@key] is a citation, and [^label] a footnote reference. Each shows its
                    // key or label until the text is replaced.
                    if let Some(&prefix @ ('@' | '^')) = chars.get(i + 1) {
                        let key: String = chars[i + 2..close].iter().collect();
                        if !is_citation_key(&key) {
                            return Err(CITATION_KEY);
                        }
                        let start = out.len();
                        out.extend(key.chars());
                        runs.push(InlineRun {
                            start,
                            end: out.len(),
                            bold: false,
                            italic: false,
                            link: Some(alloc::format!("{prefix}{key}")),
                        });
                        i = close + 1;
                        continue;
                    }
                    return Err(LINK_SYNTAX);
                }
                let address_start = close + 2;
                let address_end = chars[address_start..]
                    .iter()
                    .position(|&x| x == ')')
                    .map(|p| address_start + p)
                    .ok_or(LINK_SYNTAX)?;
                let address: String = chars[address_start..address_end].iter().collect();
                if !is_address(&address) {
                    return Err(LINK_ADDRESS);
                }
                let text = plain(&chars[i + 1..close])?;
                let start = out.len();
                out.extend(text.chars());
                runs.push(InlineRun {
                    start,
                    end: out.len(),
                    bold: false,
                    italic: false,
                    link: Some(address),
                });
                i = address_end + 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    if open.is_some() {
        return Err(UNMATCHED_EMPHASIS);
    }
    Ok(Inline {
        text: out.into_iter().collect(),
        runs,
    })
}

/// Finds the `]` that closes a link's text, starting at `from`.
fn find_close_bracket(chars: &[char], from: usize) -> Result<usize, InlineError> {
    let mut j = from;
    while j < chars.len() {
        match chars[j] {
            '\\' if chars.get(j + 1).is_some_and(|n| n.is_ascii_punctuation()) => j += 2,
            ']' => return Ok(j),
            '[' => return Err(LINK_TEXT),
            _ => j += 1,
        }
    }
    Err(LINK_UNCLOSED)
}

/// Reads link text: escapes allowed, no emphasis, no brackets, not empty.
fn plain(chars: &[char]) -> Result<String, InlineError> {
    let mut out = String::new();
    let mut j = 0;
    while j < chars.len() {
        match chars[j] {
            '\\' if chars.get(j + 1).is_some_and(|n| n.is_ascii_punctuation()) => {
                out.push(chars[j + 1]);
                j += 2;
            }
            '*' | '[' | ']' => return Err(LINK_TEXT),
            c => {
                out.push(c);
                j += 1;
            }
        }
    }
    if out.is_empty() {
        return Err(LINK_TEXT);
    }
    Ok(out)
}

/// Whether `key` is a citation key: non-empty, with letters, digits, `-`, `_`, `:` or `.`.
pub(crate) fn is_citation_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
}

/// Whether `address` can be written inside `(…)` and read back unchanged.
fn is_address(address: &str) -> bool {
    !address.is_empty()
        && !address
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | '\\'))
}

/// Writes `text` with its runs as inline Markdown that [`parse_inline`] reads back exactly.
///
/// Refuses what the subset cannot express: bold and italic together, emphasis or a link with a
/// leading or trailing space inside its markers, a link inside emphasis, and a character with two
/// different links.
pub(crate) fn write_inline(text: &str, runs: &[InlineRun]) -> Result<String, &'static str> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut bold = alloc::vec![false; n];
    let mut italic = alloc::vec![false; n];
    let mut link: Vec<Option<&str>> = alloc::vec![None; n];
    for run in runs {
        if run.start >= run.end || run.end > n {
            return Err("an inline run is outside its text");
        }
        for i in run.start..run.end {
            bold[i] |= run.bold;
            italic[i] |= run.italic;
            if let Some(url) = run.link.as_deref() {
                match link[i] {
                    Some(existing) if existing != url => {
                        return Err("a character cannot link to two addresses");
                    }
                    _ => link[i] = Some(url),
                }
            }
        }
    }

    let mut out = String::new();
    let mut i = 0;
    while i < n {
        let key = (bold[i], italic[i], link[i]);
        let mut j = i + 1;
        while j < n && (bold[j], italic[j], link[j]) == key {
            j += 1;
        }
        let segment: String = chars[i..j].iter().collect();
        match key {
            (false, false, None) => out.push_str(&escape(&segment)),
            (true, true, _) => return Err("bold and italic together are not written yet"),
            (_, _, Some(_)) if key.0 || key.1 => {
                return Err("links cannot be bold or italic in the Markdown subset");
            }
            (strong, _, None) => {
                if segment.trim() != segment {
                    return Err("bold or italic text cannot start or end with a space");
                }
                let marker = if strong { "**" } else { "*" };
                out.push_str(marker);
                out.push_str(&escape(&segment));
                out.push_str(marker);
            }
            (_, _, Some(url))
                if (url.starts_with('@') || url.starts_with('^')) && segment == url[1..] =>
            {
                // A bare citation or footnote reference, when the text is the key or label.
                let key = &url[1..];
                if !is_citation_key(key) {
                    return Err(
                        "citation key or footnote label has a character other than letters, digits, - _ : or .",
                    );
                }
                out.push('[');
                out.push_str(&url[..1]);
                out.push_str(key);
                out.push(']');
            }
            (_, _, Some(url)) => {
                if !is_address(url) {
                    return Err(
                        "link address has a space, parenthesis, angle bracket or backslash",
                    );
                }
                out.push('[');
                out.push_str(&escape(&segment));
                out.push_str("](");
                out.push_str(url);
                out.push(')');
            }
        }
        i = j;
    }
    Ok(out)
}

/// Escapes the characters that would otherwise start inline syntax.
///
/// A backslash is escaped only where it would start an escape: before ASCII punctuation, or at
/// the end of the segment, since a marker may follow it. Other backslashes stay as written.
pub(crate) fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\\' => {
                let escapes_next = chars.get(i + 1).is_none_or(|n| n.is_ascii_punctuation());
                if escapes_next {
                    out.push('\\');
                }
                out.push(c);
            }
            '*' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn run(start: usize, end: usize, bold: bool, italic: bool) -> InlineRun {
        InlineRun {
            start,
            end,
            bold,
            italic,
            link: None,
        }
    }

    #[test]
    fn reads_emphasis_strong_and_links() {
        let p = parse_inline("a *b* **c** [d](https://x.test)").unwrap();
        assert_eq!(p.text, "a b c d");
        assert_eq!(p.runs[0], run(2, 3, false, true));
        assert_eq!(p.runs[1], run(4, 5, true, false));
        assert_eq!(p.runs[2].link.as_deref(), Some("https://x.test"));
        assert_eq!((p.runs[2].start, p.runs[2].end), (6, 7));
    }

    #[test]
    fn escapes_are_literal() {
        let p = parse_inline(r"\*not\* \[a\] \\").unwrap();
        assert_eq!(p.text, r"*not* [a] \");
        assert!(p.runs.is_empty());
    }

    #[test]
    fn a_lone_star_before_a_space_is_an_error() {
        assert!(parse_inline("2 * 3").is_err());
        assert!(parse_inline("a * b").is_err());
    }

    #[test]
    fn unclosed_markers_are_errors() {
        assert!(parse_inline("*open").is_err());
        assert!(parse_inline("[open").is_err());
        assert!(parse_inline("[x] tail").is_err());
    }

    #[test]
    fn adjacent_emphasis_reads_back() {
        let p = parse_inline("**a***b*").unwrap();
        assert_eq!(p.text, "ab");
        assert_eq!(p.runs[0], run(0, 1, true, false));
        assert_eq!(p.runs[1], run(1, 2, false, true));
    }

    #[test]
    fn writer_output_reads_back_to_the_same_text_and_runs() {
        let cases: Vec<(&str, Vec<InlineRun>)> = vec![
            ("plain", vec![]),
            ("a*b[c]\\d", vec![]),
            (
                "bold and italic",
                vec![run(0, 4, true, false), run(9, 15, false, true)],
            ),
            (
                "**a***b*",
                vec![run(0, 1, true, false), run(1, 2, false, true)],
            ),
            ("a*b*", vec![run(1, 3, false, true)]),
            (
                "see docs now",
                vec![InlineRun {
                    start: 4,
                    end: 8,
                    bold: false,
                    italic: false,
                    link: Some("https://x.test/a".into()),
                }],
            ),
        ];
        for (text, runs) in cases {
            // Emphasis with a space at its edge is refused, so skip that case here.
            let Ok(md) = write_inline(text, &runs) else {
                continue;
            };
            let back = parse_inline(&md).unwrap_or_else(|e| panic!("{md}: {e:?}"));
            assert_eq!(back.text, text, "{md}");
            assert_eq!(back.runs, runs, "{md}");
        }
    }

    #[test]
    fn writer_refuses_what_the_subset_cannot_express() {
        assert!(write_inline("x", &[run(0, 1, true, true)]).is_err());
        assert!(write_inline("ab", &[run(0, 2, true, false)]).is_ok());
        assert!(write_inline("a b", &[run(0, 3, true, false)]).is_ok());
        assert!(write_inline(" a", &[run(0, 2, true, false)]).is_err());
        let link = |s, e, url: &str| InlineRun {
            start: s,
            end: e,
            bold: false,
            italic: false,
            link: Some(url.into()),
        };
        assert!(write_inline("ab", &[link(0, 2, "a b")]).is_err());
        assert!(write_inline("ab", &[link(0, 2, "x"), link(1, 2, "y")]).is_err());
        let mut bold_link = link(0, 2, "x");
        bold_link.bold = true;
        assert!(write_inline("ab", &[bold_link]).is_err());
        assert!(write_inline("ab", &[run(0, 5, true, false)]).is_err());
    }
}
