// SPDX-License-Identifier: MIT OR Apache-2.0
//! The content stream of a page, read for its text.
//!
//! The graphics and text state needed to place text is followed: the current transformation
//! matrix (`cm`, `q`, `Q`), the text matrices (`BT`, `Td`, `Tm`, `T*`), the font and size, and the
//! spacing operators. Form XObjects are read in place. Paths and images are not drawn; they are
//! counted, so the import can say what it left out.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::PdfError;
use super::font::Font;
use super::objects::{Dict, Document, Lexer, Obj};

/// A matrix `[a b c d e f]`, applied to row vectors as PDF does.
type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/// How deeply forms may nest. A form that calls itself stops here.
const MAX_FORM_DEPTH: usize = 16;
/// The most glyphs read from one page, so a hostile page cannot exhaust memory.
const MAX_GLYPHS: usize = 2_000_000;

/// `a` then `b`: the product of two matrices, for row vectors.
fn mul(a: Matrix, b: Matrix) -> Matrix {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn translate(x: f64, y: f64) -> Matrix {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

/// The average scale of a matrix: the square root of its determinant's magnitude.
fn scale(m: Matrix) -> f64 {
    (m[0] * m[3] - m[1] * m[2]).abs().sqrt()
}

/// A glyph read from a page, where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The characters the glyph stands for. Empty when the font gave none.
    pub text: String,
    /// The origin, in points from the left of the page.
    pub x: f64,
    /// The baseline, in points from the bottom of the page.
    pub y: f64,
    /// The font size on the page, in points.
    pub size: f64,
    /// The horizontal advance on the page, in points.
    pub width: f64,
    /// Whether the characters are a guess, from a font with no character map.
    pub guessed: bool,
    /// The font is bold or italic. The import reads the text as plain, and says so.
    pub emphasis: bool,
}

/// What a page's content stream holds, for the import.
#[derive(Debug, Default)]
pub struct PageContent {
    /// The glyphs, in drawing order.
    pub glyphs: Vec<Glyph>,
    /// Images drawn (inline or XObject).
    pub images: usize,
    /// Paths painted (lines, boxes, filled shapes) and shadings.
    pub drawings: usize,
    /// Codes shown with no character map at all; they are left out of the text.
    pub unmapped: usize,
}

/// Reads the text of a page: its content streams, with the resources they use.
pub fn read_page(
    doc: &Document<'_>,
    content: &[u8],
    resources: &Dict,
) -> Result<PageContent, PdfError> {
    let mut reader = Reader {
        doc,
        fonts: BTreeMap::new(),
        out: PageContent::default(),
        ctm: IDENTITY,
        stack: Vec::new(),
        text: TextState::default(),
        path_curves: false,
        path_rects: 0,
    };
    reader.run(content, resources, 0)?;
    Ok(reader.out)
}

/// The text state: set by `BT` and the text operators.
#[derive(Clone)]
struct TextState {
    font: Option<Rc<Font>>,
    size: f64,
    /// Character spacing, in unscaled text units.
    char_spacing: f64,
    /// Word spacing, in unscaled text units.
    word_spacing: f64,
    /// Horizontal scaling, as a fraction.
    horizontal: f64,
    /// Leading, for `T*`.
    leading: f64,
    /// Rise, for super- and subscripts.
    rise: f64,
    /// The text matrix, and the line matrix it starts from.
    tm: Matrix,
    tlm: Matrix,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            font: None,
            size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal: 1.0,
            leading: 0.0,
            rise: 0.0,
            tm: IDENTITY,
            tlm: IDENTITY,
        }
    }
}

struct Reader<'d, 'a> {
    doc: &'d Document<'a>,
    /// Fonts by the number of their object, read once.
    fonts: BTreeMap<u32, Rc<Font>>,
    out: PageContent,
    ctm: Matrix,
    stack: Vec<Matrix>,
    text: TextState,
    /// The path being built has curves or lines, and so is not a rectangle.
    path_curves: bool,
    /// Rectangles in the path being built.
    path_rects: usize,
}

impl Reader<'_, '_> {
    fn run(&mut self, content: &[u8], resources: &Dict, depth: usize) -> Result<(), PdfError> {
        if depth > MAX_FORM_DEPTH {
            return Ok(());
        }
        let mut lexer = Lexer::over(content);
        let mut operands: Vec<Obj> = Vec::new();
        loop {
            lexer.skip_ws();
            let Some(&b) = content.get(lexer.pos) else {
                break;
            };
            if b.is_ascii_alphabetic() || b == b'\'' || b == b'"' || b == b'*' {
                let Some(word) = lexer.word() else {
                    // A byte that is neither: skipped, so the reading goes on.
                    lexer.pos += 1;
                    continue;
                };
                let op = String::from_utf8_lossy(word).into_owned();
                match op.as_str() {
                    "true" => operands.push(Obj::Bool(true)),
                    "false" => operands.push(Obj::Bool(false)),
                    "null" => operands.push(Obj::Null),
                    "BI" => {
                        // An inline image: its data is skipped up to the end keyword.
                        self.out.images += 1;
                        skip_inline_image(&mut lexer);
                        operands.clear();
                    }
                    _ => {
                        self.operate(&op, &operands, resources, depth)?;
                        operands.clear();
                    }
                }
            } else {
                match lexer.object(0) {
                    Ok(obj) => operands.push(obj),
                    // A token that is not an object: skipped, so the reading goes on.
                    Err(_) => {
                        lexer.pos += 1;
                    }
                }
            }
            if self.out.glyphs.len() > MAX_GLYPHS {
                return Err(PdfError::Damaged("the page has too many glyphs"));
            }
        }
        Ok(())
    }

    fn operate(
        &mut self,
        op: &str,
        args: &[Obj],
        resources: &Dict,
        depth: usize,
    ) -> Result<(), PdfError> {
        let n = |i: usize| args.get(i).and_then(Obj::as_f64).unwrap_or(0.0);
        match op {
            "q" => self.stack.push(self.ctm),
            "Q" => {
                if let Some(m) = self.stack.pop() {
                    self.ctm = m;
                }
            }
            "cm" => {
                let m = [n(0), n(1), n(2), n(3), n(4), n(5)];
                self.ctm = mul(m, self.ctm);
            }
            "BT" => {
                self.text.tm = IDENTITY;
                self.text.tlm = IDENTITY;
            }
            "ET" => {}
            "Tc" => self.text.char_spacing = n(0),
            "Tw" => self.text.word_spacing = n(0),
            "Tz" => self.text.horizontal = n(0) / 100.0,
            "TL" => self.text.leading = n(0),
            "Ts" => self.text.rise = n(0),
            "Tf" => {
                self.text.size = n(1);
                if let Some(name) = args.first().and_then(Obj::as_name) {
                    self.text.font = self.font_named(name, resources);
                }
            }
            "Td" => self.move_line(n(0), n(1)),
            "TD" => {
                self.text.leading = -n(1);
                self.move_line(n(0), n(1));
            }
            "Tm" => {
                let m = [n(0), n(1), n(2), n(3), n(4), n(5)];
                self.text.tlm = m;
                self.text.tm = m;
            }
            "T*" => self.move_line(0.0, -self.text.leading),
            "Tj" => {
                if let Some(Obj::Str(bytes)) = args.first() {
                    self.show(bytes);
                }
            }
            "'" => {
                self.move_line(0.0, -self.text.leading);
                if let Some(Obj::Str(bytes)) = args.first() {
                    self.show(bytes);
                }
            }
            "\"" => {
                self.text.word_spacing = n(0);
                self.text.char_spacing = n(1);
                self.move_line(0.0, -self.text.leading);
                if let Some(Obj::Str(bytes)) = args.get(2) {
                    self.show(bytes);
                }
            }
            "TJ" => {
                if let Some(Obj::Array(items)) = args.first() {
                    for item in items {
                        match item {
                            Obj::Str(bytes) => self.show(bytes),
                            other => {
                                // A number moves the next glyph left by that many thousandths.
                                let adjust = other.as_f64().unwrap_or(0.0);
                                let tx = -adjust / 1000.0 * self.text.size * self.text.horizontal;
                                self.text.tm = mul(translate(tx, 0.0), self.text.tm);
                            }
                        }
                    }
                }
            }
            "Do" => {
                if let Some(name) = args.first().and_then(Obj::as_name) {
                    self.draw_xobject(name, resources, depth)?;
                }
            }
            // Path construction: not drawn, only counted when painted. A filled path with curves
            // or lines is a glyph outline, which is text, so only rectangles and strokes are drawings.
            "m" | "l" | "c" | "v" | "y" => self.path_curves = true,
            "re" => self.path_rects += 1,
            "h" | "W" | "W*" => {}
            "n" => self.end_path(),
            "S" | "s" | "B" | "B*" | "b" | "b*" => {
                self.out.drawings += 1;
                self.end_path();
            }
            "f" | "F" | "f*" => {
                if self.path_rects > 0 && !self.path_curves {
                    self.out.drawings += 1;
                }
                self.end_path();
            }
            "sh" => self.out.drawings += 1,
            _ => {}
        }
        Ok(())
    }

    /// Forgets the path being built, once it is painted or ended.
    fn end_path(&mut self) {
        self.path_curves = false;
        self.path_rects = 0;
    }

    /// `Td`: moves the line matrix by `(tx, ty)`, and the text matrix to it.
    fn move_line(&mut self, tx: f64, ty: f64) {
        self.text.tlm = mul(translate(tx, ty), self.text.tlm);
        self.text.tm = self.text.tlm;
    }

    /// Shows the codes of a string: records each glyph and moves past it.
    fn show(&mut self, bytes: &[u8]) {
        let Some(font) = self.text.font.clone() else {
            self.out.unmapped += bytes.len();
            return;
        };
        let size = self.text.size;
        let horizontal = self.text.horizontal;
        for code in font.codes(bytes) {
            let (text, guessed) = match font.text(code) {
                Some((text, guessed)) => (text, guessed),
                None => {
                    self.out.unmapped += 1;
                    (String::new(), false)
                }
            };
            let single_space = font.code_bytes() == 1 && code == 32;
            let spacing = if single_space {
                self.text.word_spacing
            } else {
                0.0
            };
            let advance =
                (font.width(code) / 1000.0 * size + self.text.char_spacing + spacing) * horizontal;

            // Where the glyph is on the page: the text space, then the text matrix, then the CTM.
            let text_space = [size * horizontal, 0.0, 0.0, size, 0.0, self.text.rise];
            let to_page = mul(mul(text_space, self.text.tm), self.ctm);
            let page_scale = scale(mul(self.text.tm, self.ctm));
            if !text.is_empty() && self.out.glyphs.len() < MAX_GLYPHS {
                self.out.glyphs.push(Glyph {
                    text,
                    x: to_page[4],
                    y: to_page[5],
                    size: size * page_scale,
                    width: advance * page_scale,
                    guessed,
                    emphasis: font.is_emphasis(),
                });
            }
            self.text.tm = mul(translate(advance, 0.0), self.text.tm);
        }
    }

    /// Looks up a font by its name in the resources, reading it once.
    fn font_named(&mut self, name: &str, resources: &Dict) -> Option<Rc<Font>> {
        let fonts = self.doc.dict_of(&self.doc.entry(resources, "Font"))?;
        let object = fonts.get(name)?.clone();
        if let Obj::Ref(number) = object {
            if let Some(font) = self.fonts.get(&number) {
                return Some(font.clone());
            }
            let font = Rc::new(Font::load(self.doc, &Obj::Ref(number)));
            self.fonts.insert(number, font.clone());
            return Some(font);
        }
        Some(Rc::new(Font::load(self.doc, &object)))
    }

    /// `Do`: an image is counted, and a form is read in place with its own matrix and resources.
    fn draw_xobject(&mut self, name: &str, resources: &Dict, depth: usize) -> Result<(), PdfError> {
        let xobjects = self.doc.dict_of(&self.doc.entry(resources, "XObject"));
        let Some(object) = xobjects.as_ref().and_then(|x| x.get(name)).cloned() else {
            return Ok(());
        };
        let stream = match self.doc.resolve(&object) {
            Obj::Stream(s) => s,
            _ => return Ok(()),
        };
        match stream.dict.get("Subtype").and_then(Obj::as_name) {
            Some("Image") => self.out.images += 1,
            Some("Form") => {
                let matrix = match self.doc.entry(&stream.dict, "Matrix") {
                    Obj::Array(items) if items.len() == 6 => {
                        let v: Vec<f64> = items.iter().map(|i| i.as_f64().unwrap_or(0.0)).collect();
                        [v[0], v[1], v[2], v[3], v[4], v[5]]
                    }
                    _ => IDENTITY,
                };
                let form_resources = self
                    .doc
                    .dict_of(&self.doc.entry(&stream.dict, "Resources"))
                    .unwrap_or_else(|| resources.clone());
                let body = match self.doc.decode(&stream) {
                    Ok(body) => body,
                    Err(_) => return Ok(()),
                };
                self.stack.push(self.ctm);
                self.ctm = mul(matrix, self.ctm);
                let result = self.run(&body, &form_resources, depth + 1);
                if let Some(m) = self.stack.pop() {
                    self.ctm = m;
                }
                result?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// Skips an inline image: its dictionary and data, up to the `EI` that ends it.
fn skip_inline_image(lexer: &mut Lexer<'_>) {
    let data = lexer.data();
    let mut i = lexer.pos;
    // The data starts after `ID` and one white-space byte.
    if let Some(at) = find(data, i, b"ID") {
        i = at + 3;
    }
    while i + 1 < data.len() {
        let before_ok = i == 0 || data[i - 1].is_ascii_whitespace();
        let after_ok =
            i + 2 >= data.len() || data[i + 2].is_ascii_whitespace() || data[i + 2] == b'%';
        if data[i] == b'E' && data[i + 1] == b'I' && before_ok && after_ok {
            lexer.pos = i + 2;
            return;
        }
        i += 1;
    }
    lexer.pos = data.len();
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}
