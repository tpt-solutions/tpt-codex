// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reads BibTeX entries into [`Reference`]s, for [`Library::from_bibtex`](crate::Library::from_bibtex).
//!
//! Read: entries `@type{key, field = value, ...}` with values in braces, in quotes, as numbers, or
//! as macros (standard month names and `@string` definitions), joined with `#`. `@comment`,
//! `@preamble` and `@string` are handled. Only the fields used for author-year citations are kept:
//! `author`, `title`, and `year` (or the year of `date`).
//!
//! Not read: the name particles beyond the common `von` rule, and TeX accents. A backslash is
//! dropped, so `{\"O}zt\"urk` reads as `Oztrk`. Such values need to be written as UTF-8.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::citations::{CslError, Name, Reference};

/// One entry, with the line and column where it starts, in source order.
pub(crate) type Parsed = Vec<((usize, usize), Reference)>;

/// Reads every entry of a BibTeX source.
pub(crate) fn parse(src: &str) -> Result<Parsed, CslError> {
    let mut s = Scanner::new(src);
    let mut strings = month_macros();
    let mut entries = Vec::new();
    loop {
        while s.peek().is_some_and(|c| c != '@') {
            s.bump();
        }
        if s.peek().is_none() {
            break;
        }
        let start = (s.line, s.column);
        s.bump();
        s.skip_ws();
        let kind = s.word(|c| c.is_ascii_alphabetic()).to_ascii_lowercase();
        s.skip_ws();
        let (open, close) = match s.bump() {
            Some('{') => ('{', '}'),
            Some('(') => ('(', ')'),
            _ => {
                return Err(s.error(
                    "expected { or ( after the entry type",
                    "write an entry as @article{key, title = {...}}",
                ));
            }
        };
        match kind.as_str() {
            "comment" => s.skip_group(open, close)?,
            "preamble" => {
                parse_value(&mut s, &strings)?;
                s.expect_close(close)?;
            }
            "string" => {
                s.skip_ws();
                let name = s.word(is_name_char).to_ascii_lowercase();
                s.skip_ws();
                s.expect('=', "expected = after the macro name")?;
                let value = parse_value(&mut s, &strings)?;
                if name.is_empty() {
                    return Err(s.error("a @string has no name", "write @string{name = {...}}"));
                }
                strings.insert(name, value);
                s.expect_close(close)?;
            }
            _ => {
                let entry = parse_entry(&mut s, &strings, close)?;
                entries.push((start, entry));
            }
        }
    }
    Ok(entries)
}

/// Reads the body of an ordinary entry, after its opening brace: the key, then the fields.
fn parse_entry(
    s: &mut Scanner,
    strings: &BTreeMap<String, String>,
    close: char,
) -> Result<Reference, CslError> {
    s.skip_ws();
    let key = s.word(|c| !c.is_whitespace() && !matches!(c, ',' | '{' | '}' | '(' | ')' | '='));
    if key.is_empty() {
        return Err(s.error(
            "an entry has no key",
            "put a key after the brace, such as @article{smith2020, ...}",
        ));
    }
    s.skip_ws();
    if !matches!(s.peek(), Some(c) if c == ',' || c == close) {
        return Err(s.error(
            "expected a comma after the key",
            "separate the key from the fields with a comma",
        ));
    }
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    loop {
        s.skip_ws();
        match s.peek() {
            None => {
                return Err(s.error(
                    "the entry is not closed",
                    "add the closing brace after its last field",
                ));
            }
            Some(c) if c == close => {
                s.bump();
                break;
            }
            Some(',') => {
                s.bump();
            }
            Some(_) => {
                let name = s.word(is_name_char).to_ascii_lowercase();
                if name.is_empty() {
                    return Err(s.error(
                        "expected a field name",
                        "write fields as name = {value}, separated by commas",
                    ));
                }
                s.skip_ws();
                s.expect('=', "expected = after the field name")?;
                let value = parse_value(s, strings)?;
                fields.insert(name, value);
            }
        }
    }
    Ok(reference_from(key, &fields))
}

/// Reads a value: pieces joined with `#`, each braced, quoted, a number, or a macro.
fn parse_value(s: &mut Scanner, strings: &BTreeMap<String, String>) -> Result<String, CslError> {
    let mut out = String::new();
    loop {
        s.skip_ws();
        match s.peek() {
            Some('{') => {
                s.bump();
                out.push_str(&s.braced()?);
            }
            Some('"') => {
                s.bump();
                out.push_str(&s.quoted()?);
            }
            Some(c) if c.is_ascii_digit() => out.push_str(&s.word(|c| c.is_ascii_digit())),
            Some(c) if is_name_char(c) => {
                let name = s.word(is_name_char).to_ascii_lowercase();
                match strings.get(&name) {
                    Some(value) => out.push_str(value),
                    None => {
                        return Err(s.error(
                            "unknown macro",
                            "define it with @string{name = {...}}, or write the value in braces",
                        ));
                    }
                }
            }
            _ => {
                return Err(s.error(
                    "expected a value",
                    "wrap the value in braces, such as title = {Field notes}",
                ));
            }
        }
        s.skip_ws();
        if s.peek() == Some('#') {
            s.bump();
        } else {
            return Ok(out);
        }
    }
}

/// The standard month macros, which BibTeX defines before any file is read.
fn month_macros() -> BTreeMap<String, String> {
    const MONTHS: [(&str, &str); 12] = [
        ("jan", "January"),
        ("feb", "February"),
        ("mar", "March"),
        ("apr", "April"),
        ("may", "May"),
        ("jun", "June"),
        ("jul", "July"),
        ("aug", "August"),
        ("sep", "September"),
        ("oct", "October"),
        ("nov", "November"),
        ("dec", "December"),
    ];
    MONTHS
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.' | '/' | '+')
}

/// Builds the reference from an entry's fields. Missing fields are empty or `None`.
fn reference_from(id: String, fields: &BTreeMap<String, String>) -> Reference {
    let title = fields.get("title").map(|t| clean(t)).unwrap_or_default();
    let authors = fields.get("author").map(|a| names(a)).unwrap_or_default();
    let year = fields
        .get("year")
        .or_else(|| fields.get("date"))
        .and_then(|y| {
            let digits: String = clean(y)
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            digits.parse::<i64>().ok()
        });
    Reference {
        id,
        title,
        authors,
        year,
    }
}

/// Splits an `author` value into names: separated by a top-level `and`.
fn names(value: &str) -> Vec<Name> {
    let mut groups: Vec<Vec<String>> = alloc::vec![Vec::new()];
    for word in top_level_words(value) {
        if word.eq_ignore_ascii_case("and") {
            groups.push(Vec::new());
        } else if let Some(group) = groups.last_mut() {
            group.push(word);
        }
    }
    groups
        .into_iter()
        .filter(|words| !words.is_empty())
        .filter_map(|words| name(&words))
        .collect()
}

/// One name, as `Last, First` or `First Last`, or a corporate name in braces.
fn name(words: &[String]) -> Option<Name> {
    let joined = words.join(" ");
    let (family, given) = if let Some(comma) = top_level_find(&joined, ',') {
        (clean(&joined[..comma]), clean(&joined[comma + 1..]))
    } else if words.len() == 1 && is_wrapped(&words[0]) {
        (clean(&words[0]), String::new())
    } else {
        // The family name starts at the first lowercase word after the given names (`van`, `de`),
        // or is the last word if there is none.
        let start = words
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, w)| clean(w).chars().next().is_some_and(|c| c.is_lowercase()))
            .map(|(i, _)| i)
            .unwrap_or(words.len() - 1);
        (
            clean(&words[start..].join(" ")),
            clean(&words[..start].join(" ")),
        )
    };
    if family.is_empty() && given.is_empty() {
        None
    } else {
        Some(Name { family, given })
    }
}

/// Whether the whole word is one braced group, as corporate names are written.
fn is_wrapped(word: &str) -> bool {
    word.starts_with('{') && word.ends_with('}') && balanced(&word[1..word.len() - 1])
}

fn balanced(s: &str) -> bool {
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    depth == 0
}

/// The whitespace-separated words of `s` that are not inside braces.
fn top_level_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '{' => {
                depth += 1;
                current.push(c);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(c);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    words.push(core::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// The byte index of the first `needle` that is not inside braces.
fn top_level_find(s: &str, needle: char) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            c if c == needle && depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Removes braces and TeX backslashes, and collapses whitespace.
fn clean(s: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c != '{' && c != '}' {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A character reader that keeps the line and column of its position for errors.
struct Scanner {
    chars: Vec<char>,
    at: usize,
    line: usize,
    column: usize,
}

impl Scanner {
    fn new(src: &str) -> Self {
        Self {
            chars: src.chars().collect(),
            at: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn error(&self, message: &'static str, hint: &'static str) -> CslError {
        CslError {
            line: self.line,
            column: self.column,
            message,
            hint,
        }
    }

    fn expect(&mut self, want: char, message: &'static str) -> Result<(), CslError> {
        if self.peek() == Some(want) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(message, "check the = and the value around it"))
        }
    }

    fn expect_close(&mut self, close: char) -> Result<(), CslError> {
        self.skip_ws();
        if self.peek() == Some(',') {
            self.bump();
            self.skip_ws();
        }
        if self.peek() == Some(close) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(
                "expected the closing brace of the entry",
                "check for a missing value or a stray character",
            ))
        }
    }

    fn word(&mut self, keep: impl Fn(char) -> bool) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek().filter(|c| keep(*c)) {
            out.push(c);
            self.bump();
        }
        out
    }

    /// Reads a braced value's text, after its opening brace. Inner braces are kept.
    fn braced(&mut self) -> Result<String, CslError> {
        let mut depth = 1usize;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(self.error(
                        "a braced value is not closed",
                        "add the closing brace after the value",
                    ));
                }
                Some('{') => {
                    depth += 1;
                    out.push('{');
                }
                Some('}') => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push('}');
                }
                Some(c) => out.push(c),
            }
        }
    }

    /// Reads a quoted value's text, after its opening quote. A quote inside braces does not end it.
    fn quoted(&mut self) -> Result<String, CslError> {
        let mut depth = 0usize;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(self.error(
                        "a quoted value is not closed",
                        "add the closing quote after the value",
                    ));
                }
                Some('{') => {
                    depth += 1;
                    out.push('{');
                }
                Some('}') => {
                    depth = depth.saturating_sub(1);
                    out.push('}');
                }
                Some('"') if depth == 0 => return Ok(out),
                Some(c) => out.push(c),
            }
        }
    }

    /// Skips a group that holds nested `open` and `close` characters, after its opening one.
    fn skip_group(&mut self, open: char, close: char) -> Result<(), CslError> {
        let mut depth = 1usize;
        loop {
            match self.bump() {
                None => {
                    return Err(self.error(
                        "the comment or preamble is not closed",
                        "add the closing brace after it",
                    ));
                }
                Some(c) if c == open => depth += 1,
                Some(c) if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some(_) => {}
            }
        }
    }
}
