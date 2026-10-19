//! The document templates shipped with BrailleCraft (templates/*.template and templates/index.tsv): a template's
//! settings are "Key: Value" lines (TranslationTables names the four tables), followed by "Styles:" and the styles.
//! File, New lists the templates by name; a new document takes its template's tables and styles.

use std::path::{Path, PathBuf};

use crate::engine::StyleTable;

/// One template of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    /// The short code of the template a document file names (small letters).
    pub code: String,
    /// The template file in the folder, without ".template".
    pub file: String,
    /// The name File, New shows.
    pub name: String,
}

/// The template File, New selects when no default has been chosen.
pub const BUILT_IN_DEFAULT: &str = "English (UEB) - basic";

pub struct Templates {
    folder: PathBuf,
    pub entries: Vec<TemplateEntry>,
}

impl Templates {
    /// Reads `folder`/index.tsv: lines of code, file and name, separated by tabs; "#" starts a comment line.
    pub fn open(folder: impl AsRef<Path>) -> Result<Templates, String> {
        let folder = folder.as_ref().to_path_buf();
        let path = folder.join("index.tsv");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 3 {
                return Err(format!("{}: {line}", path.display()));
            }
            entries.push(TemplateEntry { code: f[0].to_lowercase(), file: f[1].to_string(), name: f[2].to_string() });
        }
        Ok(Templates { folder, entries })
    }

    /// The template of that name (as File, New shows it), whatever the case of its letters.
    pub fn by_name(&self, name: &str) -> Option<&TemplateEntry> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// The template file's text.
    pub fn text(&self, entry: &TemplateEntry) -> Result<String, String> {
        let path = self.folder.join(format!("{}.template", entry.file));
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The names in alphabetical order, as File, New lists them.
    pub fn names_sorted(&self) -> Vec<String> {
        let mut names: Vec<String> = self.entries.iter().map(|e| e.name.clone()).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }
}

/// The value of a "Key: Value" setting of a template's text, before its styles.
pub fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim() == "Styles:" {
            break;
        }
        if let Some(rest) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) {
            return Some(rest.trim());
        }
    }
    None
}

/// A template's Hyphenation as a document file keeps it: 0 None, 1 Dash, 2 DashAndHyphen, 3 French, 4 French1103.
/// The value is not set before the header is read, so a template without the key would give the document an unset
/// field (spec 05 section A10); every shipped template has the key. BrailleCraft
/// gives such a template 2, DashAndHyphen, the value 214 of its 226 templates hold.
pub fn hyphenation(text: &str) -> u16 {
    match setting(text, "Hyphenation").map(str::trim) {
        Some("None") => 0,
        Some("Dash") => 1,
        Some("French") => 3,
        Some("French1103") => 4,
        _ => 2,
    }
}

/// The four table codes of a template's TranslationTables line ("A+B/C+D"): print-to-braille rules and scan table,
/// braille-to-print rules and scan table, in small letters.
pub fn tables(text: &str) -> Option<[String; 4]> {
    let v = setting(text, "TranslationTables")?;
    let (p2b, b2p) = v.split_once('/')?;
    let (a, b) = p2b.split_once('+')?;
    let (c, d) = b2p.split_once('+')?;
    Some([a, b, c, d].map(|s| s.trim().to_lowercase()))
}

/// A template's page numbering, in the order a document file keeps it: braille page numbers on even pages and on
/// odd pages, print page numbers on even pages and on odd pages (0 none, 1 upper left, 2 upper right, 3 lower left,
/// 4 lower right), then the first page whose number is shown. Nothing sets these fields before the header is
/// read, so a template that left a key out would leave its field unset (FirstPageNumberDisplayed: the document's
/// value as it was); every shipped template has all five keys. BrailleCraft gives such a template the defaults
/// (braille numbers lower right, print numbers upper right, spec 19; numbers from page 1). Not known: what a
/// value outside the list does; the default is taken. The Contents* keys go straight into the document's Table
/// of Contents settings; BrailleCraft does not read them yet (there is no Generate Table of Contents).
pub fn page_numbers(text: &str) -> [u32; 5] {
    let corner = |key: &str, default: u32| match setting(text, key) {
        Some("None") => 0,
        Some("UpperLeft") => 1,
        Some("UpperRight") => 2,
        Some("LowerLeft") => 3,
        Some("LowerRight") => 4,
        _ => default,
    };
    [
        corner("BraillePageNumbersEvenPages", 4),
        corner("BraillePageNumbersOddPages", 4),
        corner("PrintPageNumbersEvenPages", 2),
        corner("PrintPageNumbersOddPages", 2),
        setting(text, "FirstPageNumberDisplayed").and_then(|v| v.parse().ok()).unwrap_or(1),
    ]
}

/// A template's styles.
pub fn styles(text: &str) -> StyleTable {
    StyleTable::from_template_text(text)
}

// ------------------------------------------------------------------ the style dump and the text template writer

/// The first line of a text template BrailleCraft writes, after the "| name" line.
pub const TEMPLATE_SIGNATURE: &str = "-braillecraft-template 1";
/// The element around the XML style dump.
pub const STYLE_DUMP_ELEMENT: &str = "braillecraftstyledump";

/// One stored style string as the dumps write it: each byte of its internal text as it is, apart from the code marks
/// 0x1C and 0x1F ("[" and "]", or "&#001C;" and "&#001F;" in XML), and in XML '"', '&', '<' and '>' as entities.
/// With `escape`, a '[', ']' or '\' that is text gets a backslash before it, so it is not read as a code mark.
fn dump_bytes(out: &mut Vec<u8>, running: &str, xml: bool, escape: bool) {
    for b in crate::internal::encode_lossy(running, b'?') {
        let put: &[u8] = match b {
            0x1C if xml => b"&#001C;",
            0x1C => b"[",
            0x1F if xml => b"&#001F;",
            0x1F => b"]",
            b'"' if xml => b"&quot;",
            b'&' if xml => b"&amp;",
            b'<' if xml => b"&lt;",
            b'>' if xml => b"&gt;",
            b'[' if escape => b"\\[",
            b'\\' if escape => b"\\\\",
            b']' if escape => b"\\]",
            _ => {
                out.push(b);
                continue;
            }
        };
        out.extend_from_slice(put);
    }
}

/// The style dump of /dumpstyles (plain) and /dumptemplate (`xml`), and the styles part of a text template (plain,
/// `escape`): the styles in the order given (the document's style-name list), each with its beginning and ending
/// codes. Plain: name, then two spaces and the beginning codes, then two spaces and the ending codes, each on a line.
/// XML: a style element per style with startcodes and endcodes elements. Line ends are "\n"; the bytes are the
/// document's own 8-bit text (internal text), not converted.
pub fn style_dump(styles: &[(String, String, String)], xml: bool, escape: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if xml {
        out.extend_from_slice(format!("<{STYLE_DUMP_ELEMENT}>\n").as_bytes());
    }
    for (name, begin, end) in styles {
        if xml {
            out.extend_from_slice(b"  <style name=\"");
            dump_bytes(&mut out, name, true, escape);
            out.extend_from_slice(b"\">\n    <startcodes>");
            dump_bytes(&mut out, begin, true, escape);
            out.extend_from_slice(b"</startcodes>\n    <endcodes>");
            dump_bytes(&mut out, end, true, escape);
            out.extend_from_slice(b"</endcodes>\n  </style>\n");
        } else {
            dump_bytes(&mut out, name, false, escape);
            out.extend_from_slice(b"\n  ");
            dump_bytes(&mut out, begin, false, escape);
            out.extend_from_slice(b"\n  ");
            dump_bytes(&mut out, end, false, escape);
            out.push(b'\n');
        }
    }
    if xml {
        out.extend_from_slice(format!("</{STYLE_DUMP_ELEMENT}>\n").as_bytes());
    }
    out
}

const CORNERS: &str = "None,UpperLeft,UpperRight,LowerLeft,LowerRight";
const HYPHENATION: &str = "None,Dash,DashAndHyphen,French,French1103";
const FALSE_TRUE: &str = "False,True";
const GUIDE_DOTS: &str = "Dot_3,Dot_5,Dot_6,Dots_36";
const PAGE_NUM_STYLE: &str = "Braille,Print,Both";

/// A text template's header: the document's settings as the "Key: Value" lines name them. A choice holds the
/// number of its item; a number past the end of the list or below 0 is written as the first item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxtHeader {
    /// The four table names (written in capitals, "A+B/C+D").
    pub tables: [String; 4],
    /// Braille page numbers on even and odd pages, print page numbers on even and odd pages (choices), the first
    /// page number shown.
    pub page_numbering: [i64; 5],
    pub hyphenation: i64,
    pub contents_style: String,
    pub suspend_ref_page_nums: i64,
    pub remove_internal_newlines: i64,
    pub center_top_level: i64,
    pub number_top_level: i64,
    pub runover_one_level: i64,
    pub runover_multi_level: i64,
    pub initial_indent_one_level: i64,
    pub initial_indent_multi_level: i64,
    pub incremental_indent: i64,
    pub right_margin_one_level: i64,
    pub right_margin_multi_level: i64,
    /// The guide dot setting as the document holds it: a braille character (0x27, 0x22, 0x2C or 0x2D), which is
    /// past the end of the choice list, so it is always written "Dot_3".
    pub guide_dot: i64,
    pub add_max_indent: i64,
    pub page_num_style: i64,
    pub lowest_level: i64,
}

/// A value of a choice list ("a,b,c"): item `value`, or the first item when `value` is below 0 or past the end.
fn choice(list: &str, value: i64) -> &str {
    let items: Vec<&str> = list.split(',').collect();
    usize::try_from(value).ok().and_then(|i| items.get(i).copied()).unwrap_or(items[0])
}

/// The number of `text` in a choice list; None when it is not there.
fn choice_number(list: &str, text: &str) -> Option<i64> {
    list.split(',').position(|i| i == text).map(|i| i as i64)
}

impl DxtHeader {
    /// The 24 header lines in their order, as key and value text ("Styles" is not among them).
    pub fn entries(&self) -> Vec<(&'static str, String)> {
        let n = |v: i64| v.to_string();
        let t = &self.tables;
        vec![
            ("TranslationTables", format!("{}+{}/{}+{}", t[0], t[1], t[2], t[3]).to_ascii_uppercase()),
            ("BraillePageNumbersEvenPages", choice(CORNERS, self.page_numbering[0]).into()),
            ("BraillePageNumbersOddPages", choice(CORNERS, self.page_numbering[1]).into()),
            ("PrintPageNumbersEvenPages", choice(CORNERS, self.page_numbering[2]).into()),
            ("PrintPageNumbersOddPages", choice(CORNERS, self.page_numbering[3]).into()),
            ("FirstPageNumberDisplayed", n(self.page_numbering[4])),
            ("Hyphenation", choice(HYPHENATION, self.hyphenation).into()),
            ("ContentsStyle", self.contents_style.clone()),
            ("ContentsSuspendRefPageNums", choice(FALSE_TRUE, self.suspend_ref_page_nums).into()),
            ("ContentsRemoveInternalNewlines", choice(FALSE_TRUE, self.remove_internal_newlines).into()),
            ("ContentsCenterTopLevel", choice(FALSE_TRUE, self.center_top_level).into()),
            ("ContentsNumberTopLevel", choice(FALSE_TRUE, self.number_top_level).into()),
            ("ContentsRunoverIfOneLevel", n(self.runover_one_level)),
            ("ContentsRunoverIfMultiLevel", n(self.runover_multi_level)),
            ("ContentsInitialIndentIfOneLevel", n(self.initial_indent_one_level)),
            ("ContentsInitialIndentIfMultiLevel", n(self.initial_indent_multi_level)),
            ("ContentsIncrementalIndent", n(self.incremental_indent)),
            ("ContentsIncrementalRightMarginIfOneLevel", n(self.right_margin_one_level)),
            ("ContentsIncrementalRightMarginIfMultiLevel", n(self.right_margin_multi_level)),
            ("ContentsGuideDotBraille", choice(GUIDE_DOTS, self.guide_dot).into()),
            ("ContentsAddMaxIndentToRunover", choice(FALSE_TRUE, self.add_max_indent).into()),
            ("ContentsPageNumStyle", choice(PAGE_NUM_STYLE, self.page_num_style).into()),
            ("ContentsLowestLevelIncluded", n(self.lowest_level)),
        ]
    }

    /// The header of a text template's lines ("Key: Value"), up to "Styles:". A key that is missing (six shipped
    /// templates have no ContentsSuspendRefPageNums or ContentsRemoveInternalNewlines) keeps the value a new
    /// document has.
    pub fn from_text(text: &str) -> DxtHeader {
        let mut h = header_from_settings(&crate::dxfile::new_settings(), Default::default(), [4, 4, 2, 2, 1], 2, "");
        let get = |key: &str| setting(text, key);
        let num = |key: &str, d: i64| get(key).and_then(|v| v.parse().ok()).unwrap_or(d);
        let pick = |key: &str, list: &str, d: i64| get(key).and_then(|v| choice_number(list, v)).unwrap_or(d);
        if let Some(v) = get("TranslationTables") {
            let parts: Vec<&str> = v.split(['+', '/']).collect();
            if parts.len() == 4 {
                h.tables = std::array::from_fn(|i| parts[i].trim().to_string());
            }
        }
        let keys = ["BraillePageNumbersEvenPages", "BraillePageNumbersOddPages", "PrintPageNumbersEvenPages", "PrintPageNumbersOddPages"];
        for (i, k) in keys.iter().enumerate() {
            h.page_numbering[i] = pick(k, CORNERS, h.page_numbering[i]);
        }
        h.page_numbering[4] = num("FirstPageNumberDisplayed", h.page_numbering[4]);
        h.hyphenation = pick("Hyphenation", HYPHENATION, h.hyphenation);
        h.contents_style = get("ContentsStyle").unwrap_or("").to_string();
        h.suspend_ref_page_nums = pick("ContentsSuspendRefPageNums", FALSE_TRUE, h.suspend_ref_page_nums);
        h.remove_internal_newlines = pick("ContentsRemoveInternalNewlines", FALSE_TRUE, h.remove_internal_newlines);
        h.center_top_level = pick("ContentsCenterTopLevel", FALSE_TRUE, h.center_top_level);
        h.number_top_level = pick("ContentsNumberTopLevel", FALSE_TRUE, h.number_top_level);
        h.runover_one_level = num("ContentsRunoverIfOneLevel", h.runover_one_level);
        h.runover_multi_level = num("ContentsRunoverIfMultiLevel", h.runover_multi_level);
        h.initial_indent_one_level = num("ContentsInitialIndentIfOneLevel", h.initial_indent_one_level);
        h.initial_indent_multi_level = num("ContentsInitialIndentIfMultiLevel", h.initial_indent_multi_level);
        h.incremental_indent = num("ContentsIncrementalIndent", h.incremental_indent);
        h.right_margin_one_level = num("ContentsIncrementalRightMarginIfOneLevel", h.right_margin_one_level);
        h.right_margin_multi_level = num("ContentsIncrementalRightMarginIfMultiLevel", h.right_margin_multi_level);
        h.guide_dot = pick("ContentsGuideDotBraille", GUIDE_DOTS, h.guide_dot);
        h.add_max_indent = pick("ContentsAddMaxIndentToRunover", FALSE_TRUE, h.add_max_indent);
        h.page_num_style = pick("ContentsPageNumStyle", PAGE_NUM_STYLE, h.page_num_style);
        h.lowest_level = num("ContentsLowestLevelIncluded", h.lowest_level);
        h
    }
}

/// A document's text template header: its tables, page numbering and hyphenation, and its Table of Contents
/// settings (settings +64 to +87, spec 05-B). `contents_style` is the name of the document's contents style, which
/// BrailleCraft's document does not keep (the caller gives it, from the document's template).
pub fn dxt_header(doc: &crate::dxfile::DxDocument, contents_style: &str) -> DxtHeader {
    let s = if doc.settings.len() == 224 { doc.settings.clone() } else { crate::dxfile::new_settings() };
    header_from_settings(&s, doc.tables.clone(), doc.page_numbering, doc.hyphenation, contents_style)
}

/// The header from a document file's settings block and the fields a document keeps apart from it.
fn header_from_settings(s: &[u8], tables: [String; 4], page_numbering: [u32; 5], hyphenation: u16, contents_style: &str) -> DxtHeader {
    let w = |at: usize| i64::from(u16::from_be_bytes([s[at], s[at + 1]]));
    DxtHeader {
        tables,
        page_numbering: page_numbering.map(i64::from),
        hyphenation: i64::from(hyphenation),
        contents_style: contents_style.to_string(),
        suspend_ref_page_nums: i64::from(s[66] & 2 != 0),
        remove_internal_newlines: i64::from(s[66] & 4 != 0),
        center_top_level: i64::from(s[66] & 1 != 0),
        number_top_level: i64::from(s[67] != 0),
        runover_one_level: w(68),
        runover_multi_level: w(70),
        initial_indent_one_level: w(72),
        initial_indent_multi_level: w(74),
        incremental_indent: w(76),
        right_margin_one_level: w(78),
        right_margin_multi_level: w(80),
        guide_dot: i64::from(s[82]),
        add_max_indent: i64::from(s[83]),
        page_num_style: w(84),
        lowest_level: w(86),
    }
}

/// A text template: "| " and the file name, the signature line, the 24 header lines, "Styles: " (with its space),
/// then the plain style dump with text brackets and backslashes escaped. No copyright lines. Line ends are "\n".
pub fn write_dxt(header: &DxtHeader, styles: &[(String, String, String)], file_name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"| ");
    out.extend_from_slice(&crate::internal::encode_lossy(file_name, b'?'));
    out.push(b'\n');
    out.extend_from_slice(TEMPLATE_SIGNATURE.as_bytes());
    out.push(b'\n');
    for (key, value) in header.entries() {
        out.extend_from_slice(key.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(&crate::internal::encode_lossy(&value, b'?'));
        out.push(b'\n');
    }
    out.extend_from_slice(b"Styles: \n");
    out.extend_from_slice(&style_dump(styles, false, true));
    out
}

/// A text template's styles (the lines after "Styles:"): a name line, then the beginning and ending codes each on a
/// line after two spaces; "[" and "]" mark codes and a backslash makes the next character text. Running text, codes
/// marked 0x1C ... 0x1F.
pub fn dxt_styles(text: &[u8]) -> Vec<(String, String, String)> {
    let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).collect();
    let Some(start) = lines.iter().position(|l| l.trim_ascii() == b"Styles:") else { return Vec::new() };
    let codes = |l: &[u8]| {
        let l = l.strip_prefix(b"  ").unwrap_or(l);
        let mut out = Vec::with_capacity(l.len());
        let mut i = 0;
        while i < l.len() {
            match l[i] {
                b'\\' if i + 1 < l.len() => {
                    out.push(l[i + 1]);
                    i += 1;
                }
                b'[' => out.push(0x1C),
                b']' => out.push(0x1F),
                b => out.push(b),
            }
            i += 1;
        }
        crate::internal::decode(&out)
    };
    lines[start + 1..]
        .chunks(3)
        .filter(|c| c.len() == 3)
        .map(|c| (codes(c[0]), codes(c[1]), codes(c[2])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Templates {
        Templates::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates")).unwrap()
    }

    #[test]
    fn the_shipped_templates_have_tables_and_styles() {
        let t = shipped();
        assert_eq!(t.entries.len(), 226);
        let english = t.by_name(BUILT_IN_DEFAULT).unwrap();
        assert_eq!(english.code, "enuncp");
        let text = t.text(english).unwrap();
        assert_eq!(tables(&text).unwrap(), ["enuncp", "enudxp", "enuncb", "gendxb"].map(String::from));
        let s = styles(&text);
        assert_eq!(s.lookup(b"biblical-langs", 0), Some(b"\x1clnb~qbi\x1f\x1cg1l\x1f".to_vec()));
        for e in &t.entries {
            assert!(tables(&t.text(e).unwrap()).is_some(), "{}", e.name);
        }
        let names = t.names_sorted();
        assert!(names.windows(2).all(|w| w[0].to_lowercase() <= w[1].to_lowercase()));
    }

    fn st(name: &str, begin: &str, end: &str) -> (String, String, String) {
        (name.into(), begin.into(), end.into())
    }

    #[test]
    fn style_dump_plain() {
        let styles = [st("acronym", "\x1ci\x1f", ""), st("act", "\x1csc1:0\x1f", "\x1csc1:0\x1f")];
        assert_eq!(style_dump(&styles, false, false), b"acronym\n  [i]\n  \nact\n  [sc1:0]\n  [sc1:0]\n");
        // With escape, brackets and backslashes that are text get a backslash.
        assert_eq!(style_dump(&[st("TOC:left", "Chapter", ",*apt]")], false, true), b"TOC:left\n  Chapter\n  ,*apt\\]\n");
    }

    #[test]
    fn style_dump_xml_escapes() {
        let out = String::from_utf8(style_dump(&[st("a&b", "\x1cfr\x1f<x>", "\"q\"")], true, false)).unwrap();
        assert_eq!(
            out,
            "<braillecraftstyledump>\n  <style name=\"a&amp;b\">\n    <startcodes>&#001C;fr&#001F;&lt;x&gt;</startcodes>\n    <endcodes>&quot;q&quot;</endcodes>\n  </style>\n</braillecraftstyledump>\n"
        );
    }

    fn new_doc() -> crate::dxfile::DxDocument {
        crate::dxfile::DxDocument {
            kind: crate::DocumentKind::Print,
            version: 1,
            page_numbering: [4, 4, 2, 2, 1],
            translated_line: false,
            six_key: false,
            tables: Default::default(),
            text: String::new(),
            styles: Vec::new(),
            style_index: Vec::new(),
            coded_view: false,
            hyphenation: 2,
            view_font: 1201,
            settings: crate::dxfile::new_settings(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
            print_page: (0, 0),
            caret: 0,
            ebraille: None,
        }
    }

    fn header() -> DxtHeader {
        let doc = crate::dxfile::DxDocument {
            tables: ["eoaacp", "EOAACP", "eoaacb", "gendxb"].map(String::from),
            page_numbering: [4, 4, 2, 2, 2],
            hyphenation: 2,
            settings: crate::dxfile::new_settings(),
            ..new_doc()
        };
        dxt_header(&doc, "BANA Textbook")
    }

    #[test]
    fn write_dxt_header_order() {
        let out = String::from_utf8(write_dxt(&header(), &[], "Esperanto - basic.dxt")).unwrap();
        let lines: Vec<&str> = out.split('\n').collect();
        assert_eq!(lines[0], "| Esperanto - basic.dxt");
        assert_eq!(lines[1], TEMPLATE_SIGNATURE);
        // 23 "Key: Value" lines, then "Styles: " with its space: the 24 entries of the header.
        let keys: Vec<&str> = lines[2..25].iter().map(|l| l.split(':').next().unwrap()).collect();
        assert_eq!(keys, header().entries().iter().map(|e| e.0).collect::<Vec<_>>());
        assert_eq!(keys.len(), 23);
        assert_eq!(keys[0], "TranslationTables");
        assert_eq!(keys[19], "ContentsGuideDotBraille");
        assert_eq!(keys[20], "ContentsAddMaxIndentToRunover");
        assert_eq!(lines[24], "ContentsLowestLevelIncluded: 99");
        assert_eq!(lines[25], "Styles: ");
    }

    #[test]
    fn write_dxt_choice_out_of_range_is_first_item() {
        // A new document holds '"' (34, Dot 5 as the Table of Contents dialog keeps it): past the end of the list.
        let h = header();
        assert_eq!(h.guide_dot, 34);
        let out = String::from_utf8(write_dxt(&h, &[], "x.dxt")).unwrap();
        assert!(out.contains("\nContentsGuideDotBraille: Dot_3\n"));
        assert!(out.contains("\nHyphenation: DashAndHyphen\n"));
        assert_eq!(choice(CORNERS, -1), "None");
        assert_eq!(choice(CORNERS, 4), "LowerRight");
        assert_eq!(choice(CORNERS, 5), "None");
        // The dialog's other three characters are past the end too.
        for c in [0x27, 0x2c, 0x2d] {
            assert_eq!(choice(GUIDE_DOTS, c), "Dot_3");
        }
    }

    #[test]
    fn write_dxt_tables_upper_case() {
        let out = String::from_utf8(write_dxt(&header(), &[], "x.dxt")).unwrap();
        assert!(out.contains("\nTranslationTables: EOAACP+EOAACP/EOAACB+GENDXB\n"));
    }

    /// The text template's lines from "TranslationTables" on.
    fn from_tables_on(bytes: &[u8]) -> &[u8] {
        let at = bytes.windows(17).position(|w| w == b"TranslationTables").unwrap();
        &bytes[at..]
    }

    fn rewritten(bytes: &[u8]) -> Vec<u8> {
        let text = String::from_utf8_lossy(bytes);
        write_dxt(&DxtHeader::from_text(&text), &dxt_styles(bytes), "Esperanto - basic.dxt")
    }

    #[test]
    fn write_dxt_matches_shipped_template() {
        // The shipped Esperanto template, read and written again, gives the same lines from its header on.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/esperanto-basic.template");
        let bytes = std::fs::read(&path).unwrap();
        let out = rewritten(&bytes);
        assert_eq!(String::from_utf8_lossy(from_tables_on(&out)), String::from_utf8_lossy(from_tables_on(&bytes)));
        assert!(out.starts_with(b"| Esperanto - basic.dxt\n-braillecraft-template 1\nTranslationTables: "));
        // The same template as a .dxt file from elsewhere (its path in BC_SHIPPED_DXT), compared from its header on;
        // the lines before (the name, copyright lines and its own first line) differ by design.
        if let Some(shipped) = std::env::var_os("BC_SHIPPED_DXT") {
            let bytes = std::fs::read(&shipped).unwrap();
            let out = rewritten(&bytes);
            assert_eq!(String::from_utf8_lossy(from_tables_on(&out)), String::from_utf8_lossy(from_tables_on(&bytes)));
        }
    }

    #[test]
    fn dxt_from_document_guide_dot_always_dot_3() {
        let mut doc = new_doc();
        for c in [0x22u8, 0x27, 0x2c, 0x2d] {
            doc.settings[82] = c;
            let out = String::from_utf8(write_dxt(&dxt_header(&doc, ""), &[], "x.dxt")).unwrap();
            assert!(out.contains("\nContentsGuideDotBraille: Dot_3\n"), "{c:#x}");
        }
    }

    #[test]
    fn page_numbering_comes_from_the_template() {
        let t = shipped();
        let english = t.text(t.by_name(BUILT_IN_DEFAULT).unwrap()).unwrap();
        assert_eq!(page_numbers(&english), [4, 4, 2, 2, 2]);
        assert_eq!(page_numbers("TranslationTables: a+b/c+d\nStyles:\n"), [4, 4, 2, 2, 1]);
    }

    #[test]
    fn every_shipped_template_has_the_page_number_keys() {
        let t = shipped();
        let keys = ["BraillePageNumbersEvenPages", "BraillePageNumbersOddPages", "PrintPageNumbersEvenPages", "PrintPageNumbersOddPages", "FirstPageNumberDisplayed"];
        assert_eq!(t.entries.len(), 226);
        for e in &t.entries {
            let text = t.text(e).unwrap();
            for k in keys {
                assert!(setting(&text, k).is_some(), "{}: {k}", e.name);
            }
        }
    }
}
