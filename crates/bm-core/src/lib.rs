//! BrailleMaker core.
//!
//! Everything that is not user interface lives here: the document model, translation through the BrailleMaker
//! engine, document templates, braille page formatting, import and export, and the embosser job layer. The
//! desktop shell and the web build both call into this crate.

pub mod codepages;
pub mod doctext;
pub mod dxfile;
pub mod emboss;
pub mod emboss_out;
pub mod embossers;
pub mod engine;
pub mod format;
pub mod lbf;
pub mod rule_script;
pub mod table_list;
pub mod templates;

/// The two kinds of document. A print document holds ink text; a braille document holds braille cells.
/// The kind decides which translation direction applies and how some codes are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Print,
    Braille,
}

impl DocumentKind {
    /// The kind a translation produces from this kind.
    pub fn translated(self) -> DocumentKind {
        match self {
            DocumentKind::Print => DocumentKind::Braille,
            DocumentKind::Braille => DocumentKind::Print,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_flips_the_document_kind() {
        assert_eq!(DocumentKind::Print.translated(), DocumentKind::Braille);
        assert_eq!(DocumentKind::Braille.translated(), DocumentKind::Print);
    }
}
