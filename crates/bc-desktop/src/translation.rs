//! Translation for File, Translate and the translated line, through the BrailleCraft engine and the tables shipped
//! with the program (the folder "tables" beside it, found at start-up).
//!
//! A document names its tables by code, as the document files do: the print-to-braille rules table and its scan
//! table, then the braille-to-print rules table and its scan table. The engine writes and reads braille in North
//! American ASCII braille; a braille document holds it in the encoding chosen in Global, Internationalization, so
//! braille is converted on the way in and out. A document's codes are kept apart from its text
//! (bc_core::doctext::Shown); the engine takes them as 0x1C code 0x1F, and brackets in the text as text.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use bc_core::doctext::Shown;
use bc_core::engine::{LanguageSource, Languages, StyleTable, TableSet};
use bc_core::table_list::TableList;
use bc_core::templates::{self, Templates};

use crate::app::Style;
use crate::encodings::{self, Encoding};

/// The tables of a new document: English (Unified English Braille), contracted, both ways.
pub const DEFAULT_TABLES: [&str; 4] = ["enuncp", "enudxp", "enuncb", "gendxb"];

thread_local! {
    static TABLES: RefCell<Option<Rc<TableSet>>> = const { RefCell::new(None) };
    static LIST: RefCell<TableList> = RefCell::new(TableList::default());
    /// Language codes switching tables: for print documents and for braille documents.
    static LANGUAGES: RefCell<Option<(Languages, Languages)>> = const { RefCell::new(None) };
    static TEMPLATES: RefCell<Option<Templates>> = const { RefCell::new(None) };
    /// Each template's styles, read the first time a document with that template is translated.
    static STYLES: RefCell<HashMap<String, Rc<StyleTable>>> = RefCell::new(HashMap::new());
    /// The region Document, Translation Tables lists (a region bit, or all); Western Hemisphere at first.
    static REGION: Cell<u32> = const { Cell::new(1) };
    /// The folder of the rule scripts coded files are made with (tables/rule-scripts).
    static RULES: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
    /// The command line's /template, for this run.
    static RUN_TEMPLATE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A LaTeX file from a print document's running text (codes marked 0x1C ... 0x1F), as the reference program writes
/// it (spec 22): the text through the text export table (txxtxa with the binary scan table; when that table cannot be
/// used, the body as the document file stores it goes on unchanged), then the LaTeX rule script, with nothing written
/// before or after what the script writes.
pub fn latex(running: &str) -> Result<Vec<u8>, String> {
    let middle = text_export(running);
    let mut script = rule_script("latex.rules")?;
    script.run(&middle).map_err(|e| e.to_string())
}

/// An HTML with MathJax file from a print document's running text, as the reference program writes it (spec 21):
/// the text through the text export table as for LaTeX, then the MathJax rule script, between the fixed head and
/// foot the original writes.
pub fn mathjax(running: &str) -> Result<Vec<u8>, String> {
    let middle = text_export(running);
    let mut script = rule_script("mathjax.rules")?;
    let body = script.run(&middle).map_err(|e| e.to_string())?;
    Ok(bc_core::html_export::with_mathjax(&body))
}

/// An HTML file from a print document's running text (spec 21): the body as the document file stores it through the
/// HTML recipe (bc_core::html_export).
pub fn html(running: &str) -> Result<Vec<u8>, String> {
    let body = bc_core::dxfile::body_bytes(running);
    let names = bc_core::html_export::SCRIPTS;
    let mut scripts = [rule_script(names[0])?, rule_script(names[1])?, rule_script(names[2])?, rule_script(names[3])?];
    bc_core::html_export::html(&body, &mut scripts)
}

/// The running text through the text export table (txxtxa with the binary scan table); when that table cannot be
/// used, the body as the document file stores it, unchanged.
fn text_export(running: &str) -> Vec<u8> {
    let translated = TABLES.with(|t| {
        let t = t.borrow();
        let tr = t.as_ref()?.translator_with_scan("txxtxa", "binary").ok()?;
        tr.translate_marked(running, None, None).ok()
    });
    // The engine writes bytes; its text holds one character per byte.
    match translated {
        Some(s) => s.chars().map(|c| c as u32 as u8).collect(),
        None => bc_core::dxfile::body_bytes(running),
    }
}

/// A rule script from the shipped folder (tables/rule-scripts).
fn rule_script(name: &str) -> Result<bc_core::rule_script::RuleScript, String> {
    let folder = RULES.with(|r| r.borrow().clone()).ok_or("The rule scripts were not found.")?;
    bc_core::rule_script::RuleScript::open(&folder.join(name)).map_err(|e| format!("{name}: {e}"))
}

/// A coded braille (.bru) or coded text (.txt) file from a document body as the document file stores it (codes as
/// 0x1C name 0x1F): the body through the coding script for braille or for print documents, then the line wrapping
/// script, as the reference program makes these files (spec 04d-C, spec 20).
pub fn coded(body: &[u8], braille: bool) -> Result<Vec<u8>, String> {
    let folder = RULES.with(|r| r.borrow().clone()).ok_or("The rule scripts were not found.")?;
    let open = |name: &str| bc_core::rule_script::RuleScript::open(&folder.join(name)).map_err(|e| format!("{name}: {e}"));
    let mut scripts = [open(if braille { "coded-braille.rules" } else { "coded-text.rules" })?, open("coded-wrap.rules")?];
    bc_core::rule_script::run_all(&mut scripts, body).map_err(|e| e.to_string())
}

/// The files the import chain reads, from the shipped tables folder (tables/scan and tables/rule-scripts).
pub struct ImportFiles(std::path::PathBuf);

pub fn import_files() -> Result<ImportFiles, String> {
    let rules = RULES.with(|r| r.borrow().clone()).ok_or("The rule scripts were not found.")?;
    Ok(ImportFiles(rules.parent().map(Path::to_path_buf).unwrap_or(rules)))
}

impl bc_core::import_chain::Files for ImportFiles {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.0.join("scan").join(name)).map_err(|e| e.to_string())
    }
    fn rule_script(&self, name: &str) -> Result<bc_core::rule_script::RuleScript, String> {
        bc_core::rule_script::RuleScript::open(&self.0.join("rule-scripts").join(name)).map_err(|e| e.to_string())
    }
    fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        bc_core::import_chain::UNIMAP_TABLES
            .iter()
            .map(|n| std::fs::read(self.0.join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| format!("{n}: {e}")))
            .collect()
    }
}

/// Reads the table index and the list of table sets by language from `folder`; translation reports an error until
/// this has run.
pub fn load(folder: &Path) -> Result<usize, String> {
    let set = Rc::new(TableSet::open(folder).map_err(|e| e.to_string())?);
    let n = set.entries().len();
    let list = TableList::read(folder.join("languages.tsv"))?;
    let print = Languages::from_list(&list, LanguageSource::Shipped(set.clone()));
    let mut braille = print.clone();
    braille.print_document = false;
    TABLES.with(|t| *t.borrow_mut() = Some(set));
    RULES.with(|r| *r.borrow_mut() = Some(folder.join("rule-scripts")));
    LANGUAGES.with(|l| *l.borrow_mut() = Some((print, braille)));
    LIST.with(|l| *l.borrow_mut() = list);
    Ok(n)
}

/// Reads the document templates from `folder` (File, New lists them; documents take their styles).
pub fn load_templates(folder: &Path) -> Result<usize, String> {
    let t = Templates::open(folder)?;
    let n = t.entries.len();
    TEMPLATES.with(|s| *s.borrow_mut() = Some(t));
    Ok(n)
}

/// Runs `f` with the document templates (none when they were not found).
pub fn with_templates<R>(f: impl FnOnce(Option<&Templates>) -> R) -> R {
    TEMPLATES.with(|t| f(t.borrow().as_ref()))
}

/// The template File, New selects: the one marked as default when it is still there, otherwise the built-in
/// default.
pub fn default_template() -> String {
    let found = |n: &String| with_templates(|t| t.is_some_and(|t| t.by_name(n).is_some()));
    RUN_TEMPLATE
        .with(|r| r.borrow().clone())
        .filter(found)
        .or_else(|| crate::settings_store::load_default_template().filter(found))
        .unwrap_or_else(|| templates::BUILT_IN_DEFAULT.to_string())
}

/// The command line's /template:<name>: the default template for this run, when the template is found (the
/// setting kept for later runs is left as it is). Answers whether it was found.
pub fn set_run_template(name: &str) -> bool {
    let known = with_templates(|t| t.is_some_and(|t| t.by_name(name).is_some()));
    if known {
        RUN_TEMPLATE.with(|r| *r.borrow_mut() = Some(name.to_string()));
    }
    known
}

/// A template's four tables, by its name.
pub fn template_tables(name: &str) -> Option<[String; 4]> {
    with_templates(|t| {
        let t = t?;
        templates::tables(&t.text(t.by_name(name)?).ok()?)
    })
}

/// A template's page numbering, by its name (the order bc_core::templates::page_numbers gives); the reference
/// program's default for an unknown template.
/// A template's Hyphenation (0 None to 4 French 1103); 2 for an unknown template.
pub fn template_hyphenation(name: &str) -> u16 {
    with_templates(|t| {
        let t = t?;
        Some(templates::hyphenation(&t.text(t.by_name(name)?).ok()?))
    })
    .unwrap_or(2)
}

pub fn template_page_numbers(name: &str) -> [u32; 5] {
    with_templates(|t| {
        let t = t?;
        Some(templates::page_numbers(&t.text(t.by_name(name)?).ok()?))
    })
    .unwrap_or([4, 4, 2, 2, 1])
}

/// A template's styles, by its name; none for an unknown template.
fn template_styles(name: &str) -> Option<Rc<StyleTable>> {
    if let Some(s) = STYLES.with(|s| s.borrow().get(name).cloned()) {
        return Some(s);
    }
    let styles = with_templates(|t| {
        let t = t?;
        Some(Rc::new(templates::styles(&t.text(t.by_name(name)?).ok()?)))
    })?;
    STYLES.with(|s| s.borrow_mut().insert(name.to_string(), styles.clone()));
    Some(styles)
}

/// The styles a document made with this template starts with: the template's Styles section, in its order. A name
/// ending in "." is a paragraph style (spec 05: inferred from the template names), so the others are the ones that
/// can be nested within another style.
pub fn template_style_list(name: &str) -> Vec<Style> {
    template_styles(name)
        .map(|t| {
            t.entries()
                .map(|(name, begin, end)| Style {
                    name: name.to_string(),
                    nested: !name.ends_with('.'),
                    begin: codes_from_dialog(&begin),
                    end: codes_from_dialog(&end),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A document's styles as the engine looks them up.
pub fn style_table(styles: &[Style]) -> StyleTable {
    let bytes = |t: &str| -> Vec<u8> {
        t.chars()
            .filter(|&c| (c as u32) < 256)
            .map(|c| match c {
                bc_core::doctext::OPEN => 0x1c,
                bc_core::doctext::CLOSE => 0x1f,
                c => c as u8,
            })
            .collect()
    };
    StyleTable::new(styles.iter().map(|s| (s.name.clone(), bytes(&s.begin), bytes(&s.end))).collect())
}

/// Style codes as a template or a dialog writes them ("[l][kps]") to running text (codes marked 0x1C ... 0x1F).
pub fn codes_from_dialog(text: &str) -> String {
    Shown::marked(text).to_running()
}

/// Style codes in running text as a dialog shows them, each code in brackets.
pub fn codes_for_dialog(running: &str) -> String {
    running
        .chars()
        .map(|c| match c {
            bc_core::doctext::OPEN => '[',
            bc_core::doctext::CLOSE => ']',
            c => c,
        })
        .collect()
}

/// Translates running text (codes marked 0x1C ... 0x1F, brackets as text; bc_core::doctext) with the rules table
/// `code`, the document's styles and the language switches of the table list.
fn translate(code: &str, running: &str, styles: &StyleTable, print_document: bool) -> Result<String, String> {
    LANGUAGES.with(|l| {
        let l = l.borrow();
        let languages = l.as_ref().map(|(p, b)| if print_document { p } else { b });
        with_table(code, |t| t.translate_marked(running, Some(styles), languages).map_err(|e| e.to_string()))
    })
}

/// Runs `f` with the list of table sets by language.
pub fn with_list<R>(f: impl FnOnce(&TableList) -> R) -> R {
    LIST.with(|l| f(&l.borrow()))
}

pub fn region() -> u32 {
    REGION.with(Cell::get)
}

pub fn set_region(region: u32) {
    REGION.with(|r| r.set(region));
}

fn with_table<R>(code: &str, f: impl FnOnce(&bc_core::engine::Translator) -> Result<R, String>) -> Result<R, String> {
    TABLES.with(|t| {
        let t = t.borrow();
        let Some(set) = t.as_ref() else {
            return Err("The translation tables were not found beside the program.".to_string());
        };
        let tr = set.translator(code).map_err(|e| e.to_string())?;
        f(&tr)
    })
}

/// "656 tables", or that none were found, for the About box.
pub fn table_count() -> String {
    TABLES.with(|t| match t.borrow().as_ref() {
        Some(s) => format!("{} tables", s.entries().len()),
        None => "no tables found".to_string(),
    })
}

/// Whether the shipped tables have this rules table code.
pub fn has_table(code: &str) -> bool {
    TABLES.with(|t| t.borrow().as_ref().is_some_and(|s| s.entry(code).is_some()))
}

/// The engine's output (running text) in the editor's form, with the echo of the document's initial codes at its
/// start left off (the document keeps them apart).
fn result(out: &str, initial: &str) -> Shown {
    let lead = Shown::marked(initial).to_running();
    Shown::from_running(out.strip_prefix(lead.as_str()).unwrap_or(out))
}

/// Print (a document or a line of one, with its codes) to braille in the display encoding `enc`, with the rules
/// table `code`, the document's initial codes and its styles. A whole document goes through the engine in one run,
/// as the reference program's translation run reads the whole document; the result keeps the codes.
pub fn forward(code: &str, text: &Shown, initial: &str, enc: &Encoding, styles: &StyleTable) -> Result<Shown, String> {
    let running = format!("{}{}", Shown::marked(initial).to_running(), text.to_running());
    let out = translate(code, &running, styles, true)?;
    Ok(convert_braille(&result(&out, initial), encodings::find(encodings::DEFAULT), enc))
}

/// Braille (a document or a line of one, in the display encoding `enc`, with its codes) to print, with the rules
/// table `code`, the document's initial codes and its styles.
pub fn back(code: &str, text: &Shown, initial: &str, enc: &Encoding, styles: &StyleTable) -> Result<Shown, String> {
    let ascii = convert_braille(text, enc, encodings::find(encodings::DEFAULT));
    let running = format!("{}{}", Shown::marked(initial).to_running(), ascii.to_running());
    let out = translate(code, &running, styles, false)?;
    Ok(result(&out, initial))
}

/// Braille text from one encoding to another, cell for cell; the codes keep their text.
pub fn convert_braille(doc: &Shown, from: &Encoding, to: &Encoding) -> Shown {
    map_cells(doc, |run| to.cells_to_text(&from.text_to_cells(run)))
}

/// Braille text in `enc`'s characters written as Unicode braille patterns (U+2800 plus the dots), as the original
/// shows braille when "Prefer Unicode Braille Patterns" is chosen; the codes keep their text.
pub fn as_patterns(doc: &Shown, enc: &Encoding) -> Shown {
    map_cells(doc, |run| enc.text_to_cells(run))
}

/// `doc` with each run of text between its codes changed by `convert`.
fn map_cells(doc: &Shown, convert: impl Fn(&str) -> String) -> Shown {
    let chars: Vec<char> = doc.text.chars().collect();
    let mut text = String::with_capacity(doc.text.len());
    let mut at = 0;
    let convert = |run: &[char]| convert(&run.iter().collect::<String>());
    for &(start, end) in &doc.codes {
        text.push_str(&convert(&chars[at.min(start)..start]));
        text.extend(&chars[start..end.min(chars.len())]);
        at = end;
    }
    text.push_str(&convert(&chars[at.min(chars.len())..]));
    Shown { text, codes: doc.codes.clone(), inner: doc.inner.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGLISH: &str = templates::BUILT_IN_DEFAULT;

    fn loaded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        load(&root.join("tables")).unwrap();
        load_templates(&root.join("templates")).unwrap();
    }

    /// The styles a new English (UEB) document holds.
    fn english() -> StyleTable {
        style_table(&template_style_list(ENGLISH))
    }

    /// Print text whose bracketed runs are codes, to braille in North American, as the editor shows it.
    fn fwd(text: &str) -> String {
        let na = encodings::find(encodings::DEFAULT);
        forward("enuncp", &Shown::marked(text), "", na, &english()).unwrap().text
    }

    #[test]
    fn latex_is_a_whole_document_from_the_text_export_table_and_script() {
        loaded();
        let out = String::from_utf8_lossy(&latex(&Shown::marked("Cost 5% & up[p]Next").to_running()).unwrap()).into_owned();
        assert!(out.starts_with("\\documentclass{article}"), "{out}");
        assert!(out.contains("\\begin{document}"), "{out}");
        assert!(out.contains("5\\%") && out.contains("\\&"), "{out}");
        assert!(out.contains("\\par"), "{out}");
        assert!(out.trim_end().ends_with("\\end{document}"), "{out}");
    }

    #[test]
    fn english_both_ways_with_the_shipped_tables() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        assert_eq!(fwd("The cat sat."), ",! cat sat4");
        assert_eq!(fwd("[g1l]The cat sat."), "[g1l],the cat sat4");
        // The document's initial codes are in force and their echo is left off.
        let s = forward("enuncp", &Shown::plain("The cat sat."), "[g1l]", na, &english()).unwrap();
        assert_eq!(s.text, ",the cat sat4");
        assert_eq!(back("enuncb", &Shown::plain(",! cat sat4"), "", na, &english()).unwrap().text, "The cat sat.");
        assert!(forward("nosuch", &Shown::plain("x"), "", na, &english()).is_err());
        assert_eq!(template_tables(ENGLISH).unwrap(), DEFAULT_TABLES.map(String::from));
    }

    #[test]
    fn brackets_in_the_text_are_text_both_ways() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        // A print bracket is translated as a bracket, not taken for a code.
        let s = forward("enuncp", &Shown::plain("See [hds2] now."), "", na, &english()).unwrap();
        assert!(s.codes.is_empty(), "{s:?}");
        // The braille cells "]" (er) and "[" (ow) are cells: ",*apt]" is "Chapter".
        let s = back("enuncb", &Shown::plain(",*apt] ,\"o d[n"), "", na, &english()).unwrap();
        assert_eq!(s.text, "Chapter One down");
    }

    #[test]
    fn template_styles_and_language_codes_take_effect() {
        loaded();
        assert_eq!(
            fwd("the cat [es~G1Passage]the cat[ee~G1Passage] the cat"),
            "! cat [es~g1passage]the cat[ee~g1passage] ! cat"
        );
        assert_eq!(fwd("a cat [lnb~qip]ab[lnb] the cat"), "a cat [lnb~qip]\";2ab;;[lnb] ! cat");
    }

    #[test]
    fn a_document_starts_with_its_templates_styles_and_a_style_added_to_it_takes_effect() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        let mut styles = template_style_list(ENGLISH);
        assert_eq!(styles.len(), 119);
        let passage = styles.iter().find(|s| s.name == "G1Passage").cloned().unwrap();
        assert!(passage.nested);
        assert!(!styles.iter().find(|s| s.name == "h1.").unwrap().nested);
        styles.push(Style { name: "Mine".to_string(), nested: true, begin: passage.begin, end: passage.end });
        let text = Shown::marked("the cat [es~Mine]the cat[ee~Mine] the cat");
        assert_eq!(
            forward("enuncp", &text, "", na, &style_table(&styles)).unwrap().text,
            "! cat [es~mine]the cat[ee~mine] ! cat"
        );
    }

    #[test]
    fn braille_in_another_encoding_goes_through_the_engine_in_north_american() {
        loaded();
        let german = encodings::find("german");
        let na = encodings::find(encodings::DEFAULT);
        let shown = forward("enuncp", &Shown::plain("The cat sat."), "", german, &english()).unwrap();
        assert_eq!(convert_braille(&shown, german, na).text, ",! cat sat4");
        assert_eq!(back("enuncb", &shown, "", german, &english()).unwrap().text, "The cat sat.");
    }

    #[test]
    fn a_whole_document_goes_through_the_engine_with_its_line_codes() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        let print = Shown::marked("[es~h1.]Chapter One[ee~h1.]\nThe cat sat.\nIt was warm.");
        let braille = forward("enuncp", &print, "", na, &english()).unwrap();
        assert_eq!(braille.text, "[es~h1.],*apt] ,\"o[ee~h1.]\n,! cat sat4\n,x 0 w>m4");
        assert_eq!(braille.codes.len(), 2);
        let back = back("enuncb", &braille, "", na, &english()).unwrap();
        assert_eq!(back.text, "[es~h1.]Chapter One[ee~h1.]\nThe cat sat.\nIt was warm.");
        assert_eq!(back.codes.len(), 2);
    }
}
