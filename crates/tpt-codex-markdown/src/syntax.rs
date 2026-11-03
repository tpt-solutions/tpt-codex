// SPDX-License-Identifier: MIT OR Apache-2.0
//! Line-level classification shared by the reader and the writer.
//!
//! Both directions use the same predicates, so anything the writer emits is
//! guaranteed to read back as the same block.

/// Returns `(level, text)` for an ATX heading line such as `## Title`.
///
/// Requires a space after the hashes, or nothing at all (an empty heading,
/// which the reader rejects). A line of seven or more hashes is not a heading.
pub(crate) fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    if rest.is_empty() {
        return Some((hashes as u8, ""));
    }
    let text = rest.strip_prefix(' ')?;
    Some((hashes as u8, text.trim()))
}

/// Returns a reason if the line starts syntax this crate does not read yet.
///
/// Unsupported syntax is an error rather than being read as plain text, so
/// nothing is silently dropped or reinterpreted.
pub(crate) fn unsupported(line: &str) -> Option<&'static str> {
    if line.starts_with('\t') || line.starts_with("    ") {
        return Some("indented code block");
    }
    let t = line.trim_start_matches(' ');
    if t.is_empty() {
        return None;
    }
    if t == "-" || t == "*" || t == "+" {
        return Some("empty list item");
    }
    for marker in [
        "- ", "* ", "+ ", "> ", ">", "```", "~~~", "|", ":::", "$$", "<", "![", "[^",
    ] {
        if t.starts_with(marker) {
            return Some(match marker {
                "- " | "* " | "+ " => "bullet list",
                "> " | ">" => "block quote",
                "```" | "~~~" => "fenced code block",
                "|" => "table",
                ":::" => "container block",
                "$$" => "math block",
                "<" => "raw HTML",
                "![" => "image",
                _ => "footnote",
            });
        }
    }
    if is_ordered_list_item(t) {
        return Some("ordered list");
    }
    if is_thematic_break_or_setext(t) {
        return Some("thematic break or setext heading underline");
    }
    None
}

fn is_ordered_list_item(t: &str) -> bool {
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return false;
    }
    matches!(t.as_bytes().get(digits), Some(b'.' | b')'))
        && matches!(t.as_bytes().get(digits + 1), Some(b' ') | None)
}

fn is_thematic_break_or_setext(t: &str) -> bool {
    let marks: &[u8] = b"-*_=";
    let first = match t.bytes().next() {
        Some(b) if marks.contains(&b) => b,
        _ => return false,
    };
    t.bytes().all(|b| b == first || b == b' ')
}
