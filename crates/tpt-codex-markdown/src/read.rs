// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;

use tpt_codex_core::{Block, BlockId, Document, LayoutMode, Metadata, TextStyle};

use crate::error::ParseError;
use crate::syntax::{
    ListMarker, MathOpen, fence_closes, fence_open, heading, image_line, list_item, math_closes,
    math_open, thematic_break, unsupported,
};

/// Reads the supported Markdown subset into a [`Document`].
///
/// The result always passes [`Document::validate`]. Errors carry a 1-based
/// line and column.
pub fn from_markdown(src: &str) -> Result<Document, ParseError> {
    let lines: Vec<&str> = src.lines().collect();
    let mut metadata = Metadata::default();
    let body_start = read_front_matter(&lines, &mut metadata)?;

    let mut b = Builder::default();
    let mut open = Open::default();

    for (offset, line) in lines[body_start..].iter().enumerate() {
        let lineno = body_start + offset + 1;

        if let Some(source_lines) = open.math.as_mut() {
            if math_closes(line) {
                let source = source_lines.join(
                    "
",
                );
                open.math = None;
                if source.is_empty() {
                    return Err(error(
                        lineno,
                        "math block is empty",
                        "put the math source between the two $$ lines",
                    ));
                }
                b.push_root(Block::Math { source });
            } else {
                // Math source is kept exactly, including leading spaces.
                source_lines.push(String::from(*line));
            }
            continue;
        }

        if let Some(code) = open.code.as_mut() {
            if fence_closes(line, code.ch, code.len) {
                if let Some(code) = open.code.take() {
                    b.push_root(Block::Code {
                        language: code.language,
                        code: code.lines.join("\n"),
                    });
                }
            } else {
                // Content loses up to as many leading spaces as the opening fence had.
                let n = line
                    .bytes()
                    .take(code.indent)
                    .take_while(|&c| c == b' ')
                    .count();
                code.lines.push(String::from(&line[n..]));
            }
            continue;
        }

        if line.trim().is_empty() {
            open.close_paragraph(&mut b);
            open.after_image = false;
            if open.list.is_some() {
                open.list_blank = true;
            }
            continue;
        }
        let after_image = core::mem::take(&mut open.after_image);

        if let Some(item) = list_item(line) {
            if item.text.is_empty() {
                return Err(error(
                    lineno,
                    "list item is empty",
                    "write the item's text after the marker",
                ));
            }
            if let Some(list) = open.list.as_mut() {
                if list.key == item.key() {
                    if open.list_blank {
                        return Err(error(
                            lineno,
                            "loose lists (blank lines between items) are not supported yet",
                            "remove the blank lines between the items",
                        ));
                    }
                    list.items.push(b.alloc(text_block(item.text)));
                    continue;
                }
            }
            if open.para.is_some() && item.ordered && item.number != 1 {
                return Err(error(
                    lineno,
                    "a numbered list that does not start at 1 cannot follow a paragraph",
                    "start the list at 1, or put a blank line before it",
                ));
            }
            open.close_paragraph(&mut b);
            open.close_list(&mut b);
            open.list = Some(OpenList::start(&mut b, item));
            open.list_blank = false;
            continue;
        }

        if let Some(fence) = fence_open(line) {
            if fence.info.contains(['\\', '&']) {
                return Err(error(
                    lineno,
                    "code block info string cannot contain backslashes or entities",
                    "use a plain language name such as rust",
                ));
            }
            open.close_paragraph(&mut b);
            open.close_list(&mut b);
            open.code = Some(OpenCode {
                ch: fence.ch,
                len: fence.len,
                indent: fence.indent,
                language: (!fence.info.is_empty()).then(|| String::from(fence.info)),
                lines: Vec::new(),
            });
            continue;
        }

        if let Some(opener) = math_open(line) {
            let opener = opener.map_err(|reason| {
                error(
                    lineno,
                    reason,
                    "write the math as $$, then the source lines, then $$",
                )
            })?;
            open.close_paragraph(&mut b);
            open.close_list(&mut b);
            match opener {
                MathOpen::Single(source) => b.push_root(Block::Math {
                    source: String::from(source),
                }),
                MathOpen::Block => open.math = Some(Vec::new()),
            }
            continue;
        }

        if let Some(ch) = thematic_break(line) {
            if ch == b'-' && open.para.is_some() {
                return Err(error(
                    lineno,
                    "a line of dashes under text is a setext heading, which is not supported yet",
                    "put a blank line between the text and the dashes, or use *** for a rule",
                ));
            }
            open.close_paragraph(&mut b);
            open.close_list(&mut b);
            b.push_root(Block::ThematicBreak);
            continue;
        }

        if let Some((level, text)) = heading(line) {
            open.close_paragraph(&mut b);
            open.close_list(&mut b);
            if text.is_empty() {
                return Err(error(
                    lineno,
                    "heading has no text",
                    "add text after the hashes, or write a paragraph instead",
                ));
            }
            b.push_root(Block::Text {
                content: String::from(text),
                style: TextStyle {
                    bold: false,
                    italic: false,
                    heading_level: Some(level),
                },
            });
            continue;
        }

        if let Some(image) = image_line(line) {
            if open.para.is_some() || (open.list.is_some() && !open.list_blank) {
                return Err(error(
                    lineno,
                    "an image directly after text is not supported yet",
                    "put a blank line before the image",
                ));
            }
            open.close_list(&mut b);
            let (alt, uri) = image
                .map_err(|reason| error(lineno, reason, "use ![alt text](path) with no title"))?;
            b.push_root(Block::Media {
                source: tpt_codex_core::MediaSource::Uri {
                    uri: String::from(uri),
                },
                alt_text: (!alt.is_empty()).then(|| String::from(alt)),
            });
            open.after_image = true;
            continue;
        }

        if let Some(reason) = unsupported(line) {
            return Err(error(
                lineno,
                &alloc::format!("{reason} is not supported yet"),
                "rewrite as plain paragraphs, headings, lists and code blocks",
            ));
        }

        if after_image {
            return Err(error(
                lineno,
                "text directly after an image is not supported yet",
                "put a blank line between the image and the text",
            ));
        }
        if open.list.is_some() {
            if !open.list_blank {
                return Err(error(
                    lineno,
                    "continuation lines inside a list item are not supported yet",
                    "put each list item on one line",
                ));
            }
            open.close_list(&mut b);
        }
        match &mut open.para {
            Some(text) => {
                text.push('\n');
                text.push_str(line);
            }
            None => open.para = Some(String::from(*line)),
        }
    }

    if open.code.is_some() {
        return Err(error(
            lines.len(),
            "code block is not closed",
            "close the block with a fence of the same character and at least the same length",
        ));
    }
    if open.math.is_some() {
        return Err(error(
            lines.len(),
            "math block is not closed",
            "close the block with a line containing only $$",
        ));
    }
    open.close_paragraph(&mut b);
    open.close_list(&mut b);

    let mut doc = Document::new(
        BlockId(0),
        Block::Container {
            layout_mode: LayoutMode::Flow,
            children: b.root,
        },
    );
    doc.metadata = metadata;
    for (i, block) in b.blocks.into_iter().enumerate() {
        doc.insert(BlockId(i as u64 + 1), block);
    }
    Ok(doc)
}

fn error(line: usize, message: &str, suggestion: &'static str) -> ParseError {
    ParseError {
        line,
        column: 1,
        message: String::from(message),
        suggestion: Some(suggestion),
    }
}

fn text_block(content: &str) -> Block {
    Block::Text {
        content: String::from(content),
        style: TextStyle::default(),
    }
}

/// Block ids are allocated in the order blocks are created. The root is `BlockId(0)`.
#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    root: Vec<BlockId>,
}

impl Builder {
    fn alloc(&mut self, block: Block) -> BlockId {
        self.blocks.push(block);
        BlockId(self.blocks.len() as u64)
    }

    fn push_root(&mut self, block: Block) {
        let id = self.alloc(block);
        self.root.push(id);
    }
}

/// The block that is still open while reading lines.
#[derive(Default)]
struct Open {
    para: Option<String>,
    list: Option<OpenList>,
    /// A blank line has been seen since the list's last item.
    list_blank: bool,
    code: Option<OpenCode>,
    /// Source lines of a multi-line math block that is still open.
    math: Option<Vec<String>>,
    /// The previous line was an image. Text on the next line would join it in CommonMark.
    after_image: bool,
}

impl Open {
    fn close_paragraph(&mut self, b: &mut Builder) {
        if let Some(content) = self.para.take() {
            b.push_root(text_block(&content));
        }
    }

    fn close_list(&mut self, b: &mut Builder) {
        if let Some(list) = self.list.take() {
            b.push_root(Block::List {
                ordered: list.key.0,
                start: list.start,
                items: list.items,
            });
        }
        self.list_blank = false;
    }
}

struct OpenList {
    key: (bool, u8),
    start: u32,
    items: Vec<BlockId>,
}

impl OpenList {
    fn start(b: &mut Builder, item: ListMarker<'_>) -> Self {
        let first = b.alloc(text_block(item.text));
        OpenList {
            key: item.key(),
            start: item.number,
            items: alloc::vec![first],
        }
    }
}

/// A fenced code block whose closing fence has not been read yet.
struct OpenCode {
    ch: u8,
    len: usize,
    indent: usize,
    language: Option<String>,
    lines: Vec<String>,
}

/// Reads an optional front-matter block. Returns the index of the first body line.
fn read_front_matter(lines: &[&str], metadata: &mut Metadata) -> Result<usize, ParseError> {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Ok(0);
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        let lineno = i + 1;
        if line.trim_end() == "---" {
            return Ok(i + 1);
        }
        if line.trim().is_empty() {
            continue;
        }
        let (key, raw) = line.split_once(':').ok_or_else(|| ParseError {
            line: lineno,
            column: 1,
            message: String::from("front matter line is not 'key: value'"),
            suggestion: Some("write each entry as key: value"),
        })?;
        let value = parse_value(raw.trim()).map_err(|message| ParseError {
            line: lineno,
            column: raw.len() - raw.trim_start().len() + key.len() + 2,
            message,
            suggestion: Some("wrap the value in double quotes; escape \\\" and \\\\"),
        })?;
        let slot = match key.trim() {
            "title" => &mut metadata.title,
            "author" => &mut metadata.author,
            "date" => &mut metadata.date,
            "class" => &mut metadata.class,
            other => {
                return Err(ParseError {
                    line: lineno,
                    column: 1,
                    message: alloc::format!("unknown front matter key '{other}'"),
                    suggestion: Some("supported keys: title, author, date, class"),
                });
            }
        };
        *slot = Some(value);
    }
    Err(ParseError {
        line: lines.len(),
        column: 1,
        message: String::from("front matter is not closed"),
        suggestion: Some("add a '---' line after the last front matter entry"),
    })
}

/// Parses a bare or double-quoted value. Quoted values accept `\"` and `\\`.
fn parse_value(raw: &str) -> Result<String, String> {
    if !raw.starts_with('"') {
        return Ok(String::from(raw));
    }
    let inner = raw[1..]
        .strip_suffix('"')
        .ok_or_else(|| String::from("quoted value is missing its closing quote"))?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(e @ ('"' | '\\')) => out.push(e),
                _ => return Err(String::from("invalid escape in quoted value")),
            }
        } else if c == '"' {
            return Err(String::from("unescaped quote inside quoted value"));
        } else {
            out.push(c);
        }
    }
    Ok(out)
}
