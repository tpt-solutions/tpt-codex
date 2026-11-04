// SPDX-License-Identifier: MIT OR Apache-2.0
//! Line-level classification shared by the reader and the writer.
//!
//! Both directions use the same predicates, so anything the writer emits is
//! guaranteed to read back as the same block.

/// Largest number a list item may start with (nine digits, as in CommonMark).
pub(crate) const MAX_LIST_NUMBER: u32 = 999_999_999;

/// Number of leading spaces.
fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b' ').count()
}

/// The line without up to three leading spaces, or `None` if it is indented further.
fn strip_block_indent(line: &str) -> Option<&str> {
    let n = leading_spaces(line);
    if n > 3 {
        return None;
    }
    Some(&line[n..])
}

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

/// Returns the mark character of a thematic break: three or more of one of `*`, `-`, `_`,
/// optionally separated by spaces, and nothing else.
pub(crate) fn thematic_break(line: &str) -> Option<u8> {
    let t = strip_block_indent(line)?.trim_end();
    let first = *t.as_bytes().first()?;
    if !matches!(first, b'*' | b'-' | b'_') {
        return None;
    }
    let marks = t.bytes().filter(|&b| b == first).count();
    let only_marks = t.bytes().all(|b| b == first || b == b' ');
    (marks >= 3 && only_marks).then_some(first)
}

/// An opening code fence: `` ``` `` or `~~~`, with its info string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fence<'a> {
    /// The fence character, `` ` `` or `~`.
    pub ch: u8,
    /// How many fence characters open the block (at least 3).
    pub len: usize,
    /// Leading spaces before the opening fence (0 to 3).
    pub indent: usize,
    /// The info string, trimmed. Empty when there is none.
    pub info: &'a str,
}

/// Returns the opening fence for a line, if it is one.
///
/// A backtick fence whose info string contains a backtick is not a fence (CommonMark).
pub(crate) fn fence_open(line: &str) -> Option<Fence<'_>> {
    let indent = leading_spaces(line);
    let body = strip_block_indent(line)?;
    let ch = *body.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = body.bytes().take_while(|&b| b == ch).count();
    if len < 3 {
        return None;
    }
    let info = body[len..].trim();
    if ch == b'`' && info.contains('`') {
        return None;
    }
    Some(Fence {
        ch,
        len,
        indent,
        info,
    })
}

/// Returns `true` when `line` closes a fence made of `len` or more copies of `ch`.
pub(crate) fn fence_closes(line: &str, ch: u8, len: usize) -> bool {
    let Some(body) = strip_block_indent(line) else {
        return false;
    };
    let run = body.bytes().take_while(|&b| b == ch).count();
    run >= len && body[run..].trim().is_empty()
}

/// A list item marker and the item's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ListMarker<'a> {
    /// `true` for `1.` or `1)`, `false` for `-`, `*` or `+`.
    pub ordered: bool,
    /// The number of an ordered item; `1` for a bullet.
    pub number: u32,
    /// The marker character: `-`, `*`, `+`, `.` or `)`.
    pub delim: u8,
    /// The item text, trimmed. Empty for an empty item.
    pub text: &'a str,
}

impl ListMarker<'_> {
    /// Items with the same key continue one list. Changing the key starts a new list.
    pub(crate) fn key(&self) -> (bool, u8) {
        (self.ordered, self.delim)
    }
}

/// Returns the list marker for a line such as `- item` or `3) item`.
pub(crate) fn list_item(line: &str) -> Option<ListMarker<'_>> {
    let body = strip_block_indent(line)?;
    let first = *body.as_bytes().first()?;
    let (ordered, number, delim, rest) = if matches!(first, b'-' | b'*' | b'+') {
        (false, 1, first, &body[1..])
    } else if first.is_ascii_digit() {
        let digits = body.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 9 {
            return None;
        }
        let delim = *body.as_bytes().get(digits)?;
        if delim != b'.' && delim != b')' {
            return None;
        }
        let number: u32 = body[..digits].parse().ok()?;
        (true, number, delim, &body[digits + 1..])
    } else {
        return None;
    };
    if rest.is_empty() {
        return Some(ListMarker {
            ordered,
            number,
            delim,
            text: "",
        });
    }
    let text = rest.strip_prefix(' ')?;
    Some(ListMarker {
        ordered,
        number,
        delim,
        text: text.trim(),
    })
}

/// A standalone image line: `![alt](uri)`.
///
/// Returns `Some(Ok((alt, uri)))` for a supported image, `Some(Err(reason))`
/// for a line that starts like an image but uses syntax this crate does not read,
/// and `None` for any other line.
pub(crate) fn image_line(line: &str) -> Option<Result<(&str, &str), &'static str>> {
    let body = strip_block_indent(line)?.trim_end();
    if !body.starts_with("![") {
        return None;
    }
    Some(parse_image(body))
}

fn parse_image(body: &str) -> Result<(&str, &str), &'static str> {
    let inner = body[2..]
        .strip_suffix(')')
        .ok_or("image must end with ')' on the same line")?;
    let (alt, uri) = inner
        .split_once("](")
        .ok_or("image must be written as ![alt](uri)")?;
    if alt.contains(['[', ']', '\\', '&']) {
        return Err("image alt text cannot contain brackets, backslashes or entities");
    }
    if uri.is_empty() {
        return Err("image has no uri");
    }
    if uri.contains(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | '\\' | '&'))
    {
        return Err("image titles, angle brackets and escapes are not supported");
    }
    Ok((alt, uri))
}

/// Returns a reason if the line starts syntax this crate does not read.
///
/// Unsupported syntax is an error rather than being read as plain text, so
/// nothing is silently dropped or reinterpreted. Fences, lists, thematic
/// breaks, headings and images are handled by the reader before this check.
pub(crate) fn unsupported(line: &str) -> Option<&'static str> {
    if line.starts_with('\t') || line.starts_with("    ") {
        return Some("indented code block");
    }
    let t = line.trim_start_matches(' ');
    if t.is_empty() {
        return None;
    }
    if t.starts_with('>') {
        return Some("block quote");
    }
    if t.starts_with('|') {
        return Some("table");
    }
    if t.starts_with(":::") {
        return Some("container block");
    }
    if t.starts_with("$$") {
        return Some("math block");
    }
    if t.starts_with('<') {
        return Some("raw HTML");
    }
    if t.starts_with("[^") {
        return Some("footnote");
    }
    if t.bytes().all(|b| b == b'=') {
        return Some("setext heading underline");
    }
    None
}

/// Returns `true` when the line would start a block other than a paragraph line.
///
/// The writer uses this to refuse paragraph text that would read back as another block.
pub(crate) fn starts_block(line: &str) -> bool {
    heading(line).is_some()
        || fence_open(line).is_some()
        || thematic_break(line).is_some()
        || image_line(line).is_some()
        || list_item(line).is_some()
        || unsupported(line).is_some()
}
