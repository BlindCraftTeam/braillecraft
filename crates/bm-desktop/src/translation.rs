//! Translation for File, Translate and the translated line, through the BrailleMaker engine and the tables shipped
//! with the program (the folder "tables" beside it, found at start-up).
//!
//! A document names its tables by code, as the document files do: the print-to-braille rules table and its scan
//! table, then the braille-to-print rules table and its scan table. The engine writes and reads braille in North
//! American ASCII braille; a braille document holds it in the encoding chosen in Global, Internationalization, so
//! braille is converted on the way in and out. Codes are shown in documents as [code]; the engine takes them as
//! 0x1C code 0x1F.

use std::cell::{Cell, RefCell};
use std::path::Path;

use bm_core::engine::TableSet;
use bm_core::table_list::TableList;

use crate::encodings::{self, Encoding};

/// The tables of a new document: English (Unified English Braille), contracted, both ways.
pub const DEFAULT_TABLES: [&str; 4] = ["enuncp", "enudxp", "enuncb", "gendxb"];

thread_local! {
    static TABLES: RefCell<Option<TableSet>> = const { RefCell::new(None) };
    static LIST: RefCell<TableList> = RefCell::new(TableList::default());
    /// The region Document, Translation Tables lists (a region bit, or all); Western Hemisphere at first.
    static REGION: Cell<u32> = const { Cell::new(1) };
}

/// Reads the table index and the list of table sets by language from `folder`; translation reports an error until
/// this has run.
pub fn load(folder: &Path) -> Result<usize, String> {
    let set = TableSet::open(folder).map_err(|e| e.to_string())?;
    let n = set.entries().len();
    TABLES.with(|t| *t.borrow_mut() = Some(set));
    let list = TableList::read(folder.join("languages.tsv"))?;
    LIST.with(|l| *l.borrow_mut() = list);
    Ok(n)
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

/// Engine output codes (0x1C ... 0x1F) as documents show them.
fn shown_codes(s: &str) -> String {
    s.replace('\u{1c}', "[").replace('\u{1f}', "]")
}

/// Print text (codes as [code]) to braille in the display encoding `enc`, with the rules table `code`.
pub fn forward(code: &str, text: &str, enc: &Encoding) -> Result<String, String> {
    let ascii = with_table(code, |t| t.translate(text).map_err(|e| e.to_string()))?;
    Ok(convert_braille(&shown_codes(&ascii), encodings::find(encodings::DEFAULT), enc))
}

/// Braille in the display encoding `enc` (codes as [code]) to print, with the rules table `code`.
pub fn back(code: &str, text: &str, enc: &Encoding) -> Result<String, String> {
    let ascii = convert_braille(text, enc, encodings::find(encodings::DEFAULT));
    // In braille "[" and "]" are cells, so the engine is given the codes as 0x1C ... 0x1F, in the same places the
    // document shows them.
    let marked = marks(&ascii);
    let print = with_table(code, |t| t.translate(&marked).map_err(|e| e.to_string()))?;
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

    fn loaded() {
        load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables")).unwrap();
    }

    #[test]
    fn english_both_ways_with_the_shipped_tables() {
        loaded();
        let na = encodings::find(encodings::DEFAULT);
        assert_eq!(forward("enuncp", "The cat sat.", na).unwrap(), ",! cat sat4");
        assert_eq!(forward("enuncp", "[g1l]The cat sat.", na).unwrap(), "[g1l],the cat sat4");
        assert_eq!(back("enuncb", ",! cat sat4", na).unwrap(), "The cat sat.");
        assert!(forward("nosuch", "x", na).is_err());
    }

    #[test]
    fn braille_in_another_encoding_goes_through_the_engine_in_north_american() {
        loaded();
        let german = encodings::find("german");
        let shown = forward("enuncp", "The cat sat.", german).unwrap();
        assert_eq!(convert_braille(&shown, german, encodings::find(encodings::DEFAULT)), ",! cat sat4");
        assert_eq!(back("enuncb", &shown, german).unwrap(), "The cat sat.");
    }

    #[test]
    fn braille_codes_become_engine_marks() {
        assert_eq!(marks("ab[pg]c[x"), "ab\u{1c}pg\u{1f}c[x");
    }
}
