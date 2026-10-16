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

    /// Translates a text (codes written in square brackets, as the editor shows them), giving the result in the
    /// same form: codes in brackets, a line break after each new-line and paragraph code.
    pub fn translate(&self, text: &str) -> Result<String, JsError> {
        self.0.translate(&Shown::marked(text).to_running()).map(|out| Shown::from_running(&out).text).map_err(error)
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

    /// The document's embosser page layout as the file keeps it (spec 25 section 10), as JSON
    /// {"layout": {form, top_margin, binding, interpoint, banner, interline}, "cells", "lines"}; empty when the file
    /// holds none.
    #[wasm_bindgen(getter)]
    pub fn embosser_layout(&self) -> String {
        let Some(l) = &self.0.layout else { return String::new() };
        let layout = Layout {
            form: l.form.clone(),
            top_margin: u32::from(l.top_margin),
            binding: u32::from(l.binding),
            interpoint: l.interpoint,
            banner: l.banner != 0,
            interline: u32::from(l.interline),
        };
        serde_json::json!({"layout": layout, "cells": l.cells, "lines": l.lines}).to_string()
    }

    /// As `save`, with the document's embosser setup name and page layout (JSON as `embosser_layout` gives it, the
    /// layout written only when it names a form), as the desktop program saves them.
    pub fn save_with(&self, text: &str, embosser: &str, layout: &str, cells: u32, lines: u32) -> Result<Vec<u8>, JsError> {
        let mut doc = DxDocument { text: Shown::marked(text).to_running(), embosser: embosser.to_string(), ..self.0.clone() };
        if !layout.is_empty() {
            let l: Layout = serde_json::from_str(layout).map_err(error)?;
            if !l.form.is_empty() {
                let short = |n: u32| n.min(u32::from(u16::MAX)) as u16;
                doc.layout = Some(dxfile::DxLayout {
                    lines: short(lines),
                    top_margin: short(l.top_margin),
                    cells: short(cells),
                    binding: short(l.binding),
                    interpoint: l.interpoint,
                    banner: u8::from(l.banner),
                    interline: l.interline.min(2) as u8,
                    unknown: self.0.settings.get(108 + 11).copied().unwrap_or(0),
                    form: l.form,
                });
            }
        }
        Ok(dxfile::write(&doc))
    }
}

/// The page (from 1) on which a character position of a document laid out `cells` by `lines` falls.
#[wasm_bindgen]
pub fn page_of(text: &str, cells: u32, lines: u32, braille: bool, position: usize) -> usize {
    bm_core::format::locate(&laid_out(text, cells, lines, braille), position).0
}

/// The lines of a document's chosen pages (indexes, -1 for a blank page; empty for every page), as JSON
/// [[line, ...] or null for a blank page], for File Print.
#[wasm_bindgen]
pub fn page_lines(text: &str, cells: u32, lines: u32, braille: bool, pages: Vec<i32>) -> String {
    let laid = laid_out(text, cells, lines, braille);
    let side = |p: &bm_core::format::Page| serde_json::json!(p.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>());
    let out: Vec<serde_json::Value> = if pages.is_empty() {
        laid.iter().map(side).collect()
    } else {
        pages.iter().map(|&i| usize::try_from(i).ok().and_then(|i| laid.get(i)).map_or(serde_json::Value::Null, side)).collect()
    };
    serde_json::Value::Array(out).to_string()
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

/// A document's pages: a braille document in braille pages, a print document in print pages (as the desktop
/// program lays them out for File Print and File Emboss).
fn laid_out(text: &str, cells: u32, lines: u32, braille: bool) -> Vec<bm_core::format::Page> {
    let (c, l) = (cells.max(1) as usize, lines.max(1) as usize);
    let layout = if braille { bm_core::format::Layout::braille(c, l) } else { bm_core::format::Layout::print(c, l) };
    bm_core::format::format(&Shown::marked(text), &layout, None)
}

/// How many pages a document takes laid out `cells` by `lines`.
#[wasm_bindgen]
pub fn page_count(text: &str, cells: u32, lines: u32, braille: bool) -> usize {
    laid_out(text, cells, lines, braille).len()
}

/// A page range as the Print and Emboss dialogs take it ("1-7,b,9"), for a document of `page_count` pages, as JSON:
/// {"kind": "pages" | "malformed" | "empty" | "none", "pages": [index or -1 for a blank page], "missing", "repeated"}.
#[wasm_bindgen]
pub fn parse_range(text: &str, page_count: usize) -> String {
    use bm_core::range::Range;
    let (range, missing, repeated) = bm_core::range::parse_range(text, page_count);
    let (kind, pages) = match range {
        Range::Pages(p) => ("pages", p.iter().map(|x| x.map_or(-1, |n| n as i64)).collect()),
        Range::Malformed => ("malformed", Vec::new()),
        Range::Empty => ("empty", Vec::new()),
        Range::NoneFound => ("none", Vec::new()),
    };
    serde_json::json!({"kind": kind, "pages": pages, "missing": missing, "repeated": repeated}).to_string()
}

/// The embosser models (embossers.txt, and the user's own models if any), for Embosser Setup and File Emboss.
#[wasm_bindgen]
pub struct Embossers(bm_core::embossers::Catalog);

#[wasm_bindgen]
impl Embossers {
    #[wasm_bindgen(constructor)]
    pub fn new(shipped: &str, user: Option<String>) -> Result<Embossers, JsError> {
        bm_core::embossers::Catalog::from_texts(shipped, user.as_deref()).map(Embossers).map_err(error)
    }

    /// Every model as the setup lists it, sort-to-top models first.
    pub fn names(&self) -> Vec<String> {
        self.0.names()
    }

    /// A new setup of a listed model with the model's own values, as JSON (the shape the desktop program keeps).
    pub fn new_setup(&self, label: &str, european: bool) -> String {
        serde_json::to_string(&Setup::for_model(&self.0, label, european)).unwrap_or_default()
    }

    /// The forms of a listed model as JSON: [{"id", "name", "cells", "lines"}], cells and lines being the most the
    /// form takes.
    pub fn forms(&self, label: &str) -> String {
        let forms: Vec<_> = find_model(&self.0, label)
            .map(|m| m.forms.iter().map(|f| serde_json::json!({"id": f.id, "name": form_name(&f.id), "cells": f.maximum.0, "lines": f.maximum.1})).collect())
            .unwrap_or_default();
        serde_json::Value::Array(forms).to_string()
    }

    /// A model's driver name (Generic, Index, Enabling and the like).
    pub fn driver(&self, label: &str) -> String {
        find_model(&self.0, label).map(|m| m.driver().to_string()).unwrap_or_default()
    }

    /// Whether a model has a flag (CanDoInterpoint, TranSend and the like).
    pub fn flag(&self, label: &str, key: &str) -> bool {
        find_model(&self.0, label).is_some_and(|m| m.flag(key))
    }

    /// The output encodings a model offers.
    pub fn encodings(&self, label: &str) -> Vec<String> {
        find_model(&self.0, label).map(|m| m.encodings().iter().map(|s| s.to_string()).collect()).unwrap_or_default()
    }

    /// The checks before an embossing job (spec 25 9.1) for a setup and a document layout given as JSON: "" when the
    /// job may go ahead, else "stop:" or "ask:" and the message (2364 stops the job, 2362 is a Yes/No question).
    pub fn check(&self, setup: &str, layout: &str, cells: u32, lines: u32) -> Result<String, JsError> {
        use bm_core::emboss_setup::{check, does_not_fit_message, form_not_defined_message, Check};
        let (s, l) = parse(setup, layout)?;
        let Some(model) = find_model(&self.0, &s.model) else { return Ok("stop:Embosser not found.".into()) };
        let (setup_name, form) = (s.shown_name(), form_name(&l.form));
        Ok(match check(model, &l, cells, lines) {
            Check::Ready => String::new(),
            Check::FormNotDefined => format!("stop:{}", form_not_defined_message(&setup_name, &form, cells, lines)),
            Check::DoesNotFit => format!("ask:{}", does_not_fit_message(&setup_name, &form, cells, lines)),
        })
    }

    /// The bytes of an embossing job: a document (`braille` or print) laid out `cells` by `lines`, the chosen pages (indexes, -1 for a
    /// blank page; empty for every page), `copies` times, through a setup and layout given as JSON. `encoding` is the
    /// byte for each of the 64 cells in the setup's output encoding, or empty for North American English.
    #[allow(clippy::too_many_arguments)]
    pub fn job(&self, setup: &str, layout: &str, text: &str, braille: bool, cells: u32, lines: u32, copies: u32, pages: Vec<i32>, encoding: Vec<u8>) -> Result<Vec<u8>, JsError> {
        let (s, l) = parse(setup, layout)?;
        let model = find_model(&self.0, &s.model).ok_or_else(|| JsError::new("Embosser not found."))?;
        let laid = laid_out(text, cells, lines, braille);
        let side = |p: &bm_core::format::Page| p.lines.iter().map(|l| l.text.clone()).collect::<Vec<String>>();
        let sides: Vec<Option<Vec<String>>> = if pages.is_empty() {
            laid.iter().map(|p| Some(side(p))).collect()
        } else {
            pages.iter().map(|&i| usize::try_from(i).ok().and_then(|i| laid.get(i)).map(side)).collect()
        };
        let encoding = <[u8; 64]>::try_from(encoding.as_slice()).ok();
        let options = bm_core::emboss_setup::job_options(&s, model, &l, cells, lines, copies, encoding);
        Ok(bm_core::emboss::job(&options, &sides, copies))
    }
}

use bm_core::emboss_setup::{find_model, form_name, Layout, Setup};

fn parse(setup: &str, layout: &str) -> Result<(Setup, Layout), JsError> {
    Ok((serde_json::from_str(setup).map_err(error)?, serde_json::from_str(layout).map_err(error)?))
}

/// Message 2892: no embosser setup chosen for the document.
#[wasm_bindgen]
pub fn no_setup_message() -> String {
    bm_core::emboss_setup::NO_SETUP_MESSAGE.to_string()
}

/// A form's description as the setup lists it.
#[wasm_bindgen(js_name = formName)]
pub fn form_name_of(id: &str) -> String {
    form_name(id)
}

/// The choice lists of the embosser setup sheet, as JSON: ports, bauds, parities, handshaking, versions, line ends,
/// page ends and letter cases.
#[wasm_bindgen]
pub fn setup_lists() -> String {
    use bm_core::emboss_setup::*;
    serde_json::json!({
        "ports": PORTS, "bauds": BAUDS, "parities": PARITIES, "handshaking": HANDSHAKING, "versions": VERSIONS,
        "lineEnds": LINE_ENDS, "pageEnds": PAGE_ENDS, "cases": CASES,
    })
    .to_string()
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
            layout: None,
        };
        let read = read_document(&dxfile::write(&doc)).ok().unwrap();
        assert!(read.braille());
        assert_eq!((read.text(), read.embosser()), (",! cat".to_string(), "Index Everest-D V5".to_string()));
        let again = read_document(&read.save(",! dog")).ok().unwrap();
        assert_eq!(again.text(), ",! dog");
    }

    #[test]
    fn translation_gives_the_editor_form() {
        let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tables/en-ueb-g2-forward.bmt")).unwrap();
        let e = Engine::new(&source).ok().unwrap();
        let out = e.translate("The cat.\nThe mat.").ok().unwrap();
        assert!(!out.chars().any(|c| ('\u{1c}'..='\u{1f}').contains(&c)), "{out:?}");
        assert_eq!(out.lines().count(), 2, "{out:?}");
    }

    #[test]
    fn an_index_setup_checks_and_embosses_the_chosen_pages() {
        let shipped = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../embossers/embossers.txt")).unwrap();
        let emb = Embossers::new(&shipped, None).ok().unwrap();
        assert!(emb.names().iter().any(|n| n == "Index Everest-D V5"));
        let setup = emb.new_setup("Index Everest-D V5", false);
        let layout = r#"{"form":"11_w","top_margin":0,"binding":0,"interpoint":false,"banner":false,"interline":0}"#;
        assert_eq!(emb.check(&setup, layout, 40, 25).ok().unwrap(), "");
        assert!(emb.check(&setup, layout, 60, 25).ok().unwrap().starts_with("ask:"));
        let range: serde_json::Value = serde_json::from_str(&parse_range("2,b", 2)).unwrap();
        assert_eq!(range["pages"], serde_json::json!([1, -1]));
        let text = "ab[l]cd[l]ef";
        assert_eq!(page_count(text, 40, 2, true), 2);
        // Positions are the editor's: a line break follows each [l], so "e" is at 12.
        let shown = "ab[l]\ncd[l]\nef";
        assert_eq!(page_of(shown, 40, 2, true, 12), 2);
        assert_eq!(page_of(shown, 40, 2, true, 1), 1);
        let job = emb.job(&setup, layout, text, true, 40, 2, 1, vec![1], Vec::new()).ok().unwrap();
        assert!(job.starts_with(b"\x1bD"), "{:?}", String::from_utf8_lossy(&job));
        assert!(String::from_utf8_lossy(&job).contains("ef"));
        assert!(!String::from_utf8_lossy(&job).contains("ab"));
    }
}
