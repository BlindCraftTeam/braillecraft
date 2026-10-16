//! BrailleMaker in the browser (queue line 11): the core's translation, document files, braille page layout and
//! embossing, called from the web interface. The browser has no folder of tables to open, so the page fetches the
//! shipped table sources and hands them in as text.

use bm_core::doctext::Shown;
use bm_core::dxfile::{self, DxDocument};
use bm_core::engine::Translator;
use bm_core::DocumentKind;
use wasm_bindgen::prelude::*;

fn error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// A translator made from one table source (a shipped .bmt file's text).
#[wasm_bindgen]
pub struct Engine(Translator);

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<Engine, JsError> {
        Translator::from_source(source).map(Engine).map_err(error)
    }

    /// Translates a text (codes written in square brackets, as the editor shows them).
    pub fn translate(&self, text: &str) -> Result<String, JsError> {
        self.0.translate(&Shown::marked(text).to_running()).map_err(error)
    }
}

/// A print or braille document file (.dxp, .dxb), as the web interface shows it.
#[wasm_bindgen]
pub struct Document(DxDocument);

#[wasm_bindgen]
impl Document {
    /// True for a braille document, false for a print document.
    #[wasm_bindgen(getter)]
    pub fn braille(&self) -> bool {
        self.0.kind == DocumentKind::Braille
    }

    /// The text as the editor shows it: codes in square brackets.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        Shown::from_running(&self.0.text).text
    }

    /// The four table names: print to braille, its scan table, braille to print, its scan table.
    #[wasm_bindgen(getter)]
    pub fn tables(&self) -> Vec<String> {
        self.0.tables.to_vec()
    }

    /// The name of the document's embosser setup, empty for none.
    #[wasm_bindgen(getter)]
    pub fn embosser(&self) -> String {
        self.0.embosser.clone()
    }

    /// The file's bytes with `text` (editor form) in place of the document's text; everything else as read.
    pub fn save(&self, text: &str) -> Vec<u8> {
        let doc = DxDocument { text: Shown::marked(text).to_running(), ..self.0.clone() };
        dxfile::write(&doc)
    }
}

/// Reads a .dxp or .dxb file.
#[wasm_bindgen]
pub fn read_document(bytes: &[u8]) -> Result<Document, JsError> {
    dxfile::read(bytes).map(Document).map_err(error)
}

/// Braille text (North American ASCII braille, codes in square brackets) laid out in pages of `cells` by `lines`:
/// the lines of each page with CR LF, a form feed after every page (the formatted text of a braille file).
#[wasm_bindgen]
pub fn layout_braille(text: &str, cells: usize, lines: usize) -> String {
    let pages = bm_core::format::format(&Shown::marked(text), &bm_core::format::Layout::braille(cells, lines), None);
    bm_core::format::formatted_text(&pages)
}

/// The bytes of an embossing job on the plain (Generic) driver: braille text laid out in pages of `cells` by
/// `lines`, `copies` times, as Write to File gives them.
#[wasm_bindgen]
pub fn emboss_job(text: &str, cells: u32, lines: u32, copies: u32) -> Vec<u8> {
    let pages = bm_core::format::format(&Shown::marked(text), &bm_core::format::Layout::braille(cells as usize, lines as usize), None);
    let sides: Vec<Option<Vec<String>>> = pages.iter().map(|p| Some(p.lines.iter().map(|l| l.text.clone()).collect())).collect();
    let options = bm_core::emboss::Options { cells, lines, ..bm_core::emboss::Options::default() };
    bm_core::emboss::job(&options, &sides, copies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_emboss_on_two_pages() {
        // Three lines on pages of two lines: two pages, each ended by a form feed.
        let text = "ab[l]cd[l]ef";
        let laid = layout_braille(text, 10, 2);
        assert_eq!(laid.matches('\x0c').count(), 2);
        let job = emboss_job(text, 10, 2, 1);
        assert_eq!(job.iter().filter(|&&b| b == 0x0c).count(), 2);
        assert!(job.starts_with(b"ab\r\n"));
    }

    #[test]
    fn a_document_reads_and_saves() {
        let doc = DxDocument {
            kind: DocumentKind::Braille,
            version: 1,
            page_numbering: [0; 5],
            translated_line: false,
            six_key: false,
            tables: ["amerc".into(), "AMEDXP".into(), "AMEBPC".into(), "AMEDXB".into()],
            text: ",! cat".into(),
            styles: Vec::new(),
            style_index: Vec::new(),
            settings: Vec::new(),
            embosser: "Index Everest-D V5".into(),
            sections: Vec::new(),
        };
        let read = read_document(&dxfile::write(&doc)).ok().unwrap();
        assert!(read.braille());
        assert_eq!((read.text(), read.embosser()), (",! cat".to_string(), "Index Everest-D V5".to_string()));
        let again = read_document(&read.save(",! dog")).ok().unwrap();
        assert_eq!(again.text(), ",! dog");
    }
}
