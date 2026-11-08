// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF objects: parsing them from the bytes of a file, finding them through the cross-reference
//! tables, and decoding the streams. Only what text extraction needs is read.

use std::collections::BTreeMap;
use std::io::Read;

use super::PdfError;

/// The most levels of nested arrays, dictionaries and references followed. Deeper input is refused,
/// so a hostile file cannot exhaust the stack.
const MAX_DEPTH: usize = 64;
/// The most bytes a decoded stream may produce.
const MAX_DECODED: u64 = 64 * 1024 * 1024;

/// A dictionary: names (without the slash) to objects.
pub type Dict = BTreeMap<String, Obj>;

/// A PDF object.
#[derive(Debug, Clone, PartialEq)]
pub enum Obj {
    /// `null`, and any reference to an object that is not in the file.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A real number.
    Real(f64),
    /// A name, without its slash, with `#xx` escapes read.
    Name(String),
    /// A string's bytes.
    Str(Vec<u8>),
    /// An array.
    Array(Vec<Obj>),
    /// A dictionary.
    Dict(Dict),
    /// A reference to an indirect object, by its number.
    Ref(u32),
    /// A stream: its dictionary and its bytes as they are in the file (still encoded).
    Stream(Box<Stream>),
}

/// A stream object.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// The stream's dictionary.
    pub dict: Dict,
    /// The bytes between `stream` and `endstream`, encoded as the filters say.
    pub raw: Vec<u8>,
}

impl Obj {
    /// The number, for an integer or a real.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Obj::Int(i) => Some(*i as f64),
            Obj::Real(r) => Some(*r),
            _ => None,
        }
    }

    /// The integer, for an integer or a real with no fraction.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Obj::Int(i) => Some(*i),
            Obj::Real(r) if r.fract() == 0.0 => Some(*r as i64),
            _ => None,
        }
    }

    /// The name, for a name.
    pub fn as_name(&self) -> Option<&str> {
        match self {
            Obj::Name(n) => Some(n),
            _ => None,
        }
    }
}

/// Where the cross-reference tables say an object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// At this byte offset, as `N G obj`.
    Offset(usize),
    /// Inside the object stream with this number, at this index.
    InStream { stream: u32, index: usize },
}

/// A PDF file opened for reading: its objects, found through its cross-reference tables.
pub struct Document<'a> {
    data: &'a [u8],
    entries: BTreeMap<u32, Entry>,
    /// The trailer dictionary of the newest cross-reference section.
    pub trailer: Dict,
}

impl<'a> Document<'a> {
    /// Opens a PDF. The cross-reference tables are read from the end of the file, following their
    /// `Prev` links. A file whose tables are damaged is read by scanning it for objects instead.
    pub fn open(data: &'a [u8]) -> Result<Self, PdfError> {
        if !data[..data.len().min(1024)]
            .windows(5)
            .any(|w| w == b"%PDF-")
        {
            return Err(PdfError::NotPdf);
        }
        let mut doc = Document {
            data,
            entries: BTreeMap::new(),
            trailer: Dict::new(),
        };
        let tables = doc.read_tables();
        let usable = matches!(&tables, Ok((_, trailer)) if trailer.contains_key("Root"));
        if usable {
            let (entries, trailer) = tables?;
            doc.entries = entries;
            doc.trailer = trailer;
        } else {
            doc.scan()?;
        }
        if doc.trailer.contains_key("Encrypt") {
            return Err(PdfError::Encrypted);
        }
        Ok(doc)
    }

    /// Reads the cross-reference sections from the newest to the oldest. An object's newest
    /// entry is kept.
    fn read_tables(&self) -> Result<(BTreeMap<u32, Entry>, Dict), PdfError> {
        let start = self.startxref()?;
        let mut entries = BTreeMap::new();
        let mut newest: Option<Dict> = None;
        let mut next = Some(start);
        let mut seen = Vec::new();
        while let Some(at) = next.take() {
            if seen.contains(&at) || seen.len() > 1000 {
                break;
            }
            seen.push(at);
            if at >= self.data.len() {
                return Err(PdfError::Damaged(
                    "a cross-reference table is past the end of the file",
                ));
            }
            let trailer = if self.data[at..].starts_with(b"xref") {
                self.read_table(at, &mut entries)?
            } else {
                self.read_table_stream(at, &mut entries)?
            };
            if newest.is_none() {
                newest = Some(trailer.clone());
            }
            next = trailer
                .get("Prev")
                .and_then(Obj::as_i64)
                .and_then(|p| usize::try_from(p).ok());
        }
        Ok((entries, newest.unwrap_or_default()))
    }

    fn startxref(&self) -> Result<usize, PdfError> {
        let at = rfind(self.data, b"startxref").ok_or(PdfError::Damaged("no startxref"))?;
        let mut lexer = Lexer::at(self.data, at + "startxref".len());
        let offset = lexer
            .int()
            .ok_or(PdfError::Damaged("startxref has no offset"))?;
        usize::try_from(offset).map_err(|_| PdfError::Damaged("startxref is negative"))
    }

    /// A classic cross-reference table at `at`, then its trailer. Returns the trailer.
    fn read_table(&self, at: usize, entries: &mut BTreeMap<u32, Entry>) -> Result<Dict, PdfError> {
        let mut lexer = Lexer::at(self.data, at + 4);
        loop {
            lexer.skip_ws();
            if lexer.rest().starts_with(b"trailer") {
                lexer.pos += 7;
                break;
            }
            let first = lexer
                .int()
                .ok_or(PdfError::Damaged("bad xref subsection"))?;
            let count = lexer
                .int()
                .ok_or(PdfError::Damaged("bad xref subsection"))?;
            for i in 0..count {
                let offset = lexer.int().ok_or(PdfError::Damaged("bad xref entry"))?;
                let _generation = lexer.int().ok_or(PdfError::Damaged("bad xref entry"))?;
                let kind = lexer.word().ok_or(PdfError::Damaged("bad xref entry"))?;
                let number =
                    u32::try_from(first + i).map_err(|_| PdfError::Damaged("xref number"))?;
                if kind == b"n" && offset > 0 {
                    entries
                        .entry(number)
                        .or_insert(Entry::Offset(offset as usize));
                }
            }
        }
        match lexer.object(0)? {
            Obj::Dict(trailer) => Ok(trailer),
            _ => Err(PdfError::Damaged("the trailer is not a dictionary")),
        }
    }

    /// A cross-reference stream (PDF 1.5) at `at`. Returns its dictionary as the trailer.
    fn read_table_stream(
        &self,
        at: usize,
        entries: &mut BTreeMap<u32, Entry>,
    ) -> Result<Dict, PdfError> {
        let (_, object) = self.indirect_at(at)?;
        let Obj::Stream(stream) = object else {
            return Err(PdfError::Damaged(
                "the cross-reference object is not a stream",
            ));
        };
        let dict = &stream.dict;
        let widths: Vec<usize> = dict
            .get("W")
            .and_then(|w| match w {
                Obj::Array(a) => a
                    .iter()
                    .map(|v| v.as_i64().and_then(|n| usize::try_from(n).ok()))
                    .collect(),
                _ => None,
            })
            .ok_or(PdfError::Damaged("cross-reference stream has no /W"))?;
        if widths.len() != 3 {
            return Err(PdfError::Damaged(
                "cross-reference stream /W has three entries",
            ));
        }
        let size = dict.get("Size").and_then(Obj::as_i64).unwrap_or(0).max(0) as u32;
        let index: Vec<u32> = match dict.get("Index") {
            Some(Obj::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_i64().and_then(|n| u32::try_from(n).ok()))
                .collect(),
            _ => vec![0, size],
        };
        let data = self.decode(&stream)?;
        let row: usize = widths.iter().sum();
        if row == 0 {
            return Err(PdfError::Damaged("cross-reference stream rows are empty"));
        }
        let mut rows = data.chunks_exact(row);
        for pair in index.chunks_exact(2) {
            for i in 0..pair[1] {
                let Some(r) = rows.next() else { break };
                let mut fields = [0u64; 3];
                let mut at = 0;
                for (k, &w) in widths.iter().enumerate() {
                    let mut value = 0u64;
                    for &byte in &r[at..at + w] {
                        value = (value << 8) | u64::from(byte);
                    }
                    fields[k] = value;
                    at += w;
                }
                // A field of width zero has its default: type 1 for the type field.
                let kind = if widths[0] == 0 { 1 } else { fields[0] };
                let number = pair[0] + i;
                match kind {
                    1 => {
                        if fields[1] > 0 {
                            entries
                                .entry(number)
                                .or_insert(Entry::Offset(fields[1] as usize));
                        }
                    }
                    2 => {
                        entries.entry(number).or_insert(Entry::InStream {
                            stream: fields[1] as u32,
                            index: fields[2] as usize,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(dict.clone())
    }

    /// Reads the file for objects, when its tables cannot be used. The last trailer with a root,
    /// or else the catalog, becomes the trailer.
    fn scan(&mut self) -> Result<(), PdfError> {
        let data = self.data;
        let mut offsets = BTreeMap::new();
        let mut trailers = Vec::new();
        let mut i = 0;
        while i < data.len() {
            if data[i..].starts_with(b"obj") && (i == 0 || !data[i - 1].is_ascii_alphanumeric()) {
                if let Some(start) = object_header_before(data, i) {
                    if let Some(number) = Lexer::at(data, start).int() {
                        if let Ok(number) = u32::try_from(number) {
                            offsets.insert(number, Entry::Offset(start));
                        }
                    }
                }
            }
            if data[i..].starts_with(b"trailer") {
                let mut lexer = Lexer::at(data, i + 7);
                if let Ok(Obj::Dict(d)) = lexer.object(0) {
                    trailers.push(d);
                }
            }
            i += 1;
        }
        self.entries = offsets;
        // Objects inside object streams are found through the streams' headers.
        let streams: Vec<u32> = self
            .entries
            .keys()
            .filter_map(|n| {
                let (_, object) = self.object_by_entry(*n).ok()?;
                match object {
                    Obj::Stream(s)
                        if s.dict.get("Type").and_then(Obj::as_name) == Some("ObjStm") =>
                    {
                        Some(*n)
                    }
                    _ => None,
                }
            })
            .collect();
        for stream in streams {
            if let Ok((header, _)) = self.object_stream_objects(stream) {
                for (index, number) in header.into_iter().enumerate() {
                    self.entries
                        .entry(number)
                        .or_insert(Entry::InStream { stream, index });
                }
            }
        }
        let trailer = trailers.into_iter().rev().find(|t| t.contains_key("Root"));
        self.trailer = match trailer {
            Some(t) => t,
            None => {
                let catalog = self.find_catalog()?;
                let mut t = Dict::new();
                t.insert("Root".into(), Obj::Ref(catalog));
                t
            }
        };
        Ok(())
    }

    fn find_catalog(&self) -> Result<u32, PdfError> {
        let mut found = None;
        for &number in self.entries.keys() {
            if let Ok(Obj::Dict(d)) = self.get(number) {
                if d.get("Type").and_then(Obj::as_name) == Some("Catalog") {
                    found = Some(number);
                }
            }
        }
        found.ok_or(PdfError::Damaged("no catalog"))
    }

    /// The object with number `number`, or `Null` when the file does not have it.
    pub fn get(&self, number: u32) -> Result<Obj, PdfError> {
        match self.entries.get(&number) {
            None => Ok(Obj::Null),
            Some(_) => self.object_by_entry(number).map(|(_, o)| o),
        }
    }

    /// Follows references until the object is not one.
    pub fn resolve(&self, object: &Obj) -> Obj {
        let mut current = object.clone();
        for _ in 0..MAX_DEPTH {
            match current {
                Obj::Ref(n) => match self.get(n) {
                    Ok(next) => current = next,
                    Err(_) => return Obj::Null,
                },
                _ => return current,
            }
        }
        Obj::Null
    }

    /// The dictionary a key refers to, after resolving references.
    pub fn dict_of(&self, object: &Obj) -> Option<Dict> {
        match self.resolve(object) {
            Obj::Dict(d) => Some(d),
            Obj::Stream(s) => Some(s.dict),
            _ => None,
        }
    }

    /// The value of `key` in a dictionary, resolved.
    pub fn entry(&self, dict: &Dict, key: &str) -> Obj {
        dict.get(key).map(|o| self.resolve(o)).unwrap_or(Obj::Null)
    }

    /// The decoded bytes of a stream object.
    pub fn decode(&self, stream: &Stream) -> Result<Vec<u8>, PdfError> {
        decode_stream(self, stream)
    }

    /// The decoded bytes of a page's contents: one stream, or several joined by line ends. Parts
    /// that are not streams are skipped.
    pub fn decode_contents(&self, parts: &[Obj]) -> Result<Vec<u8>, PdfError> {
        let mut out = Vec::new();
        for part in parts {
            if let Obj::Stream(stream) = part {
                out.extend_from_slice(&self.decode(stream)?);
                out.push(b'\n');
            }
        }
        Ok(out)
    }

    fn object_by_entry(&self, number: u32) -> Result<(u32, Obj), PdfError> {
        match self.entries.get(&number).copied() {
            Some(Entry::Offset(at)) => {
                let (found, object) = self.indirect_at(at)?;
                if found != number {
                    return Err(PdfError::Damaged("an object is not where the table says"));
                }
                Ok((number, object))
            }
            Some(Entry::InStream { stream, index }) => {
                let (header, objects) = self.object_stream_objects(stream)?;
                let _ = header;
                objects
                    .get(index)
                    .cloned()
                    .map(|o| (number, o))
                    .ok_or(PdfError::Damaged(
                        "an object is missing from its object stream",
                    ))
            }
            None => Ok((number, Obj::Null)),
        }
    }

    /// The `N G obj ... endobj` at `at`: its number, and its object.
    fn indirect_at(&self, at: usize) -> Result<(u32, Obj), PdfError> {
        let mut lexer = Lexer::at(self.data, at);
        let number = lexer
            .int()
            .ok_or(PdfError::Damaged("an object has no number"))?;
        let _generation = lexer
            .int()
            .ok_or(PdfError::Damaged("an object has no generation"))?;
        if lexer.word() != Some(b"obj".as_slice()) {
            return Err(PdfError::Damaged("an object has no obj keyword"));
        }
        let value = lexer.object(0)?;
        let value = match value {
            Obj::Dict(dict) if lexer.keyword_follows(b"stream") => {
                let raw = lexer.stream_body(&dict)?;
                Obj::Stream(Box::new(Stream { dict, raw }))
            }
            other => other,
        };
        Ok((
            u32::try_from(number).map_err(|_| PdfError::Damaged("object number"))?,
            value,
        ))
    }

    /// The object numbers in an object stream's header, and the objects it holds, in order.
    fn object_stream_objects(&self, stream: u32) -> Result<(Vec<u32>, Vec<Obj>), PdfError> {
        let (_, object) = self.object_by_entry(stream)?;
        let Obj::Stream(s) = object else {
            return Err(PdfError::Damaged("an object stream is not a stream"));
        };
        let count = s.dict.get("N").and_then(Obj::as_i64).unwrap_or(0).max(0) as usize;
        let first = s
            .dict
            .get("First")
            .and_then(Obj::as_i64)
            .unwrap_or(0)
            .max(0) as usize;
        let data = self.decode(&s)?;
        let mut lexer = Lexer::over(&data);
        let mut numbers = Vec::with_capacity(count);
        let mut offsets = Vec::with_capacity(count);
        for _ in 0..count {
            let number = lexer
                .int()
                .ok_or(PdfError::Damaged("bad object stream header"))?;
            let offset = lexer
                .int()
                .ok_or(PdfError::Damaged("bad object stream header"))?;
            numbers.push(u32::try_from(number).map_err(|_| PdfError::Damaged("object number"))?);
            offsets.push(first + usize::try_from(offset).unwrap_or(0));
        }
        let mut objects = Vec::with_capacity(count);
        for offset in offsets {
            let mut lexer = Lexer::at(&data, offset);
            objects.push(lexer.object(0).unwrap_or(Obj::Null));
        }
        Ok((numbers, objects))
    }
}

/// If `obj` at `i` ends an `N G obj` header, the offset of `N`.
fn object_header_before(data: &[u8], i: usize) -> Option<usize> {
    // Walk back over white space, the generation, white space, the number.
    let mut j = i;
    let skip_ws = |mut k: usize| {
        while k > 0 && data[k - 1].is_ascii_whitespace() {
            k -= 1;
        }
        k
    };
    let digits = |mut k: usize| {
        let end = k;
        while k > 0 && data[k - 1].is_ascii_digit() {
            k -= 1;
        }
        (k, end)
    };
    j = skip_ws(j);
    let (gen_start, gen_end) = digits(j);
    if gen_start == gen_end {
        return None;
    }
    let k = skip_ws(gen_start);
    if k == gen_start {
        return None;
    }
    let (num_start, num_end) = digits(k);
    if num_start == num_end {
        return None;
    }
    Some(num_start)
}

/// The last occurrence of `needle`.
fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn is_white(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | 0)
}

fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// A cursor over PDF bytes, reading objects and tokens.
pub struct Lexer<'a> {
    data: &'a [u8],
    /// The next byte to read.
    pub pos: usize,
}

impl<'a> Lexer<'a> {
    /// The bytes the lexer reads.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    /// A lexer over `data` from the start.
    pub fn over(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// A lexer over `data` from `pos`.
    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..).unwrap_or(&[])
    }

    /// Skips white space and comments.
    pub fn skip_ws(&mut self) {
        while let Some(&b) = self.data.get(self.pos) {
            if is_white(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(&c) = self.data.get(self.pos) {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// The next regular word (a keyword, or a number's text), without reading past it.
    pub fn word(&mut self) -> Option<&'a [u8]> {
        self.skip_ws();
        let start = self.pos;
        while let Some(&b) = self.data.get(self.pos) {
            if is_white(b) || is_delimiter(b) {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            None
        } else {
            Some(&self.data[start..self.pos])
        }
    }

    /// The next token, read as an integer. The position moves past it only when it is one.
    pub fn int(&mut self) -> Option<i64> {
        let save = self.pos;
        match self
            .word()
            .and_then(|w| std::str::from_utf8(w).ok()?.parse::<i64>().ok())
        {
            Some(n) => Some(n),
            None => {
                self.pos = save;
                None
            }
        }
    }

    /// Whether the next keyword is `keyword`; if so, it is read.
    fn keyword_follows(&mut self, keyword: &[u8]) -> bool {
        let save = self.pos;
        if self.word() == Some(keyword) {
            true
        } else {
            self.pos = save;
            false
        }
    }

    /// The bytes of a stream after its dictionary, up to `endstream`. The `/Length` is used when
    /// it is right, and the end keyword is searched for otherwise.
    fn stream_body(&mut self, dict: &Dict) -> Result<Vec<u8>, PdfError> {
        // The keyword is followed by an end of line.
        if self.rest().starts_with(b"\r\n") {
            self.pos += 2;
        } else if self.rest().starts_with(b"\n") || self.rest().starts_with(b"\r") {
            self.pos += 1;
        }
        let start = self.pos;
        if let Some(Obj::Int(len)) = dict.get("Length") {
            if let Ok(len) = usize::try_from(*len) {
                let end = start + len;
                let mut after = Lexer::at(self.data, end);
                after.skip_ws();
                if after.rest().starts_with(b"endstream") {
                    self.pos = after.pos + "endstream".len();
                    return Ok(self.data[start..end].to_vec());
                }
            }
        }
        let end = find(self.data, start, b"endstream")
            .ok_or(PdfError::Damaged("a stream has no endstream"))?;
        let mut body_end = end;
        if body_end > start && self.data[body_end - 1] == b'\n' {
            body_end -= 1;
        }
        if body_end > start && self.data[body_end - 1] == b'\r' {
            body_end -= 1;
        }
        self.pos = end + "endstream".len();
        Ok(self.data[start..body_end].to_vec())
    }

    /// One object. Dictionaries and arrays are read whole; a stream is not (see `indirect_at`).
    pub fn object(&mut self, depth: usize) -> Result<Obj, PdfError> {
        if depth > MAX_DEPTH {
            return Err(PdfError::Damaged("objects are nested too deeply"));
        }
        self.skip_ws();
        let Some(&b) = self.data.get(self.pos) else {
            return Err(PdfError::Damaged("the file ends inside an object"));
        };
        match b {
            b'/' => {
                self.pos += 1;
                let raw = self.regular_run();
                Ok(Obj::Name(unescape_name(raw)))
            }
            b'(' => self.literal_string().map(Obj::Str),
            b'<' => {
                if self.data.get(self.pos + 1) == Some(&b'<') {
                    self.pos += 2;
                    self.dict_body(depth).map(Obj::Dict)
                } else {
                    self.pos += 1;
                    Ok(Obj::Str(self.hex_string()))
                }
            }
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.data.get(self.pos) {
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Obj::Array(items));
                        }
                        None => return Err(PdfError::Damaged("an array is not closed")),
                        _ => items.push(self.object(depth + 1)?),
                    }
                }
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => self.number_or_reference(),
            _ => {
                let word = self
                    .word()
                    .ok_or(PdfError::Damaged("an object is expected here"))?;
                match word {
                    b"true" => Ok(Obj::Bool(true)),
                    b"false" => Ok(Obj::Bool(false)),
                    b"null" => Ok(Obj::Null),
                    _ => Err(PdfError::Damaged(
                        "an unknown keyword where an object is expected",
                    )),
                }
            }
        }
    }

    /// The bytes of a name, up to a delimiter.
    fn regular_run(&mut self) -> &'a [u8] {
        let start = self.pos;
        while let Some(&b) = self.data.get(self.pos) {
            if is_white(b) || is_delimiter(b) {
                break;
            }
            self.pos += 1;
        }
        &self.data[start..self.pos]
    }

    fn number_or_reference(&mut self) -> Result<Obj, PdfError> {
        let save = self.pos;
        let text = self
            .word()
            .ok_or(PdfError::Damaged("a number is expected"))?;
        let as_text =
            std::str::from_utf8(text).map_err(|_| PdfError::Damaged("a number is not text"))?;
        if as_text.contains('.') || as_text.contains(['e', 'E']) {
            return as_text
                .parse::<f64>()
                .map(Obj::Real)
                .map_err(|_| PdfError::Damaged("a bad real number"));
        }
        let Ok(value) = as_text.parse::<i64>() else {
            self.pos = save;
            return Err(PdfError::Damaged("a bad integer"));
        };
        // `N G R` is a reference. Anything else leaves the position after the integer.
        let after_int = self.pos;
        if let Some(g) = self
            .word()
            .and_then(|w| std::str::from_utf8(w).ok()?.parse::<i64>().ok())
        {
            if self.word() == Some(b"R".as_slice()) && g >= 0 && value >= 0 {
                return Ok(Obj::Ref(value as u32));
            }
        }
        self.pos = after_int;
        Ok(Obj::Int(value))
    }

    fn dict_body(&mut self, depth: usize) -> Result<Dict, PdfError> {
        let mut dict = Dict::new();
        loop {
            self.skip_ws();
            if self.data.get(self.pos) == Some(&b'>') && self.data.get(self.pos + 1) == Some(&b'>')
            {
                self.pos += 2;
                return Ok(dict);
            }
            if self.data.get(self.pos) != Some(&b'/') {
                return Err(PdfError::Damaged("a dictionary key is not a name"));
            }
            self.pos += 1;
            let key = unescape_name(self.regular_run());
            self.skip_ws();
            let value = self.object(depth + 1)?;
            dict.insert(key, value);
        }
    }

    /// A literal string, `(...)`, with its escapes read.
    fn literal_string(&mut self) -> Result<Vec<u8>, PdfError> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1;
        while let Some(&b) = self.data.get(self.pos) {
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(b);
                }
                b'\\' => {
                    let Some(&e) = self.data.get(self.pos) else {
                        break;
                    };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.data.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut value = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.data.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        value = value * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(value as u8);
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        Err(PdfError::Damaged("a string is not closed"))
    }

    /// A hexadecimal string, after its `<`.
    fn hex_string(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(&b) = self.data.get(self.pos) {
            self.pos += 1;
            if b == b'>' {
                break;
            }
            if let Some(v) = hex_value(b) {
                match high.take() {
                    Some(h) => out.push(h << 4 | v),
                    None => high = Some(v),
                }
            }
        }
        if let Some(h) = high {
            out.push(h << 4);
        }
        out
    }
}

/// The value of a hexadecimal digit.
pub fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn unescape_name(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'#' && i + 2 < raw.len() {
            if let (Some(h), Some(l)) = (
                raw.get(i + 1).and_then(|&b| hex_value(b)),
                raw.get(i + 2).and_then(|&b| hex_value(b)),
            ) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// The decoded bytes of a stream: its filters applied in order, then its predictor.
pub fn decode_stream(doc: &Document<'_>, stream: &Stream) -> Result<Vec<u8>, PdfError> {
    let filters: Vec<String> = match doc.entry(&stream.dict, "Filter") {
        Obj::Name(n) => vec![n],
        Obj::Array(items) => items
            .iter()
            .filter_map(|i| i.as_name().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    let parms: Vec<Dict> = match doc.entry(&stream.dict, "DecodeParms") {
        Obj::Dict(d) => vec![d],
        Obj::Array(items) => items
            .iter()
            .map(|i| doc.dict_of(i).unwrap_or_default())
            .collect(),
        _ => Vec::new(),
    };
    let mut data = stream.raw.clone();
    for (i, filter) in filters.iter().enumerate() {
        let parm = parms.get(i).cloned().unwrap_or_default();
        data = match filter.as_str() {
            "FlateDecode" | "Fl" => {
                let inflated = inflate(&data)?;
                predict(&inflated, &parm)?
            }
            "ASCIIHexDecode" | "AHx" => ascii_hex(&data),
            "ASCII85Decode" | "A85" => ascii85(&data)?,
            other => return Err(PdfError::Unsupported(format!("the {other} filter"))),
        };
    }
    Ok(data)
}

/// Inflates zlib data, or raw deflate data when there is no zlib header.
fn inflate(data: &[u8]) -> Result<Vec<u8>, PdfError> {
    let mut out = Vec::new();
    let zlib = flate2::read::ZlibDecoder::new(data)
        .take(MAX_DECODED)
        .read_to_end(&mut out);
    if zlib.is_ok() {
        return Ok(out);
    }
    out.clear();
    flate2::read::DeflateDecoder::new(data)
        .take(MAX_DECODED)
        .read_to_end(&mut out)
        .map_err(|_| PdfError::Damaged("a stream does not inflate"))?;
    Ok(out)
}

/// Undoes a PNG predictor (`/Predictor` 10 and up) when the decode parameters ask for one.
fn predict(data: &[u8], parm: &Dict) -> Result<Vec<u8>, PdfError> {
    let predictor = parm.get("Predictor").and_then(Obj::as_i64).unwrap_or(1);
    if predictor < 10 {
        return Ok(data.to_vec());
    }
    let colors = parm.get("Colors").and_then(Obj::as_i64).unwrap_or(1).max(1) as usize;
    let bits = parm
        .get("BitsPerComponent")
        .and_then(Obj::as_i64)
        .unwrap_or(8)
        .max(1) as usize;
    let columns = parm
        .get("Columns")
        .and_then(Obj::as_i64)
        .unwrap_or(1)
        .max(1) as usize;
    let bpp = (colors * bits).div_ceil(8).max(1);
    let stride = (columns * colors * bits).div_ceil(8);
    let mut out: Vec<u8> = Vec::new();
    let mut previous = vec![0u8; stride];
    for row in data.chunks(stride + 1) {
        if row.len() < stride + 1 {
            break;
        }
        let kind = row[0];
        let mut current = vec![0u8; stride];
        for i in 0..stride {
            let raw = row[i + 1];
            let left = if i >= bpp { current[i - bpp] } else { 0 };
            let up = previous[i];
            let up_left = if i >= bpp { previous[i - bpp] } else { 0 };
            current[i] = match kind {
                0 => raw,
                1 => raw.wrapping_add(left),
                2 => raw.wrapping_add(up),
                3 => raw.wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8),
                4 => raw.wrapping_add(paeth(left, up, up_left)),
                _ => return Err(PdfError::Damaged("an unknown PNG filter type")),
            };
        }
        out.extend_from_slice(&current);
        previous = current;
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut high: Option<u8> = None;
    for &b in data {
        if b == b'>' {
            break;
        }
        if let Some(v) = hex_value(b) {
            match high.take() {
                Some(h) => out.push(h << 4 | v),
                None => high = Some(v),
            }
        }
    }
    if let Some(h) = high {
        out.push(h << 4);
    }
    out
}

fn ascii85(data: &[u8]) -> Result<Vec<u8>, PdfError> {
    let mut out = Vec::new();
    let mut group: Vec<u32> = Vec::with_capacity(5);
    for b in data.iter().copied() {
        match b {
            b'~' => break,
            b if is_white(b) => continue,
            b'z' if group.is_empty() => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                group.push(u32::from(b - b'!'));
                if group.len() == 5 {
                    let value = group
                        .iter()
                        .fold(0u32, |acc, &d| acc.wrapping_mul(85).wrapping_add(d));
                    out.extend_from_slice(&value.to_be_bytes());
                    group.clear();
                }
            }
            _ => return Err(PdfError::Damaged("a bad ASCII85 character")),
        }
    }
    if !group.is_empty() {
        let n = group.len();
        while group.len() < 5 {
            group.push(84);
        }
        let value = group
            .iter()
            .fold(0u32, |acc, &d| acc.wrapping_mul(85).wrapping_add(d));
        out.extend_from_slice(&value.to_be_bytes()[..n - 1]);
    }
    Ok(out)
}
