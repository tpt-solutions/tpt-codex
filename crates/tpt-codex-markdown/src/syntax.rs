// SPDX-License-Identifier: MIT OR Apache-2.0
//! Line-level classification shared by the reader and the writer.
//!
//! Both directions use the same predicates, so anything the writer emits is
//! guaranteed to read back as the same block.

use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Canvas, Figure, is_figure_label};

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

/// How a line that starts with `$$` opens a math block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MathOpen<'a> {
    /// `$$` alone: the source follows on the next lines, up to a closing `$$` line.
    Block,
    /// `$$ source $$` on one line. The source is trimmed.
    Single(&'a str),
}

/// Returns how a `$$` line opens a math block.
///
/// `Some(Err(reason))` is a line that starts with `$$` but is not a form this crate reads.
/// `None` is any other line.
pub(crate) fn math_open(line: &str) -> Option<Result<MathOpen<'_>, &'static str>> {
    let body = strip_block_indent(line)?.trim_end();
    let rest = body.strip_prefix("$$")?;
    if rest.trim().is_empty() {
        return Some(Ok(MathOpen::Block));
    }
    if let Some(inner) = rest.strip_suffix("$$") {
        if !inner.contains("$$") && !inner.trim().is_empty() {
            return Some(Ok(MathOpen::Single(inner.trim())));
        }
    }
    Some(Err(
        "math must be written as $$ on its own line, or as $$ source $$ on one line",
    ))
}

/// Returns `true` when `line` closes a multi-line math block: `$$` alone, with up to three
/// leading spaces. Other lines inside the block are source, kept exactly.
pub(crate) fn math_closes(line: &str) -> bool {
    strip_block_indent(line).is_some_and(|body| body.trim_end() == "$$")
}

/// A standalone image line: `![alt](uri)`, optionally followed by figure attributes such as
/// `{#fig:overview width=80%}`.
///
/// Returns `Some(Ok((alt, uri, attributes)))` for a supported image, where `attributes` is the
/// text between the braces (empty when there are none). `Some(Err(reason))` is a line that
/// starts like an image but uses syntax this crate does not read, and `None` is any other line.
pub(crate) fn image_line(line: &str) -> Option<Result<(&str, &str, &str), &'static str>> {
    let body = strip_block_indent(line)?.trim_end();
    if !body.starts_with("![") {
        return None;
    }
    Some(parse_image(body))
}

fn parse_image(body: &str) -> Result<(&str, &str, &str), &'static str> {
    let (alt, after_alt) = body[2..]
        .split_once("](")
        .ok_or("image must be written as ![alt](uri)")?;
    let end = after_alt
        .find(')')
        .ok_or("image must end with ')' on the same line")?;
    let uri = &after_alt[..end];
    if alt.contains(['[', ']', '\\', '&']) {
        return Err("image alt text cannot contain brackets, backslashes or entities");
    }
    if uri.is_empty() {
        return Err("image has no uri");
    }
    if uri.contains(|c: char| c.is_whitespace() || matches!(c, '(' | '<' | '>' | '\\' | '&')) {
        return Err("image titles, angle brackets and escapes are not supported");
    }
    let tail = after_alt[end + 1..].trim_start();
    let attributes = if tail.is_empty() {
        ""
    } else {
        tail.strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .ok_or("figure attributes must be written as {#label width=80%} after the image")?
    };
    Ok((alt, uri, attributes))
}

/// Reads the attributes of a figure: the text between the braces of `{#fig:id width=80%}`.
///
/// Tokens are separated by spaces. `#label` names the figure for cross-references, and
/// `width=N%` sets its width from 1% to 100%. Anything else is an error.
pub(crate) fn figure_attributes(inner: &str) -> Result<Figure, &'static str> {
    let mut figure = Figure::default();
    for token in inner.split_whitespace() {
        if let Some(label) = token.strip_prefix('#') {
            if !is_figure_label(label) {
                return Err("figure label must be letters, digits, and - _ : .");
            }
            if figure.id.replace(String::from(label)).is_some() {
                return Err("a figure can have only one #label");
            }
        } else if let Some(width) = token.strip_prefix("width=") {
            let percent = width
                .strip_suffix('%')
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| (1..=100).contains(n))
                .ok_or("width must be a whole percentage from 1% to 100%, such as width=80%")?;
            if figure.width_percent.replace(percent).is_some() {
                return Err("a figure can have only one width");
            }
        } else {
            return Err("unknown figure attribute; supported attributes are #label and width=N%");
        }
    }
    Ok(figure)
}

/// The container a `:::` line opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContainerOpen<'a> {
    /// `::: grid { id="N" }`: a live grid whose body is a pipe table.
    Grid {
        /// The sheet id the grid displays.
        id: u64,
    },
    /// `::: canvas { ratio="W:H" }`: a slide, whose body holds ordinary blocks.
    Canvas {
        /// Width and height of the ratio, each from 1 to 32.
        ratio: (u32, u32),
    },
    /// A container name this crate does not know.
    Unknown(&'a str),
}

/// Returns how a `:::` line opens a container, or `None` if it is not an opening line.
///
/// A closing `:::` line is not an opening line. Attribute errors are returned as the reason.
pub(crate) fn container_open(line: &str) -> Option<Result<ContainerOpen<'_>, &'static str>> {
    let body = strip_block_indent(line)?.trim_end();
    let rest = body.strip_prefix(":::")?.trim_start();
    if rest.is_empty() {
        return None;
    }
    let name_len = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (name, attrs) = rest.split_at(name_len);
    let attrs = attrs.trim();
    let attrs = if attrs.is_empty() {
        Vec::new()
    } else {
        match attrs.strip_prefix('{').and_then(|a| a.strip_suffix('}')) {
            Some(inner) => match key_values(inner) {
                Ok(pairs) => pairs,
                Err(reason) => return Some(Err(reason)),
            },
            None => {
                return Some(Err(
                    "container attributes must be written as { key=\"value\" }",
                ));
            }
        }
    };
    Some(match name {
        "grid" => grid_id(&attrs).map(|id| ContainerOpen::Grid { id }),
        "canvas" => canvas_ratio(&attrs).map(|ratio| ContainerOpen::Canvas { ratio }),
        _ => Ok(ContainerOpen::Unknown(name)),
    })
}

/// The size of a slide with the given ratio, in points: 960 wide, as wide as 16:9.
///
/// The writer finds the ratio again by comparing sizes, so this function must not change.
pub(crate) fn canvas_size((width, height): (u32, u32)) -> Canvas {
    Canvas {
        width: 960.0,
        height: 960.0 * f64::from(height) / f64::from(width),
    }
}

/// Returns `true` when the line closes a container: `:::` alone, with up to three spaces.
pub(crate) fn container_closes(line: &str) -> bool {
    strip_block_indent(line).is_some_and(|body| body.trim_end() == ":::")
}

/// Reads `key="value"` pairs. Values cannot contain quotes.
fn key_values(inner: &str) -> Result<Vec<(&str, &str)>, &'static str> {
    const FORM: &str = "attributes must be written as key=\"value\"";
    let mut pairs = Vec::new();
    let mut rest = inner.trim_start();
    while !rest.is_empty() {
        let key_len = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        let (key, after_key) = rest.split_at(key_len);
        if key.is_empty() {
            return Err(FORM);
        }
        let value_and_rest = after_key.strip_prefix("=\"").ok_or(FORM)?;
        let (value, after_value) = value_and_rest.split_once('"').ok_or(FORM)?;
        pairs.push((key, value));
        rest = after_value.trim_start();
    }
    Ok(pairs)
}

fn grid_id(attrs: &[(&str, &str)]) -> Result<u64, &'static str> {
    let mut id = None;
    for &(key, value) in attrs {
        match key {
            "id" => {
                let parsed = value
                    .parse::<u64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("grid id must be a positive whole number, such as id=\"1\"")?;
                if id.replace(parsed).is_some() {
                    return Err("grid has more than one id");
                }
            }
            _ => return Err("unknown grid attribute; the only attribute is id"),
        }
    }
    id.ok_or("grid needs an id, such as ::: grid { id=\"1\" }")
}

fn canvas_ratio(attrs: &[(&str, &str)]) -> Result<(u32, u32), &'static str> {
    let mut ratio = None;
    for &(key, value) in attrs {
        match key {
            "ratio" => {
                let parsed = value
                    .split_once(':')
                    .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
                    .filter(|(w, h)| (1..=32).contains(w) && (1..=32).contains(h))
                    .ok_or("canvas ratio must be two whole numbers from 1 to 32, such as ratio=\"16:9\"")?;
                if ratio.replace(parsed).is_some() {
                    return Err("canvas has more than one ratio");
                }
            }
            _ => return Err("unknown canvas attribute; the only attribute is ratio"),
        }
    }
    ratio.ok_or("canvas needs a ratio, such as ::: canvas { ratio=\"16:9\" }")
}

/// Reads one row of a grid's pipe table, `| a | b |`, into its trimmed cells.
///
/// `\|` is a pipe inside a cell and `\\` is a backslash. Returns `None` for a line that does
/// not start with `|`.
pub(crate) fn table_row(line: &str) -> Option<Result<Vec<String>, &'static str>> {
    let body = line.trim();
    if !body.starts_with('|') {
        return None;
    }
    if !body.ends_with('|') || body.len() < 2 {
        return Some(Err("table rows must start and end with |"));
    }
    let inner = &body[1..body.len() - 1];
    let mut cells: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.clone().next() {
                Some(next @ ('|' | '\\')) => {
                    cell.push(next);
                    chars.next();
                }
                _ => cell.push(c),
            },
            '|' => cells.push(core::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    cells.push(cell);
    Some(Ok(cells.iter().map(|c| String::from(c.trim())).collect()))
}

/// Returns `true` for a separator row's cells: each one is three or more dashes.
pub(crate) fn is_separator(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|c| c.len() >= 3 && c.bytes().all(|b| b == b'-'))
}

/// Returns a reason if the line starts syntax this crate does not read.
///
/// Unsupported syntax is an error rather than being read as plain text, so
/// nothing is silently dropped or reinterpreted. Fences, lists, thematic
/// breaks, headings, images and math are handled by the reader before this check.
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
    if t.starts_with('<') {
        return Some("raw HTML");
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
        || math_open(line).is_some()
        || list_item(line).is_some()
        || unsupported(line).is_some()
}
