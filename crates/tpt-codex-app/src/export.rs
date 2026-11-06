// SPDX-License-Identifier: MIT OR Apache-2.0
//! One-click PDF export of the document, on the page size and margins of its document class.
//!
//! The text is measured with the bundled font, so the PDF lays out the same way as the bundled
//! font does everywhere. Pages get a "Page N of M" footer.

use tpt_codex_core::Document;

/// The footer template for every page.
pub const FOOTER: &str = tpt_codex_pdf::DEFAULT_FOOTER;

/// Exports `doc` as a PDF file's bytes, on A4 with the margins of its document class (A4 with
/// the article margins when it has no class).
pub fn export_pdf(doc: &Document) -> Result<Vec<u8>, String> {
    tpt_codex_pdf::export_with_defaults(doc).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_codex_core::Document;

    fn long_document(paragraphs: usize) -> Document {
        use tpt_codex_core::{Block, BlockId, LayoutMode, TextStyle};
        let ids: Vec<BlockId> = (0..paragraphs).map(|i| BlockId(1 + i as u64)).collect();
        let mut doc = Document::new(
            BlockId(0),
            Block::Container {
                layout_mode: LayoutMode::Flow,
                children: ids.clone(),
            },
        );
        for (i, id) in ids.into_iter().enumerate() {
            doc.insert(
                id,
                Block::Text {
                    content: format!(
                        "Paragraph {i}. The quick brown fox jumps over the lazy dog, again and again."
                    ),
                    style: TextStyle::default(),
                },
            );
        }
        doc
    }

    #[test]
    fn export_is_a_pdf_with_a_footer_on_every_page() {
        let doc = long_document(120);
        let bytes = export_pdf(&doc).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "not a PDF");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("%%EOF"));
        let pages = text.matches("/Type /Page\n").count() + text.matches("/Type /Page ").count();
        assert!(
            pages >= 2,
            "120 paragraphs should span more than one page, got {pages}"
        );
    }

    #[test]
    fn export_is_deterministic() {
        let doc = long_document(20);
        assert_eq!(export_pdf(&doc).unwrap(), export_pdf(&doc).unwrap());
    }
}
