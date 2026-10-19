//! Save As eBraille (braille documents) and EPUB 3 (print documents), as written (spec 23): a zip
//! package holding the mimetype, the content folder ("ebraille" or "epub") with its style sheet and one content
//! page part0001.xml, index.html with the table of contents, page list and landmarks navs, package.opf with the
//! document's metadata, and META-INF/container.xml. The content page is the document body as XHTML: block styles
//! (a name ending in ".") as div elements with the style's class, list styles (and block styles whose items start
//! with level codes) as ol and li, page break styles as doc-pagebreak elements, tables, links; in a braille
//! document every text character becomes its Unicode braille cell. The metadata comes from Document, eBraille Metadata (the `Metadata` struct) and is written as the
//! package's metadata element; the document file keeps it as package.opf text (`opf_text`, `metadata_from_opf`).
//! BrailleCraft's own rule: the saved package follows EPUB 3 so reading apps open it (see `package`).
//!
//! Not known, and marked where it is used: the element and class of a block style that has its own entry in the
//! style map (div and the style's class are written), the depth of a list style, the page list labels, the markup of the line
//! break code, and which control of the dialog writes which property.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::docx;

/// The style sheet written as the content folder's css/ebrl.css (the standard, with our own namespace).
pub const STYLE_SHEET: &str = include_str!("../data/ebraille.css");

/// The namespace of the maxdepth attribute, as the style sheet declares it.
pub const NAMESPACE: &str = "https://blindcraft.dev/braillecraft";

/// A braille system of the document, as the Braille Systems dialog lists it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrailleSystem {
    /// The language; with no grade, the whole system text as it is written (a translation table entry's eBraille
    /// system, such as "EBAE grade2", or a value read back from a document file).
    pub language: String,
    /// grade0, grade1, grade2 or no-grade; empty for a system text taken as it is.
    pub grade: String,
    /// None, comp6, math, music or phonetic.
    pub specialization: String,
}

impl BrailleSystem {
    /// The a11y:brailleSystem value: language, grade and specialization joined by hyphens (INFERENCE: the
    /// dialog's own joining is not known); a system with no grade is its text alone.
    pub fn value(&self) -> String {
        if self.grade.trim().is_empty() {
            return self.language.trim().to_string();
        }
        let mut value = format!("{}-{}", self.language.trim(), self.grade.trim());
        if !self.specialization.is_empty() && self.specialization != "None" {
            value.push('-');
            value.push_str(self.specialization.trim());
        }
        value
    }

    /// A system from its a11y:brailleSystem value: "language-grade[-specialization]" when the grade is one the
    /// dialog offers, else the whole text as the language with no grade.
    pub fn from_value(value: &str) -> BrailleSystem {
        let value = value.trim();
        let parts: Vec<&str> = value.rsplitn(3, '-').collect();
        let plain = BrailleSystem { language: value.to_string(), grade: String::new(), specialization: String::new() };
        match parts.as_slice() {
            [spec, grade, language] if GRADES.contains(grade) && SPECIALIZATIONS.contains(spec) => {
                BrailleSystem { language: language.to_string(), grade: grade.to_string(), specialization: spec.to_string() }
            }
            [grade, _, ..] if GRADES.contains(grade) => {
                let language = value[..value.len() - grade.len() - 1].to_string();
                BrailleSystem { language, grade: grade.to_string(), specialization: "None".to_string() }
            }
            _ => plain,
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl Default for Metadata {
    /// Empty metadata with the identifier type UUID: the package template's identifier is "urn:uuid:{GUID}".
    fn default() -> Metadata {
        Metadata {
            titles: Vec::new(),
            creators: Vec::new(),
            producers: Vec::new(),
            languages: Vec::new(),
            braille_systems: Vec::new(),
            date: String::new(),
            id_type: 2,
            identifier: String::new(),
            date_copyrighted: String::new(),
            tactile_graphics: false,
            complete_transcription: false,
            publishers: Vec::new(),
            education_levels: Vec::new(),
            description: String::new(),
            rights: Vec::new(),
            source_type: 0,
            source: String::new(),
            subjects: Vec::new(),
            properties: Vec::new(),
            field_attributes: Vec::new(),
            edited_xml: None,
        }
    }
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

    /// The metadata with the document's translation table defaults put in where it names none: `language` (the
    /// table entry's eBraille language) as the one dc:language when there is no language, and `system` (its
    /// eBraille braille system) as the one a11y:brailleSystem when there is no system. The table list's
    /// placeholders "language" and "system" mean none and are not written.
    pub fn with_defaults(&self, language: Option<&str>, system: Option<&str>) -> Metadata {
        let mut m = self.clone();
        if m.languages.iter().all(|l| l.trim().is_empty()) {
            if let Some(l) = language.map(str::trim).filter(|l| !l.is_empty() && *l != "language") {
                m.languages = vec![l.to_string()];
            }
        }
        if m.braille_systems.iter().all(|s| s.value().is_empty()) {
            if let Some(s) = system.map(str::trim).filter(|s| !s.is_empty() && *s != "system") {
                m.braille_systems = vec![BrailleSystem { language: s.to_string(), grade: String::new(), specialization: String::new() }];
            }
        }
        m
    }
}

/// The names the Options... dialogs offer: XML attributes an OPF metadata element may carry (INFERENCE: the
/// list is not known).
pub const ATTRIBUTE_NAMES: [&str; 6] = ["id", "xml:lang", "dir", "scheme", "refines", "property"];
/// The property names the package code holds (the a11y:, dc: and dcterms: names); INFERENCE: that they are the
/// list the Additional page offers.
pub const PROPERTY_NAMES: [&str; 62] = [
    "a11y:brailleCellType",
    "a11y:brailleSystem",
    "a11y:certifiedBy",
    "a11y:certifierCredential",
    "a11y:certifierReport",
    "a11y:completeTranscription",
    "a11y:graphicType",
    "a11y:producer",
    "a11y:tactileGraphics",
    "dc:contributor",
    "dc:coverage",
    "dc:created",
    "dc:creator",
    "dc:date",
    "dc:dateAccepted",
    "dc:dateSubmitted",
    "dc:description",
    "dc:extent",
    "dc:format",
    "dc:hasFormat",
    "dc:hasPart",
    "dc:hasVersion",
    "dc:identifier",
    "dc:instructionalMethod",
    "dc:isFormatOf",
    "dc:isPartOf",
    "dc:isReferencedBy",
    "dc:isReplacedBy",
    "dc:isRequiredBy",
    "dc:issued",
    "dc:isVersionOf",
    "dc:language",
    "dc:license",
    "dc:mediator",
    "dc:medium",
    "dc:provenance",
    "dc:publisher",
    "dc:references",
    "dc:relation",
    "dc:replaces",
    "dc:requires",
    "dc:rights",
    "dc:rightsHolder",
    "dc:source",
    "dc:spatial",
    "dc:subject",
    "dc:tableOfContents",
    "dc:temporal",
    "dc:title",
    "dc:type",
    "dc:valid",
    "dcterms:dateCopyrighted",
    "dcterms:educationLevel",
    "dcterms:modified",
    "rendition:flow",
    "rendition:layout",
    "rendition:orientation",
    "rendition:spread",
    "media:active-class",
    "media:duration",
    "media:narrator",
    "media:playback-active-class",
];
pub const GRADES: [&str; 4] = ["grade0", "grade1", "grade2", "no-grade"];
pub const SPECIALIZATIONS: [&str; 5] = ["None", "comp6", "math", "music", "phonetic"];

/// How a style is written, from its name and its beginning codes (running text, codes marked 0x1C ... 0x1F):
/// - block: the name ends with "." (a paragraph style, written as an element);
/// - toc: a beginning code starts with "rps" (a table of contents entry: the templates' h1. [rps1], h2. [rps2]);
/// - page break: a beginning code starts with "lea" (the print page number marker: a doc-pagebreak element and a
///   page list entry; RefPage. and Lea in the templates);
/// - list: a beginning code starts with "mfcs" (Poetry. [mfcs~poetry]);
/// - guide dots: the style named GuideDots.
///
/// Level codes (hl, hi) do not make a list, and a style that is none of these writes nothing (a character style).
pub fn kind_of(name: &str, begin: &str) -> StyleKind {
    let codes: Vec<&str> = begin.split('\u{1c}').skip(1).filter_map(|c| c.split('\u{1f}').next()).collect();
    let list = codes.iter().any(|c| c.starts_with("mfcs")).then(|| {
        // Not known: the depth the formatter gives a list style; the deepest [hlN] level among its beginning
        // codes is taken, else 1.
        let levels = codes.iter().filter_map(|c| c.strip_prefix("hl").and_then(|n| n.chars().take_while(|d| d.is_ascii_digit()).collect::<String>().parse::<u8>().ok()));
        levels.max().unwrap_or(1).max(1)
    });
    StyleKind {
        block: name.ends_with('.'),
        toc: codes.iter().any(|c| c.starts_with("rps")),
        page_break: codes.iter().any(|c| c.starts_with("lea")),
        list,
        guide_dots: name == "GuideDots",
    }
}

/// What the page builder needs to know of a style: how it is written. A style can be more than one of these (a
/// heading is a block style and a table of contents entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleKind {
    /// A paragraph style (its name ends with "."): an element with the style's class.
    pub block: bool,
    /// A table of contents entry.
    pub toc: bool,
    /// A print page break (a doc-pagebreak element, a page list entry).
    pub page_break: bool,
    /// A list style with its deepest level (the maxdepth attribute).
    pub list: Option<u8>,
    /// The GuideDots style: a contents entry's link to its page.
    pub guide_dots: bool,
}

impl StyleKind {
    /// A paragraph style that is nothing else.
    pub const BLOCK: StyleKind = StyleKind { block: true, toc: false, page_break: false, list: None, guide_dots: false };

    /// Whether the style writes an element.
    fn writes(&self) -> bool {
        self.page_break || self.block || self.guide_dots
    }
}

/// The element a block style is written as. Not known: the element and class of a style that has its own entry
/// in the style map; every block style is written as div with its own class.
const BLOCK_ELEMENT: &str = "div";

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

/// What an open element of the page is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Role {
    /// A block element; with its table of contents id when it is an entry.
    Block(Option<String>),
    /// A list, with its class and what made it.
    List(String, ListFrom),
    /// A page break element, and where its content starts.
    PageBreak(usize),
    /// The GuideDots link.
    Link,
}

/// What made a list element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListFrom {
    /// A list style (an [mfcs] beginning code): the list is the style's element.
    Style,
    /// A block style whose text starts with a level code ([hlN]): the style's element became the list.
    Block,
    /// A block style with text before its first level code: the list is opened inside the style's element and
    /// closes with it.
    Inside,
}

/// The depth written in a list made from level codes until the list ends and its deepest level is known.
const DEPTH_MARK: &str = "\u{1}";

struct Open {
    element: &'static str,
    role: Role,
    /// Where its start tag begins in the page.
    start: usize,
}

struct Builder<'a> {
    out: String,
    kinds: &'a dyn Fn(&str) -> StyleKind,
    /// Open elements, innermost last.
    open: Vec<Open>,
    /// The list level counter and the deepest level of the open list, and whether a list item is open.
    level: u32,
    maxdepth: u32,
    item_open: bool,
    /// The open list was made from level codes: its depth is written as DEPTH_MARK and filled in at its end
    /// with the deepest level reached.
    implied: bool,
    deepest: u32,
    in_table: bool,
    headings: Vec<(String, String)>,
    pages: Vec<(String, String)>,
    /// The text of the open table of contents or page break element, for its nav entry.
    capture: Option<String>,
}

/// The markup pieces at the top of a fragment: its `a` elements whole, and all its text (the text of every
/// element inside it too), both as written.
fn links_and_text(fragment: &str) -> (String, String) {
    let (mut links, mut text) = (String::new(), String::new());
    let mut depth = 0usize;
    let mut link_from: Option<usize> = None;
    let mut rest = 0;
    while let Some(found) = fragment[rest..].find('<') {
        let at = rest + found;
        text.push_str(&fragment[rest..at]);
        let end = fragment[at..].find('>').map_or(fragment.len(), |e| at + e + 1);
        let tag = &fragment[at..end];
        let is_a = tag.starts_with("<a ") || tag.starts_with("<a>") || tag.starts_with("<a/");
        if tag.starts_with("</") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                if let Some(from) = link_from.take() {
                    links.push_str(&fragment[from..end]);
                }
            }
        } else if tag.ends_with("/>") {
            if depth == 0 && is_a {
                links.push_str(tag);
            }
        } else {
            if depth == 0 && is_a {
                link_from = Some(at);
            }
            depth += 1;
        }
        rest = end;
    }
    text.push_str(&fragment[rest..]);
    (links, text)
}

/// The contents list's markup (a list of class c-contents): in each item of the list itself, its first span
/// becomes `<span class="title">` with `<span class="leaders"/>` at its end, and each link gets class="page".
fn contents_list(list: &str) -> String {
    let mut out = String::with_capacity(list.len() + 64);
    // The open element names, outermost first.
    let mut stack: Vec<String> = Vec::new();
    // Per item of the list itself: whether its first span has been seen, and the depth of that span when open.
    let mut span_seen = false;
    let mut title_at: Option<usize> = None;
    let mut rest = 0;
    while let Some(found) = list[rest..].find('<') {
        let at = rest + found;
        out.push_str(&list[rest..at]);
        let end = list[at..].find('>').map_or(list.len(), |e| at + e + 1);
        let tag = &list[at..end];
        let name: String = tag.trim_start_matches(['<', '/']).chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        let in_item = stack.len() == 2 && stack[1] == "li";
        if tag.starts_with("</") {
            if title_at == Some(stack.len()) && name == "span" {
                out.push_str("<span class=\"leaders\"/>");
                title_at = None;
            }
            stack.pop();
            out.push_str(tag);
        } else {
            let closes = tag.ends_with("/>");
            if stack.len() == 1 && name == "li" {
                span_seen = false;
            }
            if in_item && name == "span" && !span_seen {
                span_seen = true;
                if closes {
                    out.push_str("<span class=\"title\"><span class=\"leaders\"/></span>");
                } else {
                    out.push_str("<span class=\"title\">");
                    title_at = Some(stack.len() + 1);
                    stack.push(name);
                }
            } else if in_item && name == "a" {
                out.push_str(&with_class(tag, "page"));
                if !closes {
                    stack.push(name);
                }
            } else {
                out.push_str(tag);
                if !closes {
                    stack.push(name);
                }
            }
        }
        rest = end;
    }
    out.push_str(&list[rest..]);
    out
}

/// A start tag with its class attribute set to `class` (replaced when it has one).
fn with_class(tag: &str, class: &str) -> String {
    if let Some(at) = tag.find(" class=\"") {
        let from = at + 8;
        let to = tag[from..].find('"').map_or(tag.len(), |e| from + e);
        return format!("{}{class}{}", &tag[..from], &tag[to..]);
    }
    let cut = if tag.ends_with("/>") { tag.len() - 2 } else { tag.len() - 1 };
    format!("{} class=\"{class}\"{}", &tag[..cut], &tag[cut..])
}

impl<'a> Builder<'a> {
    fn text(&mut self, text: &str) {
        if let Some(c) = &mut self.capture {
            c.push_str(text);
        }
        escape(text, &mut self.out);
    }

    /// The list levels opened and closed to reach `level` (the [hlN] code): `</ol>` down
    /// to the level, `<li class="lvlNofM">` and `<ol>` up to it, then the item of the new level.
    fn list_level(&mut self, level: u32) {
        self.deepest = self.deepest.max(level);
        let depth = if self.implied { DEPTH_MARK.to_string() } else { self.maxdepth.to_string() };
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
                self.out.push_str(&format!("<li class=\"lvl{}of{depth}\">", self.level));
            }
            self.out.push_str("<ol>");
            self.level += 1;
        }
        self.out.push_str(&format!("<li class=\"lvl{level}of{depth}\">"));
        self.item_open = true;
    }

    /// A level code ([hlN]) in a plain block style with no list open: BrailleCraft's own rule, so that each item
    /// of a list made of level codes (an imported Markdown or HTML list: the list. style with [hl1] before each
    /// item) is its own li element and does not run into the next. A block whose text starts with the level code
    /// becomes the list (`<ol class="...">`); a block with text before it keeps its element and the list opens
    /// inside it. The list's depth (bc:maxdepth and the lvlNofM classes) is its deepest level, filled in at its end.
    fn implied_list(&mut self, level: u32) {
        let Some(open) = self.open.last_mut() else { return };
        let tag_end = self.out[open.start..].find('>').map_or(self.out.len(), |e| open.start + e + 1);
        let class = self.out[open.start..tag_end]
            .split_once("class=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map_or(String::new(), |(c, _)| c.to_string());
        if self.out.len() == tag_end {
            self.out.replace_range(open.start..tag_end, &format!("<ol class=\"{class}\" bc:maxdepth=\"{DEPTH_MARK}\">"));
            open.element = "ol";
            open.role = Role::List(class, ListFrom::Block);
        } else {
            let start = self.out.len();
            self.out.push_str(&format!("<ol bc:maxdepth=\"{DEPTH_MARK}\">"));
            self.open.push(Open { element: "ol", role: Role::List(class, ListFrom::Inside), start });
        }
        self.implied = true;
        self.deepest = 1;
        self.level = 1;
        self.item_open = false;
        self.list_level(level);
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

    /// A style's start: a page break style first (`<NAME class="refpagenum" ...>`, the aria-label-src attribute
    /// only for a block style, a span for any other); else a style that is not a block writes the GuideDots link or
    /// nothing; else a list style its list and any other block style its element.
    fn style_start(&mut self, name: &str) {
        let kind = (self.kinds)(name);
        let start = self.out.len();
        if kind.page_break {
            let element = if kind.block { BLOCK_ELEMENT } else { "span" };
            self.out.push_str(&format!("<{element} class=\"refpagenum\" id=\"\" role=\"doc-pagebreak\" "));
            if kind.block {
                self.out.push_str("aria-label-src=\"content\" ");
            }
            self.out.push_str("hrefsrc=\"content\" hrefprefix=\"page\">");
            self.open.push(Open { element, role: Role::PageBreak(self.out.len()), start });
            self.capture = Some(String::new());
        } else if !kind.block {
            if kind.guide_dots {
                self.out.push_str("<a hrefsrc=\"content\" hrefprefix=\"#page\">");
                self.open.push(Open { element: "a", role: Role::Link, start });
            }
        } else if let Some(depth) = kind.list {
            let class = class_of(name);
            self.out.push_str(&format!("<ol class=\"{class}\" bc:maxdepth=\"{depth}\">"));
            self.open.push(Open { element: "ol", role: Role::List(class, ListFrom::Style), start });
            self.maxdepth = u32::from(depth);
            self.level = 1;
            self.out.push_str(&format!("<li class=\"lvl1of{depth}\">"));
            self.item_open = true;
        } else {
            let class = class_of(name);
            let toc = kind.toc.then(|| format!("toc{:03}", self.headings.len() + 1));
            match &toc {
                Some(id) => {
                    self.out.push_str(&format!("<{BLOCK_ELEMENT} class=\"{class}\" id=\"{id}\">"));
                    self.capture = Some(String::new());
                }
                None => self.out.push_str(&format!("<{BLOCK_ELEMENT} class=\"{class}\">")),
            }
            if let Some(id) = &toc {
                // The entry's place in the list is kept from its start; its text is filled in at its end.
                self.headings.push((id.clone(), String::new()));
            }
            self.open.push(Open { element: BLOCK_ELEMENT, role: Role::Block(toc), start });
        }
    }

    fn style_end(&mut self, name: &str) {
        if !(self.kinds)(name).writes() {
            return;
        }
        let Some(open) = self.open.pop() else { return };
        match open.role {
            Role::List(class, from) => {
                self.close_list();
                self.out.push_str("</ol>");
                if self.implied {
                    let list = self.out[open.start..].replace(DEPTH_MARK, &self.deepest.max(1).to_string());
                    self.out.truncate(open.start);
                    self.out.push_str(&list);
                    self.implied = false;
                }
                if class == "c-contents" {
                    let list = contents_list(&self.out[open.start..]);
                    self.out.truncate(open.start);
                    self.out.push_str(&list);
                }
                // A list opened inside a block style ends with that style's end: the block closes too.
                if from == ListFrom::Inside {
                    if let Some(block) = self.open.pop() {
                        self.out.push_str(&format!("</{}>", block.element));
                    }
                }
            }
            Role::Block(Some(id)) => {
                let text = self.capture.take().unwrap_or_default();
                if let Some(entry) = self.headings.iter_mut().find(|(i, _)| *i == id) {
                    entry.1 = text;
                }
                self.out.push_str(&format!("</{}>", open.element));
            }
            Role::PageBreak(content) => {
                let text = self.capture.take().unwrap_or_default();
                // The page list ids count up: page001, page002 ... Not known: the label the page list shows
                // (the element's text is taken) and whether the anchor step gives the element this same id.
                let id = format!("page{:03}", self.pages.len() + 1);
                let (links, inner) = links_and_text(&self.out[content..]);
                self.out.truncate(content);
                if let Some(at) = self.out[open.start..].find("id=\"\"").map(|a| open.start + a) {
                    self.out.replace_range(at..at + 5, &format!("id=\"{id}\""));
                }
                // The page number's text after leaders: the element keeps its links, then its whole text.
                self.out.push_str(&links);
                self.out.push_str("<span class=\"leaders\"/><span class=\"number\">");
                self.out.push_str(&inner);
                self.out.push_str("</span>");
                self.pages.push((id, text));
                self.out.push_str(&format!("</{}>", open.element));
            }
            Role::Block(None) | Role::Link => {
                self.out.push_str(&format!("</{}>", open.element));
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
                    // INFERENCE: the line break code counts a line; written as a break here.
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
                    if self.open.last().is_some_and(|o| matches!(o.role, Role::List(..))) {
                        self.list_level(level);
                    } else if level >= 1
                        && !self.in_table
                        && !self.open.iter().any(|o| matches!(o.role, Role::List(..)))
                        && self.open.last().is_some_and(|o| o.role == Role::Block(None))
                    {
                        self.implied_list(level);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The content page from the running text (codes marked 0x1C ... 0x1F); `kinds` tells how each style is written.
pub fn page(running: &str, kinds: &dyn Fn(&str) -> StyleKind) -> Page {
    let mut b = Builder { out: String::with_capacity(running.len() * 2), kinds, open: Vec::new(), level: 0, maxdepth: 1, item_open: false, implied: false, deepest: 0, in_table: false, headings: Vec::new(), pages: Vec::new(), capture: None };
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
    while let Some(open) = b.open.pop() {
        // A page break left open has no id: its empty id attribute is not written (an id must not be empty).
        if matches!(open.role, Role::PageBreak(_)) {
            if let Some(at) = b.out[open.start..].find(" id=\"\"").map(|a| open.start + a) {
                b.out.replace_range(at..at + 6, "");
            }
        }
        b.out.push_str(&format!("</{}>", open.element));
    }
    if b.implied {
        b.out = b.out.replace(DEPTH_MARK, &b.deepest.max(1).to_string());
    }
    if b.in_table {
        b.out.push_str("</td></tr></table>");
    }
    Page { body: b.out, headings: b.headings, pages: b.pages }
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

/// A text node's characters, the entities undone first and put back after.
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

/// The time now in UTC: year, month, day, hour, minute, second.
pub fn utc_now() -> (i64, u32, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (y, m, d) = civil(secs.div_euclid(86400));
    let day = secs.rem_euclid(86400) as u32;
    (y, m, d, day / 3600, day / 60 % 60, day % 60)
}

/// Today as YYYY-MM-DD.
pub fn today() -> String {
    let (y, m, d, ..) = utc_now();
    format!("{y:04}-{m:02}-{d:02}")
}

/// The time now in UTC as CCYY-MM-DDThh:mm:ssZ, the form EPUB 3 requires of dcterms:modified.
pub fn modified_now() -> String {
    let (y, m, d, h, mi, sec) = utc_now();
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{sec:02}Z")
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

/// The identifier as a URN by its type; with no identifier entered, a new UUID. Not known: whether the save
/// replaces the template's "{GUID}" placeholder itself; a new UUID is written in its place.
fn urn(kind: u8, value: &str) -> String {
    let value = value.trim();
    match (kind, value.is_empty()) {
        (_, true) => format!("urn:uuid:{}", uuid()),
        (0, _) => format!("urn:isbn:{value}"),
        (1, _) => format!("urn:doi:{value}"),
        _ => format!("urn:uuid:{value}"),
    }
}

/// The identifier as the dialog's preview and the document file show it: the URN by its type, with the
/// template's placeholder "urn:uuid:{GUID}" when no identifier is entered (the save makes a new UUID then).
pub fn identifier_text(meta: &Metadata) -> String {
    let value = meta.identifier.trim();
    match (meta.id_type, value.is_empty()) {
        (_, true) => "urn:uuid:{GUID}".to_string(),
        (0, _) => format!("urn:isbn:{value}"),
        (1, _) => format!("urn:doi:{value}"),
        _ => format!("urn:uuid:{value}"),
    }
}

/// The format element every save adds to the metadata.
pub const FORMAT_ELEMENT: &str = "<dc:format>eBraille 1.0</dc:format>";

/// The package's metadata element text (the template package.opf, with the dialog's values): the
/// template's elements in its order, with the Required page's lists written one element each; the Recommended
/// page's as Dublin Core elements, the education levels as dcterms:educationLevel; the Additional page's as meta
/// elements with their attributes; then `<dc:format>eBraille 1.0</dc:format>`, which every save adds. The
/// metadata as edited on the XML Preview page is written as it is, the format element added at its end when it
/// has none.
pub fn metadata_xml(meta: &Metadata, identifier: &str) -> String {
    if let Some(edited) = &meta.edited_xml {
        if edited.contains("<dc:format") {
            return edited.clone();
        }
        let Some(at) = edited.rfind("</metadata>") else { return edited.clone() };
        // On a line of its own before the end tag's line when that line holds only the end tag.
        let line = edited[..at].rfind('\n').map_or(0, |n| n + 1);
        if edited[line..at].trim().is_empty() {
            return format!("{}    {FORMAT_ELEMENT}\n{}", &edited[..line], &edited[line..]);
        }
        return format!("{}{FORMAT_ELEMENT}{}", &edited[..at], &edited[at..]);
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
    for s in meta.braille_systems.iter().map(BrailleSystem::value).filter(|v| !v.is_empty()) {
        out.push_str(&format!("    <meta property=\"a11y:brailleSystem\">{}</meta>\n", xml_text(&s)));
    }
    out.push_str(&format!("    <meta property=\"a11y:completeTranscription\">{}</meta>\n", meta.complete_transcription));
    let producers: Vec<&String> = meta.producers.iter().filter(|p| !p.trim().is_empty()).collect();
    if producers.is_empty() {
        out.push_str("    <meta property=\"a11y:producer\"></meta>\n");
    }
    for p in producers {
        out.push_str(&format!("    <meta property=\"a11y:producer\">{}</meta>\n", xml_text(p.trim())));
    }
    // Not known: what the Tactile Graphics box writes; the template's value PDF when it is checked, none when not.
    out.push_str(&format!("    <meta property=\"a11y:tactileGraphics\">{}</meta>\n", if meta.tactile_graphics { "PDF" } else { "none" }));
    out.push_str(&format!("    <meta property=\"dcterms:dateCopyrighted\">{}</meta>\n", xml_text(&copyrighted)));
    out.push_str(&format!("    <meta property=\"dcterms:modified\">{}</meta>\n", xml_text(&modified_now())));
    for p in meta.publishers.iter().filter(|p| !p.trim().is_empty()) {
        out.push_str(&format!("    <dc:publisher{}>{}</dc:publisher>\n", meta.attributes_of("publisher"), xml_text(p.trim())));
    }
    for e in meta.education_levels.iter().filter(|e| !e.trim().is_empty()) {
        out.push_str(&format!("    <meta property=\"dcterms:educationLevel\">{}</meta>\n", xml_text(e.trim())));
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
    out.push_str(&format!("    {FORMAT_ELEMENT}\n"));
    out.push_str("  </metadata>\n");
    out
}

/// The package element's start tag with its root attributes (the template's) and `unique` as the
/// unique-identifier: in a saved package the dc:identifier element's id (PUB_ID); in the document file's stored
/// text (`opf_text`) the identifier text, as that text has always held it.
fn package_start(unique: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" xmlns:a11y=\"http://idpf.org/epub/vocab/package/a11y/#\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xml:lang=\"en\" version=\"3.0\" unique-identifier=\"{}\">\n",
        xml_text(unique)
    )
}

/// The package.opf text a document file keeps for its eBraille metadata (the version 2 section's
/// ebraille/package.opf): the package element with its metadata, in the template's form (no manifest and no spine).
/// Not known: whether the stored text also holds the empty manifest and spine.
pub fn opf_text(meta: &Metadata) -> String {
    let identifier = identifier_text(meta);
    let mut out = package_start(&identifier);
    out.push_str(&metadata_xml(meta, &identifier));
    out.push_str("</package>\n");
    out
}

/// The metadata of a package.opf text (a document file's ebraille/package.opf): each element of its metadata
/// element into the field it is written from. When an element maps to no field (an element or attribute the
/// dialog does not write), the metadata element is kept whole as the edited XML, so it is written back as it was.
/// Text that is not XML, or holds no metadata element, gives empty metadata.
pub fn metadata_from_opf(text: &str) -> Metadata {
    let mut m = Metadata::default();
    let Ok(root) = docx::read_xml(text) else { return m };
    let Some(meta) = root.element("metadata") else { return m };
    let mut unmapped = false;
    let urn_of = |v: &str| -> (u8, String) {
        let v = v.trim();
        for (kind, prefix) in [(0u8, "urn:isbn:"), (1, "urn:doi:"), (2, "urn:uuid:")] {
            if let Some(rest) = v.strip_prefix(prefix) {
                return (kind, if rest == "{GUID}" { String::new() } else { rest.to_string() });
            }
        }
        (2, v.to_string())
    };
    let date_of = |v: &str| if v.trim() == "{Date}" { String::new() } else { v.trim().to_string() };
    for e in meta.element_children() {
        let value = e.text();
        let attrs: Vec<(String, String)> = e.attrs.clone();
        let field = match e.name.as_str() {
            "dc:creator" => Some("creator"),
            "dc:title" => Some("title"),
            "dc:date" => Some("date"),
            "dc:identifier" => Some("identifier"),
            "dc:publisher" => Some("publisher"),
            "dc:description" => Some("description"),
            "dc:rights" => Some("rights"),
            "dc:source" => Some("source"),
            "dc:subject" => Some("subject"),
            _ => None,
        };
        if let Some(field) = field {
            if !attrs.is_empty() && !m.field_attributes.iter().any(|(f, _)| f == field) {
                m.field_attributes.push((field.to_string(), attrs.clone()));
            }
            let push = |list: &mut Vec<String>| {
                if !value.trim().is_empty() {
                    list.push(value.trim().to_string());
                }
            };
            match field {
                "creator" => push(&mut m.creators),
                "title" => push(&mut m.titles),
                "date" => m.date = date_of(&value),
                "identifier" => (m.id_type, m.identifier) = urn_of(&value),
                "publisher" => push(&mut m.publishers),
                "description" => m.description = value.trim().to_string(),
                "rights" => push(&mut m.rights),
                "source" => (m.source_type, m.source) = urn_of(&value),
                _ => push(&mut m.subjects),
            }
            continue;
        }
        match e.name.as_str() {
            "dc:language" if attrs.is_empty() => {
                if !value.trim().is_empty() {
                    m.languages.push(value.trim().to_string());
                }
            }
            // Added on every save.
            "dc:format" if value.trim() == "eBraille 1.0" => {}
            "meta" => {
                let Some(property) = e.attr("property") else {
                    unmapped = true;
                    continue;
                };
                let others: Vec<(String, String)> = attrs.iter().filter(|(n, _)| n != "property").cloned().collect();
                let plain = others.is_empty();
                match property {
                    "a11y:brailleSystem" if plain => m.braille_systems.push(BrailleSystem::from_value(&value)),
                    "a11y:completeTranscription" if plain => m.complete_transcription = value.trim() == "true",
                    "a11y:producer" if plain => {
                        if !value.trim().is_empty() {
                            m.producers.push(value.trim().to_string());
                        }
                    }
                    "a11y:tactileGraphics" if plain && (value.trim() == "PDF" || value.trim() == "none") => m.tactile_graphics = value.trim() == "PDF",
                    "dcterms:dateCopyrighted" if plain => m.date_copyrighted = date_of(&value),
                    // Written afresh on every save.
                    "dcterms:modified" if plain => {}
                    "dcterms:educationLevel" if plain => {
                        if !value.trim().is_empty() {
                            m.education_levels.push(value.trim().to_string());
                        }
                    }
                    name => m.properties.push(Property { name: name.to_string(), value, attributes: others }),
                }
            }
            _ => unmapped = true,
        }
    }
    if unmapped {
        if let (Some(from), Some(to)) = (text.find("<metadata"), text.rfind("</metadata>")) {
            m.edited_xml = Some(format!("  {}\n", &text[from..to + "</metadata>".len()]));
        }
    }
    m
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
    /// The document's translation table entry's eBraille language and braille system, written when the metadata
    /// names none (Metadata::with_defaults).
    pub default_language: Option<&'a str>,
    pub default_system: Option<&'a str>,
    /// The file name the package is saved as (or the document's name; a path or an extension may be given): with
    /// no title in the metadata and no heading, the package's title is this name without its extension.
    pub name: &'a str,
}

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const OPS: &str = "http://www.idpf.org/2007/ops";

/// The id of the package's dc:identifier element, which the package element's unique-identifier names.
pub const PUB_ID: &str = "pub-id";

/// The metadata element's elements with no text (`<dc:creator></dc:creator>`, `<meta property="..."></meta>`, or
/// written empty as `<x/>`) left out, each with its own line when it stands alone on it: EPUB 3 does not allow an
/// empty Dublin Core or meta element.
fn without_empty_elements(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(at) = rest.find('<') {
        let (before, from) = rest.split_at(at);
        let Some(close) = from.find('>') else { break };
        let tag = &from[..=close];
        let name: String = tag[1..].chars().take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/').collect();
        let mut gone = None;
        if !tag.starts_with("</") && (name.starts_with("dc:") || name == "meta") {
            if tag.ends_with("/>") {
                gone = Some(tag.len());
            } else {
                let after = &from[tag.len()..];
                let end_tag = format!("</{name}>");
                let text_end = after.find('<').unwrap_or(after.len());
                if after[..text_end].trim().is_empty() && after[text_end..].starts_with(&end_tag) {
                    gone = Some(tag.len() + text_end + end_tag.len());
                }
            }
        }
        out.push_str(before);
        match gone {
            Some(n) => {
                let mut next = &from[n..];
                let line = out.rfind('\n').map_or(0, |i| i + 1);
                if out[line..].trim().is_empty() && next.starts_with('\n') {
                    out.truncate(line);
                    next = &next[1..];
                }
                rest = next;
            }
            None => {
                out.push_str(tag);
                rest = &from[tag.len()..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The metadata element made to follow the EPUB 3 package rules reading apps check (BrailleCraft's own rule, so
/// that an EPUB 3 or eBraille save opens in them): empty elements left out; the first dc:identifier carries an id
/// (its own, or PUB_ID given to it), which is returned for the package's unique-identifier, and one is added from
/// `identifier` when there is none; a dc:title and a dc:language are added when there are none; and the
/// dcterms:modified value is the save time in UTC (CCYY-MM-DDThh:mm:ssZ), added when there is none. The other
/// elements, the eBraille ones among them, are kept as they are.
fn for_reading_apps(xml: &str, identifier: &str, title: &str, language: &str, modified: &str) -> (String, String) {
    let mut xml = without_empty_elements(xml);
    let mut added = String::new();
    let unique = match xml.find("<dc:identifier") {
        Some(at) => {
            let end = xml[at..].find('>').map_or(xml.len(), |e| at + e);
            let tag = xml[at..end].to_string();
            let own = tag.split_once(" id=\"").and_then(|(_, rest)| rest.split_once('"')).map(|(id, _)| id.to_string()).filter(|id| !id.is_empty());
            match own {
                Some(id) => id,
                None => {
                    xml.insert_str(at + "<dc:identifier".len(), &format!(" id=\"{PUB_ID}\""));
                    PUB_ID.to_string()
                }
            }
        }
        None => {
            added.push_str(&format!("    <dc:identifier id=\"{PUB_ID}\">{}</dc:identifier>\n", xml_text(identifier)));
            PUB_ID.to_string()
        }
    };
    if !xml.contains("<dc:title") {
        added.push_str(&format!("    <dc:title>{}</dc:title>\n", xml_text(title)));
    }
    if !xml.contains("<dc:language") {
        added.push_str(&format!("    <dc:language>{}</dc:language>\n", xml_text(language)));
    }
    let mut found = false;
    let mut from = 0;
    while let Some(at) = xml[from..].find("property=\"dcterms:modified\"").map(|a| from + a) {
        let Some(open_end) = xml[at..].find('>').map(|e| at + e + 1) else { break };
        let Some(close) = xml[open_end..].find("</meta>").map(|e| open_end + e) else { break };
        xml.replace_range(open_end..close, modified);
        from = open_end + modified.len();
        found = true;
    }
    if !found {
        added.push_str(&format!("    <meta property=\"dcterms:modified\">{modified}</meta>\n"));
    }
    if !added.is_empty() {
        let at = xml.rfind("</metadata>").unwrap_or(xml.len());
        let line = xml[..at].rfind('\n').map_or(0, |n| n + 1);
        let at = if xml[line..at].trim().is_empty() { line } else { at };
        xml.insert_str(at, &added);
    }
    (xml, unique)
}

/// The file name without its folder and extension.
fn stem_of(name: &str) -> &str {
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match file.rfind('.') {
        Some(dot) if dot > 0 => &file[..dot],
        _ => file,
    }
}

/// The package bytes: a zip with the mimetype entry first, stored, then index.html (the navigation document), the
/// style sheet, the content page, package.opf and META-INF/container.xml.
///
/// BrailleCraft's own rule: the package follows EPUB 3 so reading apps open it. The package.opf's
/// unique-identifier names the dc:identifier element's id; dc:title (the metadata's title, else the first table
/// of contents heading, else the file name without its extension), dc:language (the metadata's, else the
/// translation table's, else "en") and dcterms:modified (the save time in UTC) are always written; the manifest
/// lists every part (the content page, index.html as the nav, the style sheet) and the spine's itemref names the
/// content page's manifest item. The content page and index.html carry the document's title.
pub fn package(input: &Input) -> Vec<u8> {
    let folder = if input.braille { "ebraille" } else { "epub" };
    let page = page(input.running, input.kinds);
    let encode = |text: &str| -> String { if input.braille { encode_node(text, input.cell) } else { text.to_string() } };
    let meta = input.meta.with_defaults(input.default_language, input.default_system);
    // The title: the metadata's, else the first heading's text (as cells in a braille document), else the file name.
    let title = meta
        .titles
        .iter()
        .map(|t| t.trim().to_string())
        .find(|t| !t.is_empty())
        .or_else(|| {
            page.headings.iter().map(|(_, t)| t.trim()).find(|t| !t.is_empty()).map(|t| if input.braille { encode_cells(t, input.cell) } else { t.to_string() })
        })
        .unwrap_or_else(|| stem_of(input.name).trim().to_string());
    let title = if title.is_empty() { "Untitled".to_string() } else { title };
    let language = meta.languages.iter().map(|l| l.trim().to_string()).find(|l| !l.is_empty()).unwrap_or_else(|| "en".to_string());
    let lang = xml_text(&language);
    let title_xml = xml_text(&title);
    // The content page, its text nodes encoded as they are written (no second copy of the body).
    let mut content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"{XHTML}\" xmlns:bc=\"{NAMESPACE}\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head xmlns:epub=\"{OPS}\">\n<title>{title_xml}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"css/ebrl.css\"/>\n</head><body>"
    );
    if input.braille {
        content.push_str(&encode_text_nodes(&page.body, &encode));
    } else {
        content.push_str(&page.body);
    }
    content.push_str("</body></html>\n");
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
    navs.push_str(&format!("<nav epub:type=\"toc\" role=\"doc-toc\" aria-label=\"Table of Contents\"><h2>{}</h2><ol>", heading("Table of Contents", ",table of ,contents")));
    if page.headings.is_empty() {
        // EPUB 3 requires the toc nav to list at least one entry: with no heading, the content page itself under
        // the document's title.
        navs.push_str(&format!("<li><a href=\"{part}\">{title_xml}</a></li>"));
    }
    for (id, text) in &page.headings {
        navs.push_str(&format!("<li><a href=\"{part}#{id}\">{}</a></li>", encode(&{
            let mut t = String::new();
            escape(text, &mut t);
            t
        })));
    }
    navs.push_str("</ol></nav>");
    // A page list nav is written only when there are pages: EPUB 3 does not allow a nav with no list.
    if !page.pages.is_empty() {
        navs.push_str(&format!("<nav epub:type=\"page-list\" role=\"doc-pagelist\" aria-label=\"Page List\"><h2>{}</h2><ol>", heading("Page List", ",page ,list")));
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
        navs.push_str("</ol></nav>");
    }
    // No style marks a landmark: the landmarks nav lists the start of the content page as the body matter (EPUB 3
    // does not allow a nav with no list).
    navs.push_str(&format!(
        "<nav epub:type=\"landmarks\" aria-label=\"Landmarks\"><h2>{}</h2><ol><li><a epub:type=\"bodymatter\" href=\"{part}\">{}</a></li></ol></nav>",
        heading("Landmarks", ",landmarks"),
        heading("Start", ",start")
    ));
    let index = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"{XHTML}\" xmlns:epub=\"{OPS}\" xmlns:bc=\"{NAMESPACE}\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head><title>{title_xml}</title>\n<link rel=\"publication\" href=\"package.opf\" type=\"application/oebps-package+xml\"/>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"{folder}/css/ebrl.css\"/>\n</head><body>{navs}</body></html>\n"
    );
    let identifier = urn(input.meta.id_type, &input.meta.identifier);
    let mut meta = meta;
    if meta.titles.iter().all(|t| t.trim().is_empty()) {
        meta.titles = vec![title.clone()];
    }
    if meta.languages.iter().all(|l| l.trim().is_empty()) {
        meta.languages = vec![language.clone()];
    }
    let (metadata, unique) = for_reading_apps(&metadata_xml(&meta, &identifier), &identifier, &title, &language, &modified_now());
    let css_name = format!("{folder}/css/ebrl.css");
    let mut opf = package_start(&unique);
    opf.push_str(&metadata);
    opf.push_str(&format!(
        "  <manifest>\n    <item id=\"nav\" href=\"index.html\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n    <item id=\"css\" href=\"{css_name}\" media-type=\"text/css\"/>\n    <item id=\"part0001\" href=\"{part}\" media-type=\"application/xhtml+xml\"/>\n  </manifest>\n  <spine>\n    <itemref idref=\"part0001\"/>\n  </spine>\n</package>\n"
    ));
    let container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n  <rootfiles>\n    <rootfile full-path=\"package.opf\" media-type=\"application/oebps-package+xml\"/>\n  </rootfiles>\n</container>\n";
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

/// The quality notice shown before every eBraille and EPUB 3 save until its box is checked (the text
/// with the vendor's team unnamed), kept under Mutable, EBrailleQualityWarning.
pub const QUALITY_WARNING: &str = "eBraille exports should be reliably working to produce readable eBraille files, but the quality is not yet reliable. Any sample files sent to the support team will help us to improve the quality.";
pub const QUALITY_WARNING_KEY: &str = "EBrailleQualityWarning";

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: StyleKind = StyleKind::BLOCK;

    fn kinds(name: &str) -> StyleKind {
        match name {
            "h1." | "h2." => StyleKind { toc: true, ..BLOCK },
            "list." => StyleKind { list: Some(2), ..BLOCK },
            "pagenum." => StyleKind { page_break: true, ..BLOCK },
            "Lea" => StyleKind { page_break: true, ..StyleKind::default() },
            "c-contents." => StyleKind { list: Some(1), ..BLOCK },
            "GuideDots" => StyleKind { guide_dots: true, ..StyleKind::default() },
            "b" => StyleKind::default(),
            _ => BLOCK,
        }
    }

    fn input<'a>(running: &'a str, braille: bool, meta: &'a Metadata, cell: &'a dyn Fn(char) -> u8) -> Input<'a> {
        Input { running, braille, kinds: &kinds, cell, meta, default_language: None, default_system: None, name: "book.epub" }
    }

    #[test]
    fn the_page_writes_elements_lists_pages_and_tables() {
        let running = "\x1ces~h1.\x1fTitle & more\x1cee~h1.\x1f\x1ces~para.\x1fBody\x1cl\x1fnext\x1cee~para.\x1f\x1ces~list.\x1fone\x1chl2\x1ftwo\x1chl1\x1fthree\x1cee~list.\x1f\x1ces~pagenum.\x1f12\x1cee~pagenum.\x1f\x1chtbs;r:0\x1fa\x1c>\x1fb\x1c<\x1fc\x1chtbe\x1f\x1ces~1-3.\x1fx\x1cee~1-3.\x1f";
        let p = page(running, &kinds);
        // A deeper level opens a new item holding its own list.
        assert_eq!(p.body, "<div class=\"h1\" id=\"toc001\">Title &amp; more</div><div class=\"para\">Bodynext</div><ol class=\"list\" bc:maxdepth=\"2\"><li class=\"lvl1of2\">one</li><li class=\"lvl1of2\"><ol><li class=\"lvl2of2\">two</li></ol></li><li class=\"lvl1of2\">three</li></ol><div class=\"refpagenum\" id=\"page001\" role=\"doc-pagebreak\" aria-label-src=\"content\" hrefsrc=\"content\" hrefprefix=\"page\"><span class=\"leaders\"/><span class=\"number\">12</span></div><table><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></table><div class=\"_1-3\">x</div>");
        assert_eq!(p.headings, vec![("toc001".to_string(), "Title & more".to_string())]);
        assert_eq!(p.pages, vec![("page001".to_string(), "12".to_string())]);
    }

    #[test]
    fn kind_of_follows_the_style_rules() {
        let h1 = kind_of("H1.", "\x1cl\x1f\x1ckps\x1f\x1crm12\x1f\x1crps1\x1f\x1chds3:1\x1f");
        assert!(h1.block && h1.toc && !h1.page_break && h1.list.is_none());
        let refpage = kind_of("RefPage.", "\x1clea\x1f");
        assert!(refpage.block && refpage.page_break && !refpage.toc);
        let lea = kind_of("Lea", "\x1clea\x1f");
        assert!(lea.page_break && !lea.block);
        let poetry = kind_of("Poetry.", "\x1cmfcs~poetry\x1f");
        assert!(poetry.block && poetry.list == Some(1));
        let para = kind_of("Para.", "\x1chi3:1:0:2:2\x1f");
        assert_eq!(para, BLOCK);
        let bold = kind_of("Bold", "\x1cfts~b\x1f");
        assert_eq!(bold, StyleKind::default());
        assert!(!bold.writes());
        let dots = kind_of("GuideDots", "");
        assert!(dots.guide_dots && !dots.block);
    }

    #[test]
    fn a_template_heading_goes_to_the_toc_not_the_page_list() {
        let path = format!("{}/../../templates/english-ueb-basic.template", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(path).unwrap();
        let table = crate::templates::styles(&text);
        let begin = String::from_utf8(table.lookup(b"h1.", 0).unwrap()).unwrap();
        assert!(begin.contains("\x1crps1\x1f"), "{begin:?}");
        let kinds = |name: &str| kind_of(name, &String::from_utf8(table.lookup(name.as_bytes(), 0).unwrap_or_default()).unwrap_or_default());
        let p = page("\x1ces~h1.\x1fChapter One\x1cee~h1.\x1f", &kinds);
        assert_eq!(p.headings, vec![("toc001".to_string(), "Chapter One".to_string())]);
        assert!(p.pages.is_empty());
        assert_eq!(p.body, "<div class=\"h1\" id=\"toc001\">Chapter One</div>");
    }

    #[test]
    fn page_list_ids_count_up() {
        let p = page("\x1ces~pagenum.\x1f7\x1cee~pagenum.\x1f\x1ces~Lea\x1f8\x1cee~Lea\x1f", &kinds);
        let ids: Vec<&str> = p.pages.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(ids, ["page001", "page002"]);
        assert!(p.body.contains("id=\"page001\"") && p.body.contains("id=\"page002\""), "{}", p.body);
    }

    #[test]
    fn aria_label_src_only_for_block_page_styles() {
        let block = page("\x1ces~pagenum.\x1f7\x1cee~pagenum.\x1f", &kinds).body;
        assert!(block.starts_with("<div class=\"refpagenum\" id=\"page001\" role=\"doc-pagebreak\" aria-label-src=\"content\" hrefsrc=\"content\" hrefprefix=\"page\">"), "{block}");
        assert!(block.ends_with("</div>"));
        let span = page("\x1ces~Lea\x1f7\x1cee~Lea\x1f", &kinds).body;
        assert_eq!(span, "<span class=\"refpagenum\" id=\"page001\" role=\"doc-pagebreak\" hrefsrc=\"content\" hrefprefix=\"page\"><span class=\"leaders\"/><span class=\"number\">7</span></span>");
    }

    #[test]
    fn refpagenum_gets_leaders_and_number_spans() {
        // The element keeps its links, then its whole text after the leaders.
        let body = page("\x1ces~pagenum.\x1fp\x1crps:a~x\x1f\x1crps:h~#y\x1f12\x1crpe:h\x1f\x1cee~pagenum.\x1f", &kinds).body;
        assert!(body.ends_with("hrefprefix=\"page\"><a name=\"x\"/><a href=\"#y\">12</a><span class=\"leaders\"/><span class=\"number\">p12</span></div>"), "{body}");
        // A contents list: the item's first span is its title with leaders, its links take class page.
        let list = page("\x1ces~c-contents.\x1f\x1ces~Lea\x1fChapter\x1cee~Lea\x1f\x1ces~GuideDots\x1f5\x1cee~GuideDots\x1f\x1cee~c-contents.\x1f", &kinds).body;
        assert_eq!(list, "<ol class=\"c-contents\" bc:maxdepth=\"1\"><li class=\"lvl1of1\"><span class=\"title\"><span class=\"leaders\"/><span class=\"number\">Chapter</span><span class=\"leaders\"/></span><a hrefsrc=\"content\" hrefprefix=\"#page\" class=\"page\">5</a></li></ol>");
        assert_eq!(with_class("<a href=\"x\" class=\"old\">", "page"), "<a href=\"x\" class=\"page\">");
    }

    #[test]
    fn a_braille_package_holds_the_six_entries_with_cells() {
        let meta = Metadata { titles: vec!["My Book".into()], id_type: 2, ..Metadata::default() };
        let cell = |c: char| -> u8 { if c == 'a' { 1 } else if c == ',' { 0x20 } else { 0 } };
        let bytes = package(&input("\x1ces~h1.\x1fa\x1cee~h1.\x1f", true, &meta, &cell));
        let parts = docx::parts(&bytes).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["mimetype", "index.html", "ebraille/css/ebrl.css", "ebraille/part0001.xml", "package.opf", "META-INF/container.xml"]);
        assert_eq!(docx::part(&parts, "mimetype").unwrap(), b"application/epub+zip");
        let content = String::from_utf8(docx::part(&parts, "ebraille/part0001.xml").unwrap().to_vec()).unwrap();
        assert!(content.contains("<div class=\"h1\" id=\"toc001\">\u{2801}</div>"), "{content}");
        let index = String::from_utf8(docx::part(&parts, "index.html").unwrap().to_vec()).unwrap();
        // ",table of ,contents" as cells (the test's cell map knows only a and the comma).
        assert!(index.contains("<h2>\u{2820}\u{2800}\u{2801}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2820}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}\u{2800}</h2>"), "{index}");
        assert!(index.contains("<li><a href=\"ebraille/part0001.xml#toc001\">\u{2801}</a></li>"), "{index}");
        let opf = String::from_utf8(docx::part(&parts, "package.opf").unwrap().to_vec()).unwrap();
        assert!(opf.contains("<dc:title>My Book</dc:title>") && opf.contains("urn:uuid:") && opf.contains("<meta property=\"a11y:tactileGraphics\">none</meta>"), "{opf}");
        // The manifest lists every part; the spine names the content page's item.
        assert!(opf.contains("<item id=\"part0001\" href=\"ebraille/part0001.xml\" media-type=\"application/xhtml+xml\"/>"), "{opf}");
        assert!(opf.contains("<item id=\"nav\" href=\"index.html\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>"), "{opf}");
        assert!(opf.contains("  </manifest>\n  <spine>\n    <itemref idref=\"part0001\"/>\n  </spine>"), "{opf}");
        docx::read_xml(&opf).unwrap();
    }

    #[test]
    fn unique_identifier_names_the_identifier_id() {
        let meta = Metadata { id_type: 0, identifier: "978-1".into(), ..Metadata::default() };
        let cell = |_: char| 0u8;
        let bytes = package(&input("x", false, &meta, &cell));
        let parts = docx::parts(&bytes).unwrap();
        let opf = docx::read_xml(std::str::from_utf8(docx::part(&parts, "package.opf").unwrap()).unwrap()).unwrap();
        let identifier = opf.at(&["metadata", "dc:identifier"]).unwrap();
        assert_eq!(identifier.text(), "urn:isbn:978-1");
        assert_eq!(identifier.attr("id"), Some(PUB_ID));
        assert_eq!(opf.attr("unique-identifier"), Some(PUB_ID));
        // Edited metadata keeps an identifier id of its own, and gets the title, language and modified time it lacks.
        let edited = Metadata { edited_xml: Some("  <metadata>\n    <dc:identifier id=\"mine\">urn:isbn:1</dc:identifier>\n    <dc:creator></dc:creator>\n  </metadata>\n".into()), ..Metadata::default() };
        let bytes = package(&input("x", false, &edited, &cell));
        let parts = docx::parts(&bytes).unwrap();
        let text = std::str::from_utf8(docx::part(&parts, "package.opf").unwrap()).unwrap().to_string();
        let opf = docx::read_xml(&text).unwrap();
        assert_eq!(opf.attr("unique-identifier"), Some("mine"), "{text}");
        assert_eq!(opf.at(&["metadata", "dc:title"]).map(|t| t.text()), Some("book".to_string()), "{text}");
        assert_eq!(opf.at(&["metadata", "dc:language"]).map(|t| t.text()), Some("en".to_string()), "{text}");
        assert!(!text.contains("<dc:creator>") && text.contains("<dc:format>eBraille 1.0</dc:format>"), "{text}");
    }

    /// Every element id in a tree.
    fn ids_of(e: &docx::Element, out: &mut Vec<String>) {
        if let Some(id) = e.attr("id") {
            out.push(id.to_string());
        }
        for c in e.element_children() {
            ids_of(c, out);
        }
    }

    /// Whether a text is CCYY-MM-DDThh:mm:ssZ.
    fn is_utc_time(text: &str) -> bool {
        let b = text.as_bytes();
        b.len() == 20
            && b.iter().enumerate().all(|(i, c)| match i {
                4 | 7 => *c == b'-',
                10 => *c == b'T',
                13 | 16 => *c == b':',
                19 => *c == b'Z',
                _ => c.is_ascii_digit(),
            })
    }

    /// The package follows the EPUB 3 rules reading apps check: the unique-identifier names an element id,
    /// dc:language and dc:title have text, dcterms:modified is the UTC time, every manifest href is a part of the
    /// zip, the spine names a manifest item, both pages are XHTML with the document title, ids are unique, and
    /// every nav link points at an id on the content page.
    #[test]
    fn package_follows_epub_3_rules() {
        let running = "\x1ces~h1.\x1fFirst & Heading\x1cee~h1.\x1f\x1ces~para.\x1fBody\x1cee~para.\x1f\x1ces~pagenum.\x1f2\x1cee~pagenum.\x1f\x1ces~bullets.\x1f\x1chl1\x1fone\x1cl\x1f\x1chl1\x1ftwo\x1cee~bullets.\x1f\x1ces~h2.\x1fNext\x1cee~h2.\x1f";
        let cell = |c: char| (c as u32 & 0x3f) as u8;
        for (braille, meta, title) in [
            (false, Metadata::default(), "First & Heading".to_string()),
            (true, Metadata { titles: vec!["Own Title".into()], languages: vec!["en-Brai".into()], ..Metadata::default() }, "Own Title".to_string()),
            (false, Metadata::default(), "book".to_string()),
        ] {
            let running = if title == "book" { "\x1ces~para.\x1fNo heading\x1cee~para.\x1f" } else { running };
            let bytes = package(&input(running, braille, &meta, &cell));
            let parts = docx::parts(&bytes).unwrap();
            let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
            let opf_text = std::str::from_utf8(docx::part(&parts, "package.opf").unwrap()).unwrap();
            let opf = docx::read_xml(opf_text).unwrap();
            assert_eq!(opf.attr("version"), Some("3.0"));
            let metadata = opf.element("metadata").unwrap();
            let unique = opf.attr("unique-identifier").unwrap();
            let mut ids = Vec::new();
            ids_of(&opf, &mut ids);
            assert!(ids.iter().any(|i| i == unique) && metadata.elements("dc:identifier").any(|e| e.attr("id") == Some(unique)), "{opf_text}");
            let language = metadata.element("dc:language").map(|e| e.text()).unwrap_or_default();
            assert!(!language.trim().is_empty(), "{opf_text}");
            let titles: Vec<String> = metadata.elements("dc:title").map(|e| e.text()).collect();
            assert!(!titles.is_empty() && titles.iter().all(|t| !t.trim().is_empty()), "{opf_text}");
            assert_eq!(titles[0], title, "{opf_text}");
            // Every metadata element has text.
            assert!(metadata.element_children().all(|e| !e.text().trim().is_empty()), "{opf_text}");
            let modified: Vec<String> = metadata.elements("meta").filter(|e| e.attr("property") == Some("dcterms:modified")).map(|e| e.text()).collect();
            assert_eq!(modified.len(), 1, "{opf_text}");
            assert!(is_utc_time(&modified[0]), "{}", modified[0]);
            let manifest = opf.element("manifest").unwrap();
            let items: Vec<&docx::Element> = manifest.elements("item").collect();
            assert_eq!(items.len(), 3, "{opf_text}");
            for item in &items {
                let href = item.attr("href").unwrap();
                assert!(names.contains(&href), "{href} not in {names:?}");
                assert!(item.attr("id").is_some_and(|i| !i.is_empty()) && item.attr("media-type").is_some());
            }
            assert_eq!(items.iter().filter(|i| i.attr("properties") == Some("nav")).count(), 1);
            let spine: Vec<&docx::Element> = opf.element("spine").unwrap().elements("itemref").collect();
            assert!(!spine.is_empty());
            for itemref in spine {
                let item = items.iter().find(|i| i.attr("id") == itemref.attr("idref")).expect("spine idref names a manifest item");
                assert_eq!(item.attr("media-type"), Some("application/xhtml+xml"));
            }
            // The pages.
            let folder = if braille { "ebraille" } else { "epub" };
            let part = format!("{folder}/part0001.xml");
            let content_text = std::str::from_utf8(docx::part(&parts, &part).unwrap()).unwrap();
            let content = docx::read_xml(content_text).unwrap();
            let index_text = std::str::from_utf8(docx::part(&parts, "index.html").unwrap()).unwrap();
            let index = docx::read_xml(index_text).unwrap();
            for page in [&content, &index] {
                assert_eq!(page.name, "html");
                assert_eq!(page.attr("xmlns"), Some(XHTML));
                assert_eq!(page.at(&["head", "title"]).map(|t| t.text()), Some(title.clone()));
            }
            let mut content_ids = Vec::new();
            ids_of(&content, &mut content_ids);
            assert!(content_ids.iter().all(|i| !i.is_empty()), "{content_text}");
            let mut unique_ids = content_ids.clone();
            unique_ids.sort();
            unique_ids.dedup();
            assert_eq!(unique_ids.len(), content_ids.len(), "ids repeat: {content_ids:?}");
            let mut links = Vec::new();
            index.find_all(&|e| e.name == "a", &mut links);
            let navs: Vec<&docx::Element> = index.elements("body").next().unwrap().elements("nav").collect();
            assert!(navs.iter().all(|n| n.element("ol").is_some_and(|ol| ol.element("li").is_some())), "{index_text}");
            assert!(!links.is_empty());
            for a in links {
                let href = a.attr("href").unwrap();
                let (file, id) = href.split_once('#').unwrap_or((href, ""));
                assert_eq!(file, part, "{href}");
                assert!(id.is_empty() || content_ids.iter().any(|i| i == id), "{href} names no id on the content page");
            }
        }
    }

    /// A block style whose items start with level codes ([hl1], as an imported Markdown list) writes each item as
    /// its own li, the style's element becoming the list.
    #[test]
    fn level_code_items_become_li_elements() {
        let running = "\x1ces~bullets.\x1f\x1chl1\x1fWhat Accessibility Means\x1cl\x1f\x1chl1\x1fWhat to Expect\x1cl\x1f\x1chl2\x1fDeeper\x1cl\x1f\x1chl1\x1fLast\x1cl\x1f\x1cee~bullets.\x1f\x1ces~para.\x1fAfter\x1cee~para.\x1f";
        let body = page(running, &kinds).body;
        assert_eq!(body, "<ol class=\"bullets\" bc:maxdepth=\"2\"><li class=\"lvl1of2\">What Accessibility Means</li><li class=\"lvl1of2\">What to Expect</li><li class=\"lvl1of2\"><ol><li class=\"lvl2of2\">Deeper</li></ol></li><li class=\"lvl1of2\">Last</li></ol><div class=\"para\">After</div>");
        // Text before the first level code: the block keeps its element and the list opens inside it.
        let body = page("\x1ces~bullets.\x1fLead\x1chl1\x1fa\x1chl1\x1fb\x1cee~bullets.\x1f", &kinds).body;
        assert_eq!(body, "<div class=\"bullets\">Lead<ol bc:maxdepth=\"1\"><li class=\"lvl1of1\">a</li><li class=\"lvl1of1\">b</li></ol></div>");
        // A list left open at the end of the text is closed with its depth.
        let body = page("\x1ces~bullets.\x1f\x1chl1\x1fa", &kinds).body;
        assert_eq!(body, "<ol class=\"bullets\" bc:maxdepth=\"1\"><li class=\"lvl1of1\">a</li></ol>");
        // The list style of an [mfcs] code keeps its own form, and a heading's level code makes no list.
        assert!(page("\x1ces~list.\x1fone\x1chl2\x1ftwo\x1cee~list.\x1f", &kinds).body.starts_with("<ol class=\"list\" bc:maxdepth=\"2\"><li class=\"lvl1of2\">one"));
        assert_eq!(page("\x1ces~h1.\x1f\x1chl1\x1fT\x1cee~h1.\x1f", &kinds).body, "<div class=\"h1\" id=\"toc001\">T</div>");
        // Through the package: each item is an li of the content page.
        let meta = Metadata::default();
        let cell = |_: char| 0u8;
        let bytes = package(&input(running, false, &meta, &cell));
        let parts = docx::parts(&bytes).unwrap();
        let content = docx::read_xml(std::str::from_utf8(docx::part(&parts, "epub/part0001.xml").unwrap()).unwrap()).unwrap();
        let mut items = Vec::new();
        content.find_all(&|e| e.name == "li", &mut items);
        let texts: Vec<String> = items.iter().map(|li| li.text()).collect();
        // The item holding the deeper list has that list's text too.
        assert_eq!(texts, ["What Accessibility Means", "What to Expect", "Deeper", "Deeper", "Last"]);
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
            let kinds = |name: &str| kind_of(name, styles.iter().find(|s| s.0 == name).map_or("", |s| s.1.as_str()));
            let cell = |c: char| (c as u32 & 0x3f) as u8;
            let meta = Metadata::default();
            let bytes = package(&Input { running: &doc.text, braille: ext == "dxb", kinds: &kinds, cell: &cell, meta: &meta, default_language: None, default_system: None, name: &path.to_string_lossy() });
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
        let order = ["<dc:creator>A</dc:creator>", "<dc:creator>B</dc:creator>", "<dc:date>", "<dc:identifier>urn:isbn:123</dc:identifier>", "<dc:title></dc:title>", "<dc:language>en</dc:language>", "a11y:completeTranscription\">true", "<meta property=\"a11y:producer\"></meta>", "a11y:tactileGraphics\">PDF", "dcterms:dateCopyrighted", "dcterms:modified", "<meta property=\"a11y:certifiedBy\" id=\"c1\">X</meta>", "<dc:format>eBraille 1.0</dc:format>"];
        let mut at = 0;
        for piece in order {
            let found = xml[at..].find(piece).unwrap_or_else(|| panic!("{piece} missing or out of order in {xml}"));
            at += found;
        }
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20743), (2026, 10, 17));
        assert_eq!(uuid().len(), 36);
        assert_eq!(Metadata::default().id_type, 2);
    }

    #[test]
    fn metadata_has_dc_format() {
        assert!(metadata_xml(&Metadata::default(), "urn:uuid:x").contains("<dc:format>eBraille 1.0</dc:format>"));
        // Edited metadata gets it once, at its end.
        let edited = Metadata { edited_xml: Some("  <metadata>\n    <dc:title>T</dc:title>\n  </metadata>\n".into()), ..Metadata::default() };
        let xml = metadata_xml(&edited, "");
        assert_eq!(xml.matches("<dc:format>").count(), 1);
        assert!(xml.contains("<dc:format>eBraille 1.0</dc:format>\n  </metadata>"), "{xml}");
        assert_eq!(metadata_xml(&Metadata { edited_xml: Some(xml.clone()), ..Metadata::default() }, ""), xml);
    }

    #[test]
    fn education_level_is_dcterms() {
        let meta = Metadata { education_levels: vec!["Grade 4".into()], ..Metadata::default() };
        let xml = metadata_xml(&meta, "");
        assert!(xml.contains("<meta property=\"dcterms:educationLevel\">Grade 4</meta>"), "{xml}");
        assert!(!xml.contains("schema:"));
    }

    #[test]
    fn language_and_system_default_from_the_table() {
        let empty = Metadata::default();
        let m = empty.with_defaults(Some("eng-Brai"), Some("EBAE grade2"));
        let xml = metadata_xml(&m, "");
        assert!(xml.contains("<dc:language>eng-Brai</dc:language>"), "{xml}");
        assert!(xml.contains("<meta property=\"a11y:brailleSystem\">EBAE grade2</meta>"), "{xml}");
        let none = metadata_xml(&empty.with_defaults(Some("language"), Some("system")), "");
        assert!(!none.contains("<dc:language>") && !none.contains("a11y:brailleSystem"), "{none}");
        // The document's own language and system win.
        let own = Metadata { languages: vec!["fr".into()], braille_systems: vec![BrailleSystem { language: "fra".into(), grade: "grade1".into(), specialization: "None".into() }], ..Metadata::default() };
        let xml = metadata_xml(&own.with_defaults(Some("eng-Brai"), Some("EBAE grade2")), "");
        assert!(xml.contains("<dc:language>fr</dc:language>") && !xml.contains("eng-Brai"), "{xml}");
        assert!(xml.contains("a11y:brailleSystem\">fra-grade1</meta>") && !xml.contains("EBAE"), "{xml}");
    }

    #[test]
    fn the_opf_text_reads_back_as_the_same_metadata() {
        let meta = Metadata {
            titles: vec!["T & U".into()],
            creators: vec!["A".into(), "B".into()],
            producers: vec!["P".into()],
            languages: vec!["eng-Brai".into()],
            braille_systems: vec![
                BrailleSystem { language: "eng-Brai".into(), grade: "grade2".into(), specialization: "math".into() },
                BrailleSystem { language: "en".into(), grade: "grade1".into(), specialization: "None".into() },
                BrailleSystem { language: "EBAE grade2".into(), grade: String::new(), specialization: String::new() },
            ],
            date: "2026-01-02".into(),
            id_type: 0,
            identifier: "978".into(),
            date_copyrighted: "2025-05-06".into(),
            tactile_graphics: true,
            complete_transcription: true,
            publishers: vec!["Pub".into()],
            education_levels: vec!["K".into()],
            description: "D".into(),
            rights: vec!["R".into()],
            source_type: 1,
            source: "10.1/x".into(),
            subjects: vec!["S".into()],
            properties: vec![Property { name: "a11y:certifiedBy".into(), value: "X".into(), attributes: vec![("id".into(), "c1".into())] }],
            field_attributes: vec![("title".into(), vec![("xml:lang".into(), "en".into())])],
            edited_xml: None,
        };
        assert_eq!(metadata_from_opf(&opf_text(&meta)), meta);
        // No identifier: the placeholder is stored and read back as none.
        let plain = Metadata { date: "2026-01-02".into(), date_copyrighted: "2026-01-02".into(), ..Metadata::default() };
        let text = opf_text(&plain);
        assert!(text.contains("<dc:identifier>urn:uuid:{GUID}</dc:identifier>"), "{text}");
        assert_eq!(metadata_from_opf(&text), plain);
        // An element no field writes keeps the metadata element whole.
        let other = "<package><metadata><dc:coverage>x</dc:coverage><dc:title>T</dc:title></metadata></package>";
        let back = metadata_from_opf(other);
        assert_eq!(back.edited_xml.as_deref(), Some("  <metadata><dc:coverage>x</dc:coverage><dc:title>T</dc:title></metadata>\n"));
        assert_eq!(metadata_from_opf("not xml"), Metadata::default());
    }
}
