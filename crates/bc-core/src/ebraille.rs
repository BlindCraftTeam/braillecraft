//! Save As eBraille (braille documents) and EPUB 3 (print documents), as the original writes them (spec 23): a zip
//! package holding the mimetype, the content folder ("ebraille" or "epub") with its style sheet and one content
//! page part0001.xml, index.html with the table of contents, page list and landmarks navs, package.opf from the
//! original's template with the document's metadata, and META-INF/container.xml. The content page is the document
//! body as XHTML: block styles as elements with the style's class, lists as ol and li, page number styles as
//! doc-pagebreak elements, tables, links; in a braille document every text character becomes its Unicode braille
//! cell. The metadata comes from Document, eBraille Metadata (the `Metadata` struct) and is written as the
//! package's metadata element.
//!
//! What the original does that the saved decompiles do not show is marked INFERENCE in the code: the element name
//! of a block style (p), which styles are headings and list styles, the markup of the line break code, the manifest
//! items for the nav page and the style sheet, and the metadata property names of the Recommended page.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::docx;

/// The style sheet written as the content folder's css/ebrl.css (the original's, with our own namespace).
pub const STYLE_SHEET: &str = include_str!("../data/ebraille.css");

/// The namespace of the maxdepth attribute, as the style sheet declares it.
pub const NAMESPACE: &str = "https://blindcraft.dev/braillecraft";

/// A braille system of the document, as the Braille Systems dialog lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrailleSystem {
    pub language: String,
    /// grade0, grade1, grade2 or no-grade.
    pub grade: String,
    /// None, comp6, math, music or phonetic.
    pub specialization: String,
}

/// One further property of the Additional page: a name, a value and attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub attributes: Vec<(String, String)>,
}

/// The document's eBraille metadata (Document, eBraille Metadata): the Required page, the Recommended page, the
/// Additional page, and the XML preview when it was edited.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub titles: Vec<String>,
    pub creators: Vec<String>,
    pub producers: Vec<String>,
    pub languages: Vec<String>,
    pub braille_systems: Vec<BrailleSystem>,
    /// YYYY-MM-DD; empty for today.
    pub date: String,
    /// 0 ISBN, 1 DOI, 2 UUID.
    pub id_type: u8,
    pub identifier: String,
    pub date_copyrighted: String,
    pub tactile_graphics: bool,
    pub complete_transcription: bool,
    pub publishers: Vec<String>,
    pub education_levels: Vec<String>,
    pub description: String,
    pub rights: Vec<String>,
    pub source_type: u8,
    pub source: String,
    pub subjects: Vec<String>,
    pub properties: Vec<Property>,
    /// The attributes chosen with a field's Options... button, by field (title, creator, date, identifier,
    /// publisher, description, rights, source, subject): written on each element of that field (INFERENCE).
    pub field_attributes: Vec<(String, Vec<(String, String)>)>,
    /// The metadata element as edited on the XML Preview page (Allow editing); written as it is when set.
    pub edited_xml: Option<String>,
}

impl Metadata {
    pub fn attributes_of(&self, field: &str) -> String {
        let mut out = String::new();
        if let Some((_, attrs)) = self.field_attributes.iter().find(|(f, _)| f == field) {
            for (n, v) in attrs {
                if !n.trim().is_empty() {
                    out.push_str(&format!(" {}=\"{}\"", xml_text(n.trim()), xml_text(v)));
                }
            }
        }
        out
    }
}

/// The names the Options... dialogs offer: XML attributes an OPF metadata element may carry (INFERENCE: the
/// original's list was not read), and the properties the Additional page offers (the names the original's code
/// holds, spec 23 section 5).
pub const ATTRIBUTE_NAMES: [&str; 6] = ["id", "xml:lang", "dir", "scheme", "refines", "property"];
pub const PROPERTY_NAMES: [&str; 7] =
    ["a11y:brailleSystem", "a11y:brailleCellType", "a11y:graphicType", "a11y:certifiedBy", "a11y:certifierCredential", "a11y:certifierReport", "dc:language"];
pub const GRADES: [&str; 4] = ["grade0", "grade1", "grade2", "no-grade"];
pub const SPECIALIZATIONS: [&str; 5] = ["None", "comp6", "math", "music", "phonetic"];

/// How a style is written, from its name and its beginning codes (running text, codes marked 0x1C ... 0x1F):
/// GuideDots by name; a page number style holds a reference point code (rps); a list style holds level codes
/// (hl or hi), its depth the highest level named (INFERENCE: the original's maxdepth source was not read); a
/// heading style is named h1., h2., h3. and so on (INFERENCE); a paragraph style holds a layout code; anything
/// else is a character style.
pub fn kind_of(name: &str, begin: &str) -> StyleKind {
    if name == "GuideDots" {
        return StyleKind::GuideDots;
    }
    let codes: Vec<&str> = begin.split('\u{1c}').skip(1).filter_map(|c| c.split('\u{1f}').next()).collect();
    let code_name = |c: &str| -> String { c.chars().take_while(|ch| ch.is_ascii_alphabetic()).collect() };
    if codes.iter().any(|c| c.starts_with("rps")) {
        return StyleKind::PageNumber;
    }
    let levels: Vec<u8> = codes.iter().filter_map(|c| c.strip_prefix("hl").and_then(|n| n.chars().take_while(|d| d.is_ascii_digit()).collect::<String>().parse().ok())).collect();
    if !levels.is_empty() || codes.iter().any(|c| code_name(c) == "hi") {
        return StyleKind::List(levels.into_iter().max().unwrap_or(1).max(1));
    }
    let mut chars = name.chars();
    if chars.next() == Some('h') {
        if let Some(level) = chars.next().and_then(|d| d.to_digit(10)) {
            return StyleKind::Heading(level as u8);
        }
    }
    const LAYOUT: [&str; 20] = ["p", "l", "sk", "skn", "hds", "hde", "ind", "pm", "tab", "fr", "rm", "lm", "ctb", "stb", "pg", "sd", "kps", "kpe", "svrhp", "svrfp"];
    if codes.iter().any(|c| LAYOUT.contains(&code_name(c).as_str())) {
        return StyleKind::Block;
    }
    StyleKind::Inline
}

/// What the page builder needs to know of a style: how it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    /// A paragraph style: an element with the style's class.
    Block,
    /// A heading paragraph style (a table of contents entry), with its level.
    Heading(u8),
    /// A list style with its deepest level (the maxdepth attribute).
    List(u8),
    /// A print page number style (a doc-pagebreak element, a page list entry).
    PageNumber,
    /// The GuideDots style: a contents entry's link to its page.
    GuideDots,
    /// A character style: nothing written.
    Inline,
}

fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{2011}' => out.push('-'),
            c => out.push(c),
        }
    }
}

/// The class of a style: its name before the dot, in lower case, with "_" before a leading digit (the style sheet's
/// classes: h1-headingcentered, b-bodytext, _1-3).
pub fn class_of(style: &str) -> String {
    let name = style.split('.').next().unwrap_or("").to_lowercase();
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("_{name}")
    } else {
        name
    }
}

/// The content page built from the body, with the entries the navs list.
#[derive(Debug, Default)]
pub struct Page {
    pub body: String,
    /// Table of contents entries: (id, text).
    pub headings: Vec<(String, String)>,
    /// Page list entries: (id, label).
    pub pages: Vec<(String, String)>,
}

struct Builder<'a> {
    out: String,
    kinds: &'a dyn Fn(&str) -> StyleKind,
    /// Open block elements, innermost last: (element name, style kind).
    open: Vec<(&'static str, StyleKind)>,
    /// The list level counter and the deepest level of the open list, and whether a list item is open.
    level: u32,
    maxdepth: u32,
    item_open: bool,
    in_table: bool,
    headings: Vec<(String, String)>,
    pages: Vec<(String, String)>,
    /// The text of the open heading or page number element, for its nav entry.
    capture: Option<String>,
}

impl<'a> Builder<'a> {
    fn text(&mut self, text: &str) {
        if let Some(c) = &mut self.capture {
            c.push_str(text);
        }
        escape(text, &mut self.out);
    }

    /// The list levels opened and closed to reach `level` (the [hlN] code), as the original keeps them: `</ol>` down
    /// to the level, `<li class="lvlNofM">` and `<ol>` up to it, then the item of the new level.
    fn list_level(&mut self, level: u32) {
        if self.item_open {
            self.out.push_str("</li>");
            self.item_open = false;
        }
        while self.level > level {
            self.out.push_str("</ol>");
            self.level -= 1;
            if self.level >= 1 {
                self.out.push_str("</li>");
            }
        }
        while self.level < level {
            if self.level > 0 {
                self.out.push_str(&format!("<li class=\"lvl{}of{}\">", self.level, self.maxdepth));
            }
            self.out.push_str("<ol>");
            self.level += 1;
        }
        self.out.push_str(&format!("<li class=\"lvl{}of{}\">", level, self.maxdepth));
        self.item_open = true;
    }

    /// Closes the open item and every nested level down to the list element itself (level 1), whose end tag the
    /// style's end writes.
    fn close_list(&mut self) {
        if self.item_open {
            self.out.push_str("</li>");
            self.item_open = false;
        }
        while self.level > 1 {
            self.out.push_str("</ol>");
            self.level -= 1;
            self.out.push_str("</li>");
        }
        self.level = 0;
    }

    fn style_start(&mut self, name: &str) {
        let kind = (self.kinds)(name);
        let class = class_of(name);
        match kind {
            StyleKind::Block | StyleKind::Heading(_) => {
                // INFERENCE: the element of a paragraph style is p (the style sheet's rules are p.class).
                self.out.push_str(&format!("<p class=\"{class}\">"));
                self.open.push(("p", kind));
                if matches!(kind, StyleKind::Heading(_)) {
                    self.capture = Some(String::new());
                }
            }
            StyleKind::List(depth) => {
                self.out.push_str(&format!("<ol class=\"{class}\" bc:maxdepth=\"{depth}\">"));
                self.open.push(("ol", kind));
                self.maxdepth = u32::from(depth);
                self.level = 1;
                self.out.push_str(&format!("<li class=\"lvl1of{depth}\">"));
                self.item_open = true;
            }
            StyleKind::PageNumber => {
                self.out.push_str("<p class=\"refpagenum\" id=\"\" role=\"doc-pagebreak\" aria-label-src=\"content\" hrefsrc=\"content\" hrefprefix=\"page\">");
                self.open.push(("p", kind));
                self.capture = Some(String::new());
            }
            StyleKind::GuideDots => {
                self.out.push_str("<a hrefsrc=\"content\" hrefprefix=\"#page\">");
                self.open.push(("a", kind));
            }
            StyleKind::Inline => {}
        }
    }

    fn style_end(&mut self, name: &str) {
        let kind = (self.kinds)(name);
        if kind == StyleKind::Inline {
            return;
        }
        let Some((element, open_kind)) = self.open.pop() else { return };
        match open_kind {
            StyleKind::List(_) => {
                self.close_list();
                self.out.push_str("</ol>");
            }
            StyleKind::Heading(_) => {
                let text = self.capture.take().unwrap_or_default();
                let id = format!("toc{:03}", self.headings.len() + 1);
                // The id goes on the element: it was written without one, so it is put in now.
                let open_tag = self.out.rfind("<p class=\"").unwrap_or(0);
                let _ = open_tag;
                self.headings.push((id, text));
                self.out.push_str("</p>");
            }
            StyleKind::PageNumber => {
                let text = self.capture.take().unwrap_or_default();
                let id = format!("page{}", text.trim());
                // The add-anchors step fills the id from the content: id="" becomes the page id.
                if let Some(at) = self.out.rfind("id=\"\"") {
                    self.out.replace_range(at..at + 5, &format!("id=\"{}\"", id));
                }
                self.pages.push((id, text));
                self.out.push_str("</p>");
            }
            _ => {
                self.out.push_str(&format!("</{element}>"));
            }
        }
    }

    fn code(&mut self, code: &str) {
        let (name, arg) = match code.find('~') {
            Some(p) => (&code[..p], Some(&code[p + 1..])),
            None => (code, None),
        };
        match (name, arg) {
            ("es", Some(style)) => self.style_start(style),
            ("ee", Some(style)) => self.style_end(style),
            ("d", Some(text)) => {
                self.text(text);
                self.text(" ");
            }
            ("q", Some(text)) => self.text(text),
            ("l", _) | ("run", _) => {}
            ("<", _) => {
                if self.in_table {
                    self.out.push_str("</td></tr><tr><td>");
                } else {
                    // INFERENCE: the line break code counts a line in the original; written as a break here.
                    self.out.push_str("<br/>");
                }
            }
            (">", _) if self.in_table => self.out.push_str("</td><td>"),
            ("htbs", _) if code.starts_with("htbs;m") => {}
            (n, _) if n == "htbs" || code.starts_with("htbs;") => {
                self.close_list();
                self.out.push_str("<table><tr><td>");
                self.in_table = true;
            }
            ("htbe", _) => {
                if self.in_table {
                    self.out.push_str("</td></tr></table>");
                    self.in_table = false;
                }
            }
            ("rps:a", Some(target)) => {
                self.out.push_str("<a name=\"");
                escape(target, &mut self.out);
                self.out.push_str("\"/>");
            }
            ("rps:h", Some(target)) => {
                self.out.push_str("<a href=\"");
                escape(target, &mut self.out);
                self.out.push_str("\">");
            }
            ("rpe:h", _) => self.out.push_str("</a>"),
            (n, _) if n.starts_with("hl") => {
                if let Ok(level) = n[2..].parse::<u32>() {
                    if self.open.last().is_some_and(|(_, k)| matches!(k, StyleKind::List(_))) {
                        self.list_level(level);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The content page from the running text (codes marked 0x1C ... 0x1F); `kinds` tells how each style is written.
pub fn page(running: &str, kinds: &dyn Fn(&str) -> StyleKind) -> Page {
    let mut b = Builder { out: String::with_capacity(running.len() * 2), kinds, open: Vec::new(), level: 0, maxdepth: 1, item_open: false, in_table: false, headings: Vec::new(), pages: Vec::new(), capture: None };
    let mut text = String::new();
    let mut chars = running.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1c}' {
            if !text.is_empty() {
                b.text(&text);
                text.clear();
            }
            let mut code = String::new();
            for d in chars.by_ref() {
                if d == '\u{1f}' {
                    break;
                }
                code.push(d);
            }
            b.code(&code);
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        b.text(&text);
    }
    b.close_list();
    while let Some((element, _)) = b.open.pop() {
        b.out.push_str(&format!("</{element}>"));
    }
    if b.in_table {
        b.out.push_str("</td></tr></table>");
    }
    // The heading ids: each heading element in order takes its toc id.
    let mut body = b.out;
    for (id, _) in &b.headings {
        if let Some(at) = body.find("<p class=\"h") {
            // The first heading element not yet given an id.
            let mut search = at;
            loop {
                let Some(found) = body[search..].find("<p class=\"h") else { break };
                let start = search + found;
                let end = body[start..].find('>').map(|e| start + e).unwrap_or(body.len());
                if !body[start..end].contains(" id=\"") {
                    body.insert_str(end, &format!(" id=\"{id}\""));
                    break;
                }
                search = end;
            }
        }
    }
    Page { body, headings: b.headings, pages: b.pages }
}

/// The text with every character below 0x80 as its Unicode braille cell (the encode step of a braille document).
pub fn encode_cells(text: &str, cell: &dyn Fn(char) -> u8) -> String {
    text.chars()
        .map(|c| if (c as u32) < 0x80 { char::from_u32(0x2800 + u32::from(cell(c) & 0x3f)).unwrap_or(c) } else { c })
        .collect()
}

/// The XHTML text nodes of `markup` through `encode`, the markup itself kept.
fn encode_text_nodes(markup: &str, encode: &dyn Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(markup.len() * 3);
    let mut rest = markup;
    while let Some(at) = rest.find('<') {
        out.push_str(&encode(&rest[..at]));
        let end = rest[at..].find('>').map(|e| at + e + 1).unwrap_or(rest.len());
        out.push_str(&rest[at..end]);
        rest = &rest[end..];
    }
    out.push_str(&encode(rest));
    out
}

/// A text node's characters, the entities undone first and put back after (the original's encode callback).
fn encode_node(text: &str, cell: &dyn Fn(char) -> u8) -> String {
    let plain = text.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
    let cells = encode_cells(&plain, cell);
    let mut out = String::new();
    escape(&cells, &mut out);
    out
}

/// The civil date of a number of days since 1970-01-01.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Today as YYYY-MM-DD.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (y, m, d) = civil(secs.div_euclid(86400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// A random version 4 UUID.
pub fn uuid() -> String {
    let mut h = RandomState::new().build_hasher();
    h.write_u64(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0));
    let a = h.finish();
    let mut h2 = RandomState::new().build_hasher();
    h2.write_u64(a.rotate_left(17));
    let b = h2.finish();
    let bytes = [a.to_le_bytes(), b.to_le_bytes()].concat();
    let mut u = bytes;
    u[6] = (u[6] & 0x0f) | 0x40;
    u[8] = (u[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u[0], u[1], u[2], u[3], u[4], u[5], u[6], u[7], u[8], u[9], u[10], u[11], u[12], u[13], u[14], u[15]
    )
}

fn xml_text(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The identifier as a URN by its type; with no identifier entered, a new UUID as the original's template gives
/// (INFERENCE: the dialog starts on ISBN, and an empty ISBN is not written).
fn urn(kind: u8, value: &str) -> String {
    let value = value.trim();
    match (kind, value.is_empty()) {
        (_, true) => format!("urn:uuid:{}", uuid()),
        (0, _) => format!("urn:isbn:{value}"),
        (1, _) => format!("urn:doi:{value}"),
        _ => format!("urn:uuid:{value}"),
    }
}

/// The package's metadata element text (the original's template, package.opf, with the dialog's values): the
/// template's elements in its order, with the Required page's lists written one element each; the Recommended
/// page's as Dublin Core elements (INFERENCE for the education levels, written as schema:educationalLevel); the
/// Additional page's as meta elements with their attributes.
pub fn metadata_xml(meta: &Metadata, identifier: &str) -> String {
    if let Some(edited) = &meta.edited_xml {
        return edited.clone();
    }
    let date = if meta.date.is_empty() { today() } else { meta.date.clone() };
    let copyrighted = if meta.date_copyrighted.is_empty() { date.clone() } else { meta.date_copyrighted.clone() };
    let mut out = String::from("  <metadata>\n");
    let each = |out: &mut String, element: &str, field: &str, values: &[String]| {
        let attrs = meta.attributes_of(field);
        let written: Vec<&String> = values.iter().filter(|v| !v.trim().is_empty()).collect();
        if written.is_empty() {
            out.push_str(&format!("    <{element}{attrs}></{element}>\n"));
        }
        for v in written {
            out.push_str(&format!("    <{element}{attrs}>{}</{element}>\n", xml_text(v.trim())));
        }
    };
    each(&mut out, "dc:creator", "creator", &meta.creators);
    out.push_str(&format!("    <dc:date{}>{}</dc:date>\n", meta.attributes_of("date"), xml_text(&date)));
    out.push_str(&format!("    <dc:identifier{}>{}</dc:identifier>\n", meta.attributes_of("identifier"), xml_text(identifier)));
    each(&mut out, "dc:title", "title", &meta.titles);
    for l in meta.languages.iter().filter(|l| !l.trim().is_empty()) {
        out.push_str(&format!("    <dc:language>{}</dc:language>\n", xml_text(l.trim())));
    }
    for s in &meta.braille_systems {
        // INFERENCE: the braille system value is its language, grade and specialization joined by hyphens.
        let mut value = format!("{}-{}", s.language.trim(), s.grade.trim());
        if !s.specialization.is_empty() && s.specialization != "None" {
            value.push('-');
            value.push_str(s.specialization.trim());
        }
        out.push_str(&format!("    <meta property=\"a11y:brailleSystem\">{}</meta>\n", xml_text(&value)));
    }
    out.push_str(&format!("    <meta property=\"a11y:completeTranscription\">{}</meta>\n", meta.complete_transcription));
    let producers: Vec<&String> = meta.producers.iter().filter(|p| !p.trim().is_empty()).collect();
    if producers.is_empty() {
        out.push_str("    <meta property=\"a11y:producer\"></meta>\n");
    }
    for p in producers {
        out.push_str(&format!("    <meta property=\"a11y:producer\">{}</meta>\n", xml_text(p.trim())));
    }
    // INFERENCE: the template's value PDF when the Tactile Graphics box is checked, none when it is not.
    out.push_str(&format!("    <meta property=\"a11y:tactileGraphics\">{}</meta>\n", if meta.tactile_graphics { "PDF" } else { "none" }));
    out.push_str(&format!("    <meta property=\"dcterms:dateCopyrighted\">{}</meta>\n", xml_text(&copyrighted)));
    out.push_str(&format!("    <meta property=\"dcterms:modified\">{}</meta>\n", xml_text(&today())));
    for p in meta.publishers.iter().filter(|p| !p.trim().is_empty()) {
        out.push_str(&format!("    <dc:publisher{}>{}</dc:publisher>\n", meta.attributes_of("publisher"), xml_text(p.trim())));
    }
    for e in meta.education_levels.iter().filter(|e| !e.trim().is_empty()) {
        out.push_str(&format!("    <meta property=\"schema:educationalLevel\">{}</meta>\n", xml_text(e.trim())));
    }
    if !meta.description.trim().is_empty() {
        out.push_str(&format!("    <dc:description{}>{}</dc:description>\n", meta.attributes_of("description"), xml_text(meta.description.trim())));
    }
    for r in meta.rights.iter().filter(|r| !r.trim().is_empty()) {
        out.push_str(&format!("    <dc:rights{}>{}</dc:rights>\n", meta.attributes_of("rights"), xml_text(r.trim())));
    }
    if !meta.source.trim().is_empty() {
        out.push_str(&format!("    <dc:source{}>{}</dc:source>\n", meta.attributes_of("source"), xml_text(&urn(meta.source_type, &meta.source))));
    }
    for s in meta.subjects.iter().filter(|s| !s.trim().is_empty()) {
        out.push_str(&format!("    <dc:subject{}>{}</dc:subject>\n", meta.attributes_of("subject"), xml_text(s.trim())));
    }
    for p in meta.properties.iter().filter(|p| !p.name.trim().is_empty()) {
        out.push_str(&format!("    <meta property=\"{}\"", xml_text(p.name.trim())));
        for (n, v) in &p.attributes {
            out.push_str(&format!(" {}=\"{}\"", xml_text(n.trim()), xml_text(v)));
        }
        out.push_str(&format!(">{}</meta>\n", xml_text(&p.value)));
    }
    out.push_str("  </metadata>\n");
    out
}

/// What a package is made from.
pub struct Input<'a> {
    /// The document's running text (codes marked 0x1C ... 0x1F).
    pub running: &'a str,
    /// A braille document (eBraille) or a print document (EPUB 3).
    pub braille: bool,
    pub kinds: &'a dyn Fn(&str) -> StyleKind,
    /// The cell of a braille character, for the encode step of a braille document.
    pub cell: &'a dyn Fn(char) -> u8,
    pub meta: &'a Metadata,
}

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const OPS: &str = "http://www.idpf.org/2007/ops";

/// The package bytes: a zip with the mimetype entry first, stored.
pub fn package(input: &Input) -> Vec<u8> {
    let folder = if input.braille { "ebraille" } else { "epub" };
    let page = page(input.running, input.kinds);
    let encode = |text: &str| -> String { if input.braille { encode_node(text, input.cell) } else { text.to_string() } };
    let body = if input.braille { encode_text_nodes(&page.body, &encode) } else { page.body.clone() };
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"{XHTML}\" xmlns:bc=\"{NAMESPACE}\">\n<head xmlns:epub=\"{OPS}\">\n<title>Document Title</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"css/ebrl.css\"/>\n</head><body>{body}</body></html>\n"
    );
    // The nav headings: print text for EPUB, the braille text of each as cells for eBraille.
    let heading = |print: &str, braille_ascii: &str| -> String {
        if input.braille {
            let mut out = String::new();
            escape(&encode_cells(braille_ascii, input.cell), &mut out);
            out
        } else {
            print.to_string()
        }
    };
    let part = format!("{folder}/part0001.xml");
    let mut navs = String::new();
    navs.push_str(&format!("<nav epub:type=\"toc\" role=\"doc-toc\" aria-label=\"Table of Contents\"><h2>{}</h2>", heading("Table of Contents", ",table of ,contents")));
    if !page.headings.is_empty() {
        navs.push_str("<ol>");
        for (id, text) in &page.headings {
            navs.push_str(&format!("<li><a href=\"{part}#{id}\">{}</a></li>", encode(&{
                let mut t = String::new();
                escape(text, &mut t);
                t
            })));
        }
        navs.push_str("</ol>");
    }
    navs.push_str("</nav>");
    navs.push_str(&format!("<nav epub:type=\"page-list\" role=\"doc-pagelist\" aria-label=\"Page List\"><h2>{}</h2>", heading("Page List", ",page ,list")));
    if !page.pages.is_empty() {
        navs.push_str("<ol>");
        for (id, label) in &page.pages {
            let mut t = String::new();
            escape(label, &mut t);
            let shown = encode(&t);
            if input.braille {
                navs.push_str(&format!("<li><a href=\"{part}#{id}\" title=\"{}\">{shown}</a></li>", xml_text(label)));
            } else {
                navs.push_str(&format!("<li><a href=\"{part}#{id}\">{shown}</a></li>"));
            }
        }
        navs.push_str("</ol>");
    }
    navs.push_str("</nav>");
    navs.push_str(&format!("<nav epub:type=\"landmarks\" aria-label=\"Landmarks\"><h2>{}</h2></nav>", heading("Landmarks", ",landmarks")));
    let index = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"{XHTML}\" xmlns:epub=\"{OPS}\" xmlns:bc=\"{NAMESPACE}\">\n<head><title>Document Title</title>\n<link rel=\"publication\" href=\"package.opf\" type=\"application/oebps-package+xml\"/>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"{folder}/css/ebrl.css\"/>\n</head><body>{navs}</body></html>\n"
    );
    let identifier = urn(input.meta.id_type, &input.meta.identifier);
    let unique = if input.meta.id_type == 2 && input.meta.identifier.trim().is_empty() { identifier.clone() } else { identifier.clone() };
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" xmlns:a11y=\"http://idpf.org/epub/vocab/package/a11y/#\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xml:lang=\"en\" version=\"3.0\" unique-identifier=\"{}\">\n{}  <manifest>\n    <item id=\"part0001\" href=\"{part}\" media-type=\"application/xhtml+xml\"/>\n    <item id=\"nav\" href=\"index.html\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n    <item id=\"css\" href=\"{folder}/css/ebrl.css\" media-type=\"text/css\"/>\n  </manifest>\n  <spine>\n    <itemref idref=\"part0001\"/>\n  </spine>\n</package>\n",
        xml_text(&unique),
        metadata_xml(input.meta, &identifier)
    );
    let container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n  <rootfiles>\n    <rootfile full-path=\"package.opf\" media-type=\"application/oebps-package+xml\"/>\n  </rootfiles>\n</container>\n";
    let css_name = format!("{folder}/css/ebrl.css");
    let entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/epub+zip"),
        ("index.html", index.as_bytes()),
        (css_name.as_str(), STYLE_SHEET.as_bytes()),
        (part.as_str(), content.as_bytes()),
        ("package.opf", opf.as_bytes()),
        ("META-INF/container.xml", container.as_bytes()),
    ];
    docx::stored_zip(&entries)
}

/// The quality notice shown before every eBraille and EPUB 3 save until its box is checked (the original's text
/// with the vendor's team unnamed), kept under Mutable, EBrailleQualityWarning.
pub const QUALITY_WARNING: &str = "eBraille exports should be reliably working to produce readable eBraille files, but the quality is not yet reliable. Any sample files sent to the support team will help us to improve the quality.";
pub const QUALITY_WARNING_KEY: &str = "EBrailleQualityWarning";

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(name: &str) -> StyleKind {
        match name {
            "h1." | "h2." => StyleKind::Heading(1),
            "list." => StyleKind::List(2),
            "pagenum." => StyleKind::PageNumber,
            "GuideDots" => StyleKind::GuideDots,
            "b" => StyleKind::Inline,
            _ => StyleKind::Block,
        }
    }

    #[test]
    fn the_page_writes_elements_lists_pages_and_tables() {
        let running = "\x1ces~h1.\x1fTitle & more\x1cee~h1.\x1f\x1ces~para.\x1fBody\x1cl\x1fnext\x1cee~para.\x1f\x1ces~list.\x1fone\x1chl2\x1ftwo\x1chl1\x1fthree\x1cee~list.\x1f\x1ces~pagenum.\x1f12\x1cee~pagenum.\x1f\x1chtbs;r:0\x1fa\x1c>\x1fb\x1c<\x1fc\x1chtbe\x1f\x1ces~1-3.\x1fx\x1cee~1-3.\x1f";
        let p = page(running, &kinds);
        // A deeper level opens a new item holding its own list, as the original's level handler writes it.
        assert_eq!(p.body, "<p class=\"h1\" id=\"toc001\">Title &amp; more</p><p class=\"para\">Bodynext</p><ol class=\"list\" bc:maxdepth=\"2\"><li class=\"lvl1of2\">one</li><li class=\"lvl1of2\"><ol><li class=\"lvl2of2\">two</li></ol></li><li class=\"lvl1of2\">three</li></ol><p class=\"refpagenum\" id=\"page12\" role=\"doc-pagebreak\" aria-label-src=\"content\" hrefsrc=\"content\" hrefprefix=\"page\">12</p><table><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></table><p class=\"_1-3\">x</p>");
        assert_eq!(p.headings, vec![("toc001".to_string(), "Title & more".to_string())]);
        assert_eq!(p.pages, vec![("page12".to_string(), "12".to_string())]);
    }

    #[test]
    fn a_braille_package_holds_the_six_entries_with_cells() {
        let meta = Metadata { titles: vec!["My Book".into()], id_type: 2, ..Metadata::default() };
        let cell = |c: char| -> u8 { if c == 'a' { 1 } else if c == ',' { 0x20 } else { 0 } };
        let bytes = package(&Input { running: "\x1ces~h1.\x1fa\x1cee~h1.\x1f", braille: true, kinds: &kinds, cell: &cell, meta: &meta });
        let parts = docx::parts(&bytes).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["mimetype", "index.html", "ebraille/css/ebrl.css", "ebraille/part0001.xml", "package.opf", "META-INF/container.xml"]);
        assert_eq!(docx::part(&parts, "mimetype").unwrap(), b"application/epub+zip");
        let content = String::from_utf8(docx::part(&parts, "ebraille/part0001.xml").unwrap().to_vec()).unwrap();
        assert!(content.contains("<p class=\"h1\" id=\"toc001\">\u{2801}</p>"), "{content}");
        let index = String::from_utf8(docx::part(&parts, "index.html").unwrap().to_vec()).unwrap();
        // ",table of ,contents" as cells (the test's cell map knows only a and the comma).
        assert!(index.contains("<h2>\u{2820}\u{2800}\u{2801}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2820}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}</h2>"), "{index}");
        assert!(index.contains("<li><a href=\"ebraille/part0001.xml#toc001\">\u{2801}</a></li>"), "{index}");
        let opf = String::from_utf8(docx::part(&parts, "package.opf").unwrap().to_vec()).unwrap();
        assert!(opf.contains("<dc:title>My Book</dc:title>") && opf.contains("urn:uuid:") && opf.contains("<meta property=\"a11y:tactileGraphics\">none</meta>"), "{opf}");
    }

    /// Every shipped sample document (BC_DX_SAMPLES) packs, and its content page is well-formed XML.
    #[test]
    fn the_sample_documents_pack_as_well_formed_pages() {
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
            if ext != "dxb" && ext != "dxp" {
                continue;
            }
            let Ok(doc) = crate::dxfile::read(&std::fs::read(&path).unwrap()) else { continue };
            let styles = doc.styles.clone();
            let kinds = |name: &str| styles.iter().find(|s| s.0 == name).map_or(StyleKind::Block, |s| kind_of(&s.0, &s.1));
            let cell = |c: char| (c as u32 & 0x3f) as u8;
            let meta = Metadata::default();
            let bytes = package(&Input { running: &doc.text, braille: ext == "dxb", kinds: &kinds, cell: &cell, meta: &meta });
            let parts = docx::parts(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let folder = if ext == "dxb" { "ebraille" } else { "epub" };
            let content = String::from_utf8(docx::part(&parts, &format!("{folder}/part0001.xml")).unwrap().to_vec()).unwrap();
            docx::read_xml(&content).unwrap_or_else(|e| panic!("{}: content page not well formed: {e}", path.display()));
            let index = String::from_utf8(docx::part(&parts, "index.html").unwrap().to_vec()).unwrap();
            docx::read_xml(&index).unwrap_or_else(|e| panic!("{}: index not well formed: {e}", path.display()));
            let opf = String::from_utf8(docx::part(&parts, "package.opf").unwrap().to_vec()).unwrap();
            docx::read_xml(&opf).unwrap_or_else(|e| panic!("{}: package.opf not well formed: {e}", path.display()));
            count += 1;
        }
        assert!(count > 0, "no sample documents in {dir}");
    }

    #[test]
    fn the_metadata_follows_the_template_order() {
        let meta = Metadata { creators: vec!["A".into(), "B".into()], producers: vec![], languages: vec!["en".into()], id_type: 0, identifier: "123".into(), tactile_graphics: true, complete_transcription: true, properties: vec![Property { name: "a11y:certifiedBy".into(), value: "X".into(), attributes: vec![("id".into(), "c1".into())] }], ..Metadata::default() };
        let xml = metadata_xml(&meta, "urn:isbn:123");
        let order = ["<dc:creator>A</dc:creator>", "<dc:creator>B</dc:creator>", "<dc:date>", "<dc:identifier>urn:isbn:123</dc:identifier>", "<dc:title></dc:title>", "<dc:language>en</dc:language>", "a11y:completeTranscription\">true", "<meta property=\"a11y:producer\"></meta>", "a11y:tactileGraphics\">PDF", "dcterms:dateCopyrighted", "dcterms:modified", "<meta property=\"a11y:certifiedBy\" id=\"c1\">X</meta>"];
        let mut at = 0;
        for piece in order {
            let found = xml[at..].find(piece).unwrap_or_else(|| panic!("{piece} missing or out of order in {xml}"));
            at += found;
        }
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20743), (2026, 10, 17));
        assert_eq!(uuid().len(), 36);
    }
}
