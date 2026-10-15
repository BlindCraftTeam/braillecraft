//! Translation for File, Translate and the translated line, through the BrailleMaker engine and the tables shipped
//! with the program (the folder "tables" beside it, found at start-up).
//!
//! A document names its tables by code, as the document files do: the print-to-braille rules table and its scan
//! table, then the braille-to-print rules table and its scan table. The engine writes and reads braille in North
//! American ASCII braille; a braille document holds it in the encoding chosen in Global, Internationalization, so
//! braille is converted on the way in and out. Codes are shown in documents as [code]; the engine takes them as
//! 0x1C code 0x1F.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use bm_core::engine::{LanguageSource, Languages, StyleTable, TableSet};
use bm_core::table_list::TableList;
use bm_core::templates::{self, Templates};

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
    crate::settings_store::load_default_template()
        .filter(|n| with_templates(|t| t.is_some_and(|t| t.by_name(n).is_some())))
        .unwrap_or_else(|| templates::BUILT_IN_DEFAULT.to_string())
}

/// A template's four tables, by its name.
pub fn template_tables(name: &str) -> Option<[String; 4]> {
    with_templates(|t| {
        let t = t?;
        templates::tables(&t.text(t.by_name(name)?).ok()?)
    })
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
                .map(|(name, begin, end)| Style { name: name.to_string(), nested: !name.ends_with('.'), begin, end })
                .collect()
        })
        .unwrap_or_default()
}

/// A document's styles as the engine looks them up.
pub fn style_table(styles: &[Style]) -> StyleTable {
    StyleTable::from_text(styles.iter().map(|s| (s.name.as_str(), s.begin.as_str(), s.end.as_str())))
}

/// Translates with the rules table `code`, the document's styles and the language switches of the table list.
fn translate(code: &str, text: &str, styles: &StyleTable, print_document: bool) -> Result<String, String> {
    LANGUAGES.with(|l| {
        let l = l.borrow();
        let languages = l.as_ref().map(|(p, b)| if print_document { p } else { b });
        with_table(code, |t| t.translate_with(text, Some(styles), languages).map_err(|e| e.to_string()))
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

fn with_table<R>(code: &str, f: impl FnOnce(&bm_core::engine::Translator) -> Result<R, String>) -> Result<R, String> {
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

/// Engine output codes (0x1C ... 0x1F) as documents show them.
fn shown_codes(s: &str) -> String {
    s.replace('\u{1c}', "[").replace('\u{1f}', "]")
}

/// Print text (codes as [code]) to braille in the display encoding `enc`, with the rules table `code` and the
/// document's styles.
pub fn forward(code: &str, text: &str, enc: &Encoding, styles: &StyleTable) -> Result<String, String> {
    let ascii = translate(code, text, styles, true)?;
    Ok(convert_braille(&shown_codes(&ascii), encodings::find(encodings::DEFAULT), enc))
}

/// Braille in the display encoding `enc` (codes as [code]) to print, with the rules table `code` and the document's
/// styles.
pub fn back(code: &str, text: &str, enc: &Encoding, styles: &StyleTable) -> Result<String, String> {
    let ascii = convert_braille(text, enc, encodings::find(encodings::DEFAULT));
    // In braille "[" and "]" are cells, so the engine is given the codes as 0x1C ... 0x1F, in the same places the
    // document shows them.
    let marked = marks(&ascii);
    let print = translate(code, &marked, styles, false)?;
    Ok(shown_codes(&print))
}

/// Each [code] segment, as convert_braille finds them, written 0x1C code 0x1F.
fn marks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        let (plain, tail) = rest.split_at(i);
        out.push_str(plain);
        match tail.find(']') {
            Some(j) => {
                out.push('\u{1c}');
                out.push_str(&tail[1..j]);
                out.push('\u{1f}');
                rest = &tail[j + 1..];
            }
            None => {
                out.push_str(tail);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Braille text from one encoding to another; codes written [code] pass unchanged.
pub fn convert_braille(text: &str, from: &Encoding, to: &Encoding) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let (plain, tail) = match rest.find('[') {
            Some(i) => rest.split_at(i),
            None => (rest, ""),
        };
        out.push_str(&to.cells_to_text(&from.text_to_cells(plain)));
        match tail.find(']') {
            Some(j) if !tail.is_empty() => {
                out.push_str(&tail[..=j]);
                rest = &tail[j + 1..];
            }
            _ => {
                out.push_str(&to.cells_to_text(&from.text_to_cells(tail)));
                rest = "";
            }
        }
    }
    out
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

    #[test]
    fn english_both_ways_with_the_shipped_tables() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        assert_eq!(forward("enuncp", "The cat sat.", na, &english()).unwrap(), ",! cat sat4");
        assert_eq!(forward("enuncp", "[g1l]The cat sat.", na, &english()).unwrap(), "[g1l],the cat sat4");
        assert_eq!(back("enuncb", ",! cat sat4", na, &english()).unwrap(), "The cat sat.");
        assert!(forward("nosuch", "x", na, &english()).is_err());
        assert_eq!(template_tables(ENGLISH).unwrap(), DEFAULT_TABLES.map(String::from));
    }

    #[test]
    fn template_styles_and_language_codes_take_effect() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        assert_eq!(
            forward("enuncp", "the cat [es~G1Passage]the cat[ee~G1Passage] the cat", na, &english()).unwrap(),
            "! cat [es~g1passage]the cat[ee~g1passage] ! cat"
        );
        assert_eq!(
            forward("enuncp", "a cat [lnb~qip]ab[lnb] the cat", na, &english()).unwrap(),
            "a cat [lnb~qip]\";2ab;;[lnb] ! cat"
        );
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
        assert_eq!(
            forward("enuncp", "the cat [es~Mine]the cat[ee~Mine] the cat", na, &style_table(&styles)).unwrap(),
            "! cat [es~mine]the cat[ee~mine] ! cat"
        );
    }

    #[test]
    fn braille_in_another_encoding_goes_through_the_engine_in_north_american() {
        loaded();
        let german = encodings::find("german");
        let shown = forward("enuncp", "The cat sat.", german, &english()).unwrap();
        assert_eq!(convert_braille(&shown, german, encodings::find(encodings::DEFAULT)), ",! cat sat4");
        assert_eq!(back("enuncb", &shown, german, &english()).unwrap(), "The cat sat.");
    }

    #[test]
    fn braille_codes_become_engine_marks() {
        assert_eq!(marks("ab[pg]c[x"), "ab\u{1c}pg\u{1f}c[x");
    }
}
