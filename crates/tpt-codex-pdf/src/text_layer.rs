// SPDX-License-Identifier: MIT OR Apache-2.0
//! The text layer: the words of a PDF, drawn invisibly over its outlines, and, for a tagged file,
//! the structure that says what each part is.
//!
//! Text is drawn as glyph outlines, so a page shows the text but cannot search, select or read it.
//! This adds the text back. Each glyph is written again in an invisible text object (render mode 3)
//! at the position of its outline, with the bundled font embedded as a CID font and a ToUnicode map
//! that says which characters each glyph stands for.
//!
//! A tagged file also names each piece of that text: a paragraph, a heading, a list item, or a
//! figure with alternative text, through a structure tree (see [`crate::structure`]). Headers and
//! footers are marked as artifacts, which readers skip. The catalog says the document's language,
//! and gives the title.
//!
//! The file is extended by an incremental update: the page and catalog objects are rewritten, and
//! the new objects follow the original bytes, which are not changed. A reader that ignores the
//! layer draws the same page.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

use crate::picture::{Image, ImageData};
use crate::structure::{self, Mark, Tree};
use crate::subset::subset_truetype;

/// One glyph of the text layer, where it is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct TextGlyph {
    /// The glyph id in its face.
    pub glyph: u32,
    /// The glyph origin, in points from the left edge of the page.
    pub x: f32,
    /// The baseline, in points from the top edge of the page. Positive is down.
    pub y: f32,
    /// The glyph's horizontal advance, in points.
    pub advance: f32,
    /// The characters the glyph stands for, when it starts a cluster. Empty otherwise.
    pub text: String,
}

/// What a run of text is, for the structure of a tagged file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// Not part of the structure: the text is drawn with no marks.
    Untagged,
    /// Page furniture, such as a header or footer: marked as an artifact.
    Artifact,
    /// A paragraph of the body text.
    Paragraph,
    /// A heading of this level, from 1 to 6.
    Heading(u8),
    /// A line of the list item with this block id, in the list with `list` as its block id.
    ListItem {
        /// The block id of the list.
        list: u64,
    },
    /// A cell of a grid.
    Cell {
        /// The block id of the grid.
        table: u64,
        /// The row, counted from 1 as the grid's rows are.
        row: u32,
        /// The column, counted from 0.
        col: u32,
        /// Whether the cell is in a header row.
        header: bool,
    },
}

/// A run of glyphs from one face at one size, and what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    /// The index of the face in the faces given to [`add_text_layer`].
    pub face: usize,
    /// The size in points.
    pub size: f32,
    /// The glyphs, in drawing order.
    pub glyphs: Vec<TextGlyph>,
    /// What the run is, for the structure.
    pub role: Role,
    /// The block the run belongs to. Runs of one block are one element of the structure.
    pub block: u64,
}

/// One thing drawn on a page, in drawing order.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A run of text.
    Run(TextRun),
    /// A figure: a picture's placeholder on the page, with its alternative text, or the picture.
    Figure {
        /// The block id of the figure.
        block: u64,
        /// The alternative text, when the figure has some.
        alt: Option<String>,
        /// The picture drawn in the figure's place, when there is one.
        picture: Option<PlacedPicture>,
    },
}

/// A picture on a page: which of the images it is, and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPicture {
    /// The index of the image in the images given to [`add_text_layer`].
    pub image: usize,
    /// The left edge, in points from the left of the page.
    pub x: f32,
    /// The top edge, in points from the top of the page.
    pub y: f32,
    /// The width on the page, in points.
    pub width: f32,
    /// The height on the page, in points.
    pub height: f32,
}

/// The file's description, for a tagged file: the title and author, and the language.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocInfo {
    /// The title, shown by the reader and used by accessibility tools.
    pub title: Option<String>,
    /// The author.
    pub author: Option<String>,
    /// The language of the text, as a tag such as `en`.
    pub language: String,
}

/// The text layer could not be added to the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayerError(pub String);

impl fmt::Display for TextLayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "text layer: {}", self.0)
    }
}

impl std::error::Error for TextLayerError {}

fn fail<T>(message: impl Into<String>) -> Result<T, TextLayerError> {
    Err(TextLayerError(message.into()))
}

/// Adds the text layer to `pdf`, a file of one page per entry of `pages`, each `page_height`
/// points tall, as written by the export. `faces` holds the font file of each face the runs index.
///
/// With `info`, the file is tagged: it gets its structure tree, its language and its title. Without
/// it, the text is drawn with no marks, as it is on a slide.
///
/// The original bytes are kept; the page objects are rewritten, and the new objects follow them.
pub fn add_text_layer(
    pdf: &[u8],
    page_height: f32,
    pages: &[Vec<Item>],
    faces: &[&[u8]],
    info: Option<&DocInfo>,
    images: &[Image],
) -> Result<Vec<u8>, TextLayerError> {
    let xref = start_xref(pdf)?;
    let mut next = declared_size(pdf)?;
    let runs: Vec<&TextRun> = pages
        .iter()
        .flatten()
        .filter_map(|item| match item {
            Item::Run(run) => Some(run),
            Item::Figure { .. } => None,
        })
        .collect();

    let used: BTreeSet<usize> = runs.iter().map(|run| run.face).collect();
    if let Some(&face) = used.iter().find(|&&f| f >= faces.len()) {
        return fail(format!(
            "a run uses face {face}, but {} faces were given",
            faces.len()
        ));
    }

    let mut objects: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut fonts: BTreeMap<usize, FontObjects> = BTreeMap::new();
    for &face in &used {
        let glyphs: BTreeSet<u32> = runs
            .iter()
            .filter(|run| run.face == face)
            .flat_map(|run| run.glyphs.iter().map(|g| g.glyph))
            .collect();
        let file = subset_truetype(faces[face], &glyphs)
            .map_err(|e| TextLayerError(format!("face {face}: {e}")))?;
        let font = FontObjects::allocate(&mut next);
        font.push_objects(face, &file, &runs, &mut objects);
        fonts.insert(face, font);
    }
    let font_resources: String = fonts
        .iter()
        .map(|(face, font)| format!("/TPTText{face} {} 0 R ", font.type0))
        .collect();

    // The pictures: each image is an image object, and its alpha channel, when it has one, a
    // second object that the image refers to.
    let mut image_numbers: Vec<(u64, Option<u64>)> = Vec::with_capacity(images.len());
    for image in images {
        let number = next;
        next += 1;
        let alpha = image.alpha.as_ref().map(|_| {
            next += 1;
            next - 1
        });
        image_numbers.push((number, alpha));
        objects.extend(image_objects(image, number, alpha));
    }

    // The structure: its root, its document element and its parent tree take three numbers, then
    // each element one.
    let mut tree: Option<Tree> = info.map(|_| structure::build(pages));
    let structure_numbers = tree.as_mut().map(|tree| {
        let base = next;
        next += 3;
        for node in tree.nodes.iter_mut() {
            node.number = next;
            next += 1;
        }
        base
    });
    let info_number = match info {
        Some(info) if info.title.is_some() || info.author.is_some() => {
            next += 1;
            Some(next - 1)
        }
        _ => None,
    };

    let no_marks: Vec<Vec<Mark>> = pages
        .iter()
        .map(|items| vec![Mark::None; items.len()])
        .collect();
    let marks: &[Vec<Mark>] = tree.as_ref().map_or(&no_marks, |t| &t.marks);

    for (index, items) in pages.iter().enumerate() {
        let page_object = 3 + 2 * index as u64;
        let content = 4 + 2 * index as u64;
        let text_stream = next;
        next += 1;

        let page = object_text(pdf, page_object)?;
        if page.contains("/Font") {
            return fail(format!("page {index} already has fonts"));
        }
        let contents = format!("/Contents {content} 0 R");
        let resources = "/Resources << ";
        if !page.contains(&contents) || !page.contains(resources) {
            return fail(format!(
                "page {index} is not in the form the export writes (no contents or resources)"
            ));
        }
        let used_images: BTreeSet<usize> = items
            .iter()
            .filter_map(|item| match item {
                Item::Figure {
                    picture: Some(placed),
                    ..
                } => Some(placed.image),
                _ => None,
            })
            .collect();
        if let Some(&bad) = used_images.iter().find(|&&i| i >= images.len()) {
            return fail(format!(
                "page {index} draws image {bad}, but only {} were given",
                images.len()
            ));
        }
        let mut rewritten = page
            .replacen(
                &contents,
                &format!("/Contents [{content} 0 R {text_stream} 0 R]"),
                1,
            )
            .replacen(
                resources,
                &format!("{resources}/Font << {font_resources}>> "),
                1,
            );
        if !used_images.is_empty() {
            if page.contains("/XObject") {
                return fail(format!("page {index} already has an XObject resource"));
            }
            let xobjects: String = used_images
                .iter()
                .map(|i| format!("/TPTImage{i} {} 0 R ", image_numbers[*i].0))
                .collect();
            rewritten = rewritten.replacen(
                resources,
                &format!("{resources}/XObject << {xobjects}>> "),
                1,
            );
        }
        if tree.is_some() {
            rewritten = rewritten.replacen(
                "/Type /Page ",
                &format!("/Type /Page /StructParents {index} /Tabs /S "),
                1,
            );
        }
        objects.push((page_object, rewritten.into_bytes()));
        let page_marks = marks.get(index).map_or(&[][..], |m| m.as_slice());
        objects.push((
            text_stream,
            stream_bytes(
                "",
                content_stream(items, page_marks, page_height).as_bytes(),
            ),
        ));
    }

    if let (Some(tree), Some(base)) = (&tree, structure_numbers) {
        objects.extend(structure_objects(tree, base, pages.len(), |p| {
            3 + 2 * p as u64
        }));
    }
    if let (Some(info), Some(number)) = (info, info_number) {
        objects.push((number, info_dictionary(info).into_bytes()));
    }
    if let (Some(info), Some(base)) = (info, structure_numbers) {
        // The catalog says the language and the structure root, and the title for the viewer.
        let catalog = object_text(pdf, 1)?;
        let base_text = catalog
            .strip_suffix(">>")
            .ok_or_else(|| TextLayerError("the catalog is not a dictionary".into()))?
            .trim_end();
        let mut extra = format!(
            " /Lang ({}) /MarkInfo << /Marked true >> /StructTreeRoot {} 0 R",
            info.language, base
        );
        if info.title.is_some() {
            extra.push_str(" /ViewerPreferences << /DisplayDocTitle true >>");
        }
        objects.push((1, format!("{base_text}{extra} >>").into_bytes()));
    }
    Ok(append(pdf, &objects, next, xref, info_number))
}

/// The numbers of one embedded font's objects.
struct FontObjects {
    type0: u64,
    descendant: u64,
    descriptor: u64,
    file: u64,
    to_unicode: u64,
}

impl FontObjects {
    fn allocate(next: &mut u64) -> Self {
        let font = Self {
            type0: *next,
            descendant: *next + 1,
            descriptor: *next + 2,
            file: *next + 3,
            to_unicode: *next + 4,
        };
        *next += 5;
        font
    }

    /// The five objects of the font: the composite font, its descendant, the descriptor, the
    /// font file, and the ToUnicode map. Glyph widths and characters come from the runs.
    fn push_objects(
        &self,
        face: usize,
        font_file: &[u8],
        runs: &[&TextRun],
        out: &mut Vec<(u64, Vec<u8>)>,
    ) {
        let name = format!("TPTText{face}");
        // The first width and the first characters seen for each glyph.
        let mut widths: BTreeMap<u32, f32> = BTreeMap::new();
        let mut characters: BTreeMap<u32, String> = BTreeMap::new();
        for run in runs.iter().filter(|r| r.face == face) {
            for g in &run.glyphs {
                if run.size > 0.0 {
                    widths
                        .entry(g.glyph)
                        .or_insert(g.advance / run.size * 1000.0);
                }
                if !g.text.is_empty() {
                    characters.entry(g.glyph).or_insert_with(|| g.text.clone());
                }
            }
        }
        let mut w_entries = String::new();
        for (glyph, width) in &widths {
            let _ = write!(w_entries, " {glyph} [{}]", fmt(*width));
        }
        out.push((
            self.type0,
            format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H \
                 /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
                self.descendant, self.to_unicode
            )
            .into_bytes(),
        ));
        out.push((
            self.descendant,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} \
                 /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor {} 0 R /DW 1000 /W [{w_entries} ] /CIDToGIDMap /Identity >>",
                self.descriptor
            )
            .into_bytes(),
        ));
        out.push((
            self.descriptor,
            format!(
                "<< /Type /FontDescriptor /FontName /{name} /Flags 32 \
                 /FontBBox [-1000 -1000 2000 2000] /ItalicAngle 0 /Ascent 800 /Descent -200 \
                 /CapHeight 700 /StemV 80 /FontFile2 {} 0 R >>",
                self.file
            )
            .into_bytes(),
        ));
        out.push((
            self.file,
            stream_bytes(&format!("/Length1 {}", font_file.len()), font_file),
        ));
        out.push((
            self.to_unicode,
            stream_bytes("", to_unicode(&characters).as_bytes()),
        ));
    }
}

/// The content of one page: each item in its marks, with the text of each run in a text object
/// per glyph.
fn content_stream(items: &[Item], marks: &[Mark], page_height: f32) -> String {
    let mut out = String::from("q\n");
    for (index, item) in items.iter().enumerate() {
        let mark = marks.get(index).cloned().unwrap_or(Mark::None);
        match (item, &mark) {
            (Item::Figure { picture, .. }, _) => {
                // A figure is marked where it is drawn: its picture, if it has one, is inside.
                if let Mark::Tagged { mcid, tag } = &mark {
                    let _ = writeln!(out, "/{tag} <</MCID {mcid}>> BDC");
                }
                if let Some(placed) = picture {
                    let _ = writeln!(
                        out,
                        "q {} 0 0 {} {} {} cm /TPTImage{} Do Q",
                        fmt(placed.width),
                        fmt(placed.height),
                        fmt(placed.x),
                        fmt(page_height - (placed.y + placed.height)),
                        placed.image,
                    );
                }
                if matches!(mark, Mark::Tagged { .. }) {
                    out.push_str(
                        "EMC
",
                    );
                }
            }
            (Item::Run(run), _) => {
                match &mark {
                    Mark::None => {}
                    Mark::Artifact => out.push_str("/Artifact BMC\n"),
                    Mark::Tagged { mcid, tag } => {
                        let _ = writeln!(out, "/{tag} <</MCID {mcid}>> BDC");
                    }
                }
                write_run(&mut out, run, page_height);
                if mark != Mark::None {
                    out.push_str("EMC\n");
                }
            }
        }
    }
    out.push_str("Q\n");
    out
}

/// The invisible text of one run: each glyph in a text object of its own, at its position.
fn write_run(out: &mut String, run: &TextRun, page_height: f32) {
    for g in &run.glyphs {
        let _ = writeln!(
            out,
            "BT 3 Tr /TPTText{face} {size} Tf 1 0 0 1 {x} {y} Tm <{glyph:04X}> Tj ET",
            face = run.face,
            size = fmt(run.size),
            x = fmt(g.x),
            y = fmt(page_height - g.y),
            glyph = g.glyph,
        );
    }
}

/// The objects of the structure tree: the root, the document element, the parent tree, and one
/// object per element, numbered from `base` in the order the tree gives them.
fn structure_objects(
    tree: &Tree,
    base: u64,
    page_count: usize,
    page_object: impl Fn(usize) -> u64,
) -> Vec<(u64, Vec<u8>)> {
    let root = base;
    let document = base + 1;
    let parent_tree = base + 2;
    let parents = tree.parents();
    let reference = |n: u64| format!("{n} 0 R");
    let mut out = Vec::new();
    out.push((
        root,
        format!(
            "<< /Type /StructTreeRoot /K [{}] /ParentTree {} /ParentTreeNextKey {page_count} >>",
            reference(document),
            reference(parent_tree)
        )
        .into_bytes(),
    ));
    let tops: Vec<String> = tree
        .top
        .iter()
        .map(|&i| reference(tree.nodes[i].number))
        .collect();
    out.push((
        document,
        format!(
            "<< /Type /StructElem /S /Document /P {} /K [{}] >>",
            reference(root),
            tops.join(" ")
        )
        .into_bytes(),
    ));
    // The parent tree maps each page's identifiers to the elements that hold them.
    let mut nums = String::new();
    for (page, owners) in tree.owners.iter().enumerate() {
        let entries: Vec<String> = owners
            .iter()
            .map(|&i| reference(tree.nodes[i].number))
            .collect();
        let _ = write!(nums, " {page} [{}]", entries.join(" "));
    }
    out.push((
        parent_tree,
        format!("<< /Nums [{} ] >>", nums.trim_start()).into_bytes(),
    ));
    for (index, node) in tree.nodes.iter().enumerate() {
        let parent = match parents[index] {
            Some(p) => tree.nodes[p].number,
            None => document,
        };
        let kids = if node.children.is_empty() {
            let ids: Vec<String> = node.mcids.iter().map(u32::to_string).collect();
            format!("[{}]", ids.join(" "))
        } else {
            let refs: Vec<String> = node
                .children
                .iter()
                .map(|&c| reference(tree.nodes[c].number))
                .collect();
            format!("[{}]", refs.join(" "))
        };
        let alt = node
            .alt
            .as_deref()
            .map(|a| format!(" /Alt {}", text_string(a)))
            .unwrap_or_default();
        out.push((
            node.number,
            format!(
                "<< /Type /StructElem /S /{} /P {} /Pg {} 0 R{alt} /K {kids} >>",
                node.kind,
                reference(parent),
                page_object(node.page)
            )
            .into_bytes(),
        ));
    }
    out
}

/// The information dictionary: the title and the author, as text strings.
fn info_dictionary(info: &DocInfo) -> String {
    let mut entries = String::new();
    if let Some(title) = &info.title {
        let _ = write!(entries, " /Title {}", text_string(title));
    }
    if let Some(author) = &info.author {
        let _ = write!(entries, " /Author {}", text_string(author));
    }
    format!("<<{entries} >>")
}

/// A PDF text string: UTF-16 with a byte-order mark, in hexadecimal, so any text is safe to write.
fn text_string(text: &str) -> String {
    let mut out = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out.push('>');
    out
}

/// A ToUnicode map from glyph ids to the characters they stand for.
fn to_unicode(characters: &BTreeMap<u32, String>) -> String {
    let entries: Vec<(&u32, &String)> = characters.iter().collect();
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for chunk in entries.chunks(100) {
        let _ = writeln!(out, "{} beginbfchar", chunk.len());
        for (glyph, text) in chunk {
            let units: String = text.encode_utf16().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(out, "<{glyph:04X}> <{units}>");
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out
}

/// A stream object: its dictionary with the length and `extra` entries, then the data.
fn stream_bytes(extra: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< /Length {} {extra} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

/// Writes the new objects after the original bytes, with a cross-reference section for them and a
/// trailer that points back at the original one. `info` is the information dictionary's number,
/// when there is one.
fn append(
    pdf: &[u8],
    objects: &[(u64, Vec<u8>)],
    size: u64,
    previous_xref: usize,
    info: Option<u64>,
) -> Vec<u8> {
    let mut out = pdf.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut offsets: BTreeMap<u64, usize> = BTreeMap::new();
    for (number, body) in objects {
        offsets.insert(*number, out.len());
        out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n");
    // Consecutive object numbers share one subsection.
    let numbers: Vec<u64> = offsets.keys().copied().collect();
    let mut start = 0;
    while start < numbers.len() {
        let mut end = start;
        while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
            end += 1;
        }
        out.extend_from_slice(format!("{} {}\n", numbers[start], end - start + 1).as_bytes());
        for number in &numbers[start..=end] {
            out.extend_from_slice(format!("{:010} 00000 n \n", offsets[number]).as_bytes());
        }
        start = end + 1;
    }
    let info_entry = info.map(|n| format!(" /Info {n} 0 R")).unwrap_or_default();
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {size} /Root 1 0 R{info_entry} /Prev {previous_xref} >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    out
}

/// The offset in the `startxref` line at the end of the file.
fn start_xref(pdf: &[u8]) -> Result<usize, TextLayerError> {
    let at = rfind(pdf, b"startxref").ok_or_else(|| TextLayerError("no startxref".into()))?;
    number_at(pdf, at + b"startxref".len())
        .ok_or_else(|| TextLayerError("no offset after startxref".into()))
}

/// The `/Size` of the last trailer: the number of object numbers the file uses.
fn declared_size(pdf: &[u8]) -> Result<u64, TextLayerError> {
    let trailer = rfind(pdf, b"trailer").ok_or_else(|| TextLayerError("no trailer".into()))?;
    let at = find_from(pdf, trailer, b"/Size")
        .ok_or_else(|| TextLayerError("no /Size in the trailer".into()))?;
    number_at(pdf, at + b"/Size".len())
        .map(|n| n as u64)
        .ok_or_else(|| TextLayerError("no number after /Size".into()))
}

/// The text of object `number`: what lies between `N 0 obj` and `endobj`, trimmed.
fn object_text(pdf: &[u8], number: u64) -> Result<String, TextLayerError> {
    let header = format!("{number} 0 obj");
    let mut from = 0;
    while let Some(at) = find_from(pdf, from, header.as_bytes()) {
        // An object header starts a line: the number is not the end of a longer one.
        if at == 0 || pdf[at - 1] == b'\n' {
            let body_start = at + header.len();
            let body_end = find_from(pdf, body_start, b"endobj")
                .ok_or_else(|| TextLayerError(format!("object {number} has no endobj")))?;
            let text = std::str::from_utf8(&pdf[body_start..body_end])
                .map_err(|_| TextLayerError(format!("object {number} is not text")))?;
            return Ok(text.trim().to_string());
        }
        from = at + 1;
    }
    fail(format!("object {number} is missing"))
}

fn find_from(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

/// The unsigned integer that starts at or after `from`, skipping white space.
fn number_at(pdf: &[u8], from: usize) -> Option<usize> {
    let rest = pdf.get(from..)?;
    let skip = rest.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let digits: Vec<u8> = rest[skip..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .copied()
        .collect();
    std::str::from_utf8(&digits).ok()?.parse().ok()
}

/// A number for a PDF content stream: up to three decimals, no exponent.
fn fmt(value: f32) -> String {
    let text = format!("{value:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-0" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The image objects of one picture: the image, and its alpha channel when it has one. The PDF
/// reads a JPEG with its own filter, and a PNG's rows with the predictor that undoes its filters.
fn image_objects(image: &Image, number: u64, alpha: Option<u64>) -> Vec<(u64, Vec<u8>)> {
    let space = if image.colors == 1 {
        "DeviceGray"
    } else {
        "DeviceRGB"
    };
    let mut dict = format!(
        "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /{space} /BitsPerComponent {}",
        image.width, image.height, image.bits
    );
    let data: &[u8] = match &image.data {
        ImageData::Jpeg(bytes) => {
            dict.push_str(" /Filter /DCTDecode");
            bytes.as_slice()
        }
        ImageData::PngRows(bytes) => {
            let _ = write!(
                dict,
                " /Filter /FlateDecode /DecodeParms << /Predictor 15 /Colors {} /BitsPerComponent {} /Columns {} >>",
                image.colors, image.bits, image.width
            );
            bytes.as_slice()
        }
        ImageData::Samples(bytes) => {
            dict.push_str(" /Filter /FlateDecode");
            bytes.as_slice()
        }
    };
    if let Some(alpha) = alpha {
        let _ = write!(dict, " /SMask {alpha} 0 R");
    }
    let mut out = vec![(number, stream_bytes(&dict, data))];
    if let (Some(alpha), Some(samples)) = (alpha, image.alpha.as_ref()) {
        let mask = format!(
            "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode",
            image.width, image.height
        );
        out.push((alpha, stream_bytes(&mask, samples)));
    }
    out
}
