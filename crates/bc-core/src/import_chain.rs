//! The import chains for braille and text files, as the import recipes run them: each recipe is
//! a list of steps (built-in steps and rule scripts) run one after another over the file's bytes, and the result is
//! internal text with codes marked 0x1C ... 0x1F.
//!
//! - Formatted braille (.brf): page-end blanks taken off (only when formatting is preserved), the formatted braille
//!   reader (`braide`), the import scan with the braille scan table, the ninit codes, tables to outlines, spaces
//!   normalised three times, then the Nemeth-in-UEB scripts with the text reversed between them and reversed back.
//! - Coded braille (.bru): the import scan with the braille scan table, the ninit codes, tables to outlines, spaces
//!   normalised three times.
//! - Coded text: the file decoded in the chosen code page and written as internal text, then the same steps as coded
//!   braille with the print scan table.
//! - Formatted text: the page layout read into paragraphs, headings, tables and other styles (`pimbri`), the
//!   characters through the character map tables (`unimap`), the text written as internal text, then the same steps
//!   as coded text. (The standard Chinese character step comes between the character map and the internal text;
//!   it changes nothing until Chinese characters are marked for it, and no step here marks them.)
//! - Orbit Reader braille (.obf): line ends to line codes and capitals to small letters, the import scan with the
//!   braille scan table, then the Nemeth-in-UEB scripts as for formatted braille.
//!
//! The braille encoding step that comes first for formatted braille (the file's encoding to North American braille
//! ASCII) is the caller's, which holds the encodings. A step that fails is reported and the chain goes on with the
//! text as it was.

use crate::braide;
use crate::rule_script::RuleScript;

/// What the chain reads by name: the import scan tables (tables/scan) and the rule scripts (tables/rule-scripts).
pub trait Files {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String>;
    fn rule_script(&self, name: &str) -> Result<RuleScript, String>;
    /// The character map tables (tables/unimap, the names in `UNIMAP_TABLES`) with their bytes.
    fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String>;
    /// A Word style map in its text form (tables/word-styles/<name>.map, the names in `WORD_MAPS`).
    fn word_map(&self, name: &str) -> Result<String, String> {
        Err(format!("word-styles/{name}.map was not found."))
    }
}

/// Which recipe a file is imported with.
#[derive(Debug, Clone, PartialEq)]
pub enum Recipe {
    /// Formatted braille in North American braille ASCII, with the formatted braille import settings.
    FormattedBraille(braide::Settings),
    CodedBraille,
    /// Coded text in the named code page (as `codepages::decode` names them), with what a character that has no
    /// internal code becomes (the Word importer's Unknown Characters setting: 0 "*", 1 nothing, 2 "{U+XXXX}").
    CodedText(String, u8),
    /// Formatted text, with the Unknown Characters setting as for coded text.
    FormattedText(u8),
    Orbit,
    /// A Word document (.docx): the Word importer's settings, the Word style map's file name (`word::map_file_name`
    /// of the template's "word:mws" text) and the Unknown Characters setting.
    Docx(crate::word::Options, String, u8),
    /// HTML or SGML text that reads as XML (the HTML recipe: the tag processor with the SGML flag, then the entity,
    /// element, space, emphasis, code and post scripts, the character map, the Han script, the character step, the
    /// quotes script, the hyperlinks step with the Word importer's hyperlink settings, the optional tables script
    /// and spaces normalised three times), with the Unknown Characters setting.
    Html(crate::word::Options, u8),
    /// A Markdown file (BrailleCraft's own addition): turned into HTML (`markdown::to_html`) and run as `Html`.
    Markdown(crate::word::Options, u8),
}

/// The chain's result: the internal text, the steps that failed (each with its reason) and what the formatted
/// braille reader warns about.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    pub text: Vec<u8>,
    pub errors: Vec<String>,
    pub warnings: braide::Warnings,
}

/// The post-import step (megfdt.sbt, run after every text import): a paragraph of the style
/// B-BodyText. or para. holding only "(this_is_a_braille_document)" or "(this_is_a_print_document)" is taken out
/// of the running text, and the document is made of that kind. Answers the kind asked for: Some(true) braille,
/// Some(false) print, None when the file says nothing. `running` is the text in its running form (codes marked
/// 0x1C ... 0x1F), as the script's own patterns read it.
pub fn forced_kind(running: &mut String) -> Option<bool> {
    let mut forced = None;
    for style in ["B-BodyText.", "para."] {
        for (marker, braille) in [("(this_is_a_braille_document)", true), ("(this_is_a_print_document)", false)] {
            let needle = format!("\x1ces~{style}\x1f{marker}\x1cee~{style}\x1f");
            while let Some(at) = running.find(&needle) {
                running.replace_range(at..at + needle.len(), "");
                forced = Some(braille);
            }
        }
    }
    forced
}

#[cfg(test)]
mod forced_kind_tests {
    use super::forced_kind;

    #[test]
    fn a_marker_paragraph_forces_the_kind_and_goes() {
        let mut t = "\x1ces~h1.\x1fTitle\x1cee~h1.\x1f\x1ces~para.\x1f(this_is_a_braille_document)\x1cee~para.\x1fBody\n".to_string();
        assert_eq!(forced_kind(&mut t), Some(true));
        assert_eq!(t, "\x1ces~h1.\x1fTitle\x1cee~h1.\x1fBody\n");
        let mut p = "\x1ces~B-BodyText.\x1f(this_is_a_print_document)\x1cee~B-BodyText.\x1fText".to_string();
        assert_eq!(forced_kind(&mut p), Some(false));
        assert_eq!(p, "Text");
        let mut none = "\x1ces~para.\x1f(this_is_a_braille_document) and more\x1cee~para.\x1f".to_string();
        assert_eq!(forced_kind(&mut none), None);
        assert!(none.contains("and more"));
    }
}

const BRAILLE_SCAN: &str = "import-braille.sct";
const PRINT_SCAN: &str = "import-print.sct";

/// Every file a chain may read: the scan tables (tables/scan) and the rule scripts (tables/rule-scripts), for a
/// caller that has to fetch them first.
pub const SCAN_TABLES: [&str; 2] = [BRAILLE_SCAN, PRINT_SCAN];
pub const UNIMAP_TABLES: [&str; 9] =
    ["unimap.txt", "abd.txt", "lao.txt", "lisu.txt", "math12.txt", "suttony.txt", "vietnamese.txt", "welsh.txt", "tibetan.txt"];
pub const RULE_SCRIPTS: [&str; 7] = [
    "page-end-blanks.rules",
    "ninit-codes.rules",
    "table-outline.rules",
    "normalize-spaces.rules",
    "nemeth-in-ueb-1.rules",
    "nemeth-in-ueb-2.rules",
    "orbit-lines.rules",
];
/// The Word recipe's rule scripts, in its order (the character map and the character steps come between them).
pub const WORD_RULE_SCRIPTS: [&str; 22] = [
    // The Han scripts word-han-chars.rules loads in its states 2 to 6.
    "han-japanese.rules",
    "han-korean.rules",
    "han-pinyin.rules",
    "han-cantonese.rules",
    "han-zhuyin.rules",
    "word-tags.rules",
    "word-entities.rules",
    "word-chars.rules",
    "word-elements.rules",
    "word-emphasis.rules",
    "word-codes.rules",
    "word-han-chars.rules",
    "word-kana.rules",
    "word-quotes.rules",
    "word-publisher-pages.rules",
    "word-user-codes.rules",
    "word-math-spacing.rules",
    "word-math-spacing-2.rules",
    "word-functions.rules",
    "word-dashes.rules",
    "word-tables.rules",
    "word-control-chars.rules",
];
/// The Word style maps shipped as data (tables/word-styles), by their program names.
pub const WORD_MAPS: [&str; 6] = ["default", "australian", "bana", "french", "russian", "uk-basic"];
/// The HTML recipe's own rule scripts (the others it runs are the Word recipe's: entities, elements, emphasis, codes,
/// Han characters and quotes).
pub const SGML_RULE_SCRIPTS: [&str; 2] = ["sgml-spaces.rules", "sgml-post.rules"];

/// Runs the recipe over the file's bytes.
pub fn run(bytes: &[u8], recipe: &Recipe, files: &dyn Files) -> Imported {
    run_with(bytes, recipe, files, &mut |_, _| {})
}

/// Runs the recipe over the file's bytes and reports each step as it finishes: the step's name and the size of
/// the text after it (for a progress window, and for the large-document measurements).
pub fn run_with(bytes: &[u8], recipe: &Recipe, files: &dyn Files, progress: &mut dyn FnMut(&str, usize)) -> Imported {
    let mut out = Imported::default();
    // The chains that read the file's bytes as they are take no copy of them.
    match recipe {
        Recipe::Docx(opt, map, unknown) => {
            docx(bytes, opt, map, *unknown, files, &mut out, progress);
            return out;
        }
        Recipe::Html(opt, unknown) => {
            let html = String::from_utf8_lossy(bytes);
            let html = match html {
                std::borrow::Cow::Borrowed(h) => std::borrow::Cow::Borrowed(h.strip_prefix('\u{feff}').unwrap_or(h)),
                std::borrow::Cow::Owned(h) => std::borrow::Cow::Owned(h.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(h)),
            };
            html_chain(html, opt, *unknown, files, &mut out, progress);
            return out;
        }
        Recipe::Markdown(opt, unknown) => {
            let markdown = String::from_utf8_lossy(bytes);
            let markdown = markdown.strip_prefix('\u{feff}').unwrap_or(&markdown);
            let html = crate::markdown::to_html(markdown);
            progress("markdown to html", html.len());
            html_chain(std::borrow::Cow::Owned(html), opt, *unknown, files, &mut out, progress);
            return out;
        }
        _ => {}
    }
    let mut text = match recipe {
        // Coded text is decoded straight from the file's bytes.
        Recipe::CodedText(page, unknown) => coded_text_bytes(bytes, page, *unknown),
        _ => bytes.to_vec(),
    };
    let script =|name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>, progress: &mut dyn FnMut(&str, usize)| {
        match files.rule_script(name) {
            Ok(mut s) => match s.run(text) {
                Ok(t) => *text = t,
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Err(e) => errors.push(format!("{name}: {e}")),
        }
        progress(name, text.len());
    };
    let scan = |name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>, progress: &mut dyn FnMut(&str, usize)| {
        match files.scan_table(name).and_then(|t| crate::engine::import_scan(&t, text).map_err(|e| format!("{e:?}"))) {
            Ok(t) => *text = t,
            Err(e) => errors.push(format!("{name}: {e}")),
        }
        progress(name, text.len());
    };
    match recipe {
        Recipe::FormattedBraille(settings) => {
            if settings.preserve {
                script("page-end-blanks.rules", &mut text, &mut out.errors, progress);
            }
            let (t, w) = braide::braide(&text, *settings);
            text = t;
            out.warnings = w;
            progress("braide", text.len());
            scan(BRAILLE_SCAN, &mut text, &mut out.errors, progress);
        }
        Recipe::CodedBraille => scan(BRAILLE_SCAN, &mut text, &mut out.errors, progress),
        Recipe::CodedText(..) => {
            progress("coded text", text.len());
            scan(PRINT_SCAN, &mut text, &mut out.errors, progress);
        }
        Recipe::FormattedText(unknown) => {
            // The importer's substitution set is the code page family the file's first 512 bytes look like (-1, its
            // default, when they do not look like text).
            let set = crate::filekind::text_stats(&text[..text.len().min(512)]).map_or(-1, |s| s.code_page);
            match crate::pimbri::Factors::shipped() {
                Ok(f) => text = crate::pimbri::pimbri(&text, set, &f),
                Err(e) => out.errors.push(format!("text factors: {e}")),
            }
            progress("pimbri", text.len());
            match files.unimap_tables() {
                Ok(tables) => {
                    let refs: Vec<(&str, &[u8])> = tables.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
                    text = crate::unimap::UniMap::read(&refs).map(&text, -1, *unknown);
                }
                Err(e) => out.errors.push(format!("unimap: {e}")),
            }
            progress("unimap", text.len());
            text = coded_text_bytes(&text, "UTF-8", *unknown);
            progress("coded text", text.len());
            scan(PRINT_SCAN, &mut text, &mut out.errors, progress);
        }
        Recipe::Orbit => {
            script("orbit-lines.rules", &mut text, &mut out.errors, progress);
            scan(BRAILLE_SCAN, &mut text, &mut out.errors, progress);
            nemeth(files, &mut text, &mut out.errors, true, progress);
            out.text = text;
            return out;
        }
        Recipe::Docx(..) | Recipe::Html(..) | Recipe::Markdown(..) => unreachable!("run above"),
    }
    script("ninit-codes.rules", &mut text, &mut out.errors, progress);
    // Tables to outlines is optional in the recipes: it runs when it is there. The optional maths step that follows
    // it in the recipes is not installed, so there is nothing to run.
    if let Ok(mut s) = files.rule_script("table-outline.rules") {
        match s.run(&text) {
            Ok(t) => text = t,
            Err(e) => out.errors.push(format!("table-outline.rules: {e}")),
        }
        progress("table-outline.rules", text.len());
    }
    for _ in 0..3 {
        script("normalize-spaces.rules", &mut text, &mut out.errors, progress);
    }
    if matches!(recipe, Recipe::FormattedBraille(_)) {
        nemeth(files, &mut text, &mut out.errors, false, progress);
    }
    out.text = text;
    out
}

/// The HTML recipe (spec 28 section 15, imp-sgml): the tag processor over the text with the SGML flag and no style
/// map (so no scrub rules), then the entity, element, space, emphasis, code and post scripts, the character map, the
/// Han script, the character step (no apostrophe override) with the quotes script, the hyperlinks step, the optional
/// tables script, and spaces normalised three times. The control-character step runs inside the tag processor.
fn html_chain(html: std::borrow::Cow<str>, opt: &crate::word::Options, unknown: u8, files: &dyn Files, out: &mut Imported, progress: &mut dyn FnMut(&str, usize)) {
    let script = |name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>, progress: &mut dyn FnMut(&str, usize)| {
        match files.rule_script(name) {
            Ok(mut s) => match s.run(text) {
                Ok(t) => *text = t,
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Err(e) => errors.push(format!("{name}: {e}")),
        }
        progress(name, text.len());
    };
    let processed = match crate::word::process_html(&html, opt) {
        Ok(p) => p,
        Err(e) => {
            out.errors.push(e);
            return;
        }
    };
    // The HTML text (the Markdown conversion's own string) is not held through the steps that follow.
    drop(html);
    let mut text = processed.text.into_bytes();
    progress("tag processor", text.len());
    for name in ["word-entities.rules", "word-elements.rules", "sgml-spaces.rules", "word-emphasis.rules", "word-codes.rules", "sgml-post.rules"] {
        script(name, &mut text, &mut out.errors, progress);
    }
    match files.unimap_tables() {
        Ok(tables) => {
            let refs: Vec<(&str, &[u8])> = tables.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
            text = crate::unimap::UniMap::read(&refs).map(&text, -1, unknown);
        }
        Err(e) => out.errors.push(format!("unimap: {e}")),
    }
    progress("unimap", text.len());
    script("word-han-chars.rules", &mut text, &mut out.errors, progress);
    // utf8dusci with no arguments: the quotes classed by their neighbours, no apostrophe override.
    text = quotes_to_internal(&text, None, unknown);
    progress("utf8dusci", text.len());
    script("word-quotes.rules", &mut text, &mut out.errors, progress);
    text = crate::word::hyperlinks(&text, opt.hyperlinks, opt.remove_duplicates, if opt.auto_tag_cbc { 1 } else { 0 });
    progress("hyperlinks", text.len());
    if let Ok(mut s) = files.rule_script("table-outline.rules") {
        match s.run(&text) {
            Ok(t) => text = t,
            Err(e) => out.errors.push(format!("table-outline.rules: {e}")),
        }
        progress("table-outline.rules", text.len());
    }
    for _ in 0..3 {
        script("normalize-spaces.rules", &mut text, &mut out.errors, progress);
    }
    out.text = text;
}

/// The Word recipe (spec 28 section 3 and 15): the package through the stylesheet's rules, the page-break fix, the
/// run combining, the control-character step and the tag processor (`word::import`), then the style map's scrub
/// rules, the tag, entity, character, element, emphasis and code scripts, the character map, the Han and kana
/// scripts, the character step with its apostrophe override (U+0027 to U+02BC) and the quotes script, the
/// publishers' page script when print pages are the publishers', the user codes script when codes are allowed,
/// the hyperlinks step, the math spacing scripts when spaces are added, the function spacing script, the dashes
/// script when dash spaces are removed, the tables script, and spaces normalised once.
fn docx(bytes: &[u8], opt: &crate::word::Options, map: &str, unknown: u8, files: &dyn Files, out: &mut Imported, progress: &mut dyn FnMut(&str, usize)) {
    let script = |name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>, progress: &mut dyn FnMut(&str, usize)| {
        match files.rule_script(name) {
            Ok(mut s) => match s.run(text) {
                Ok(t) => *text = t,
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Err(e) => errors.push(format!("{name}: {e}")),
        }
        progress(name, text.len());
    };
    let map_text = match files.word_map(map) {
        Ok(t) => t,
        Err(e) => {
            out.errors.push(e);
            String::new()
        }
    };
    let processed = match crate::word::import(bytes, &map_text, opt) {
        Ok(p) => p,
        Err(e) => {
            out.errors.push(e);
            return;
        }
    };
    let mut text = processed.text.into_bytes();
    progress("tag processor", text.len());
    if !processed.scrub_rules.is_empty() {
        match RuleScript::from_bytes(processed.scrub_rules.as_bytes(), None) {
            Ok(mut s) => match s.run(&text) {
                Ok(t) => text = t,
                Err(e) => out.errors.push(format!("style map rules: {e}")),
            },
            Err(e) => out.errors.push(format!("style map rules: {e}")),
        }
        progress("style map rules", text.len());
    }
    for name in ["word-tags.rules", "word-entities.rules", "word-chars.rules", "word-elements.rules", "word-emphasis.rules", "word-codes.rules"] {
        script(name, &mut text, &mut out.errors, progress);
    }
    match files.unimap_tables() {
        Ok(tables) => {
            let refs: Vec<(&str, &[u8])> = tables.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
            text = crate::unimap::UniMap::read(&refs).map(&text, -1, unknown);
        }
        Err(e) => out.errors.push(format!("unimap: {e}")),
    }
    progress("unimap", text.len());
    script("word-han-chars.rules", &mut text, &mut out.errors, progress);
    script("word-kana.rules", &mut text, &mut out.errors, progress);
    // utf8dusci(U+02BC,U+2018,U+2019): the apostrophe override only, the quotes classed by their neighbours.
    text = quotes_to_internal(&text, Some('\u{2bc}'), unknown);
    progress("utf8dusci", text.len());
    script("word-quotes.rules", &mut text, &mut out.errors, progress);
    if opt.print_pages == crate::word::PrintPages::Publishers {
        script("word-publisher-pages.rules", &mut text, &mut out.errors, progress);
    }
    if opt.codes {
        script("word-user-codes.rules", &mut text, &mut out.errors, progress);
    }
    text = crate::word::hyperlinks(&text, opt.hyperlinks, opt.remove_duplicates, if opt.auto_tag_cbc { 1 } else { 0 });
    progress("hyperlinks", text.len());
    if opt.add_spaces {
        script("word-math-spacing.rules", &mut text, &mut out.errors, progress);
        script("word-math-spacing-2.rules", &mut text, &mut out.errors, progress);
    }
    script("word-functions.rules", &mut text, &mut out.errors, progress);
    if opt.remove_dash_spaces {
        script("word-dashes.rules", &mut text, &mut out.errors, progress);
    }
    // The tables script is optional in the recipe and is shipped, so it runs.
    script("word-tables.rules", &mut text, &mut out.errors, progress);
    script("normalize-spaces.rules", &mut text, &mut out.errors, progress);
    out.text = text;
}

/// The Nemeth-in-UEB scripts: the first, the text reversed, the second on the reversed text, reversed back. In the
/// Orbit recipe the two scripts are optional (run when they are there).
fn nemeth(files: &dyn Files, text: &mut Vec<u8>, errors: &mut Vec<String>, optional: bool, progress: &mut dyn FnMut(&str, usize)) {
    for (k, name) in ["nemeth-in-ueb-1.rules", "nemeth-in-ueb-2.rules"].into_iter().enumerate() {
        if k == 1 {
            *text = reversed(text);
            progress("reversed", text.len());
        }
        match files.rule_script(name) {
            Ok(mut s) => match s.run(text) {
                Ok(t) => *text = t,
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Err(_) if optional => {}
            Err(e) => errors.push(format!("{name}: {e}")),
        }
        progress(name, text.len());
    }
    *text = reversed(text);
    progress("reversed", text.len());
}

/// The character step (utf8dusci) over the UTF-8 text the character map gives: the quote forms, then as for coded
/// text in UTF-8 (a byte order mark at the start dropped, the zero-width joiners taken out unless after the
/// Malayalam virama, each character encoded), in one pass over the characters with no whole-text string between
/// the steps.
fn quotes_to_internal(text: &[u8], apostrophe: Option<char>, unknown: u8) -> Vec<u8> {
    let unicode = String::from_utf8_lossy(text);
    let mut chars = crate::word::quote_forms_chars(&unicode, apostrophe, None, None).peekable();
    if chars.peek() == Some(&'\u{feff}') {
        chars.next();
    }
    let mut out = Vec::with_capacity(text.len());
    let mut prev = '\0';
    let mut one = [0u8; 4];
    for c in chars {
        if c != '\u{200d}' || prev == '\u{0d4d}' {
            encode_char(&mut out, c, &mut one, unknown);
        }
        prev = c;
    }
    out
}

/// One character as internal text; a character with no internal code becomes "*", nothing or "{U+XXXX}" as
/// `unknown` says.
fn encode_char(out: &mut Vec<u8>, c: char, one: &mut [u8; 4], unknown: u8) {
    match crate::internal::encode(c.encode_utf8(one)) {
        Ok(b) => out.extend_from_slice(&b),
        Err(_) => match unknown {
            0 => out.push(b'*'),
            1 => {}
            _ => out.extend_from_slice(format!("{{U+{:04X}}}", c as u32).as_bytes()),
        },
    }
}

/// Coded text as internal text: decoded in its code page (a byte order mark at the start dropped), each zero-width
/// joiner taken out unless it follows the Malayalam virama, then encoded; a character with no internal code becomes
/// "*", nothing or "{U+XXXX}" as `unknown` says.
fn coded_text_bytes(bytes: &[u8], page: &str, unknown: u8) -> Vec<u8> {
    let text = crate::codepages::decode(page, bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    // The joiners are taken out and the characters encoded in one pass: no kept copy of the text.
    let mut out = Vec::with_capacity(text.len());
    let mut prev = '\0';
    let mut one = [0u8; 4];
    for c in text.chars() {
        if c != '\u{200d}' || prev == '\u{0d4d}' {
            encode_char(&mut out, c, &mut one, unknown);
        }
        prev = c;
    }
    out
}

/// Internal text with its codes in reverse order: each two-byte code stays one code, and a format code (0x1C to the
/// next 0x1F) stays whole and in its own order.
pub fn reversed(bytes: &[u8]) -> Vec<u8> {
    // One bit per byte says whether a unit (a byte, or a two-byte code with its first byte) starts there: an
    // eighth of the text, where a slice per unit would be sixteen times it (a large book is reversed twice).
    let mut starts = vec![0u8; bytes.len() / 8 + 1];
    let mut i = 0;
    while i < bytes.len() {
        starts[i / 8] |= 1 << (i % 8);
        i += if crate::internal::is_first_byte(bytes[i]) && i + 1 < bytes.len() { 2 } else { 1 };
    }
    let is_start = |i: usize| starts[i / 8] & (1 << (i % 8)) != 0;
    // The start of the unit that ends at `end`.
    let unit_start = |end: usize| if end >= 2 && !is_start(end - 1) { end - 2 } else { end - 1 };
    let mut out = Vec::with_capacity(bytes.len());
    let mut end = bytes.len();
    while end > 0 {
        let start = unit_start(end);
        if bytes[start..end] == [0x1f] {
            // Back to the code's start, written forward.
            let mut s = start;
            while s > 0 && bytes[s..unit_end(bytes, s, &is_start)] != [0x1c] {
                s = unit_start(s);
            }
            out.extend_from_slice(&bytes[s..end]);
            end = s;
        } else {
            out.extend_from_slice(&bytes[start..end]);
            end = start;
        }
    }
    out
}

/// The end of the unit that starts at `start`, given the unit-start map.
fn unit_end(bytes: &[u8], start: usize, is_start: &dyn Fn(usize) -> bool) -> usize {
    if start + 1 < bytes.len() && !is_start(start + 1) {
        start + 2
    } else {
        start + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Shipped;

    fn tables() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables")
    }

    impl Files for Shipped {
        fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
            std::fs::read(tables().join("scan").join(name)).map_err(|e| e.to_string())
        }
        fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
            RuleScript::open(&tables().join("rule-scripts").join(name)).map_err(|e| e.to_string())
        }
        fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
            UNIMAP_TABLES
                .iter()
                .map(|n| std::fs::read(tables().join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| e.to_string()))
                .collect()
        }
        fn word_map(&self, name: &str) -> Result<String, String> {
            std::fs::read(tables().join("word-styles").join(format!("{name}.map"))).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())
        }
    }

    /// Every sample Word document (BC_DOCX_SAMPLES) goes through the whole recipe with the default map and every
    /// shipped map loads; the result holds text and style codes and no step fails.
    #[test]
    fn the_sample_word_documents_import() {
        for m in WORD_MAPS {
            assert!(Shipped.word_map(m).is_ok(), "{m}");
            crate::word::StyleMap::from_text(&Shipped.word_map(m).unwrap()).unwrap_or_else(|e| panic!("{m}: {e}"));
        }
        let Ok(dir) = std::env::var("BC_DOCX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("docx") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let got = run(&bytes, &Recipe::Docx(crate::word::Options::default(), "default".into(), 0), &Shipped);
            assert!(got.errors.is_empty(), "{}: {:?}", path.display(), got.errors);
            assert!(got.text.contains(&0x1c) && got.text.iter().any(|b| b.is_ascii_alphabetic()), "{}: {:?}", path.display(), String::from_utf8_lossy(&got.text[..got.text.len().min(200)]));
            count += 1;
        }
        assert!(count > 0, "no sample Word documents in {dir}");
    }

    /// A Markdown document goes through the HTML recipe: headings, paragraphs, lists, emphasis and links come out as
    /// text with codes and no step fails.
    #[test]
    fn a_markdown_document_imports() {
        let md = "# Title\n\nSome *text* and a [link](http://example.org/) with Tom's \"quotes\" & more.\n\n- first\n- second\n\n## Second\n\nA `code` word and **strong** text.\n";
        let got = run(md.as_bytes(), &Recipe::Markdown(crate::word::Options::default(), 0), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        let text = crate::internal::decode(&got.text);
        // Observed 2026-10-17: the headings and paragraphs take the h1., h2. and para. styles, emphasis the fts
        // and fte codes, the list the list. style with level codes and line codes, inline code the compinline
        // style, and the link its text with the URL in parentheses (the Hyperlinks action "both").
        assert!(text.starts_with("\x1ces~h1.\x1fTitle\x1cee~h1.\x1f\x1ces~para.\x1fSome \x1cfts~i\x1ftext\x1cfte~i\x1f and a "), "{text}");
        assert!(text.contains("link (http://example.org/)"), "{text}");
        assert!(text.contains(" with Tom's \"quotes\" & more.\x1cee~para.\x1f\x1ces~list.\x1f\x1chl1\x1ffirst\x1cl\x1f\x1chl1\x1fsecond\x1cl\x1f\x1cee~list.\x1f"), "{text}");
        assert!(text.contains("\x1ces~h2.\x1fSecond\x1cee~h2.\x1f\x1ces~para.\x1fA \x1ces~compinline\x1fcode\x1cee~compinline\x1f word and \x1cfts~b\x1fstrong\x1cfte~b\x1f text.\x1cee~para.\x1f"), "{text}");
        assert!(!text.contains("<p>") && !text.contains("</"), "tags left: {text}");
    }

    #[test]
    fn reversal_keeps_codes_and_two_byte_codes_whole() {
        assert_eq!(reversed(b"ab\x1cp\x1fcd"), b"dc\x1cp\x1fba".to_vec());
        assert_eq!(reversed(&[0x61, 0xf5, 0x23, 0x62]), vec![0x62, 0xf5, 0x23, 0x61]);
        assert_eq!(reversed(&reversed(b"x \x1cind5\x1fy")), b"x \x1cind5\x1fy".to_vec());
    }

    #[test]
    fn coded_braille_gets_codes() {
        let got = run(b"|P This is |IND5 a test.", &Recipe::CodedBraille, &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        assert_eq!(got.text, b"\x1cp\x1fthis is \x1cind5\x1fa test.".to_vec());
    }

    #[test]
    fn coded_text_gets_codes_and_joins_hyphens() {
        let got = run(b"$p Hello hyphen-\r\nated.", &Recipe::CodedText("WINDOWS-1252".into(), 0), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        assert_eq!(got.text, b"\x1cp\x1fHello hyphenated.".to_vec());
    }

    #[test]
    fn coded_text_marks_characters_with_no_code() {
        // U+0378 is not a character at all, so the character table has no code for it.
        let bytes = "a\u{378}b".as_bytes();
        assert_eq!(coded_text_bytes(bytes, "UTF-8", 0), b"a*b".to_vec());
        assert_eq!(coded_text_bytes(bytes, "UTF-8", 1), b"ab".to_vec());
        assert_eq!(coded_text_bytes(bytes, "UTF-8", 2), b"a{U+0378}b".to_vec());
    }

    #[test]
    fn formatted_text_gets_styles() {
        let page = "INTRODUCTION\r\n\r\nThe rain had stopped by the time we reached the old stone bridge at last.\r\n";
        let got = run(page.as_bytes(), &Recipe::FormattedText(0), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        let text = String::from_utf8_lossy(&got.text).into_owned();
        assert!(text.contains("\x1cind5\x1fINTRODUCTION"), "{text:?}");
        assert!(text.contains("\x1ces~para.\x1fThe rain had stopped"), "{text:?}");
    }

    #[test]
    fn orbit_lines_become_line_codes() {
        let got = run(b"ABC\r\nDEF", &Recipe::Orbit, &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        // The script writes the line code in its internal form, which the scan passes on.
        assert_eq!(got.text, b"abc\x1cl\x1fdef".to_vec());
    }

    #[test]
    fn formatted_braille_runs_every_step() {
        let page = b",! QUICK BR[N FOX JUMPS OV] ! LAZY DOG4\nCONT9UES ON ! NEXT L9E\n\x0c";
        let got = run(page, &Recipe::FormattedBraille(braide::Settings::default()), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        let text = String::from_utf8_lossy(&got.text);
        assert!(text.contains(",! quick br[n fox jumps ov] ! lazy dog4 cont9ues on ! next l9e"), "{text}");
    }
}
