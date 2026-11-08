// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF import on files built here, one feature at a time: text, fonts with and without character
//! maps, compressed streams, forms, images, incremental updates, object streams, damaged tables
//! and encryption.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use tpt_codex_core::{Block, Document};
use tpt_codex_interop::{Fidelity, PdfError, import_pdf};

/// Builds a PDF file, object by object, and writes a classic cross-reference table at the end.
struct Pdf {
    body: Vec<u8>,
    /// The offset of each object, by its number.
    offsets: Vec<(u32, usize)>,
}

impl Pdf {
    fn new() -> Self {
        Self {
            body: b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec(),
            offsets: Vec::new(),
        }
    }

    fn offset_of(&self, number: u32) -> Option<usize> {
        self.offsets
            .iter()
            .find(|(n, _)| *n == number)
            .map(|(_, at)| *at)
    }

    fn object(&mut self, number: u32, content: &[u8]) {
        self.offsets.push((number, self.body.len()));
        self.body
            .extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        self.body.extend_from_slice(content);
        self.body.extend_from_slice(b"\nendobj\n");
    }

    fn stream(&mut self, number: u32, dict: &str, data: &[u8]) {
        let mut content = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
        content.extend_from_slice(data);
        content.extend_from_slice(b"\nendstream");
        self.object(number, &content);
    }

    /// The file's end: a cross-reference table over every object, and the trailer.
    fn finish(mut self, trailer: &str) -> Vec<u8> {
        let size = self.offsets.iter().map(|(n, _)| n + 1).max().unwrap_or(1);
        let xref_at = self.body.len();
        self.body
            .extend_from_slice(format!("xref\n0 {size}\n").as_bytes());
        self.body.extend_from_slice(b"0000000000 65535 f \n");
        for number in 1..size {
            match self.offset_of(number) {
                Some(at) => self
                    .body
                    .extend_from_slice(format!("{at:010} 00000 n \n").as_bytes()),
                None => self.body.extend_from_slice(b"0000000000 65535 f \n"),
            }
        }
        self.body.extend_from_slice(
            format!("trailer\n<< /Size {size} {trailer} >>\nstartxref\n{xref_at}\n%%EOF\n")
                .as_bytes(),
        );
        self.body
    }
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// The blocks of the document in order: each one's heading level and text.
fn blocks(doc: &Document) -> Vec<(Option<u8>, String)> {
    let Some(Block::Container { children, .. }) = doc.get(doc.root) else {
        panic!("the root is not a container");
    };
    children
        .iter()
        .map(|id| match doc.get(*id) {
            Some(Block::Text { content, style }) => (style.heading_level, content.clone()),
            other => panic!("expected text, got {other:?}"),
        })
        .collect()
}

/// The text the tests use: a heading, two lines of one paragraph, and a second paragraph.
const TEXT: &[u8] = b"BT /F1 24 Tf 72 740 Td (Heading) Tj ET\n\
BT /F1 12 Tf 72 700 Td (Hello world) Tj ET\n\
BT /F1 12 Tf 72 686 Td (Second line) Tj ET\n\
BT /F1 12 Tf 72 600 Td (A new paragraph) Tj ET\n";

fn expected_text() -> Vec<(Option<u8>, String)> {
    vec![
        (Some(1), "Heading".to_string()),
        (None, "Hello world Second line".to_string()),
        (None, "A new paragraph".to_string()),
    ]
}

/// A one-page file. Object 1 is the catalog, 2 the page tree, 3 the page, 4 the content, and 5 a
/// Helvetica font with no character map. `extra` adds objects and resources.
fn one_page(content: &[u8], resources: &str, extra: impl FnOnce(&mut Pdf)) -> Pdf {
    let mut pdf = Pdf::new();
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    pdf.object(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.object(
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> {resources} >> >>"
        )
        .as_bytes(),
    );
    pdf.stream(4, "", content);
    pdf.object(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    extra(&mut pdf);
    pdf
}

#[test]
fn text_becomes_paragraphs_and_the_large_type_a_heading() {
    let bytes = one_page(TEXT, "", |_| {}).finish("/Root 1 0 R");
    let import = import_pdf(&bytes).unwrap();
    assert_eq!(blocks(&import.document), expected_text());
    // Helvetica has no character map, so its text is a guess, and the report says so.
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "fonts" && i.outcome == Fidelity::Approximated),
        "{:#?}",
        import.report.items()
    );
}

#[test]
fn a_compressed_content_stream_reads_the_same() {
    let mut pdf = Pdf::new();
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    pdf.object(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.object(
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>",
    );
    pdf.stream(4, "/Filter /FlateDecode", &zlib(TEXT));
    pdf.object(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    let bytes = pdf.finish("/Root 1 0 R");
    assert_eq!(
        blocks(&import_pdf(&bytes).unwrap().document),
        expected_text()
    );
}

#[test]
fn a_font_with_a_to_unicode_map_gives_its_characters() {
    // Codes 1 to 26 are a to z, so 0x0008, 0x0009 and 0x000C are h, i and l.
    let cmap = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
        1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
        1 beginbfrange\n<0001> <001A> <0061>\nendbfrange\nendcmap\n";
    let content = b"BT /F2 12 Tf 72 700 Td <0008000900 0C> Tj ET\n";
    let mut pdf = Pdf::new();
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    pdf.object(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.object(
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
          /Resources << /Font << /F2 5 0 R >> >> >>",
    );
    pdf.stream(4, "", content);
    pdf.object(
        5,
        b"<< /Type /Font /Subtype /Type0 /BaseFont /Test /Encoding /Identity-H \
          /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>",
    );
    pdf.object(
        6,
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /DW 1000 >>",
    );
    pdf.stream(7, "", cmap);
    let import = import_pdf(&pdf.finish("/Root 1 0 R")).unwrap();
    assert_eq!(blocks(&import.document), vec![(None, "hil".to_string())]);
    // The map gives the characters, so nothing is a guess.
    assert!(!import.report.items().iter().any(|i| i.feature == "fonts"));
}

#[test]
fn an_image_and_a_drawing_are_counted_and_not_imported() {
    let content = b"0 0 m 100 100 l S\n\
        q 100 0 0 100 72 600 cm /Im1 Do Q\n\
        BT /F1 12 Tf 72 500 Td (Caption) Tj ET\n";
    let bytes = one_page(content, "/XObject << /Im1 6 0 R >>", |pdf| {
        pdf.stream(
            6,
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray",
            &[0],
        )
    })
    .finish("/Root 1 0 R");
    let import = import_pdf(&bytes).unwrap();
    assert_eq!(
        blocks(&import.document),
        vec![(None, "Caption".to_string())]
    );
    let items = import.report.items();
    assert!(
        items
            .iter()
            .any(|i| i.feature == "images" && i.outcome == Fidelity::Dropped)
    );
    assert!(
        items
            .iter()
            .any(|i| i.feature == "drawings" && i.outcome == Fidelity::Dropped)
    );
}

#[test]
fn text_in_a_form_is_read_in_place() {
    let bytes = one_page(b"/Fm1 Do\n", "/XObject << /Fm1 6 0 R >>", |pdf| {
        pdf.stream(
            6,
            "/Type /XObject /Subtype /Form /BBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >>",
            b"BT /F1 12 Tf 72 500 Td (Inside the form) Tj ET",
        )
    })
    .finish("/Root 1 0 R");
    assert_eq!(
        blocks(&import_pdf(&bytes).unwrap().document),
        vec![(None, "Inside the form".to_string())]
    );
}

fn last_startxref(bytes: &[u8]) -> usize {
    let at = bytes
        .windows(9)
        .rposition(|w| w == b"startxref")
        .expect("a startxref");
    String::from_utf8_lossy(&bytes[at + 9..])
        .split_whitespace()
        .next()
        .expect("an offset")
        .parse()
        .expect("a number")
}

#[test]
fn a_later_update_replaces_the_objects_it_names() {
    // Revision 1 says "Old text". Revision 2 appends a new content stream and a new page object
    // that uses it, and the newest entries win.
    let first =
        one_page(b"BT /F1 12 Tf 72 700 Td (Old text) Tj ET\n", "", |_| {}).finish("/Root 1 0 R");
    let previous = last_startxref(&first);
    let mut bytes = first.clone();

    let content = b"BT /F1 12 Tf 72 700 Td (New text) Tj ET\n";
    let content_at = bytes.len();
    bytes.extend_from_slice(
        format!("6 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes(),
    );
    bytes.extend_from_slice(content);
    bytes.extend_from_slice(b"\nendstream\nendobj\n");

    let page_at = bytes.len();
    bytes.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 6 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    let xref_at = bytes.len();
    bytes.extend_from_slice(
        format!(
            "xref\n3 1\n{page_at:010} 00000 n \n6 1\n{content_at:010} 00000 n \n\
             trailer\n<< /Size 7 /Root 1 0 R /Prev {previous} >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    assert_eq!(
        blocks(&import_pdf(&bytes).unwrap().document),
        vec![(None, "New text".to_string())]
    );
}

#[test]
fn a_file_with_a_damaged_table_is_read_by_scanning_it() {
    let bytes = one_page(TEXT, "", |_| {}).finish("/Root 1 0 R");
    // Point the cross-reference table at nothing, past the end of the file.
    let at = bytes.windows(9).rposition(|w| w == b"startxref").unwrap();
    let mut broken = bytes[..at].to_vec();
    broken.extend_from_slice(b"startxref\n99999999\n%%EOF\n");
    assert_eq!(
        blocks(&import_pdf(&broken).unwrap().document),
        expected_text()
    );
}

#[test]
fn an_object_stream_and_a_cross_reference_stream_read_the_same_objects() {
    // The page tree, page and font are in one object stream (object 10). Object 4, the content,
    // is a plain object, and object 11 is the cross-reference stream.
    let members: Vec<(u32, &[u8])> = vec![
        (1, b"<< /Type /Catalog /Pages 2 0 R >>"),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (
            3,
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
              /Resources << /Font << /F1 5 0 R >> >> >>",
        ),
        (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ];
    let mut header = String::new();
    let mut data = Vec::new();
    for (number, object) in &members {
        header.push_str(&format!("{number} {} ", data.len()));
        data.extend_from_slice(object);
        data.push(b' ');
    }
    let mut objstm = header.clone().into_bytes();
    let first = objstm.len();
    objstm.extend_from_slice(&data);

    let mut pdf = Pdf::new();
    pdf.stream(
        10,
        &format!(
            "/Type /ObjStm /N {} /First {first} /Filter /FlateDecode",
            members.len()
        ),
        &zlib(&objstm),
    );
    pdf.stream(4, "/Filter /FlateDecode", &zlib(TEXT));

    // The cross-reference stream: W [1 4 2], one row for each object 0 to 11.
    let row = |kind: u8, a: u32, b: u16| {
        let mut r = vec![kind];
        r.extend_from_slice(&a.to_be_bytes());
        r.extend_from_slice(&b.to_be_bytes());
        r
    };
    let at_11 = pdf.body.len();
    let mut rows = row(0, 0, 0xFFFF);
    for number in 1..=11u32 {
        let entry = match number {
            1 | 2 | 3 | 5 => {
                let index = members.iter().position(|(n, _)| *n == number).unwrap();
                row(2, 10, index as u16)
            }
            4 => row(1, pdf.offset_of(4).unwrap() as u32, 0),
            10 => row(1, pdf.offset_of(10).unwrap() as u32, 0),
            11 => row(1, at_11 as u32, 0),
            _ => row(0, 0, 0),
        };
        rows.extend(entry);
    }
    pdf.stream(
        11,
        "/Type /XRef /Size 12 /W [1 4 2] /Index [0 12] /Root 1 0 R /Filter /FlateDecode",
        &zlib(&rows),
    );
    let mut bytes = pdf.body.clone();
    bytes.extend_from_slice(format!("startxref\n{at_11}\n%%EOF\n").as_bytes());
    assert_eq!(
        blocks(&import_pdf(&bytes).unwrap().document),
        expected_text()
    );
}

#[test]
fn an_encrypted_file_is_refused() {
    let bytes = one_page(TEXT, "", |_| {}).finish("/Root 1 0 R /Encrypt 9 0 R");
    assert_eq!(import_pdf(&bytes).unwrap_err(), PdfError::Encrypted);
}

#[test]
fn a_file_that_is_not_a_pdf_is_refused() {
    assert_eq!(import_pdf(b"hello").unwrap_err(), PdfError::NotPdf);
}

#[test]
fn list_items_are_paragraphs_of_their_own() {
    // Bullets are drawn as text on lines a line apart, so each item starts a paragraph.
    let content = "BT /F1 12 Tf 72 700 Td (- one) Tj ET\n\
                   BT /F1 12 Tf 72 686 Td (- two) Tj ET\n";
    let bytes = one_page(content.as_bytes(), "", |_| {}).finish("/Root 1 0 R");
    assert_eq!(
        blocks(&import_pdf(&bytes).unwrap().document),
        vec![(None, "- one".to_string()), (None, "- two".to_string()),]
    );
}

#[test]
fn bold_and_italic_text_is_reported_as_plain_text() {
    let mut pdf = Pdf::new();
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    pdf.object(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    pdf.object(
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R \
          /Resources << /Font << /F1 5 0 R >> >> >>",
    );
    pdf.stream(4, "", b"BT /F1 12 Tf 72 700 Td (Strong words) Tj ET\n");
    pdf.object(
        5,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+Helvetica-Bold >>",
    );
    let import = import_pdf(&pdf.finish("/Root 1 0 R")).unwrap();
    assert_eq!(
        blocks(&import.document),
        vec![(None, "Strong words".to_string())]
    );
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "emphasis")
    );
}

#[test]
fn damaged_files_are_read_or_refused_but_never_panic() {
    // Bytes changed and files cut short, from a fixed seed, so a failure can be run again.
    let base = one_page(TEXT, "", |_| {}).finish("/Root 1 0 R");
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        seed >> 33
    };
    for round in 0..3000u32 {
        let mut bytes = base.clone();
        for _ in 0..1 + next() % 6 {
            let at = next() as usize % bytes.len();
            bytes[at] = next() as u8;
        }
        if round % 5 == 0 {
            let cut = next() as usize % bytes.len();
            bytes.truncate(cut.max(1));
        }
        // Any result is fine; a panic fails the test.
        let _ = import_pdf(&bytes);
    }
}

const CHROMIUM: &[u8] = include_bytes!("../../../fixtures/pdf/chromium-print.pdf");

#[test]
fn a_pdf_from_another_program_reads_its_paragraphs() {
    let import = import_pdf(CHROMIUM).unwrap();
    let text = blocks(&import.document);
    assert!(
        text.contains(&(Some(1), "Quarterly report".to_string())),
        "{text:#?}"
    );
    assert!(
        text.iter()
            .any(|(_, t)| t.starts_with("The first paragraph has bold and italic words")),
        "{text:#?}"
    );
    assert!(
        text.contains(&(
            None,
            "A second paragraph, with ligatures: first, office, efficient, flow.".to_string()
        )),
        "{text:#?}"
    );
    // Its bold and italic words are read as plain text, and the report says so.
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "emphasis")
    );
}

#[test]
fn damaged_copies_of_a_pdf_from_another_program_never_panic() {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        seed >> 33
    };
    for round in 0..1500u32 {
        let mut bytes = CHROMIUM.to_vec();
        for _ in 0..1 + next() % 8 {
            let at = next() as usize % bytes.len();
            bytes[at] = next() as u8;
        }
        if round % 4 == 0 {
            let cut = next() as usize % bytes.len();
            bytes.truncate(cut.max(1));
        }
        let _ = import_pdf(&bytes);
    }
}

#[test]
fn a_page_number_in_the_footer_is_left_out_and_reported() {
    let content = b"BT /F1 12 Tf 72 700 Td (Body text) Tj ET\n\
                    BT /F1 9 Tf 280 30 Td (Page 1 of 1) Tj ET\n";
    let import = import_pdf(&one_page(content, "", |_| {}).finish("/Root 1 0 R")).unwrap();
    assert_eq!(
        blocks(&import.document),
        vec![(None, "Body text".to_string())]
    );
    assert!(
        import
            .report
            .items()
            .iter()
            .any(|i| i.feature == "page numbers")
    );
}

#[test]
fn a_scanned_page_with_only_an_image_imports_empty_and_says_why() {
    let bytes = one_page(b"q 595 0 0 842 0 0 cm /Im1 Do Q\n", "/XObject << /Im1 6 0 R >>", |pdf| {
        pdf.stream(
            6,
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray",
            &[0],
        )
    })
    .finish("/Root 1 0 R");
    let import = import_pdf(&bytes).unwrap();
    assert!(blocks(&import.document).is_empty());
    assert!(import.report.items().iter().any(|i| i.feature == "images"));
}
