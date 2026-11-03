// SPDX-License-Identifier: MIT OR Apache-2.0
//! Math front end: parses a LaTeX-style subset into a tree.
//!
//! Layout (boxes, glue, fonts with a MATH table) is not here. It needs `tpt-text`.
//!
//! Supported:
//! - numbers (`12`, `3.14`), single-letter identifiers, operators (`+ - = < > ( ) [ ] | , ! / * : ;`),
//! - groups `{ … }`,
//! - `\frac{num}{den}`, `\sqrt{arg}`,
//! - superscripts `^` and subscripts `_`, with a braced group or one atom as argument,
//! - a fixed list of symbol commands (see [`symbol_glyph`]).
//!
//! Errors carry a 1-based line and column, a message, and a hint.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Deepest allowed nesting of groups, fractions and scripts. Limits recursion on hostile input.
pub const MAX_DEPTH: usize = 64;

/// One node of a parsed formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathNode {
    /// A number, as written (`3.14`).
    Number(String),
    /// A single identifier character.
    Ident(char),
    /// A single operator character.
    Operator(char),
    /// A symbol command such as `\pi`, with its name and glyph.
    Symbol {
        /// The command name without the backslash.
        name: &'static str,
        /// The character it stands for.
        glyph: char,
    },
    /// A braced group `{ … }`.
    Group(Vec<MathNode>),
    /// `\frac{num}{den}`.
    Frac {
        /// Numerator.
        num: Vec<MathNode>,
        /// Denominator.
        den: Vec<MathNode>,
    },
    /// `\sqrt{arg}`.
    Sqrt(Vec<MathNode>),
    /// A base with an optional superscript and subscript.
    Script {
        /// The node being decorated.
        base: Box<MathNode>,
        /// Superscript, if any.
        sup: Option<Vec<MathNode>>,
        /// Subscript, if any.
        sub: Option<Vec<MathNode>>,
    },
}

/// A structured math error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathError {
    /// 1-based line in the source.
    pub line: usize,
    /// 1-based column in the source, counted in characters.
    pub column: usize,
    /// What went wrong.
    pub message: String,
    /// A suggested fix.
    pub hint: &'static str,
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {} (hint: {})",
            self.line, self.column, self.message, self.hint
        )
    }
}

impl core::error::Error for MathError {}

/// The glyph for a supported symbol command, or `None` if the command is not supported.
pub fn symbol_glyph(name: &str) -> Option<char> {
    Some(match name {
        "pm" => '±',
        "mp" => '∓',
        "times" => '×',
        "cdot" => '·',
        "div" => '÷',
        "leq" => '≤',
        "geq" => '≥',
        "neq" => '≠',
        "approx" => '≈',
        "infty" => '∞',
        "alpha" => 'α',
        "beta" => 'β',
        "gamma" => 'γ',
        "delta" => 'δ',
        "theta" => 'θ',
        "lambda" => 'λ',
        "mu" => 'μ',
        "pi" => 'π',
        "sigma" => 'σ',
        "sum" => '∑',
        "prod" => '∏',
        "int" => '∫',
        _ => return None,
    })
}

/// Parses a formula. Whitespace between tokens is ignored.
pub fn parse_math(src: &str) -> Result<Vec<MathNode>, MathError> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = Parser {
        chars: &chars,
        pos: 0,
    };
    let nodes = p.expr(0, None)?;
    if p.pos < chars.len() {
        // The only way to stop early at top level is a stray '}'.
        return Err(p.error_at(p.pos, "unexpected '}'", "remove it or add a matching '{'"));
    }
    Ok(nodes)
}

/// Renders a parsed formula as plain text, for logs and tests. Not a layout.
pub fn to_plain_text(nodes: &[MathNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        write_node(&mut out, node);
    }
    out
}

fn write_node(out: &mut String, node: &MathNode) {
    match node {
        MathNode::Number(n) => out.push_str(n),
        MathNode::Ident(c) | MathNode::Operator(c) => out.push(*c),
        MathNode::Symbol { glyph, .. } => out.push(*glyph),
        MathNode::Group(items) => {
            out.push('(');
            for item in items {
                write_node(out, item);
            }
            out.push(')');
        }
        MathNode::Frac { num, den } => {
            out.push('(');
            for item in num {
                write_node(out, item);
            }
            out.push_str(")/(");
            for item in den {
                write_node(out, item);
            }
            out.push(')');
        }
        MathNode::Sqrt(arg) => {
            out.push('√');
            out.push('(');
            for item in arg {
                write_node(out, item);
            }
            out.push(')');
        }
        MathNode::Script { base, sup, sub } => {
            write_node(out, base);
            if let Some(sup) = sup {
                out.push('^');
                out.push('(');
                for item in sup {
                    write_node(out, item);
                }
                out.push(')');
            }
            if let Some(sub) = sub {
                out.push('_');
                out.push('(');
                for item in sub {
                    write_node(out, item);
                }
                out.push(')');
            }
        }
    }
}

struct Parser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// Parses items until the end of input or, inside a group (`in_group`), a closing brace.
    fn expr(&mut self, depth: usize, open_at: Option<usize>) -> Result<Vec<MathNode>, MathError> {
        if depth > MAX_DEPTH {
            return Err(self.error_at(
                self.pos,
                "formula is nested too deeply",
                "split the formula into smaller parts",
            ));
        }
        let mut nodes = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None => {
                    if let Some(open) = open_at {
                        return Err(self.error_at(
                            open,
                            "unclosed group: no matching '}'",
                            "add a closing '}'",
                        ));
                    }
                    return Ok(nodes);
                }
                Some('}') => {
                    if open_at.is_some() {
                        self.pos += 1;
                        return Ok(nodes);
                    }
                    // Top level: let the caller report it.
                    return Ok(nodes);
                }
                Some('^') | Some('_') => {
                    return Err(self.error_at(
                        self.pos,
                        "script has no base to attach to",
                        "put a symbol or group before '^' or '_'",
                    ));
                }
                Some(_) => {
                    let base = self.atom(depth)?;
                    let node = self.scripts(base, depth)?;
                    nodes.push(node);
                }
            }
        }
    }

    /// Reads `^` and `_` decorations after an atom.
    fn scripts(&mut self, base: MathNode, depth: usize) -> Result<MathNode, MathError> {
        let mut sup = None;
        let mut sub = None;
        loop {
            self.skip_space();
            match self.peek() {
                Some('^') if sup.is_none() => {
                    let at = self.pos;
                    self.pos += 1;
                    sup = Some(self.script_argument(at, depth)?);
                }
                Some('_') if sub.is_none() => {
                    let at = self.pos;
                    self.pos += 1;
                    sub = Some(self.script_argument(at, depth)?);
                }
                Some('^') | Some('_') => {
                    let at = self.pos;
                    let which = if self.peek() == Some('^') {
                        "superscript"
                    } else {
                        "subscript"
                    };
                    return Err(self.error_at(
                        at,
                        &alloc::format!("duplicate {which} on the same base"),
                        "combine them into one group, or use a second base",
                    ));
                }
                _ => break,
            }
        }
        if sup.is_none() && sub.is_none() {
            Ok(base)
        } else {
            Ok(MathNode::Script {
                base: Box::new(base),
                sup,
                sub,
            })
        }
    }

    fn script_argument(&mut self, at: usize, depth: usize) -> Result<Vec<MathNode>, MathError> {
        self.skip_space();
        match self.peek() {
            Some('{') => {
                self.pos += 1;
                let inner = self.expr(depth + 1, Some(self.pos - 1))?;
                Ok(inner)
            }
            None | Some('}') => Err(self.error_at(
                at,
                "script has no argument",
                "add a symbol or a braced group after '^' or '_'",
            )),
            Some(_) => {
                let arg = self.atom(depth + 1)?;
                Ok(alloc::vec![arg])
            }
        }
    }

    /// Reads one atom: number, identifier, operator, group or command.
    fn atom(&mut self, depth: usize) -> Result<MathNode, MathError> {
        let c = match self.peek() {
            Some(c) => c,
            None => {
                return Err(self.error_at(self.pos, "expected a symbol", "add a symbol here"));
            }
        };
        if c == '{' {
            let open = self.pos;
            self.pos += 1;
            let items = self.expr(depth + 1, Some(open))?;
            return Ok(MathNode::Group(items));
        }
        if c == '\\' {
            return self.command(depth);
        }
        if c.is_ascii_digit() {
            let start = self.pos;
            while matches!(self.peek(), Some(d) if d.is_ascii_digit() || d == '.') {
                self.pos += 1;
            }
            let text: String = self.chars[start..self.pos].iter().collect();
            if text.matches('.').count() > 1 {
                return Err(self.error_at(
                    start,
                    "number has more than one decimal point",
                    "check the number",
                ));
            }
            return Ok(MathNode::Number(text));
        }
        if c.is_ascii_alphabetic() {
            self.pos += 1;
            return Ok(MathNode::Ident(c));
        }
        if "+-*/=<>()[]|,!:;".contains(c) {
            self.pos += 1;
            return Ok(MathNode::Operator(c));
        }
        Err(self.error_at(
            self.pos,
            &alloc::format!("unexpected character '{c}'"),
            "remove it, or use a supported command such as \\pm",
        ))
    }

    fn command(&mut self, depth: usize) -> Result<MathNode, MathError> {
        let at = self.pos;
        self.pos += 1; // the backslash
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        match name.as_str() {
            "frac" => {
                let num = self.required_group(at, "\\frac", "numerator", depth)?;
                let den = self.required_group(at, "\\frac", "denominator", depth)?;
                Ok(MathNode::Frac { num, den })
            }
            "sqrt" => {
                let arg = self.required_group(at, "\\sqrt", "argument", depth)?;
                Ok(MathNode::Sqrt(arg))
            }
            "" => Err(self.error_at(
                at,
                "backslash is not followed by a command name",
                "write a command such as \\frac or \\pi",
            )),
            other => match symbol_glyph(other) {
                Some(glyph) => Ok(MathNode::Symbol {
                    name: symbol_name(other),
                    glyph,
                }),
                None => Err(self.error_at(
                    at,
                    &alloc::format!("unknown command \\{other}"),
                    "supported: \\frac \\sqrt and the symbols pm, times, leq, pi, alpha, infty, sum and others",
                )),
            },
        }
    }

    /// Reads a `{…}` argument that a command requires.
    fn required_group(
        &mut self,
        command_at: usize,
        command: &str,
        what: &str,
        depth: usize,
    ) -> Result<Vec<MathNode>, MathError> {
        self.skip_space();
        if self.peek() != Some('{') {
            return Err(self.error_at(
                command_at,
                &alloc::format!("{command} is missing its {what}"),
                "write the argument in braces, such as {x}",
            ));
        }
        let open = self.pos;
        self.pos += 1;
        self.expr(depth + 1, Some(open))
    }

    /// Converts a character index to a 1-based line and column, and builds the error.
    fn error_at(&self, at: usize, message: &str, hint: &'static str) -> MathError {
        let at = at.min(self.chars.len());
        let mut line = 1;
        let mut column = 1;
        for &c in &self.chars[..at] {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        MathError {
            line,
            column,
            message: String::from(message),
            hint,
        }
    }
}

/// The static command name for a supported symbol, so nodes can hold `&'static str`.
fn symbol_name(name: &str) -> &'static str {
    const NAMES: &[&str] = &[
        "pm", "mp", "times", "cdot", "div", "leq", "geq", "neq", "approx", "infty", "alpha",
        "beta", "gamma", "delta", "theta", "lambda", "mu", "pi", "sigma", "sum", "prod", "int",
    ];
    NAMES.iter().copied().find(|n| *n == name).unwrap_or("")
}
