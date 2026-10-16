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

    /// Whether the document's translated line shows, as the file keeps it.
    #[wasm_bindgen(getter)]
    pub fn translated_line(&self) -> bool {
        self.0.translated_line
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
        self.save_version(text, embosser, layout, cells, lines, 1)
    }

    /// As `save_with`, in file format `version`: 1, or 0 for the older format, which names the print-to-braille
    /// tables by their older names where they have them (spec 04d-C; a web document has no initial codes in force).
    pub fn save_version(&self, text: &str, embosser: &str, layout: &str, cells: u32, lines: u32, version: u16) -> Result<Vec<u8>, JsError> {
        let tables = if version == 0 { dxfile::older_table_names(&self.0.tables, |_| false) } else { self.0.tables.clone() };
        let mut doc = DxDocument { text: Shown::marked(text).to_running(), embosser: embosser.to_string(), version, tables, ..self.0.clone() };
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

    /// Save As refreshable braille (.lbf): `text` (editor form) with the document's styles (bm_core::lbf).
    pub fn refreshable(&self, text: &str) -> Vec<u8> {
        bm_core::lbf::write(&Shown::marked(text).to_running(), &self.0.styles)
    }
}

/// A new document from a template's text, as File, New makes one in the desktop program: the template's four
/// tables, page numbering and styles, `text` (editor form) as its text. The built-in default template's tables when
/// the template names none.
#[wasm_bindgen]
pub fn new_document(braille: bool, text: &str, template: &str) -> Document {
    use bm_core::templates;
    let styles = templates::styles(template)
        .entries()
        .map(|(name, begin, end)| (name.to_string(), Shown::marked(&begin).to_running(), Shown::marked(&end).to_running()))
        .collect();
    Document(DxDocument {
        kind: if braille { DocumentKind::Braille } else { DocumentKind::Print },
        version: 1,
        page_numbering: templates::page_numbers(template),
        translated_line: true,
        six_key: false,
        tables: templates::tables(template).unwrap_or_else(|| ["enuncp", "enudxp", "enuncb", "gendxb"].map(String::from)),
        text: Shown::marked(text).to_running(),
        styles,
        style_index: Vec::new(),
        settings: Vec::new(),
        embosser: String::new(),
        sections: Vec::new(),
        layout: None,
    })
}

/// The code pages Save As offers for formatted text, the first chosen every time (spec 04d-E).
#[wasm_bindgen]
pub fn code_pages() -> Vec<String> {
    bm_core::codepages::NAMES.iter().map(|s| s.to_string()).collect()
}

/// Save As formatted text (.prf): a print document laid out `cells` by `lines`, its formatted text in `code_page`; a
/// character the code page cannot hold is written '#', a hard space as a space (spec 04d-B, 04d-F).
#[wasm_bindgen]
pub fn formatted_text(text: &str, cells: u32, lines: u32, code_page: &str) -> Vec<u8> {
    let pages = laid_out(text, cells, lines, false);
    bm_core::codepages::encode(code_page, &bm_core::format::formatted_text(&pages).replace('\u{a0}', " "))
}

/// Save As formatted braille (.brf): a braille document laid out `cells` by `lines` (spec 04d-B). Each line loses its
/// trailing spaces; each character goes to its cell through `shown_cells` (JSON: the 64 cells' characters as the
/// shown encoding lists them, "aA" for a cell shown as a or A), a space and a hard space being the blank cell and a
/// character with no cell written '*'; each cell is written as `codes` gives it (one code point per cell, written as
/// one byte when it is 0 to 255 and '#' otherwise), or as Unicode braille in UTF-8 when `codes` is empty; CR LF ends
/// each line and a form feed each page.
#[wasm_bindgen]
pub fn formatted_braille(text: &str, cells: u32, lines: u32, shown_cells: &str, codes: Vec<u32>) -> Result<Vec<u8>, JsError> {
    let shown: Vec<String> = serde_json::from_str(shown_cells).map_err(error)?;
    let cell_of = |c: char| shown.iter().position(|s| s.contains(c));
    let mut out = Vec::new();
    for page in laid_out(text, cells, lines, true) {
        for line in &page.lines {
            for c in line.text.trim_end_matches(' ').chars() {
                let cell = if c == ' ' || c == '\u{a0}' { Some(0) } else { cell_of(c) };
                match cell {
                    Some(p) if codes.is_empty() => {
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(char::from_u32(0x2800 + p as u32).unwrap_or(' ').encode_utf8(&mut buf).as_bytes());
                    }
                    Some(p) => out.push(codes.get(p).copied().filter(|c| *c <= 0xFF).map_or(b'#', |c| c as u8)),
                    None => out.push(b'*'),
                }
            }
            out.extend_from_slice(b"\r\n");
        }
        out.push(0x0C);
    }
    Ok(out)
}

/// Save As coded braille (.bru) or coded text (.txt): the body as the document file stores it through the coding
/// script (`script`, coded-braille.rules or coded-text.rules) and the line wrapping script (`wrap`,
/// coded-wrap.rules), as the reference program makes these files (spec 04d-C, spec 20).
#[wasm_bindgen]
pub fn coded(text: &str, script: &[u8], wrap: &[u8]) -> Result<Vec<u8>, JsError> {
    use bm_core::rule_script::{run_all, RuleScript};
    let body = dxfile::body_bytes(&Shown::marked(text).to_running());
    let mut scripts = [RuleScript::from_bytes(script, None).map_err(error)?, RuleScript::from_bytes(wrap, None).map_err(error)?];
    run_all(&mut scripts, &body).map_err(error)
}

/// Save As LaTeX (.tex) for a print document (spec 22): the text through the text export table (`table`, the
/// source of table-txxtxa-binary-forward.bmt; when it cannot be used, the body as the document file stores it goes
/// on unchanged), then the LaTeX rule script (`script`, latex.rules).
#[wasm_bindgen]
pub fn latex(text: &str, table: &str, script: &[u8]) -> Result<Vec<u8>, JsError> {
    let running = Shown::marked(text).to_running();
    let translated = Translator::from_source(table).ok().and_then(|t| t.translate_marked(&running, None, None).ok());
    // The engine writes bytes; its text holds one character per byte.
    let middle: Vec<u8> = match translated {
        Some(s) => s.chars().map(|c| c as u32 as u8).collect(),
        None => dxfile::body_bytes(&running),
    };
    bm_core::rule_script::RuleScript::from_bytes(script, None).map_err(error)?.run(&middle).map_err(error)
}

/// Braille text (editor form) from one encoding to another, cell for cell, as Global, Internationalization changes
/// the encoding of braille documents; the codes keep their text. `from` and `to` are JSON lists of the 64 cells'
/// characters as each encoding lists them ("aA" for a cell shown as a or A); a Unicode braille cell is a cell too, and
/// a cell with no character in `to` is written as Unicode braille.
#[wasm_bindgen]
pub fn convert_braille(text: &str, from: &str, to: &str) -> Result<String, JsError> {
    let from: Vec<String> = serde_json::from_str(from).map_err(error)?;
    let to: Vec<String> = serde_json::from_str(to).map_err(error)?;
    let convert = |c: char| {
        let cell = match c as u32 {
            n @ 0x2800..=0x283F => Some((n - 0x2800) as usize),
            _ => from.iter().position(|s| s.contains(c)),
        };
        match cell {
            Some(p) => to.get(p).and_then(|s| s.chars().next()).unwrap_or_else(|| char::from_u32(0x2800 + p as u32).unwrap_or(' ')),
            None => c,
        }
    };
    let shown = Shown::marked(text);
    let chars: Vec<char> = shown.text.chars().collect();
    let mut out = String::with_capacity(shown.text.len());
    let mut at = 0;
    for &(start, end) in &shown.codes {
        out.extend(chars[at.min(start)..start].iter().map(|&c| convert(c)));
        out.extend(&chars[start..end.min(chars.len())]);
        at = end;
    }
    out.extend(chars[at.min(chars.len())..].iter().map(|&c| convert(c)));
    Ok(out)
}

/// The status line for the caret at `position` of a document (editor form) laid out `cells` by `lines`, as the
/// desktop program shows it: the current style, Page, Line and Column on the laid-out pages (left out in coded view,
/// `codes`), the character at the caret (with its Unicode value when `verbose`), and Keys.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn status_line(text: &str, cells: u32, lines: u32, braille: bool, position: usize, codes: bool, verbose: bool, six_key: bool) -> String {
    use bm_core::status::{char_field, style_text, StatusLine};
    let shown = Shown::marked(text);
    let pos = position as i64;
    StatusLine {
        style: style_text(&shown, pos),
        location: (!codes).then(|| bm_core::format::locate(&laid_out(text, cells, lines, braille), position)),
        character: char_field(&shown, pos, verbose),
        six_key,
        highlight: false,
        keyboard: false,
    }
    .text()
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

    fn shipped(path: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn coded_and_latex_match_the_desktop_path() {
        use bm_core::rule_script::{run_all, RuleScript};
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables");
        let text = "[p]The cat [hds2]sat[hde] here.[l]\nNext line.";
        for (braille, script) in [(true, "coded-braille.rules"), (false, "coded-text.rules")] {
            let web = coded(text, &shipped(&format!("tables/rule-scripts/{script}")), &shipped("tables/rule-scripts/coded-wrap.rules")).ok().unwrap();
            let mut scripts = [RuleScript::open(&folder.join("rule-scripts").join(script)).unwrap(), RuleScript::open(&folder.join("rule-scripts/coded-wrap.rules")).unwrap()];
            let desktop = run_all(&mut scripts, &dxfile::body_bytes(&Shown::marked(text).to_running())).unwrap();
            assert_eq!(web, desktop, "braille {braille}");
            assert!(!web.is_empty());
        }
        let table = String::from_utf8(shipped("tables/table-txxtxa-binary-forward.bmt")).unwrap();
        let web = latex(text, &table, &shipped("tables/rule-scripts/latex.rules")).ok().unwrap();
        let set = bm_core::engine::TableSet::open(&folder).unwrap();
        let running = Shown::marked(text).to_running();
        let middle: Vec<u8> = set.translator_with_scan("txxtxa", "binary").unwrap().translate_marked(&running, None, None).unwrap().chars().map(|c| c as u32 as u8).collect();
        let desktop = RuleScript::open(&folder.join("rule-scripts/latex.rules")).unwrap().run(&middle).unwrap();
        assert_eq!(web, desktop);
        assert!(String::from_utf8_lossy(&web).contains("cat"), "{}", String::from_utf8_lossy(&web));
    }

    #[test]
    fn formatted_braille_writes_cells_in_the_chosen_encoding() {
        let na: Vec<String> = bm_core::emboss::NA_CELLS.iter().map(|&b| format!("{}{}", (b as char).to_ascii_lowercase(), b as char)).collect();
        let shown = serde_json::to_string(&na).unwrap();
        let caps: Vec<u32> = bm_core::emboss::NA_CELLS.iter().map(|&b| u32::from(b)).collect();
        // A page is written whole: its 25 lines, blank ones too, then a form feed.
        let out = formatted_braille("ab c", 40, 25, &shown, caps).ok().unwrap();
        assert!(out.starts_with(b"AB C\r\n\r\n"));
        assert_eq!((out.windows(2).filter(|w| w == b"\r\n").count(), out.last()), (25, Some(&0x0c)));
        let unicode = String::from_utf8(formatted_braille("ab", 40, 25, &shown, Vec::new()).ok().unwrap()).unwrap();
        assert!(unicode.starts_with("\u{2801}\u{2803}\r\n"), "{unicode:?}");
    }

    #[test]
    fn a_new_document_from_the_default_template_saves_in_both_formats() {
        let template = String::from_utf8(shipped("templates/english-ueb-basic.template")).unwrap_or_default();
        let doc = new_document(false, "The cat.", &template);
        for version in [1, 0] {
            let bytes = doc.save_version("The dog.", "Everest", "", 40, 25, version).ok().unwrap();
            let back = read_document(&bytes).ok().unwrap();
            assert_eq!((back.text(), back.embosser(), back.braille()), ("The dog.".to_string(), "Everest".to_string(), false));
        }
    }

    #[test]
    fn the_status_line_reads_as_the_desktop_one() {
        let text = "[es~h1.]ab[ee~h1.]\ncd";
        assert_eq!(status_line(text, 40, 25, true, 8, false, false, false), "Current Style: h1.  Page:1  Line:1  Column:1  a  Keys:1");
        assert_eq!(status_line(text, 40, 25, true, 8, true, true, true), "Current Style: h1.  a {U+0061}  Keys:6");
        assert!(status_line(text, 40, 25, true, 21, false, false, false).ends_with("(end)  Keys:1"));
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
