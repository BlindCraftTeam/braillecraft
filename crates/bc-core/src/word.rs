//! Word import: the .docx package turned into internal text with codes, as the original's Word recipe does it
//! (spec 28 sections 3, 6 to 8, 12 and 14): the stylesheet's rules (ported element by element from its 76
//! templates), the page-break fix, the run combining (font attribute runs joined and written as code instructions),
//! the style map (the text form of the shipped files and the XML form a document may carry), the tag processor
//! (a scanner over the stylesheet's output text that writes the style codes) and the scrub rules it leaves for the
//! step after it. The recipe's rule scripts and the shared steps run on the result (import_chain).

use crate::docx::{self, Element, Node, Part};

// ------------------------------------------------------------------ settings

/// How hard page breaks are read: as the document's own pages, as the print pages of a scanned book, or from the
/// publisher's own page-break marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrintPages {
    /// Hard page breaks ignored (the Print Page Breaks list's first choice).
    Ignore,
    Internal,
    Scanned,
    Publishers,
}

impl PrintPages {
    /// The Print Page Breaks list's choices in order: Ignore, Mark from Word document, Treat hard page breaks as
    /// print page breaks, Use publishers' reference page tags.
    pub fn from_choice(n: usize) -> PrintPages {
        [PrintPages::Ignore, PrintPages::Internal, PrintPages::Scanned, PrintPages::Publishers][n.min(3)]
    }
}

/// The Word importer's settings, with the original's defaults (its Global, Word Importer values).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Options {
    /// Text in a braille or SimBraille font is braille (brlinline); text in a Courier font is computer braille.
    pub braille_fonts: bool,
    pub cbc: bool,
    pub ignore_bold: bool,
    pub ignore_italics: bool,
    pub ignore_underline: bool,
    pub ignore_lang: bool,
    pub ignore_typeforms: bool,
    /// Preserve hard page breaks; preserve skipped lines (empty paragraphs).
    pub hard_pages: bool,
    pub skipped_lines: bool,
    pub print_pages: PrintPages,
    pub internal_hyperlinks: bool,
    pub defer_footnotes: bool,
    pub defer_endnotes: bool,
    /// Hyperlinks: 0 the URL, 1 the text, 2 both, 3 a hyperlink code pair; duplicates removed; URLs in plain text
    /// tagged as computer braille.
    pub hyperlinks: u8,
    pub remove_duplicates: bool,
    pub auto_tag_cbc: bool,
    /// Codes typed in the Word document are read as codes (Allow embedded codes).
    pub codes: bool,
    /// Global, Import Options: spaces around math signs of comparison; spaces around dashes removed.
    pub add_spaces: bool,
    pub remove_dash_spaces: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            braille_fonts: false,
            cbc: false,
            ignore_bold: false,
            ignore_italics: false,
            ignore_underline: false,
            ignore_lang: true,
            ignore_typeforms: false,
            hard_pages: true,
            skipped_lines: false,
            print_pages: PrintPages::Internal,
            internal_hyperlinks: false,
            defer_footnotes: true,
            defer_endnotes: true,
            hyperlinks: 2,
            remove_duplicates: false,
            auto_tag_cbc: false,
            codes: false,
            add_spaces: false,
            remove_dash_spaces: false,
        }
    }
}

/// The program data file of a template's Word style map name (the "word:mws" pseudo-style's text; the original's
/// file names are kept as data under the program's own names).
pub fn map_file_name(name: &str) -> String {
    match name.trim().to_lowercase().as_str() {
        "aussie" | "australian" => "australian".into(),
        "bana" => "bana".into(),
        "francais" | "french" => "french".into(),
        "rus" | "russian" => "russian".into(),
        "ukbasic" | "uk-basic" => "uk-basic".into(),
        // The original's own map (named after the program), an empty name and any name with no map shipped.
        _ => "default".into(),
    }
}

/// The control characters step (the recipe's control-character script, run on the stylesheet's output before the
/// tag processor): in every text node, a tab or a line feed becomes a space and every other character below a
/// space is dropped.
fn strip_controls(e: &mut Element) {
    for c in &mut e.children {
        match c {
            Node::Text(t) => {
                if t.chars().any(|ch| (ch as u32) < 0x20) {
                    *t = t.chars().filter_map(|ch| match ch as u32 { 9 | 10 => Some(' '), 0..=31 => None, _ => Some(ch) }).collect();
                }
            }
            Node::Element(child) => strip_controls(child),
            Node::Pi(_) => {}
        }
    }
}

/// The hyperlinks step (spec 28 section 10): a link arrives as 0x10 URL 0x10 TEXT 0x10; `action` 0 writes the URL,
/// 1 the text, 2 the text then " (URL)" unless `remove_duplicates` and the text equals the URL, 3 the link codes
/// rps:h and rpe:h around the text; an internal link (URL starting "#") under any action but 3 writes the text
/// only. With `tag_mode` 1 or 2, URLs and e-mail addresses in plain text are wrapped in computer braille codes
/// (mode 1: cbi and wb-cb before, txi and wb after; mode 2: the compinline style).
pub fn hyperlinks(text: &[u8], action: u8, remove_duplicates: bool, tag_mode: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    let mut plain_start = 0;
    let flush_plain = |out: &mut Vec<u8>, plain: &[u8]| {
        if tag_mode == 0 {
            out.extend_from_slice(plain);
        } else {
            auto_tag(plain, tag_mode, out);
        }
    };
    while i < text.len() {
        if text[i] == 0x10 {
            flush_plain(&mut out, &text[plain_start..i]);
            let rest = &text[i + 1..];
            let Some(u) = rest.iter().position(|&b| b == 0x10) else {
                out.extend_from_slice(&text[i..]);
                plain_start = text.len();
                break;
            };
            let url = &rest[..u];
            let after = &rest[u + 1..];
            let Some(t) = after.iter().position(|&b| b == 0x10) else {
                out.extend_from_slice(&text[i..]);
                plain_start = text.len();
                break;
            };
            let link_text = &after[..t];
            let internal = url.first() == Some(&b'#');
            match action {
                3 => {
                    out.extend_from_slice(b"\x1crps:h~");
                    out.extend_from_slice(url);
                    out.push(0x1f);
                    out.extend_from_slice(link_text);
                    out.extend_from_slice(b"\x1crpe:h\x1f");
                }
                _ if internal => out.extend_from_slice(link_text),
                0 => out.extend_from_slice(url),
                1 => out.extend_from_slice(link_text),
                _ => {
                    out.extend_from_slice(link_text);
                    if !(remove_duplicates && same_link(link_text, url)) {
                        out.extend_from_slice(b" (");
                        out.extend_from_slice(url);
                        out.push(b')');
                    }
                }
            }
            i += 1 + u + 1 + t + 1;
            plain_start = i;
        } else {
            i += 1;
        }
    }
    if plain_start < text.len() {
        flush_plain(&mut out, &text[plain_start..]);
    }
    out
}

/// Whether a link's text equals its URL, as the original compares them: the text's codes skipped, upper-cased, and
/// equal when the text runs out and the URL has no further letter or digit.
fn same_link(text: &[u8], url: &[u8]) -> bool {
    let mut t = 0;
    let mut u = 0;
    loop {
        while t < text.len() && text[t] == 0x1c {
            t = text[t..].iter().position(|&b| b == 0x1f).map_or(text.len(), |p| t + p + 1);
        }
        if t >= text.len() {
            return !url[u..].iter().any(|b| b.is_ascii_alphanumeric());
        }
        if u >= url.len() || text[t].to_ascii_uppercase() != url[u].to_ascii_uppercase() {
            return false;
        }
        t += 1;
        u += 1;
    }
}

/// URLs ("http://", "https://", "mailto") and e-mail addresses in plain text wrapped in the computer braille codes
/// of `mode`.
fn auto_tag(plain: &[u8], mode: u8, out: &mut Vec<u8>) {
    let (open, close): (&[u8], &[u8]) = if mode == 1 { (b"\x1ccbi\x1f\x1cwb-cb\x1f", b"\x1ctxi\x1f\x1cwb\x1f") } else { (b"\x1ces~compinline\x1f", b"\x1cee~compinline\x1f") };
    let is_url_char = |b: u8| !b.is_ascii_whitespace() && b != 0x1c && b != 0x1f;
    let is_mail_char = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_';
    let mut i = 0;
    let mut copied = 0;
    while i < plain.len() {
        if plain[i] == 0x1c {
            i = plain[i..].iter().position(|&b| b == 0x1f).map_or(plain.len(), |p| i + p + 1);
            continue;
        }
        let rest = &plain[i..];
        let prefix = [&b"http://"[..], b"https://", b"mailto"].iter().find(|p| rest.len() >= p.len() && rest[..p.len()].eq_ignore_ascii_case(p)).map(|p| p.len());
        if let Some(_) = prefix {
            let end = i + rest.iter().position(|&b| !is_url_char(b)).unwrap_or(rest.len());
            out.extend_from_slice(&plain[copied..i]);
            out.extend_from_slice(open);
            out.extend_from_slice(&plain[i..end]);
            out.extend_from_slice(close);
            copied = end;
            i = end;
            continue;
        }
        if plain[i] == b'@' && i > 0 && is_mail_char(plain[i - 1]) && i + 1 < plain.len() && is_mail_char(plain[i + 1]) {
            let mut start = i;
            while start > copied && is_mail_char(plain[start - 1]) {
                start -= 1;
            }
            let mut end = i + 1;
            while end < plain.len() && is_mail_char(plain[end]) {
                end += 1;
            }
            out.extend_from_slice(&plain[copied..start]);
            out.extend_from_slice(open);
            out.extend_from_slice(&plain[start..end]);
            out.extend_from_slice(close);
            copied = end;
            i = end;
            continue;
        }
        i += 1;
    }
    out.extend_from_slice(&plain[copied..]);
}

/// The apostrophe and quote forms of the character step (spec 28 section 11, step 3, run when the quotes script
/// follows): U+0027, U+2018 and U+2019 each take an override when one is given; otherwise the character is classed
/// by its neighbours (a letter or digit before only: U+2019; after only: U+2018, or U+02BC when it was U+2019;
/// both or neither: U+0027).
pub fn quote_forms(text: &str, apostrophe: Option<char>, left: Option<char>, right: Option<char>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (k, &c) in chars.iter().enumerate() {
        let replaced = match c {
            '\u{27}' => apostrophe,
            '\u{2018}' => left,
            '\u{2019}' => right,
            _ => {
                out.push(c);
                continue;
            }
        };
        if let Some(r) = replaced {
            out.push(r);
            continue;
        }
        let before = k > 0 && chars[k - 1].is_alphanumeric();
        let after = chars.get(k + 1).is_some_and(|n| n.is_alphanumeric());
        out.push(match (before, after) {
            (true, false) => '\u{2019}',
            (false, true) => {
                if c == '\u{2019}' {
                    '\u{2bc}'
                } else {
                    '\u{2018}'
                }
            }
            _ => '\u{27}',
        });
    }
    out
}

// ------------------------------------------------------------------ the package's parts

/// The parts the stylesheet reads: the main document, its relationships, the notes, comments, styles and numbering.
pub struct Package {
    pub document: Element,
    pub rels: Option<Element>,
    pub footnotes: Option<Element>,
    pub endnotes: Option<Element>,
    pub comments: Option<Element>,
    pub styles: Option<Element>,
    pub numbering: Option<Element>,
    pub parts: Vec<Part>,
}

impl Package {
    pub fn open(bytes: &[u8]) -> Result<Package, String> {
        let parts = docx::parts(bytes)?;
        let document = docx::xml_part(&parts, "word/document.xml")?.ok_or("This is not a Word document (no word/document.xml).")?;
        Ok(Package {
            rels: docx::xml_part(&parts, "word/_rels/document.xml.rels")?,
            footnotes: docx::xml_part(&parts, "word/footnotes.xml")?,
            endnotes: docx::xml_part(&parts, "word/endnotes.xml")?,
            comments: docx::xml_part(&parts, "word/comments.xml")?,
            styles: docx::xml_part(&parts, "word/styles.xml")?,
            numbering: docx::xml_part(&parts, "word/numbering.xml")?,
            document,
            parts,
        })
    }

    /// A relationship by id: (Target, TargetMode).
    fn rel(&self, id: &str) -> Option<(&str, &str)> {
        let rels = self.rels.as_ref()?;
        rels.elements("Relationship").find(|r| r.attr("Id") == Some(id)).map(|r| (r.attr("Target").unwrap_or(""), r.attr("TargetMode").unwrap_or("")))
    }

    /// A part named by a relationship, under word/.
    fn rel_part(&self, id: &str) -> Option<Element> {
        let (target, _) = self.rel(id)?;
        let name = format!("word/{}", target.trim_start_matches('/'));
        docx::xml_part(&self.parts, &name).ok().flatten()
    }

    /// The name of a style by its id (styles.xml), else the id.
    pub fn style_name(&self, id: &str) -> String {
        self.styles
            .as_ref()
            .and_then(|s| s.elements("w:style").find(|st| st.attr("w:styleId") == Some(id)))
            .and_then(|st| st.element("w:name").and_then(|n| n.attr("w:val")).map(str::to_string))
            .unwrap_or_else(|| id.to_string())
    }

    fn style(&self, id: &str) -> Option<&Element> {
        self.styles.as_ref()?.elements("w:style").find(|st| st.attr("w:styleId") == Some(id))
    }

    /// The numbering of a style, through the styles it is based on.
    fn style_num_pr(&self, id: &str, depth: usize) -> Option<&Element> {
        let style = self.style(id)?;
        if let Some(n) = style.at(&["w:pPr", "w:numPr"]) {
            return Some(n);
        }
        let base = style.element("w:basedOn")?.attr("w:val")?;
        if base == id || depth > 20 {
            return None;
        }
        self.style_num_pr(base, depth + 1)
    }
}

// ------------------------------------------------------------------ the stylesheet's rules

/// The stylesheet's output tree: elements named as it names them (p, text, br, box, doc, head, styles, style,
/// body, and the bc: elements) with their attributes, and text.
struct Transform<'a> {
    pkg: &'a Package,
    opt: &'a Options,
    /// Whether the paragraph being written has a right tab stop with dot leaders (its tabs are leader tabs).
    leader: std::cell::Cell<bool>,
}

fn el(name: &str) -> Element {
    Element { name: name.to_string(), ..Element::default() }
}

fn with_attr(mut e: Element, name: &str, value: &str) -> Element {
    e.attrs.push((name.to_string(), value.to_string()));
    e
}

fn enabled(e: Option<&Element>) -> bool {
    e.is_some_and(|e| e.attr("w:val") != Some("0") && e.attr("w:val") != Some("false"))
}

fn disabled(e: Option<&Element>) -> bool {
    e.is_some_and(|e| e.attr("w:val") == Some("0") || e.attr("w:val") == Some("false"))
}

impl<'a> Transform<'a> {
    /// The whole document: the stylesheet's w:body rule.
    fn body(&self, body: &Element) -> Element {
        let mut doc = el("doc");
        let mut head = el("head");
        let mut styles = el("styles");
        if let Some(s) = &self.pkg.styles {
            for st in s.elements("w:style").filter(|st| matches!(st.attr("w:type"), Some("paragraph") | Some("character"))) {
                let mut style = with_attr(el("style"), "id", st.attr("w:styleId").unwrap_or(""));
                if let Some(f) = st.at(&["w:rPr", "w:rFonts"]).and_then(|f| f.attr("w:ascii")) {
                    style = with_attr(style, "font", f);
                }
                self.add_attributes(&mut style, st);
                styles.children.push(Node::Element(style));
            }
        }
        head.children.push(Node::Element(styles));
        doc.children.push(Node::Element(head));
        let mut out = el("body");
        if let Some(num) = body.at(&["w:sectPr", "w:pgNumType"]) {
            if let Some(start) = num.attr("w:start") {
                let name = match num.attr("w:fmt") {
                    Some("lowerRoman") => "bc:pageNumLRStart",
                    Some("upperRoman") => "bc:pageNumURStart",
                    _ => "bc:pageNumStart",
                };
                let mut e = el(name);
                e.children.push(Node::Text(start.to_string()));
                out.children.push(Node::Element(e));
            }
        }
        let has_math = body.elements("w:p").any(|p| p.element("m:oMathPara").is_some() || p.element("m:oMath").is_some() || Self::has_equation(p));
        if has_math {
            out.children.push(Node::Element(el("bc:tcs")));
        }
        let children = body.element_children().collect::<Vec<_>>();
        for (k, c) in children.iter().enumerate() {
            self.apply(c, &children, k, &mut out);
        }
        self.endnotes(body, &mut out);
        if has_math {
            out.children.push(Node::Element(el("bc:tce")));
        }
        doc.children.push(Node::Element(out));
        doc
    }

    fn has_equation(p: &Element) -> bool {
        let mut found = Vec::new();
        p.find_all(&|e| e.name == "o:OLEObject" && matches!(e.attr("ProgID"), Some("Equation.DSMT4") | Some("Equation.3")), &mut found);
        !found.is_empty()
    }

    /// The deferred endnotes at the end of the body.
    fn endnotes(&self, body: &Element, out: &mut Element) {
        if !self.opt.defer_endnotes {
            return;
        }
        for p in body.elements("w:p") {
            for r in p.elements("w:r") {
                for reference in r.elements("w:endnoteReference") {
                    let id = reference.attr("w:id").unwrap_or("");
                    let mut note = el("bc:note");
                    if let Some(n) = self.pkg.endnotes.as_ref().and_then(|e| e.elements("w:endnote").find(|n| n.attr("w:id") == Some(id))) {
                        self.apply_children(n, &mut note);
                    }
                    out.children.push(Node::Element(note));
                }
            }
        }
    }

    /// apply-templates over the children of `e`.
    fn apply_children(&self, e: &Element, out: &mut Element) {
        let children: Vec<&Element> = e.element_children().collect();
        let mut k = 0;
        for c in &e.children {
            match c {
                Node::Text(t) => out.children.push(Node::Text(t.clone())),
                Node::Element(child) => {
                    self.apply(child, &children, k, out);
                    k += 1;
                }
                Node::Pi(text) => out.children.push(Node::Pi(text.clone())),
            }
        }
    }

    /// One element through the rule that matches it; `siblings` and `k` give its element siblings and its place.
    fn apply(&self, e: &Element, siblings: &[&Element], k: usize, out: &mut Element) {
        match e.name.as_str() {
            "w:p" => self.paragraph(e, siblings, k, out),
            "w:r" => self.run(e, out),
            "w:t" | "m:t" => out.children.push(Node::Text(e.text())),
            "w:tab" => {
                // A right tab stop with dot leaders in the paragraph makes its tabs leader tabs.
                out.children.push(Node::Element(el(if self.leader.get() { "bc:ltab" } else { "bc:tab" })));
            }
            "w:br" => match e.attr("w:type") {
                Some("page") => {
                    // The page break after text in the paragraph: a hard page.
                    self.simple_hardpage(out);
                }
                None => out.children.push(Node::Element(el("bc:l"))),
                _ => {}
            },
            "w:footnoteReference" => {
                let id = e.attr("w:id").unwrap_or("");
                out.children.push(Node::Text(id.to_string()));
                if !self.opt.defer_footnotes {
                    let mut note = el("bc:note");
                    if let Some(n) = self.pkg.footnotes.as_ref().and_then(|f| f.elements("w:footnote").find(|n| n.attr("w:id") == Some(id))) {
                        for p in n.elements("w:p") {
                            let ps: Vec<&Element> = n.element_children().collect();
                            let at = ps.iter().position(|x| std::ptr::eq(*x, p)).unwrap_or(0);
                            self.apply(p, &ps, at, &mut note);
                        }
                    }
                    out.children.push(Node::Element(note));
                }
            }
            "w:endnoteReference" => {
                let id = e.attr("w:id").unwrap_or("");
                out.children.push(Node::Text(id.to_string()));
                if !self.opt.defer_endnotes {
                    let mut note = el("bc:note");
                    if let Some(n) = self.pkg.endnotes.as_ref().and_then(|f| f.elements("w:endnote").find(|n| n.attr("w:id") == Some(id))) {
                        for p in n.elements("w:p") {
                            let ps: Vec<&Element> = n.element_children().collect();
                            let at = ps.iter().position(|x| std::ptr::eq(*x, p)).unwrap_or(0);
                            self.apply(p, &ps, at, &mut note);
                        }
                    }
                    out.children.push(Node::Element(note));
                }
            }
            "w:footnoteRef" | "w:endnoteRef" => {
                // The note's own number, at its start: the id of the note three levels up.
                out.children.push(Node::Text(String::new()));
            }
            "w:commentReference" => {
                let id = e.attr("w:id").unwrap_or("");
                let whole: String = self
                    .pkg
                    .comments
                    .as_ref()
                    .and_then(|c| c.elements("w:comment").find(|c| c.attr("w:id") == Some(id)))
                    .map(|c| c.elements("w:p").flat_map(|p| p.elements("w:r")).flat_map(|r| r.elements("w:t")).map(|t| t.text()).collect())
                    .unwrap_or_default();
                if let Some((start, number)) = whole.split_once(':') {
                    if start.ends_with("PageBreak") {
                        let mut pb = el("bc:pagebreak");
                        pb.children.push(Node::Text(number.to_string()));
                        out.children.push(Node::Element(pb));
                    }
                } else if whole.ends_with("PageBreak") {
                    out.children.push(Node::Element(el("bc:pagebreak")));
                }
            }
            "w:bookmarkStart" => {
                if self.opt.internal_hyperlinks {
                    out.children.push(Node::Element(with_attr(el("bc:rps_a"), "name", e.attr("w:name").unwrap_or(""))));
                }
            }
            "w:bookmarkEnd" => {
                if self.opt.internal_hyperlinks {
                    out.children.push(Node::Element(el("bc:rpe_a")));
                }
            }
            "w:hyperlink" => {
                let rel = e.attr("r:id").and_then(|id| self.pkg.rel(id));
                match rel {
                    Some((target, "External")) => {
                        out.children.push(Node::Element(el("bc:hyperlink-byte")));
                        out.children.push(Node::Text(target.to_string()));
                        out.children.push(Node::Element(el("bc:hyperlink-byte")));
                        self.apply_children(e, out);
                        out.children.push(Node::Element(el("bc:hyperlink-byte")));
                    }
                    _ => {
                        if self.opt.internal_hyperlinks {
                            out.children.push(Node::Element(el("bc:hyperlink-byte")));
                            out.children.push(Node::Text(format!("#{}", e.attr("w:anchor").unwrap_or(""))));
                            out.children.push(Node::Element(el("bc:hyperlink-byte")));
                        }
                        self.apply_children(e, out);
                        if self.opt.internal_hyperlinks {
                            out.children.push(Node::Element(el("bc:hyperlink-byte")));
                        }
                    }
                }
            }
            "w:instrText" => {}
            "wp:anchor" => {
                if let Some(d) = e.elements("wp:docPr").find(|d| d.attr("descr").is_some()) {
                    let mut alt = el("bc:alttext");
                    alt.children.push(Node::Text(d.attr("descr").unwrap_or("").to_string()));
                    out.children.push(Node::Element(alt));
                }
            }
            "wp:docPr" => {
                if let Some(descr) = e.attr("descr") {
                    let mut alt = el("bc:alttext");
                    alt.children.push(Node::Text(descr.to_string()));
                    out.children.push(Node::Element(alt));
                }
            }
            "w:tbl" => {
                let headers = e.elements("w:tr").filter(|tr| tr.at(&["w:trPr", "w:tblHeader"]).is_some()).count();
                let mut start = el("bc:htbs");
                start.children.push(Node::Text(format!("r:{headers}:0:b:n:a")));
                out.children.push(Node::Element(start));
                self.apply_children(e, out);
                out.children.push(Node::Element(el("bc:htbe")));
                if self.opt.defer_footnotes {
                    for tr in e.elements("w:tr") {
                        for tc in tr.elements("w:tc") {
                            for p in tc.elements("w:p") {
                                self.deferred_footnotes(p, out);
                            }
                        }
                    }
                }
            }
            "w:tr" => {
                if k > 0 && siblings[..k].iter().any(|s| s.name == "w:tr") {
                    out.children.push(Node::Element(el("bc:newline")));
                }
                self.apply_children(e, out);
            }
            "w:tc" => {
                if k > 0 && siblings[..k].iter().any(|s| s.name == "w:tc") {
                    out.children.push(Node::Element(el("bc:tab")));
                }
                self.apply_children(e, out);
            }
            "m:oMathPara" => {
                let math = e.element("m:oMath").is_some();
                if !math {
                    out.children.push(Node::Element(el("bc:ts")));
                }
                self.apply_children(e, out);
                if !math {
                    out.children.push(Node::Element(el("bc:te")));
                }
            }
            "m:oMath" => {
                out.children.push(Node::Text(" ".to_string()));
                out.children.push(Node::Element(el("bc:ts")));
                self.apply_children(e, out);
                out.children.push(Node::Element(el("bc:te")));
            }
            "m:d" => {
                let beg = e.at(&["m:dPr", "m:begChr"]).and_then(|c| c.attr("m:val")).unwrap_or("(");
                let end = e.at(&["m:dPr", "m:endChr"]).and_then(|c| c.attr("m:val")).unwrap_or(")");
                out.children.push(Node::Text(beg.to_string()));
                self.apply_children(e, out);
                out.children.push(Node::Text(end.to_string()));
            }
            "m:acc" => {
                let chr = e.at(&["m:accPr", "m:chr"]).and_then(|c| c.attr("m:val"));
                if chr == Some("\u{0305}") {
                    out.children.push(Node::Element(el("bc:e")));
                    self.apply_children(e, out);
                    out.children.push(Node::Element(el("bc:bar")));
                } else {
                    out.children.push(Node::Element(el("bc:e")));
                    self.apply_children(e, out);
                    out.children.push(Node::Element(el("bc:os")));
                    let text = match chr {
                        Some("\u{030c}") => "\u{02c7}",
                        Some("\u{20e1}") => "\u{2192}",
                        Some("\u{20d6}") => "\u{2190}",
                        Some("\u{20d7}") => "\u{2194}",
                        Some("\u{0301}") => "\u{00b4}",
                        Some("\u{0300}") => "\u{0060}",
                        Some("\u{0303}") => "\u{007e}",
                        Some("\u{0306}") => "\u{02d8}",
                        Some(c) => c,
                        None => "^",
                    };
                    out.children.push(Node::Text(text.to_string()));
                    out.children.push(Node::Element(el("bc:oe")));
                }
            }
            "m:bar" => {
                out.children.push(Node::Element(el("bc:e")));
                self.apply_children(e, out);
                out.children.push(Node::Element(el("bc:bar")));
            }
            "m:sup" => {
                out.children.push(Node::Element(el("bc:ps")));
                self.apply_children(e, out);
                out.children.push(Node::Element(el("bc:pe")));
            }
            "m:sub" => {
                out.children.push(Node::Element(el("bc:bs")));
                self.apply_children(e, out);
                out.children.push(Node::Element(el("bc:be")));
            }
            "m:rad" => {
                let indexed = e.element("m:deg").and_then(|d| d.element("m:r")).and_then(|r| r.element("m:t")).is_some_and(|t| t.text() != "2");
                out.children.push(Node::Element(el(if indexed { "bc:ixrts" } else { "bc:sqrts" })));
                self.apply_children(e, out);
                out.children.push(Node::Element(el(if indexed { "bc:ixrte" } else { "bc:sqrte" })));
            }
            "m:deg" => {
                if e.element("m:r").and_then(|r| r.element("m:t")).is_some_and(|t| t.text() != "2") {
                    self.apply_children(e, out);
                    out.children.push(Node::Element(el("bc:ixrtd")));
                }
            }
            "m:func" => {
                for name in ["m:fName", "m:arg", "m:e"] {
                    if name == "m:arg" {
                        out.children.push(Node::Element(el("bc:DX_CHSPH")));
                    }
                    for c in e.elements(name) {
                        self.apply_children(c, out);
                    }
                }
            }
            "m:f" => {
                out.children.push(Node::Element(el("bc:fs")));
                for c in e.elements("m:num") {
                    self.apply_children(c, out);
                }
                out.children.push(Node::Element(el("bc:fl")));
                for c in e.elements("m:den") {
                    self.apply_children(c, out);
                }
                out.children.push(Node::Element(el("bc:fe")));
            }
            "o:OLEObject" if matches!(e.attr("ProgID"), Some("Equation.DSMT4") | Some("Equation.3")) => {
                let kind = if e.attr("ProgID") == Some("Equation.DSMT4") { "bc:mathtype" } else { "bc:equation" };
                let stream = e.attr("r:id").and_then(|id| self.pkg.rel(id)).map(|(t, _)| t.to_string()).unwrap_or_default();
                out.children.push(Node::Element(with_attr(el(kind), "stream", &stream)));
                // The shape's alt text follows on its own line (the equation itself is an OLE stream).
                if let Some(alt) = siblings.iter().find(|s| s.name == "v:shape").and_then(|s| s.attr("alt")) {
                    out.children.push(Node::Element(el("bc:l")));
                    out.children.push(Node::Text(alt.to_string()));
                }
            }
            "w:checkBox" => {
                let mut cb = el("bc:checkbox");
                let checked = e.element("w:checked").is_some() || e.element("w:default").and_then(|d| d.attr("w:val")) == Some("1");
                cb.children.push(Node::Text(if checked { "(*)" } else { "( )" }.to_string()));
                out.children.push(Node::Element(cb));
            }
            "w:sectPr" | "w:pPr" | "w:rPr" | "w:tblPr" | "w:tblGrid" | "w:trPr" | "w:tcPr" | "w:proofErr" | "w:lastRenderedPageBreak" => {}
            _ => self.apply_children(e, out),
        }
    }

    fn simple_hardpage(&self, out: &mut Element) {
        if self.opt.hard_pages {
            out.children.push(Node::Element(el("bc:hardpage")));
        }
        if self.opt.print_pages == PrintPages::Scanned {
            out.children.push(Node::Element(el("bc:scannedpagebreak")));
        }
    }

    /// The footnotes a paragraph refers to, after it.
    fn deferred_footnotes(&self, p: &Element, out: &mut Element) {
        for r in p.elements("w:r") {
            for reference in r.elements("w:footnoteReference") {
                let id = reference.attr("w:id").unwrap_or("");
                let mut note = el("bc:note");
                if let Some(n) = self.pkg.footnotes.as_ref().and_then(|f| f.elements("w:footnote").find(|n| n.attr("w:id") == Some(id))) {
                    self.apply_children(n, &mut note);
                }
                out.children.push(Node::Element(note));
            }
        }
    }

    /// The section properties of a paragraph: its own, the next paragraph's that holds them, or the body's.
    fn section<'b>(&self, p: &'b Element, siblings: &[&'b Element], k: usize) -> Option<&'b Element> {
        if let Some(s) = p.at(&["w:pPr", "w:sectPr"]) {
            return Some(s);
        }
        for s in &siblings[k + 1..] {
            if let Some(sect) = s.at(&["w:pPr", "w:sectPr"]) {
                return Some(sect);
            }
        }
        siblings[k + 1..].iter().find(|s| s.name == "w:sectPr").copied()
    }

    /// The stylesheet's sectPr rule: a hard page between sections, and the header and footer of the section.
    fn section_start(&self, sect: Option<&Element>, page_break: bool, out: &mut Element) {
        let break_type = sect.and_then(|s| s.element("w:type")).and_then(|t| t.attr("w:val"));
        if page_break && break_type != Some("continuous") && self.opt.hard_pages {
            let mut hp = el("bc:hardpage");
            if let Some(t) = break_type {
                hp = with_attr(hp, "type", t);
            }
            out.children.push(Node::Element(hp));
        }
        let Some(sect) = sect else { return };
        for (reference, root, open, close) in [("w:headerReference", "w:hdr", "bc:tls", "bc:tle"), ("w:footerReference", "w:ftr", "bc:rfs", "bc:rfe")] {
            let Some(id) = sect.element(reference).and_then(|h| h.attr("r:id")) else { continue };
            let part = self.pkg.rel_part(id);
            let runs: Vec<String> = part
                .as_ref()
                .filter(|p| p.name == root)
                .map(|p| p.elements("w:p").flat_map(|p| p.elements("w:r")).filter(|r| r.element_children().next().is_some()).map(|r| r.elements("w:t").map(|t| t.text()).collect::<String>()).collect())
                .unwrap_or_default();
            if page_break || !runs.is_empty() {
                out.children.push(Node::Element(el(open)));
                for r in runs {
                    out.children.push(Node::Text(r));
                }
                out.children.push(Node::Element(el(close)));
            }
        }
    }

    /// The stylesheet's w:p rule.
    fn paragraph(&self, p: &Element, siblings: &[&Element], k: usize, out: &mut Element) {
        let first = k == 0;
        let previous_sect = k > 0 && siblings[k - 1].at(&["w:pPr", "w:sectPr"]).is_some();
        if first || previous_sect {
            let page_break = siblings[..k].iter().any(|s| s.name == "w:p");
            self.section_start(self.section(p, siblings, k), page_break, out);
        } else if p.elements("w:r").next().and_then(|r| r.element("w:br")).and_then(|b| b.attr("w:type")) == Some("page") {
            self.simple_hardpage(out);
        }
        self.leader.set(p.at(&["w:pPr", "w:tabs"]).is_some_and(|tabs| tabs.elements("w:tab").any(|t| t.attr("w:val") == Some("right") && t.attr("w:leader") == Some("dot"))));
        let has_text = p.elements("w:r").any(|r| r.element("w:t").is_some())
            || p.element_children().any(|c| c.elements("w:r").any(|r| r.element("w:t").is_some()))
            || p.element("w:smartTag").is_some()
            || Self::has_equation(p)
            || p.element("m:oMath").is_some()
            || p.element("m:oMathPara").is_some();
        if has_text {
            let mut pe = el("p");
            if let Some(jc) = p.at(&["w:pPr", "w:jc"]).and_then(|j| j.attr("w:val")) {
                pe = with_attr(pe, "jc", jc);
            }
            if let Some(class) = p.at(&["w:pPr", "w:pStyle"]).and_then(|s| s.attr("w:val")) {
                pe = with_attr(pe, "class", class);
            }
            if let Some(num) = self.num_pr(p) {
                let ilvl = num.element("w:ilvl").and_then(|l| l.attr("w:val")).unwrap_or("0");
                let numid = self.num_id(p).unwrap_or_default();
                let mut n = with_attr(el("bc:num"), "id", &numid);
                n = with_attr(n, "lvl", ilvl);
                n = with_attr(n, "class", "word");
                pe.children.push(Node::Element(n));
            }
            self.apply_children(p, &mut pe);
            out.children.push(Node::Element(pe));
            if self.opt.defer_footnotes {
                self.deferred_footnotes(p, out);
            }
        } else if p.element("w:sectPr").is_some() {
        } else if Self::has_equation(p) || p.elements("w:r").any(|r| r.at(&["w:pict", "v:shape", "v:textbox"]).is_some() || r.at(&["mc:AlternateContent", "mc:Fallback", "w:pict", "v:shape", "v:textbox"]).is_some() || r.element("w:drawing").is_some()) || p.element("m:oMath").is_some() {
            if Self::has_equation(p) {
                let mut pe = el("p");
                self.apply_children(p, &mut pe);
                out.children.push(Node::Element(pe));
            } else {
                self.apply_children(p, out);
            }
        } else if self.opt.skipped_lines {
            out.children.push(Node::Element(el("br")));
        }
    }

    fn num_pr<'b>(&'b self, p: &'b Element) -> Option<&'b Element> {
        if let Some(n) = p.at(&["w:pPr", "w:numPr"]) {
            return Some(n);
        }
        let style = p.at(&["w:pPr", "w:pStyle"])?.attr("w:val")?;
        self.pkg.style_num_pr(style, 0)
    }

    fn num_id(&self, p: &Element) -> Option<String> {
        self.num_pr(p)?.element("w:numId")?.attr("w:val").map(str::to_string)
    }

    /// The stylesheet's w:r rule: a text box as a box, else a text element with the run's class, language, font
    /// and attributes.
    fn run(&self, r: &Element, out: &mut Element) {
        let boxes = [r.at(&["w:pict", "v:shape", "v:textbox", "w:txbxContent"]), r.at(&["mc:AlternateContent", "mc:Fallback", "w:pict", "v:shape", "v:textbox", "w:txbxContent"])];
        let mut any_box = false;
        for content in boxes.into_iter().flatten() {
            any_box = true;
            let mut b = el("box");
            self.apply_children(content, &mut b);
            out.children.push(Node::Element(b));
        }
        if any_box {
            return;
        }
        let mut text = el("text");
        if let Some(t) = r.element("w:t") {
            for (n, v) in &t.attrs {
                text.attrs.push((n.clone(), v.clone()));
            }
        }
        if let Some(class) = r.at(&["w:rPr", "w:rStyle"]).and_then(|s| s.attr("w:val")) {
            text = with_attr(text, "class", class);
        }
        if !self.opt.ignore_lang {
            if let Some(lang) = r.at(&["w:rPr", "w:lang"]) {
                if let Some(v) = lang.attr("w:val").or_else(|| lang.attr("w:bidi")) {
                    text = with_attr(text, "lng", v);
                }
            }
        }
        if let Some(f) = r.at(&["w:rPr", "w:rFonts"]).and_then(|f| f.attr("w:ascii")) {
            text = with_attr(text, "font", f);
        }
        self.add_attributes(&mut text, r);
        self.apply_children(r, &mut text);
        out.children.push(Node::Element(text));
    }

    /// The attribset and attribreset attributes: the attributes the run or style turns on and off.
    fn add_attributes(&self, target: &mut Element, e: &Element) {
        let pr = e.element("w:rPr");
        let b = pr.and_then(|p| p.element("w:b"));
        let i = pr.and_then(|p| p.element("w:i"));
        let u = pr.and_then(|p| p.element("w:u"));
        let vertical = pr.and_then(|p| p.element("w:vertAlign")).and_then(|v| v.attr("w:val"));
        let mut set = String::new();
        if !self.opt.ignore_bold && enabled(b) {
            set.push('b');
        }
        if !self.opt.ignore_italics && enabled(i) {
            set.push('i');
        }
        if !self.opt.ignore_underline && enabled(u) {
            set.push('u');
        }
        if vertical == Some("superscript") {
            set.push('S');
        }
        if vertical == Some("subscript") {
            set.push('s');
        }
        if !set.is_empty() {
            target.attrs.push(("attribset".into(), set));
        }
        let mut reset = String::new();
        if !self.opt.ignore_bold && disabled(b) {
            reset.push('b');
        }
        if !self.opt.ignore_italics && disabled(i) {
            reset.push('i');
        }
        if !self.opt.ignore_underline && disabled(u) {
            reset.push('u');
        }
        if vertical.is_some_and(|v| v != "superscript") {
            reset.push('S');
        }
        if vertical.is_some_and(|v| v != "subscript") {
            reset.push('s');
        }
        // Typeforms are not ignored: with no mapping set, each typeform is reset (the placeholder functions give
        // nothing).
        if !self.opt.ignore_typeforms {
            reset.push_str("c12345");
        }
        if !reset.is_empty() {
            target.attrs.push(("attribreset".into(), reset));
        }
    }
}

/// The page-break fix (spec 28 14a): a paragraph whose first element holds only a page break has the break moved
/// before it; a RefPageNumber paragraph between two paragraphs of one List class is folded into the one before.
fn fix_page_breaks(e: &mut Element) {
    let mut k = 0;
    while k < e.children.len() {
        let is_p = matches!(&e.children[k], Node::Element(c) if c.name == "p");
        if !is_p {
            if let Node::Element(c) = &mut e.children[k] {
                fix_page_breaks(c);
            }
            k += 1;
            continue;
        }
        let Node::Element(p) = &e.children[k] else { unreachable!() };
        let first_child_pagebreak = p.element_children().next().filter(|f| {
            let inner: Vec<&Element> = f.element_children().collect();
            inner.len() == 1 && inner[0].name == "bc:pagebreak"
        });
        if let Some(first) = first_child_pagebreak {
            let pb = first.element_children().next().cloned().unwrap_or_default();
            let Node::Element(p) = &mut e.children[k] else { unreachable!() };
            let at = p.children.iter().position(|c| matches!(c, Node::Element(_))).unwrap_or(0);
            p.children.remove(at);
            e.children.insert(k, Node::Element(pb));
            k += 2;
            continue;
        }
        let class = p.attr("class").map(str::to_string);
        if class.as_deref() == Some("RefPageNumber") && k > 0 && k + 1 < e.children.len() {
            let prev_class = match &e.children[k - 1] {
                Node::Element(c) if c.name == "p" => c.attr("class").map(str::to_string),
                _ => None,
            };
            let next_class = match &e.children[k + 1] {
                Node::Element(c) if c.name == "p" => c.attr("class").map(str::to_string),
                _ => None,
            };
            if prev_class.as_deref().is_some_and(|c| c.starts_with("List")) && next_class == prev_class {
                let Node::Element(p) = &e.children[k] else { unreachable!() };
                let first = p.element_children().next().cloned();
                if let Some(mut first) = first {
                    first.attrs.retain(|(n, _)| n != "class");
                    first.attrs.push(("class".into(), "RefPageNumberEmbed".into()));
                    if let Node::Element(prev) = &mut e.children[k - 1] {
                        prev.children.push(Node::Element(first));
                    }
                }
                e.children.remove(k);
                continue;
            }
        }
        k += 1;
    }
}

// ------------------------------------------------------------------ combineruns

/// The attribute bits of a run (spec 28 section 12): bold 8, italic 0x10, underline 0x20, superscript 0x40,
/// subscript 0x80, the typeforms 0x100 to 0x1000 and script 0x2000; computer braille 4, braille 1 and 2.
fn attribute_bits(letters: &str) -> u32 {
    letters
        .chars()
        .map(|c| match c {
            'b' => 0x08,
            'i' => 0x10,
            'u' => 0x20,
            'S' => 0x40,
            's' => 0x80,
            'c' => 0x2000,
            '1' => 0x100,
            '2' => 0x200,
            '3' => 0x400,
            '4' => 0x800,
            '5' => 0x1000,
            _ => 0,
        })
        .fold(0, |a, b| a | b)
}

/// The code instructions written when attribute bits turn on and off, in the original's orders.
fn attribute_codes(from: u32, to: u32, braille_fonts: bool, out: &mut Vec<Node>) {
    let off = from & !to;
    let on = to & !from;
    let closers: [(u32, &str); 13] = [
        (0x20, "$fte~u`"),
        (0x10, "$fte~i`"),
        (0x08, "$fte~b`"),
        (0x40, "$pe`"),
        (0x80, "$be`"),
        (0x2000, "$fte~s`"),
        (0x100, "$fte~1`"),
        (0x200, "$fte~2`"),
        (0x400, "$fte~3`"),
        (0x800, "$fte~4`"),
        (0x1000, "$fte~5`"),
        (0x02, "$compress`$ee~brlinline-fr`"),
        (0x04, "$ee~compinline`"),
    ];
    for (bit, code) in closers {
        if off & bit != 0 && (bit > 0x04 || braille_fonts || bit == 0x04) {
            out.push(Node::Pi(code.to_string()));
        }
    }
    if off & 0x01 != 0 && off & 0x02 == 0 && braille_fonts {
        out.push(Node::Pi("$compress`$ee~brlinline`".to_string()));
    }
    let openers: [(u32, &str); 13] = [
        (0x04, "$es~compinline`"),
        (0x02, "$es~brlinline-fr`$as-is`"),
        (0x1000, "$fts~5`"),
        (0x800, "$fts~4`"),
        (0x400, "$fts~3`"),
        (0x200, "$fts~2`"),
        (0x100, "$fts~1`"),
        (0x2000, "$fts~s`"),
        (0x80, "$bs`"),
        (0x40, "$ps`"),
        (0x08, "$fts~b`"),
        (0x10, "$fts~i`"),
        (0x20, "$fts~u`"),
    ];
    if on & 0x01 != 0 && on & 0x02 == 0 && braille_fonts {
        out.push(Node::Pi("$es~brlinline`$as-is`".to_string()));
    }
    for (bit, code) in openers {
        if on & bit != 0 && (bit > 0x04 || braille_fonts || bit == 0x04) {
            out.push(Node::Pi(code.to_string()));
        }
    }
}

/// The run combining step (spec 28 section 12): in each paragraph the runs' attribute bits are followed (the
/// paragraph style's, then each run's set and reset letters, and the font: Courier gives computer braille when
/// that setting is on, a braille font braille), the codes for the changes are written as code instructions, and
/// adjacent runs with the same bits and language are joined. Then the body's children stand under the root.
fn combine_runs(doc: &mut Element, pkg: &Package, opt: &Options) {
    let style_bits: Vec<(String, u32)> = doc
        .element("head")
        .and_then(|h| h.element("styles"))
        .map(|s| s.elements("style").map(|st| (st.attr("id").unwrap_or("").to_string(), attribute_bits(st.attr("attribset").unwrap_or("")) & !attribute_bits(st.attr("attribreset").unwrap_or("")))).collect())
        .unwrap_or_default();
    let body = doc.children.iter_mut().find_map(|c| match c {
        Node::Element(e) if e.name == "body" => Some(e),
        _ => None,
    });
    let Some(body) = body else { return };
    combine_in(body, 0, &style_bits, pkg, opt);
    let children = std::mem::take(&mut body.children);
    let name = body.name.clone();
    doc.children = children;
    doc.name = name;
}

fn font_bits(font: &str, opt: &Options) -> u32 {
    let f = font.to_lowercase();
    let mut bits = 0;
    if opt.cbc && f.contains("courier") {
        bits |= 0x04;
    }
    if opt.braille_fonts && (f.contains("simbraille") || f.contains("braille")) {
        bits |= if f.contains("simbraille") { 0x02 } else { 0x01 };
    }
    bits
}

fn combine_in(e: &mut Element, inherited: u32, style_bits: &[(String, u32)], pkg: &Package, opt: &Options) {
    let mut flags = inherited;
    if e.name == "p" {
        if let Some(class) = e.attr("class") {
            let name = pkg.style_name(class);
            if let Some((_, bits)) = style_bits.iter().find(|(id, _)| *id == class || *id == name) {
                flags |= bits;
            }
        }
    }
    let mut out: Vec<Node> = Vec::with_capacity(e.children.len());
    // The codes follow the attribute state from the enclosing element's state: a paragraph whose style is bold
    // starts its first run with the bold code.
    let mut current = inherited;
    let mut last_lng: Option<String> = None;
    for child in std::mem::take(&mut e.children) {
        match child {
            Node::Element(mut run) if run.name == "text" => {
                let set = attribute_bits(run.attr("attribset").unwrap_or("")) | font_bits(run.attr("font").unwrap_or(""), opt);
                let reset = attribute_bits(run.attr("attribreset").unwrap_or(""));
                let bits = (flags | set) & !reset;
                let lng = run.attr("lng").map(str::to_string);
                // A run of tabs only is kept as it is.
                let tabs_only = !run.children.is_empty() && run.children.iter().all(|c| matches!(c, Node::Element(t) if t.name == "bc:tab"));
                if !tabs_only && bits != current {
                    attribute_codes(current, bits, opt.braille_fonts, &mut out);
                    current = bits;
                }
                run.attrs.retain(|(n, _)| n != "attribset" && n != "attribreset" && n != "font");
                // Joined with the run before it when nothing changed.
                let same = !tabs_only && lng == last_lng && matches!(out.last(), Some(Node::Element(prev)) if prev.name == "text");
                if same {
                    if let Some(Node::Element(prev)) = out.last_mut() {
                        prev.children.append(&mut run.children);
                        continue;
                    }
                }
                last_lng = lng;
                out.push(Node::Element(run));
            }
            Node::Element(mut other) => {
                combine_in(&mut other, flags, style_bits, pkg, opt);
                out.push(Node::Element(other));
            }
            other => out.push(other),
        }
    }
    if current != inherited {
        attribute_codes(current, inherited, opt.braille_fonts, &mut out);
    }
    e.children = out;
}

// ------------------------------------------------------------------ the output text

fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

/// The tree as the text the tag processor scans: start and end tags with their attributes, the text with its
/// three markup characters as entities, and the code instructions as processing instructions.
pub fn serialize(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.name);
    for (n, v) in &e.attrs {
        out.push(' ');
        out.push_str(n);
        out.push_str("=\"");
        escape(v, out);
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &e.children {
        match c {
            Node::Text(t) => escape(t, out),
            Node::Element(child) => serialize(child, out),
            Node::Pi(text) => {
                out.push_str("<?bc ");
                out.push_str(text);
                out.push_str("?>");
            }
        }
    }
    out.push_str("</");
    out.push_str(&e.name);
    out.push('>');
}

// ------------------------------------------------------------------ the style map

/// One entry of the style map (spec 28 6b and the text form's reader): what it matches (the element, or any; the
/// class, the Word style name) and what it gives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapEntry {
    pub element: String,
    pub class: String,
    pub style: String,
    pub code_start: String,
    pub code_end: String,
    pub continue_class: String,
    pub note_style: String,
    pub append: String,
    pub level: i64,
    pub despace: bool,
    pub no_center: bool,
    /// The entry's aggregation field (the original's one field at +0x10): the text form starts it at the entry's
    /// kind, 1 and 2 for a BuiltIn block's paragraph and any-element entries, 3 and 4 for a User block's, 0 for the
    /// alt text entries; the AggregationStyle key then overwrites the paragraph entry's value (None 0, SingleLevel
    /// 1, MultiLevel 2, Paragraphs 3), and at the block's end a 0 with a forward or backward flag becomes 1 (level
    /// below 1) or 2. The XML form sets 2 for aggregate. The scrub rules are written by this value.
    pub aggregate: i64,
    /// The text form's AggregateForward and AggregateBackward flags (the paragraph entry only).
    pub forward: bool,
    pub backward: bool,
    /// 2 when the element's content is skipped (no style, no codes).
    pub mode: u8,
}

/// The style map: its entries in order, first match wins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleMap {
    pub entries: Vec<MapEntry>,
    /// Read from the XML form (docProps/custom.xml), whose loader also writes the closing run-lea rule; the text
    /// form's loader does not.
    pub xml_form: bool,
}

fn boolean(value: &str, default: bool) -> bool {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => true,
        "false" | "no" | "0" => false,
        _ => default,
    }
}

/// The scrub rules of a continued style (spec 28 6b): its end followed by its tmp: start becomes a paragraph and a
/// runover code, tmp: end and tmp: start a line and a runover code, with the aggregate rule also tmp: end and the
/// plain start a paragraph code; then the tmp: forms become the style's own codes.
fn continue_rules(out: &mut String, s: &str, aggregate: bool) {
    const O: char = '\x1c';
    const C: char = '\x1f';
    out.push_str(&format!("{O}ee|~{s}{C}{O}es|~tmp:{s}{C}~{O}p{C}{O}run{C}~\n"));
    out.push_str(&format!("{O}ee|~tmp:{s}{C}{O}es|~tmp:{s}{C}~{O}l{C}{O}run{C}~\n"));
    if aggregate {
        out.push_str(&format!("{O}ee|~tmp:{s}{C}{O}es|~{s}{C}~{O}p{C}~\n"));
    }
    out.push_str(&format!("{O}es|~tmp:{s}{C}~{O}es|~{s}{C}~\n"));
    out.push_str(&format!("{O}ee|~tmp:{s}{C}~{O}ee|~{s}{C}~\n"));
}

impl StyleMap {
    /// The text form of the shipped style map files: blocks "BuiltIn", "User" or "AltText" with a quoted name and
    /// { Key = Value } lines, read as the original reads them: each block gives two entries, one for the paragraph
    /// element and one for any element; MappedName takes a trailing dot; MappedNameInNote, Level, IgnoreCentering,
    /// AggregateForward, AggregateBackward, AggregationStyle and IgnoreContents = Always are kept; Category,
    /// Ignore, IgnoreEmphases, DisableInToc and IgnoreSuperscript are read and not kept. The two alt text entries
    /// come first with the style "note.".
    pub fn from_text(text: &str) -> Result<StyleMap, String> {
        let mut map = StyleMap::default();
        // The reader's own first two entries: the alt text element with the styles "note." and
        // "note"; an AltText block changes these two instead of adding entries.
        for style in ["note.", "note"] {
            map.entries.push(MapEntry { element: "bc:alttext".into(), style: style.into(), despace: style.ends_with('.'), ..MapEntry::default() });
        }
        let mut lines = text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).peekable();
        while let Some(line) = lines.next() {
            // An AltText block has no name.
            let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
            if !matches!(kind, "BuiltIn" | "User" | "AltText") {
                return Err(format!("style map: unknown block {kind}"));
            }
            let name = rest.trim().trim_matches('"').to_string();
            let (kind_p, kind_any) = match kind {
                "BuiltIn" => (1, 2),
                "User" => (3, 4),
                _ => (0, 0),
            };
            let mut p = MapEntry { element: "p".into(), class: name.clone(), aggregate: kind_p, ..MapEntry::default() };
            let mut any = MapEntry { class: name.clone(), aggregate: kind_any, ..MapEntry::default() };
            if kind == "AltText" {
                // The block's keys go to the two alt text entries already in the list.
                p = map.entries[0].clone();
                any = map.entries[1].clone();
            }
            match lines.next() {
                Some("{") => {}
                other => return Err(format!("style map: {{ expected after {name}, found {other:?}")),
            }
            loop {
                let Some(l) = lines.next() else { return Err("style map: block not closed".into()) };
                if l == "}" {
                    break;
                }
                let Some((key, value)) = l.split_once('=') else { return Err(format!("style map: {l}")) };
                let value = value.trim().trim_matches('"').to_string();
                match key.trim() {
                    // The paragraph entry's style takes a trailing dot when it has none (an empty name becomes
                    // "."); the any-element entry keeps the name as written (the key parser, its first key).
                    "MappedName" => {
                        any.style = value.clone();
                        let mut s = value;
                        if !s.ends_with('.') {
                            s.push('.');
                        }
                        p.style = s;
                    }
                    "MappedNameInNote" => {
                        p.note_style = value.clone();
                        any.note_style = value;
                    }
                    "Level" => {
                        let n = value.parse::<i64>().unwrap_or(0);
                        p.level = n;
                        any.level = n;
                    }
                    "AggregateForward" => p.forward = value != "Never",
                    "AggregateBackward" => p.backward = value != "Never",
                    "IgnoreCentering" => p.no_center = boolean(&value, false),
                    "IgnoreContents" => {
                        if value == "Always" {
                            p.mode = 2;
                            any.mode = 2;
                        }
                    }
                    "AggregationStyle" => p.aggregate = ["None", "SingleLevel", "MultiLevel", "Paragraphs"].iter().position(|v| *v == value).unwrap_or(0) as i64,
                    "Category" | "Ignore" | "IgnoreEmphases" | "DisableInToc" | "IgnoreSuperscript" => {}
                    other => return Err(format!("style map: unknown key {other}")),
                }
            }
            if (p.forward || p.backward) && p.aggregate == 0 {
                p.aggregate = if p.level < 1 { 1 } else { 2 };
            }
            for e in [&mut p, &mut any] {
                e.despace = e.style.ends_with('.');
            }
            // A FootnoteText or EndnoteText block's paragraph entry takes the any-element entry's style.
            if p.class == "FootnoteText" || p.class == "EndnoteText" {
                p.style = any.style.clone();
            }
            if kind == "AltText" {
                map.entries[0] = p;
                map.entries[1] = any;
            } else {
                map.entries.push(p);
                map.entries.push(any);
            }
        }
        // The loader's own five entries after the blocks (the names read from the loader's machine code): the
        // box, paragraph, note, body and text elements; the paragraph one takes the style of the "Body Text" block
        // when there is one, else "para.", so an unmapped paragraph is still a paragraph; body and text have no
        // style, so those tags write nothing.
        let body_text = map.entries.iter().find(|e| e.class == "Body Text").map(|e| e.style.clone()).unwrap_or_else(|| "para.".into());
        for (element, style) in [("box", "box".to_string()), ("p", body_text), ("note", "note.".into()), ("body", String::new()), ("text", String::new())] {
            map.entries.push(MapEntry { element: element.into(), despace: style.ends_with('.'), style, ..MapEntry::default() });
        }
        Ok(map)
    }

    /// The XML form a document may carry in docProps/custom.xml (spec 28 6b): each child of the root is an entry
    /// with the attributes element (or name), class, style, code_start, code_end, continue, notestyle, level,
    /// despace, append, aggregate and center.
    pub fn from_xml(root: &Element) -> StyleMap {
        let mut map = StyleMap { xml_form: true, ..StyleMap::default() };
        for c in root.element_children() {
            let a = |n: &str| c.attr(n).unwrap_or("").to_string();
            let style = a("style");
            let despace = style.ends_with('.') || boolean(c.attr("despace").unwrap_or(""), false);
            map.entries.push(MapEntry {
                element: if c.attr("element").is_some() { a("element") } else { a("name") },
                class: a("class"),
                code_start: a("code_start"),
                code_end: a("code_end"),
                continue_class: a("continue"),
                note_style: a("notestyle"),
                append: a("append"),
                level: c.attr("level").and_then(|l| l.trim().parse().ok()).unwrap_or(0),
                despace,
                no_center: !boolean(c.attr("center").unwrap_or(""), true),
                aggregate: if boolean(c.attr("aggregate").unwrap_or(""), false) { 2 } else { 0 },
                style,
                ..MapEntry::default()
            });
        }
        map
    }

    /// The scrub rules the map leaves for the step after the tag processor (spec 28 6b): for an aggregated style,
    /// its end followed by its start becomes a line code; for a continued style the tmp: forms become paragraph and
    /// runover codes; and a [run] before [lea] is dropped.
    pub fn scrub_rules(&self) -> String {
        const O: char = '\x1c';
        const C: char = '\x1f';
        let mut out = String::new();
        for e in &self.entries {
            let s = &e.style;
            if !e.continue_class.is_empty() {
                // The XML form: aggregate gives the line code rule, continue the tmp: rules.
                let aggregate_rule = e.aggregate == 2;
                if aggregate_rule {
                    out.push_str(&format!("{O}ee|~{s}{C}{O}es|~{s}{C}~{O}l{C}~\n"));
                }
                continue_rules(&mut out, s, aggregate_rule);
            } else {
                match e.aggregate {
                    // The text form: the end followed by the start of the style becomes a line code,
                    // unless the block aggregates forward only, which gives the continue rules (the tmp: form).
                    1 | 2 => {
                        if !e.forward || e.backward {
                            out.push_str(&format!("{O}ee|~{s}{C}{O}es|~{s}{C}~{O}l{C}~\n"));
                        } else if e.aggregate == 2 {
                            // Forward-only aggregation (no shipped map sets it): the loader formats
                            // "[ee~S][es~S][hlN]" and writes it as one rule; read as the end and start becoming the
                            // level code (INFERENCE: the bracket text's split into match and replacement was not
                            // traced).
                            out.push_str(&format!("{O}ee|~{s}{C}{O}es|~{s}{C}~{O}hl{}{C}~\n", e.level));
                        } else {
                            out.push_str(&format!("{O}ee|~{s}{C}{O}es|~{s}{C}~{O}l{C}~\n"));
                        }
                    }
                    // Paragraphs (a User block's paragraph entry as read): the end followed by the start becomes a
                    // paragraph code.
                    3 => out.push_str(&format!("{O}ee|~{s}{C}{O}es|~{s}{C}~{O}p{C}~\n")),
                    _ => {}
                }
            }
            // Contents ignored (mode 2): the style's start enters the dropping state until its end.
            if e.mode == 2 {
                out.push_str(&format!("{O}es|~{s}{C}~{O}es|~{s}{C}~1~-1~\n"));
                out.push_str(&format!("{O}ee|~{s}{C}~{O}ee|~{s}{C}~-1~1~\n"));
            }
        }
        if self.xml_form {
            out.push_str(&format!("{O}run{C}{O}lea{C}~{O}lea{C}~\n"));
        }
        out
    }

    /// Whether an entry continues itself: the XML form's continue attribute (the text form never sets it, spec 28
    /// 6c; the entry constructor clears it and no key writes it).
    fn continues(e: &MapEntry) -> bool {
        !e.continue_class.is_empty()
    }
}

// ------------------------------------------------------------------ numbering

/// The numbering of the document (word/numbering.xml), for the list numbers the bc:num elements ask for (spec 28
/// 8a): each num's abstract definition, its levels' formats and texts, and the counters as the numbers are given.
pub struct Numbering {
    nums: Vec<(String, String)>,
    abstracts: Vec<(String, Vec<Level>)>,
    counters: Vec<(String, [i64; 9])>,
}

#[derive(Debug, Clone, Default)]
struct Level {
    ilvl: usize,
    start: i64,
    fmt: String,
    text: String,
}

impl Numbering {
    pub fn new(numbering: Option<&Element>) -> Numbering {
        let mut n = Numbering { nums: Vec::new(), abstracts: Vec::new(), counters: Vec::new() };
        let Some(root) = numbering else { return n };
        for a in root.elements("w:abstractNum") {
            let id = a.attr("w:abstractNumId").unwrap_or("").to_string();
            let levels = a
                .elements("w:lvl")
                .map(|l| Level {
                    ilvl: l.attr("w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0),
                    start: l.element("w:start").and_then(|s| s.attr("w:val")).and_then(|v| v.parse().ok()).unwrap_or(1),
                    fmt: l.element("w:numFmt").and_then(|s| s.attr("w:val")).unwrap_or("decimal").to_string(),
                    text: l.element("w:lvlText").and_then(|s| s.attr("w:val")).unwrap_or("").to_string(),
                })
                .collect();
            n.abstracts.push((id, levels));
        }
        for num in root.elements("w:num") {
            let id = num.attr("w:numId").unwrap_or("").to_string();
            let abs = num.element("w:abstractNumId").and_then(|a| a.attr("w:val")).unwrap_or("").to_string();
            n.nums.push((id, abs));
        }
        n
    }

    fn level(&self, num: &str, ilvl: usize) -> Option<&Level> {
        let abs = &self.nums.iter().find(|(id, _)| id == num)?.1;
        self.abstracts.iter().find(|(id, _)| id == abs)?.1.iter().find(|l| l.ilvl == ilvl)
    }

    /// The next number of list `num` at level `ilvl`, formatted; the original's debug text when the list is not
    /// defined. Deeper levels start again.
    pub fn format(&mut self, num: &str, ilvl: usize) -> String {
        if self.level(num, ilvl).is_none() {
            let abs = self.nums.iter().find(|(id, _)| id == num).map(|(_, a)| a.clone()).unwrap_or_default();
            return format!("###{{word value=0, id={num}, abstractId={abs}, lvl={ilvl}}}###");
        }
        let ilvl = ilvl.min(8);
        if !self.counters.iter().any(|(id, _)| id == num) {
            let mut starts = [0i64; 9];
            for (k, s) in starts.iter_mut().enumerate() {
                *s = self.level(num, k).map_or(1, |l| l.start) - 1;
            }
            self.counters.push((num.to_string(), starts));
        }
        let at = self.counters.iter().position(|(id, _)| id == num).unwrap_or(0);
        self.counters[at].1[ilvl] += 1;
        for k in ilvl + 1..9 {
            let start = self.level(num, k).map_or(1, |l| l.start);
            self.counters[at].1[k] = start - 1;
        }
        let values = self.counters[at].1;
        let level = self.level(num, ilvl).cloned().unwrap_or_default();
        let mut out = String::new();
        let text: Vec<char> = level.text.chars().collect();
        let mut k = 0;
        while k < text.len() {
            if text[k] == '%' && k + 1 < text.len() && text[k + 1].is_ascii_digit() && text[k + 1] != '0' {
                let n = text[k + 1].to_digit(10).unwrap_or(1) as usize;
                let fmt = self.level(num, n - 1).map_or("decimal".to_string(), |l| l.fmt.clone());
                out.push_str(&Self::number_text(values[(n - 1).min(8)], &fmt));
                k += 2;
            } else {
                out.push(text[k]);
                k += 1;
            }
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out
    }

    /// A number in a numbering format (spec 28 8a); the alphabet of the letter formats really ends "xzy".
    fn number_text(value: i64, fmt: &str) -> String {
        let letters = |upper: bool| {
            let alphabet: Vec<char> = "abcdefghijklmnopqrstuvwxzy".chars().collect();
            let v = value.max(1) as usize;
            let c = alphabet[(v - 1) % 26];
            let s: String = std::iter::repeat(c).take((v - 1) / 26 + 1).collect();
            if upper { s.to_uppercase() } else { s }
        };
        match fmt {
            "bullet" => "\u{2022}".to_string(),
            "decimalEnclosedCircle" | "decimalEnclosedParen" => format!("( {value} )"),
            "decimalEnclosedFullstop" => format!("{value}."),
            "lowerLetter" => letters(false),
            "upperLetter" => letters(true),
            "lowerRoman" => roman(value).to_lowercase(),
            "upperRoman" => roman(value),
            "none" => String::new(),
            _ => format!("{value}"),
        }
    }
}

fn roman(mut n: i64) -> String {
    if n <= 0 {
        return format!("{n}");
    }
    let table = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut out = String::new();
    for (v, s) in table {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    out
}

// ------------------------------------------------------------------ the tag processor

const O: char = '\x1c';
const C: char = '\x1f';

struct Open {
    name: String,
    entry: Option<MapEntry>,
    mode: u8,
    style: String,
}

/// The tag processor's state (spec 28 6c).
struct Processor<'a> {
    map: &'a StyleMap,
    pkg: &'a Package,
    numbering: Numbering,
    opt: &'a Options,
    out: String,
    stack: Vec<Open>,
    text_seen: bool,
    after_tag: bool,
    pending_newline: bool,
    pending_spaces: usize,
    note_mode: bool,
    scanned: bool,
    parity: i64,
    last_lng: String,
    last_font: String,
    level_written: bool,
    table_row: usize,
    table_col: usize,
}

impl<'a> Processor<'a> {
    fn flush_pending(&mut self) {
        if self.pending_newline {
            self.out.push('\n');
            self.pending_newline = false;
        }
        for _ in 0..self.pending_spaces {
            self.out.push(' ');
        }
        self.pending_spaces = 0;
    }

    fn write(&mut self, text: &str) {
        self.flush_pending();
        self.out.push_str(text);
    }

    fn code(&mut self, name: &str) {
        self.write(&format!("{O}{name}{C}"));
    }

    /// The style in force on the innermost open element that has one.
    fn open_style(&self) -> Option<&str> {
        self.stack.iter().rev().find(|o| !o.style.is_empty()).map(|o| o.style.as_str())
    }

    fn text(&mut self, text: &str) {
        for c in text.chars() {
            match c {
                '\r' | '\n' => {
                    if self.text_seen && !self.after_tag {
                        self.pending_newline = true;
                    }
                }
                ' ' => {
                    let exact = self.open_style().is_some_and(|s| s.eq_ignore_ascii_case("exacttranslation"));
                    let drop = !self.text_seen || (self.after_tag && self.stack.last().is_some_and(|o| o.entry.as_ref().is_some_and(|e| e.despace)));
                    if !drop || exact {
                        self.pending_spaces += 1;
                    }
                }
                c => {
                    self.flush_pending();
                    self.out.push(c);
                    self.text_seen = true;
                    self.after_tag = false;
                }
            }
        }
    }

    /// The first entry the element matches: the same element or any, the same class or any.
    fn find_entry(&self, name: &str, class: &str) -> Option<MapEntry> {
        self.map.entries.iter().find(|e| (e.element.is_empty() || e.element.eq_ignore_ascii_case(name)) && (e.class.is_empty() || e.class.eq_ignore_ascii_case(class))).cloned()
    }

    fn start(&mut self, e: &Element) {
        let name = e.name.as_str();
        self.after_tag = true;
        match name {
            "bc:checkbox" => {}
            "bc:note" => self.note_mode = !self.note_mode,
            "bc:num" => {
                let lvl: usize = e.attr("lvl").and_then(|l| l.parse().ok()).unwrap_or(0);
                if !self.level_written {
                    self.code(&format!("hl{}", lvl + 1));
                    self.level_written = true;
                }
                let id = e.attr("id").unwrap_or("").to_string();
                let text = self.numbering.format(&id, lvl);
                self.write(&text);
            }
            "bc:hardpage" => {
                let wanted = match e.attr("type") {
                    Some("evenPage") => Some(0),
                    Some("oddPage") => Some(1),
                    _ => None,
                };
                self.parity += 1;
                if let Some(w) = wanted {
                    if self.parity % 2 != w {
                        self.parity += 1;
                    }
                }
                self.code("pg");
                if self.stack.iter().any(|o| o.style.ends_with('.')) {
                    self.code("run");
                }
            }
            "bc:scannedpagebreak" => self.scanned = true,
            "bc:pagebreak" => {
                if self.scanned {
                    if e.children.is_empty() {
                        self.code("lea;f1");
                    } else {
                        self.code("lea");
                    }
                }
            }
            "bc:pageNumStart" | "bc:pageNumLRStart" | "bc:pageNumURStart" => {
                if self.scanned {
                    let digits: String = e.text().chars().filter(|c| c.is_ascii_digit()).collect();
                    let n: i64 = digits.parse().unwrap_or(0);
                    match name {
                        "bc:pageNumStart" => self.write(&format!("{O}lea;f0a{C}{n}")),
                        "bc:pageNumLRStart" => self.write(&format!("{O}lea;f0r{C}{}", roman(n).to_lowercase())),
                        _ => self.write(&format!("{O}lea;f0r{C}{}", roman(n))),
                    }
                }
            }
            "bc:rps_a" => self.code(&format!("rps:a~{}", e.attr("name").unwrap_or(""))),
            "bc:rpe_a" => self.code("rpe:a"),
            "bc:tab" => self.code(">"),
            "bc:ltab" => self.write(&format!("{O}fr;p~\".{C}")),
            "bc:newline" => self.write(&format!("{O}&lt;{C}")),
            "bc:DX_CHSPH" => self.write("\x1e"),
            "bc:xlsheetData" => {
                self.table_row = 1;
                self.table_col = 1;
            }
            "bc:mathtype" | "bc:equation" => {}
            n if n.starts_with("bcml:") => self.code(&n[6..]),
            _ => self.mapped_start(e),
        }
    }

    /// The elements the tag processor handles by name (spec 28 6c); every other element, the bc: ones included,
    /// goes through the style map and is copied as raw text when no entry matches (the recipe's later scripts turn
    /// those tags into codes).
    fn by_name(name: &str) -> bool {
        matches!(
            name,
            "bc:checkbox"
                | "bc:note"
                | "bc:num"
                | "bc:hardpage"
                | "bc:scannedpagebreak"
                | "bc:pagebreak"
                | "bc:pageNumStart"
                | "bc:pageNumLRStart"
                | "bc:pageNumURStart"
                | "bc:rps_a"
                | "bc:rpe_a"
                | "bc:tab"
                | "bc:ltab"
                | "bc:newline"
                | "bc:DX_CHSPH"
                | "bc:xlsheetData"
                | "bc:mathtype"
                | "bc:equation"
        ) || name.starts_with("bcml:")
    }

    /// A mapped element's start (spec 28 6c): the class is the Word style name; the entry's mode decides what is
    /// written.
    fn mapped_start(&mut self, e: &Element) {
        let name = e.name.clone();
        let mut class = e.attr("class").unwrap_or("").to_string();
        if class.is_empty() && name == "p" {
            class = "Normal".into();
        }
        if !class.is_empty() {
            class = self.pkg.style_name(&class);
        }
        let entry = self.find_entry(&name, &class);
        let Some(entry) = entry else {
            // No entry: the start tag is copied as text, as it was written (a self-closing tag whole, with no end
            // tag to follow).
            let mut raw = String::new();
            let mut shallow = e.clone();
            shallow.children.clear();
            serialize(&shallow, &mut raw);
            if !e.children.is_empty() {
                // The element has content: its start tag alone, not the self-closing form.
                raw.truncate(raw.len() - 2);
                raw.push('>');
            }
            self.write(&raw);
            if !e.children.is_empty() {
                self.stack.push(Open { name, entry: None, mode: 3, style: String::new() });
            }
            return;
        };
        let mut style = if entry.style == "%class%" { class.clone() } else { entry.style.clone() };
        if self.note_mode && !entry.note_style.is_empty() {
            style = entry.note_style.clone();
        }
        if StyleMap::continues(&entry) && (entry.continue_class.is_empty() || entry.continue_class.eq_ignore_ascii_case(&class)) {
            style = format!("tmp:{class}");
        }
        let jc_center = e.attr("jc") == Some("center");
        let mode = if entry.mode == 2 {
            2
        } else if style.ends_with('.') && jc_center && !entry.no_center {
            1
        } else {
            0
        };
        match mode {
            2 => {}
            1 => self.code("hds"),
            _ => {
                if !style.is_empty() {
                    self.code(&format!("es~{style}"));
                }
                if !entry.code_start.is_empty() {
                    let s = entry.code_start.clone();
                    self.write(&s);
                }
            }
        }
        if !entry.append.is_empty() {
            let a = entry.append.clone();
            self.write(&a);
        }
        if name == "p" {
            self.level_written = false;
        }
        let level: i64 = e.attr("level").and_then(|l| l.parse().ok()).unwrap_or(entry.level);
        if level >= 1 && !self.level_written && entry.aggregate != 1 {
            self.code(&format!("hl{level}"));
            self.level_written = true;
        }
        if name == "text" {
            let lng = e.attr("lng").map(|l| l.split('-').next().unwrap_or("").to_string()).unwrap_or_default();
            if lng != self.last_lng {
                if lng.is_empty() {
                    self.code("lng");
                } else {
                    self.code(&format!("lng~{lng}"));
                }
                self.last_lng = lng;
            }
            let font = e.attr("font").unwrap_or("").to_string();
            if font != self.last_font {
                if font.is_empty() {
                    self.code("font");
                } else {
                    self.code(&format!("font~{font}"));
                }
                self.last_font = font;
            }
        }
        self.stack.push(Open { name, entry: Some(entry), mode, style });
    }

    fn end(&mut self, e: &Element) {
        let name = e.name.as_str();
        self.after_tag = true;
        match name {
            "bc:note" => self.note_mode = !self.note_mode,
            "bc:pagebreak" => {
                if self.scanned {
                    self.code("run");
                    self.scanned = self.opt.print_pages == PrintPages::Scanned;
                }
            }
            "bc:pageNumLRStart" | "bc:pageNumURStart" => {
                if self.scanned {
                    self.code("run");
                }
            }
            n if Self::by_name(n) => {}
            _ => {
                if e.children.is_empty() && self.stack.last().is_none_or(|o| o.name != name) {
                    // A self-closing tag with no entry was copied whole and opened nothing.
                    return;
                }
                let Some(open) = self.stack.pop() else { return };
                match open.mode {
                    3 => self.write(&format!("</{}>", open.name)),
                    2 => {}
                    1 => self.code("hde"),
                    _ => {
                        if let Some(entry) = &open.entry {
                            if !entry.code_end.is_empty() {
                                let s = entry.code_end.clone();
                                self.write(&s);
                            }
                        }
                        if !open.style.is_empty() {
                            self.code(&format!("ee~{}", open.style));
                        }
                    }
                }
            }
        }
    }

    fn element(&mut self, e: &Element) {
        let self_closing = e.children.is_empty();
        self.start(e);
        if e.name == "bc:pagebreak" && self.scanned && !self_closing {
            // The content is the print page number, as text.
            self.text(&e.text());
        } else if e.name.starts_with("bc:pageNum") || e.name == "bc:checkbox" || e.name == "bc:mathtype" || e.name == "bc:equation" {
            // Handled whole at the start.
        } else {
            for c in &e.children {
                match c {
                    Node::Text(t) => self.text(t),
                    Node::Element(child) => self.element(child),
                    Node::Pi(text) => {
                        // The code instruction: $ and ` are the code marks, &quot; a quote.
                        let t = text.replace('$', "\x1c").replace('`', "\x1f").replace("&quot;", "\"");
                        self.write(&t);
                        self.after_tag = true;
                    }
                }
            }
        }
        if !self_closing || !Self::by_name(&e.name) {
            self.end(e);
        }
    }
}

// ------------------------------------------------------------------ the import

/// What the Word import gives before the recipe's rule scripts: the text with codes (UTF-8, codes as 0x1C name
/// 0x1F) and the scrub rules of the style map, which the step after the tag processor applies.
pub struct Processed {
    pub text: String,
    pub scrub_rules: String,
}

/// The stylesheet's output for a package, the page-break fix and the run combining applied, as the tree the tag
/// processor reads.
pub fn transform(pkg: &Package, opt: &Options) -> Element {
    let body = pkg.document.element("w:body").cloned().unwrap_or_default();
    let t = Transform { pkg, opt, leader: std::cell::Cell::new(false) };
    let mut doc = t.body(&body);
    fix_page_breaks(&mut doc);
    combine_runs(&mut doc, pkg, opt);
    doc
}

/// The whole import up to the scrub rules: the package's document through the stylesheet's rules, the run
/// combining, the style map (`map_text`, the text form of a shipped map, unless the document carries its own XML
/// map in docProps/custom.xml) and the tag processor.
pub fn import(bytes: &[u8], map_text: &str, opt: &Options) -> Result<Processed, String> {
    let pkg = Package::open(bytes)?;
    let own_map = docx::xml_part(&pkg.parts, "docProps/custom.xml")?.and_then(|custom| {
        custom.elements("property").find_map(|p| {
            let value = p.element("vt:lpwstr")?.text();
            let trimmed = value.trim();
            trimmed.starts_with('<').then(|| docx::read_xml(trimmed).ok()).flatten().map(|root| StyleMap::from_xml(&root))
        })
    });
    let map = match own_map {
        Some(m) => m,
        None => StyleMap::from_text(map_text)?,
    };
    let mut tree = transform(&pkg, opt);
    strip_controls(&mut tree);
    let numbering = Numbering::new(pkg.numbering.as_ref());
    let mut p = Processor {
        map: &map,
        pkg: &pkg,
        numbering,
        opt,
        out: String::new(),
        stack: Vec::new(),
        text_seen: false,
        after_tag: false,
        pending_newline: false,
        pending_spaces: 0,
        note_mode: false,
        scanned: opt.print_pages == PrintPages::Scanned,
        parity: 1,
        last_lng: String::new(),
        last_font: String::new(),
        level_written: false,
        table_row: 1,
        table_col: 1,
    };
    for c in &tree.children {
        match c {
            Node::Text(t) => p.text(t),
            Node::Element(e) => p.element(e),
            Node::Pi(text) => {
                let t = text.replace('$', "\x1c").replace('`', "\x1f").replace("&quot;", "\"");
                p.write(&t);
            }
        }
    }
    Ok(Processed { text: p.out, scrub_rules: map.scrub_rules() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLES: &str = r#"<w:styles><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>"#;

    fn docx(document: &str) -> Vec<u8> {
        docx::stored_zip(&[("word/document.xml", document.as_bytes()), ("word/styles.xml", STYLES.as_bytes())])
    }

    const MAP: &str = "BuiltIn \"Normal\"\n{\n  MappedName = \"para.\"\n}\nBuiltIn \"heading 1\"\n{\n  MappedName = \"h1.\"\n  IgnoreCentering = false\n}\n";

    #[test]
    fn paragraphs_take_their_mapped_styles_and_runs_their_codes() {
        let doc = r#"<w:document><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">Plain </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t> end</w:t></w:r></w:p></w:body></w:document>"#;
        let out = import(&docx(doc), MAP, &Options::default()).unwrap();
        // The heading's style carries bold, so its run is bold; the second paragraph's bold run is marked.
        assert_eq!(out.text, "\x1ces~h1.\x1f\x1cfts~b\x1fTitle\x1cfte~b\x1f\x1cee~h1.\x1f\x1ces~para.\x1fPlain \x1cfts~b\x1fbold\x1cfte~b\x1f end\x1cee~para.\x1f");
        // The text form's rules end with the last block's any-element line rule (its mapped name was written with
        // the dot, so both entries carry it); the run-lea rule is the XML form's.
        assert!(out.scrub_rules.ends_with("\x1cee|~h1.\x1f\x1ces|~h1.\x1f~\x1cl\x1f~\n"), "{:?}", out.scrub_rules);
    }

    #[test]
    fn unmapped_tags_pass_as_text_and_tables_and_breaks_give_their_codes() {
        let doc = r#"<w:document><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>x</w:t><w:br/><w:t>y</w:t></w:r></w:p></w:body></w:document>"#;
        let out = import(&docx(doc), MAP, &Options::default()).unwrap();
        // Table codes and the line element are not in the processor's own table, so they pass as raw tags for the
        // recipe's scripts; the cell paragraphs take the paragraph style.
        assert_eq!(
            out.text,
            "<bc:htbs>r:0:0:b:n:a</bc:htbs>\x1ces~para.\x1fa\x1cee~para.\x1f\x1c>\x1f\x1ces~para.\x1fb\x1cee~para.\x1f\x1c&lt;\x1f\x1ces~para.\x1fc\x1cee~para.\x1f<bc:htbe/>\x1ces~para.\x1fx<bc:l/>y\x1cee~para.\x1f"
        );
    }

    #[test]
    fn numbering_formats_as_the_original() {
        let numbering = docx::read_xml(r#"<w:numbering><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1.%2"/></w:lvl></w:abstractNum><w:num w:numId="5"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#).unwrap();
        let mut n = Numbering::new(Some(&numbering));
        assert_eq!(n.format("5", 0), "1. ");
        assert_eq!(n.format("5", 1), "1.a ");
        assert_eq!(n.format("5", 1), "1.b ");
        assert_eq!(n.format("5", 0), "2. ");
        assert_eq!(Numbering::number_text(26, "lowerLetter"), "y");
        assert_eq!(Numbering::number_text(4, "upperRoman"), "IV");
        assert!(n.format("9", 0).starts_with("###{word value=0, id=9"));
    }

    #[test]
    fn the_text_form_style_map_reads_as_the_original() {
        let m = StyleMap::from_text("# a comment\nUser \"Quote\"\n{\n  MappedName = \"quote\"\n  AggregateForward = Always\n  Level = 2\n  IgnoreContents = Always\n}\nBuiltIn \"heading 1\"\n{\n  MappedName = \"h1.\"\n}\n").unwrap();
        // The alt text defaults come first; a User block's paragraph entry keeps its kind 3 as its aggregation.
        assert_eq!((m.entries[0].element.as_str(), m.entries[0].style.as_str(), m.entries[1].style.as_str()), ("bc:alttext", "note.", "note"));
        let p = &m.entries[2];
        assert_eq!((p.element.as_str(), p.class.as_str(), p.style.as_str(), p.level, p.mode), ("p", "Quote", "quote.", 2, 2));
        assert!(p.forward && !p.backward && p.aggregate == 3);
        // The any-element entry keeps the name without the dot and the ignored contents.
        assert_eq!((m.entries[3].element.as_str(), m.entries[3].style.as_str(), m.entries[3].mode, m.entries[3].aggregate), ("", "quote", 2, 4));
        let rules = m.scrub_rules();
        assert!(rules.contains("\x1cee|~quote.\x1f\x1ces|~quote.\x1f~\x1cp\x1f~\n"), "{rules:?}");
        assert!(rules.contains("\x1ces|~quote\x1f~\x1ces|~quote\x1f~1~-1~\n"), "{rules:?}");
        // A BuiltIn block writes the line code rule for both of its entries (the same text when the mapped name was
        // written with its dot).
        assert_eq!(rules.matches("\x1cee|~h1.\x1f\x1ces|~h1.\x1f~\x1cl\x1f~\n").count(), 2, "{rules:?}");
    }
}
