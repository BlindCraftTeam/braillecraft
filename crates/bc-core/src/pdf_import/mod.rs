//! PDF import: the reference program's PDF front end (its Python import script and table finder, run with the
//! values its Global PDF Options give, spec 29) ported to Rust, followed by the steps its import recipe runs after
//! it (character mapping, letter and accent composition, and the final text clean-up; its Han step passes text
//! through in this chain).
//!
//! A tagged PDF (its catalog's MarkInfo says Marked and it has a structure tree) is read through the structure tree:
//! each element's text is the text drawn inside its marked content, and the element's tag gives the codes around it.
//! Any other PDF, and a tagged one whose tree gives no text, is "scraped": the characters of each page are laid out
//! into lines and boxes (layout.rs, pdfminer's analysis), each box becomes a paragraph, and ledger tables are found
//! (table.rs). PDF text comes from the vendored pdf-extract.
//!
//! The result is running text with codes marked 0x1C ... 0x1F (doctext::OPEN and CLOSE), as a document holds it.

pub mod layout;
pub mod table;

use std::collections::{HashMap, HashSet};

use pdf_extract::{Dictionary, Document, Object, ObjectId, OutputDev, OutputError, Transform};

use crate::doctext::{CLOSE, OPEN};
use layout::{Char, LaParams, Layout, Node, Rect};
use table::TableFinder;

/// The Global PDF Options values the importer uses (spec 29 section 2): the stored values are thousandths, here as
/// the numbers the script receives.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PdfOptions {
    /// Read tagged PDFs through their structure tree (PDF/A in the dialog).
    pub allow_tagged: bool,
    /// Bands at the top and bottom of each page left out, in points.
    pub header_height: f64,
    pub footer_height: f64,
    pub la: LaParams,
}

impl Default for PdfOptions {
    fn default() -> Self {
        PdfOptions { allow_tagged: true, header_height: 0.0, footer_height: 0.0, la: LaParams::default() }
    }
}

fn code(text: &str) -> String {
    format!("{OPEN}{text}{CLOSE}")
}

/// What a structure tag becomes: codes before, codes after, and whether it starts a table level (spec 29 4a).
fn tag_markup(tag: &str, styles: &HashSet<String>) -> (String, String, bool) {
    let style = |long: &str, short: &str| {
        let name = if styles.contains(long) { long } else { short };
        (code(&format!("es~{name}")), code(&format!("ee~{name}")), false)
    };
    match tag {
        "H1" | "Title" | "Subtitle" => style("H1-HeadingCentered.", "h1."),
        "H2" => style("H2-HeadingCell5.", "h2."),
        "P" | "NormalParagraphStyle" => style("B-BodyText.", "para."),
        "Article" | "Figure" | "Lbl" | "LBody" | "Link" | "NonStruct" | "Span" | "Story" => (String::new(), String::new(), false),
        "LI" | "TOCI" => (String::new(), code("l"), false),
        "Table" => (code("htbs"), code("htbe"), true),
        "TD" | "TH" => (String::new(), code(">"), true),
        "TR" => (String::new(), code("<"), true),
        other => {
            let t = other.to_lowercase();
            (code(&format!("es~{t}")), code(&format!("ee~{t}")), false)
        }
    }
}

/// map_elem_name_to_markup: inside a table, a tag of the map that does not start a table level gives no codes.
fn markup(tag: &str, table_depth: i32, styles: &HashSet<String>) -> (String, String, bool) {
    let in_map = matches!(
        tag,
        "H1" | "H2" | "P" | "NormalParagraphStyle" | "Title" | "Subtitle" | "Article" | "Figure" | "Lbl" | "LBody" | "Link"
            | "NonStruct" | "Span" | "Story" | "LI" | "TOCI" | "Table" | "TD" | "TH" | "TR"
    );
    let m = tag_markup(tag, styles);
    if in_map && table_depth > 0 && !m.2 {
        return (String::new(), String::new(), false);
    }
    m
}

// ------------------------------------------------------------------ reading the pages

/// One page as the importer needs it.
struct Page {
    object: ObjectId,
    /// The media box as the file gives it.
    media_box: Rect,
    /// The page's own characters (not those of figures), in device space from (0, 0).
    chars: Vec<Char>,
    /// The page box from (0, 0).
    bbox: Rect,
}

/// The output device: characters with their boxes per page, and the text of each marked-content id.
struct Collector {
    pages: Vec<Page>,
    page_ids: Vec<(ObjectId, Rect, i64)>,
    ctm: [f64; 6],
    figure_depth: usize,
    /// The text of each (page, MCID), as the reference's tagged device collects it.
    spans: HashMap<(ObjectId, i64), String>,
    mcid: Option<i64>,
}

fn apply(m: &[f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

impl OutputDev for Collector {
    fn begin_page(&mut self, _page_num: u32, _media_box: &pdf_extract::MediaBox, _art: Option<(f64, f64, f64, f64)>) -> Result<(), OutputError> {
        let (object, mb, rotate) = self.page_ids[self.pages.len()];
        // pdfminer's page matrix: the media box to (0, 0), turned by the page's rotation.
        let (x0, y0, x1, y1) = (mb.x0, mb.y0, mb.x1, mb.y1);
        self.ctm = match rotate {
            90 => [0.0, -1.0, 1.0, 0.0, -y0, x1],
            180 => [-1.0, 0.0, 0.0, -1.0, x1, y1],
            270 => [0.0, 1.0, -1.0, 0.0, y1, -x0],
            _ => [1.0, 0.0, 0.0, 1.0, -x0, -y0],
        };
        let (a, b) = (apply(&self.ctm, x0, y0), apply(&self.ctm, x1, y1));
        let bbox = Rect::new(0.0, 0.0, (a.0 - b.0).abs(), (a.1 - b.1).abs());
        self.pages.push(Page { object, media_box: mb, chars: Vec::new(), bbox });
        Ok(())
    }
    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn output_character(&mut self, trm: &Transform, width: f64, spacing: f64, font_size: f64, char: &str) -> Result<(), OutputError> {
        self.output_glyph(trm, width, spacing, font_size, 0.0, char)
    }
    fn output_glyph(&mut self, trm: &Transform, width: f64, _spacing: f64, font_size: f64, descent: f64, char: &str) -> Result<(), OutputError> {
        let Some(page) = self.pages.last_mut() else { return Ok(()) };
        // The tagged device adds every character to the current MCID's text, figures included.
        if let Some(m) = self.mcid {
            if let Some(s) = self.spans.get_mut(&(page.object, m)) {
                s.push_str(char);
            }
        }
        if self.figure_depth > 0 {
            return Ok(());
        }
        // pdfminer's LTChar box: (0, descent) to (advance, descent + size) in text space, which the text rendering
        // matrix (holding the rise and horizontal scaling already) takes to user space; then the page matrix.
        let d = descent * font_size;
        let m = [trm.m11, trm.m12, trm.m21, trm.m22, trm.m31, trm.m32];
        let p0 = apply(&m, 0.0, d);
        let p1 = apply(&m, width * font_size, d + font_size);
        let (ax, ay) = apply(&self.ctm, p0.0, p0.1);
        let (bx, by) = apply(&self.ctm, p1.0, p1.1);
        let rect = Rect::new(ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
        page.chars.push(Char { rect, text: char.to_string() });
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn begin_marked_content(&mut self, _tag: &str, mcid: Option<i64>) -> Result<(), OutputError> {
        // Only an MCID changes the current one; it stays in force after EMC, as in the reference's device.
        if let (Some(m), Some(page)) = (mcid, self.pages.last()) {
            self.mcid = Some(m);
            self.spans.insert((page.object, m), String::new());
        }
        Ok(())
    }
    fn begin_figure(&mut self) -> Result<(), OutputError> {
        self.figure_depth += 1;
        Ok(())
    }
    fn end_figure(&mut self) -> Result<(), OutputError> {
        self.figure_depth = self.figure_depth.saturating_sub(1);
        Ok(())
    }
}

fn number(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(pdf_extract::real_f64(*r)),
        _ => None,
    }
}

/// A page attribute, inherited from the page tree.
fn inherited<'a>(doc: &'a Document, page: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut d = page;
    for _ in 0..64 {
        if let Ok(o) = d.get(key) {
            return Some(match o {
                Object::Reference(r) => doc.get_object(*r).ok()?,
                o => o,
            });
        }
        d = doc.get_dictionary(d.get(b"Parent").ok()?.as_reference().ok()?).ok()?;
    }
    None
}

fn read_pages(doc: &Document) -> Result<Collector, String> {
    let mut page_ids = Vec::new();
    for (_, id) in doc.get_pages() {
        let page = doc.get_dictionary(id).map_err(|e| e.to_string())?;
        let mb: Vec<f64> = match inherited(doc, page, b"MediaBox") {
            Some(Object::Array(a)) => a.iter().filter_map(number).collect(),
            _ => Vec::new(),
        };
        let mb = if mb.len() == 4 { Rect::new(mb[0], mb[1], mb[2], mb[3]) } else { Rect::new(0.0, 0.0, 612.0, 792.0) };
        let rotate = inherited(doc, page, b"Rotate").and_then(number).map_or(0, |r| ((r as i64) + 360).rem_euclid(360));
        page_ids.push((id, mb, rotate));
    }
    let mut c = Collector { pages: Vec::new(), page_ids, ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], figure_depth: 0, spans: HashMap::new(), mcid: None };
    pdf_extract::output_doc(doc, &mut c).map_err(|e| format!("{e:?}"))?;
    Ok(c)
}

// ------------------------------------------------------------------ the tagged path

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(r) => doc.get_object(*r).unwrap_or(o),
        o => o,
    }
}

/// iterable_children: the K entry as a list of children (a single child as one).
fn children(d: &Dictionary) -> Vec<Object> {
    match d.get(b"K") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(o) => vec![o.clone()],
        Err(_) => Vec::new(),
    }
}

struct Tagged<'a> {
    doc: &'a Document,
    spans: &'a HashMap<(ObjectId, i64), String>,
    styles: &'a HashSet<String>,
}

impl Tagged<'_> {
    /// convert_tag: an element's codes and text, and whether it has any text (spaces do not count).
    fn convert(&self, elemref: &Object, pg: Option<ObjectId>, table_depth: i32) -> (String, bool) {
        // Only a reference resolves; anything else gives nothing.
        let elem = match elemref {
            Object::Reference(r) => match self.doc.get_object(*r) {
                Ok(o) => o,
                Err(_) => return (String::new(), false),
            },
            _ => return (String::new(), false),
        };
        if let Object::Array(list) = elem {
            let mut out = String::new();
            let mut any = false;
            for child in list {
                let (m, t) = self.convert(child, pg, table_depth);
                if t {
                    out.push_str(&m);
                    any = true;
                }
            }
            return (out, any);
        }
        let Object::Dictionary(d) = elem else { return (String::new(), false) };
        // An object reference (OBJR) leads to an annotation or an XObject, which has no text here.
        if d.get(b"Obj").is_ok() && matches!(d.get(b"Type"), Ok(Object::Name(n)) if n.as_slice() == b"OBJR") {
            return (String::new(), false);
        }
        let page = match d.get(b"Pg") {
            Ok(Object::Reference(r)) => Some(*r),
            _ => pg,
        };
        let (mut out, end, starts_table) = match d.get(b"S") {
            Ok(Object::Name(n)) => markup(&String::from_utf8_lossy(n), table_depth, self.styles),
            _ => (String::new(), String::new(), false),
        };
        let mut any = false;
        for child in children(d) {
            let (m, t) = match &child {
                // The reference passes its own parent's page on, not the one this element names.
                Object::Reference(_) => self.convert(&child, pg, table_depth + i32::from(starts_table)),
                Object::Integer(mcid) => {
                    let s = page.and_then(|p| self.spans.get(&(p, *mcid))).cloned().unwrap_or_default();
                    let t = !s.replace(' ', "").is_empty();
                    (s, t)
                }
                _ => (String::new(), false),
            };
            if t {
                out.push_str(&m);
                any = true;
            }
        }
        if any {
            out.push_str(&end);
            (out, true)
        } else {
            (String::new(), false)
        }
    }
}

/// The text of a tagged PDF, or None when the file is not read as tagged or its tree gives no text.
fn convert_tagged(doc: &Document, spans: &HashMap<(ObjectId, i64), String>, styles: &HashSet<String>) -> Option<String> {
    let catalog = doc.catalog().ok()?;
    // MarkInfo must be given in the catalog itself, with Marked true; the tree root must be a reference.
    let marked = matches!(catalog.get(b"MarkInfo"), Ok(Object::Dictionary(m)) if matches!(m.get(b"Marked"), Ok(Object::Boolean(true))));
    if !marked {
        return None;
    }
    let root = match catalog.get(b"StructTreeRoot") {
        Ok(Object::Reference(r)) => doc.get_dictionary(*r).ok()?,
        _ => return None,
    };
    let t = Tagged { doc, spans, styles };
    let mut out = String::new();
    let mut any = false;
    for kref in children(root) {
        let Object::Reference(r) = kref else { return None };
        let Ok(k) = doc.get_object(r) else { return None };
        let Object::Dictionary(k) = resolve(doc, k) else { continue };
        for elemref in children(k) {
            let (m, has) = t.convert(&elemref, None, 0);
            if has && !m.is_empty() {
                out.push_str(&m);
                any = true;
            }
        }
    }
    any.then_some(out)
}

// ------------------------------------------------------------------ the scrape path

struct Scraper<'a> {
    layout: &'a Layout,
    tables: &'a TableFinder<'a>,
    header: Rect,
    footer: Rect,
    done: HashSet<usize>,
    p: (String, String),
    out: &'a mut String,
}

impl Scraper<'_> {
    fn item(&mut self, n: Node) {
        match n {
            Node::Box(b) => {
                let r = self.layout.boxes[b].rect;
                if r.is_voverlap(&self.header) || r.is_voverlap(&self.footer) {
                    return;
                }
                self.out.push_str(&self.p.0.clone());
                for k in 0..self.layout.boxes[b].lines.len() {
                    self.line(self.layout.boxes[b].lines[k]);
                }
                self.out.push_str(&self.p.1.clone());
            }
            Node::Group(g) => {
                for k in 0..self.layout.group_store[g].members.len() {
                    self.item(self.layout.group_store[g].members[k]);
                }
            }
        }
    }

    fn line(&mut self, l: usize) {
        match self.tables.table_of[l] {
            None => {
                let r = self.layout.lines[l].rect;
                if r.is_voverlap(&self.header) || r.is_voverlap(&self.footer) {
                    return;
                }
                let mut text = self.layout.line_text(l);
                let c: Vec<char> = text.chars().collect();
                // A line that does not end in a space ends with a line break code.
                if c.last() == Some(&'\n') && (c.len() < 2 || c[c.len() - 2] != ' ') {
                    text = text.replace(" \n", " ").replace('\n', &code("l"));
                }
                self.out.push_str(&text);
            }
            Some(t) => {
                if self.done.insert(t) {
                    self.table(t);
                }
            }
        }
    }

    fn table(&mut self, t: usize) {
        self.out.push_str(&code("htbs"));
        for (r, row) in self.tables.tables[t].rows.iter().enumerate() {
            if r > 0 {
                self.out.push_str(&code("<"));
            }
            for (c, cell) in row.cells.iter().enumerate() {
                if c > 0 {
                    self.out.push_str(&code(">"));
                }
                self.out.push_str(&cell.text(self.layout));
            }
        }
        self.out.push_str(&code("htbe"));
    }
}

fn scrape(pages: Vec<Page>, opts: &PdfOptions, styles: &HashSet<String>) -> String {
    let mut out = String::new();
    let (p0, p1, _) = markup("P", 0, styles);
    for page in pages {
        let mb = page.media_box;
        // The bands, from the media box as the file gives it (header_height is always set by the program).
        let header = Rect::new(mb.x0, mb.y1 - opts.header_height, mb.x1, mb.y1);
        let footer = Rect::new(mb.x0, mb.y0, mb.x1, mb.y0 + opts.footer_height);
        let layout = layout::analyze(&opts.la, page.bbox, page.chars);
        let mut finder = TableFinder::new(&layout, mb, header, footer);
        finder.find_ledgers();
        let mut s = Scraper { layout: &layout, tables: &finder, header, footer, done: HashSet::new(), p: (p0.clone(), p1.clone()), out: &mut out };
        for &g in layout.groups.as_deref().unwrap_or(&[]) {
            s.item(g);
        }
    }
    out
}

// ------------------------------------------------------------------ after the script

/// u8kdkc.sbt: a letter followed by combining marks becomes the one precomposed character whose decomposition it
/// is, where there is one.
fn compose(text: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let mut j = i + 1;
        while j < chars.len() && is_combining_mark(chars[j]) {
            j += 1;
        }
        let run: String = chars[i..j].iter().collect();
        if j > i + 1 {
            let nfc: Vec<char> = run.nfc().collect();
            if nfc.len() == 1 && nfc[0].to_string().nfkd().collect::<String>() == run {
                out.push(nfc[0]);
                i = j;
                continue;
            }
        }
        out.push_str(&run);
        i = j;
    }
    out
}

/// For checks against the reference: the text lines of one page (0-based) after layout analysis, in group order,
/// with their boxes.
pub fn page_lines(bytes: &[u8], opts: &PdfOptions, page: usize) -> Result<Vec<(Rect, String)>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| e.to_string())?;
    let mut collected = read_pages(&doc)?;
    if page >= collected.pages.len() {
        return Err("no such page".into());
    }
    let p = collected.pages.swap_remove(page);
    let layout = layout::analyze(&opts.la, p.bbox, p.chars);
    Ok(table::group_lines(&layout).into_iter().map(|l| (layout.lines[l].rect, layout.line_text(l))).collect())
}

/// The steps after the script: unimap (a character internal text has no code for becomes "*"), u8kdkc.sbt, and
/// utf8dusci (a leading U+FEFF goes; U+200D goes unless after U+0D4D).
fn after_script(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mapped: String = text
        .chars()
        .map(|c| if c == OPEN || c == CLOSE || c == '\n' || crate::internal::code_of(c).is_some() || c == '\u{200d}' { c.to_string() } else { "*".to_string() })
        .collect();
    let composed = compose(&mapped);
    let mut out = String::with_capacity(composed.len());
    let mut prev = None;
    for c in composed.chars() {
        if c == '\u{200d}' && prev != Some('\u{0d4d}') {
            prev = Some(c);
            continue;
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// Imports a PDF: running text with codes marked 0x1C ... 0x1F. `styles` are the names of the styles of the
/// document the text goes into (they decide between the long and short heading and paragraph style names).
pub fn import(bytes: &[u8], opts: &PdfOptions, styles: &[String]) -> Result<String, String> {
    let styles: HashSet<String> = styles.iter().cloned().collect();
    // The PDF reader stops with a panic on some damaged files; that is reported as an error, not a crash.
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<String, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("This file could not be read as a PDF: {e}"))?;
        let collected = read_pages(&doc)?;
        let tagged = if opts.allow_tagged { convert_tagged(&doc, &collected.spans, &styles) } else { None };
        let text = match tagged {
            Some(t) => t,
            None => scrape(collected.pages, opts, &styles),
        };
        Ok(after_script(&text))
    }));
    match run {
        Ok(r) => r,
        Err(_) => Err("This PDF could not be read: the file is damaged or uses something the PDF reader does not handle.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_map_and_styles() {
        let none = HashSet::new();
        let bana: HashSet<String> = ["B-BodyText.".to_string()].into_iter().collect();
        assert_eq!(tag_markup("P", &none).0, "\u{1c}es~para.\u{1f}");
        assert_eq!(tag_markup("P", &bana).1, "\u{1c}ee~B-BodyText.\u{1f}");
        assert_eq!(tag_markup("Sect", &none).0, "\u{1c}es~sect\u{1f}");
        assert_eq!(markup("P", 1, &none), (String::new(), String::new(), false));
        assert_eq!(markup("TD", 1, &none).1, "\u{1c}>\u{1f}");
        // Not in the map: codes even inside a table.
        assert_eq!(markup("Sect", 1, &none).0, "\u{1c}es~sect\u{1f}");
    }

    #[test]
    fn steps_after_the_script() {
        assert_eq!(after_script("\u{feff}cafe\u{301} a\u{200d}b \u{d4d}\u{200d}"), "café ab \u{d4d}\u{200d}");
        assert_eq!(after_script("x\u{10fffd}y"), "x*y");
    }

    #[test]
    fn not_a_pdf() {
        assert!(import(b"hello", &PdfOptions::default(), &[]).is_err());
    }
}
