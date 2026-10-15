//! The BrailleMaker translation engine. It translates with rule tables (.btb) and scan tables (.sct), read from a
//! folder at run time; no table is part of this source.
//!
//! The text goes through three stages:
//! - the scanner (`scanner.rs`), set up by the scan table: it reads the text (runs of spaces, line ends,
//!   paragraphs), builds words, writes them in small letters with capital signs in front (capital letter, capital
//!   word, capital passage and its end) and passes codes through;
//! - the rule engine (`rules.rs`), set up by the rule table: it finds the matching rule at each place and writes
//!   its output; it expands styles through the style table and switches tables at a language code;
//! - the writer (`writer.rs`): format commands, line ends, the final text.
//!
//! Codes are written in documents as [code]. For print input "[" and "]" are taken as the code marks (0x1C and
//! 0x1F); for braille input they stay braille cells ("]" is the er sign). The output keeps 0x1C and 0x1F.
//!
//! ```no_run
//! use bm_core::engine::{Languages, StyleTable, Translator};
//! let folder = "/path/to/tables";
//! let t = Translator::open(folder, "enuncp.btb", "enudxp.sct")?
//!     .with_styles(StyleTable::from_template(format!("{folder}/enuncp.dxt"))?)
//!     .with_languages(Languages::from_folder(folder)?);
//! assert_eq!(t.translate("The cat sat.")?, ",! cat sat4");
//! # Ok::<(), bm_core::engine::EngineError>(())
//! ```

mod callbacks;
mod chars;
pub mod compile;
mod rules;
mod scan_table;
mod scanner;
mod table;
mod writer;

use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use callbacks::{Languages, StyleTable};
pub use scan_table::ScanTable;
pub use table::RuleTable;
pub use writer::Chitab;

use chars::{encode_cp437, EOF};

/// The output mode used when a document or the translated line is translated: every byte as it comes.
pub const OUTPUT_MODE_DOCUMENT: i64 = 3;
/// At most this many tables are open at once through language switches.
const MAX_TABLE_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A file could not be read: its path and the reason.
    Read(String),
    /// A table file is not in the expected layout.
    BadTable(String),
    /// The text holds a character with no code page 437 form.
    NotEncodable(char),
    /// The table uses a part of the scanner that BrailleMaker does not have yet.
    NotSupported(&'static str),
    /// A table source (.bmt) has a mistake: the line and what is wrong.
    Source(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Read(s) => write!(f, "A translation table could not be read: {s}"),
            EngineError::BadTable(s) => write!(f, "A translation table is damaged or of an unknown kind: {s}"),
            EngineError::NotEncodable(c) => write!(f, "The character {c} (U+{:04X}) cannot be translated.", *c as u32),
            EngineError::NotSupported(s) => write!(f, "The translation table uses {s}, which BrailleMaker does not have yet."),
            EngineError::Source(s) => write!(f, "A translation table source has a mistake: {s}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// A file in a folder by name, whatever the case of its letters; the name in small letters when none is there.
pub(crate) fn find_file(folder: &Path, name: &str) -> PathBuf {
    let wanted = name.to_lowercase();
    if let Ok(dir) = std::fs::read_dir(folder) {
        for entry in dir.flatten() {
            if let Some(f) = entry.file_name().to_str() {
                if f.to_lowercase() == wanted {
                    return folder.join(f);
                }
            }
        }
    }
    folder.join(wanted)
}

pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>, EngineError> {
    std::fs::read(path).map_err(|e| EngineError::Read(format!("{}: {e}", path.display())))
}

/// A rule table and its scan table, ready to translate.
pub struct Translator {
    rules: Rc<RuleTable>,
    scan: ScanTable,
    chitab: Chitab,
    styles: Option<StyleTable>,
    languages: Option<Languages>,
    /// The writer's output mode; -1 uses the rule table's own.
    pub output_mode: i64,
    /// Bit 0: write real line ends instead of spaces.
    pub line_flags: i64,
    /// Whether "[" and "]" in the text are code marks; None decides from the rule table (print input: yes).
    pub bracket_codes: Option<bool>,
}

impl Translator {
    /// Opens the rule table and scan table named, from `folder` (names match whatever the case). The character
    /// table chitab.txt is read from the same folder when it is there.
    pub fn open(folder: impl AsRef<Path>, rule_table: &str, scan_table: &str) -> Result<Translator, EngineError> {
        let folder = folder.as_ref();
        let rules = RuleTable::read(&read_file(&find_file(folder, rule_table))?)?;
        let scan = ScanTable::read(&read_file(&find_file(folder, scan_table))?)?;
        Ok(Translator {
            rules: Rc::new(rules),
            scan,
            chitab: Chitab::read_folder(folder),
            styles: None,
            languages: None,
            output_mode: OUTPUT_MODE_DOCUMENT,
            line_flags: 0,
            bracket_codes: None,
        })
    }

    /// Compiles a table source (.bmt, see `compile`) and makes a translator from it, with the built-in character
    /// table; no table files are needed.
    pub fn from_source(source: &str) -> Result<Translator, EngineError> {
        let c = compile::compile(source)?;
        let rules = RuleTable::read(&c.table)?;
        Ok(Translator {
            rules: Rc::new(rules),
            scan: c.scan,
            chitab: Chitab::builtin(),
            styles: None,
            languages: None,
            output_mode: OUTPUT_MODE_DOCUMENT,
            line_flags: 0,
            bracket_codes: None,
        })
    }

    /// Uses this character table (codes, cases and Unicode forms) in place of the one the translator has.
    pub fn with_chitab(mut self, chitab: Chitab) -> Translator {
        self.chitab = chitab;
        self
    }

    pub fn with_styles(mut self, styles: StyleTable) -> Translator {
        self.styles = Some(styles);
        self
    }

    pub fn with_languages(mut self, languages: Languages) -> Translator {
        self.languages = Some(languages);
        self
    }

    pub fn rule_table(&self) -> &RuleTable {
        &self.rules
    }

    /// Translates a text.
    pub fn translate(&self, text: &str) -> Result<String, EngineError> {
        let out = self.engine_output(text)?;
        let mut w = writer::Writer::new(
            &self.chitab,
            self.rules.tail[2],
            self.rules.tail[3],
            self.scan.flag,
            self.line_flags,
            self.output_mode,
        );
        for c in out {
            w.put(c);
        }
        w.put(EOF);
        Ok(w.text())
    }

    /// The scanner and the rule engine, with table switches: the codes handed to the writer.
    fn engine_output(&self, text: &str) -> Result<Vec<i64>, EngineError> {
        let brackets = self.bracket_codes.unwrap_or(self.rules.writer_mode() != 2);
        let text = if brackets { text.replace('[', "\u{1c}").replace(']', "\u{1f}") } else { text.to_string() };
        let codes = encode_cp437(&text).map_err(EngineError::NotEncodable)?;
        let codes: Vec<i64> = scanner::scan(&self.scan, codes)?.into_iter().filter(|&c| c != EOF).collect();
        let style = self.styles.as_ref();
        let mut stack: Vec<Rc<RuleTable>> = vec![self.rules.clone()];
        let mut e = rules::Engine::new(self.rules.clone(), &codes, style, &self.chitab);
        let mut r = e.run();
        while r == 0x15 {
            if e.name_len < 1 {
                // A bare language code goes back one table.
                if stack.len() > 1 {
                    stack.pop();
                }
            } else {
                let name = &e.name[..e.name_len as usize];
                let pair = self.languages.as_ref().and_then(|l| l.lookup(name));
                if stack.len() < MAX_TABLE_DEPTH {
                    let next = match pair {
                        Some((rule_path, _)) => Rc::new(RuleTable::read(&read_file(&rule_path)?)?),
                        None => stack.last().unwrap().clone(),
                    };
                    stack.push(next);
                }
            }
            // The new table goes on from the character after the switch code, with the rest of the input as the
            // first table's scanner read it.
            let mut rest = Vec::new();
            for i in 0..64 {
                let c = e.ring[(e.pos + i) & 63];
                if c == EOF {
                    break;
                }
                rest.push(c);
            }
            let tail_end = if e.inp.last() == Some(&EOF) { e.inp.len() - 1 } else { e.inp.len() };
            if e.ip < tail_end {
                rest.extend_from_slice(&e.inp[e.ip..tail_end]);
            }
            rest.retain(|&x| x < 256);
            let out = std::mem::take(&mut e.out);
            e = rules::Engine::new(stack.last().unwrap().clone(), &rest, style, &self.chitab);
            e.out = out;
            r = e.run();
        }
        Ok(e.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tables, when a folder holding them is named in BM_TABLES.
    fn folder() -> Option<String> {
        std::env::var("BM_TABLES").ok()
    }

    fn english(folder: &str) -> Translator {
        Translator::open(folder, "enuncp.btb", "enudxp.sct")
            .unwrap()
            .with_styles(StyleTable::from_template(format!("{folder}/enuncp.dxt")).unwrap())
            .with_languages(Languages::from_folder(folder).unwrap())
    }

    #[test]
    fn english_print_to_braille() {
        let Some(f) = folder() else { return };
        let t = english(&f);
        let cases = [
            ("The", ",!"),
            ("The cat sat.", ",! cat sat4"),
            ("THE CAT", ",,! ,,cat"),
            ("THE BIG RED DOG ran", ",,,! big r$ dog,' ran"),
            ("He said 123 and 4.5", ",he sd #abc & #d4e"),
            ("café", "caf^/e"),
        ];
        for (text, braille) in cases {
            assert_eq!(t.translate(text).unwrap(), braille, "{text}");
        }
    }

    #[test]
    fn styles_and_language_switch() {
        let Some(f) = folder() else { return };
        let t = english(&f);
        let cases = [
            (
                "the cat [es~G1Passage]the cat[ee~G1Passage] the cat",
                "! cat \u{1c}es~g1passage\u{1f}the cat\u{1c}ee~g1passage\u{1f} ! cat",
            ),
            ("[es~h1.]Chapter One[ee~h1.]", "\u{1c}es~h1.\u{1f},*apt] ,\"o\u{1c}ee~h1.\u{1f}"),
            ("a cat [lnb~qip]ab[lnb] the cat", "a cat \u{1c}lnb~qip\u{1f}\";2ab;;\u{1c}lnb\u{1f} ! cat"),
        ];
        for (text, braille) in cases {
            assert_eq!(t.translate(text).unwrap(), braille, "{text}");
        }
        let plain = Translator::open(&f, "enuncp.btb", "enudxp.sct").unwrap();
        assert_eq!(
            plain.translate("[es~h1.]Chapter One[ee~h1.]").unwrap(),
            "\u{1c}es~h1.\u{1f},*apt] ,\"o\u{1c}ee~h1.\u{1f}"
        );
    }

    #[test]
    fn unknown_characters_are_escaped() {
        let Some(f) = folder() else { return };
        let t = english(&f);
        assert_eq!(t.translate("½").unwrap(), "@$d\"6ab ");
        assert_eq!(t.translate("ª").unwrap(), "@$u\"6aa ");
    }

    #[test]
    fn english_braille_to_print() {
        let Some(f) = folder() else { return };
        let t = Translator::open(&f, "enuncb.btb", "gendxb.sct").unwrap();
        assert_eq!(t.translate(",! cat sat4").unwrap(), "The cat sat.");
    }

    /// BrailleMaker's own table source from the repository's tables folder.
    fn shipped(name: &str) -> Translator {
        let path = format!("{}/../../tables/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        Translator::from_source(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn builtin_character_table_matches_the_binary_one() {
        let Some(f) = folder() else { return };
        assert_eq!(Chitab::builtin(), Chitab::read_folder(std::path::Path::new(&f)));
    }

    #[test]
    fn shipped_english_tables() {
        let forward = shipped("en-ueb-g2-forward.bmt");
        let cases = [
            ("The", ",!"),
            ("The cat sat.", ",! cat sat4"),
            ("THE CAT", ",,! ,,cat"),
            ("THE BIG RED DOG ran", ",,,! big r$ dog,' ran"),
            ("He said 123 and 4.5", ",he sd #abc & #d4e"),
            ("café", "caf^/e"),
            ("[es~h1.]Chapter One[ee~h1.]", "\u{1c}es~h1.\u{1f},*apt] ,\"o\u{1c}ee~h1.\u{1f}"),
        ];
        for (text, braille) in cases {
            assert_eq!(forward.translate(text).unwrap(), braille, "{text}");
        }
        let back = shipped("en-ueb-g2-back.bmt");
        assert_eq!(back.translate(",! cat sat4").unwrap(), "The cat sat.");
    }

    /// A rule that takes no characters and leaves its own condition true matches here at the gamma again and again;
    /// the reference program never finishes this text. BrailleMaker stops the repeat (rules.rs, run) and gives the
    /// gamma its default output, as the Python reference port with the same stop does.
    #[test]
    fn a_rule_that_repeats_without_taking_characters_is_stopped() {
        let semitic = shipped("semitic-language-transcription-basic-forward.bmt");
        assert_eq!(
            semitic.translate("╜ Γ").unwrap(),
            "\u{1c}lng~am\u{1f}***dbd??* \u{1c}lng~\u{1f}\u{1c}lng~\u{1f}*"
        );
    }

    #[test]
    fn missing_tables_and_characters_are_reported() {
        let dir = std::env::temp_dir().join("bm-engine-no-tables");
        assert!(matches!(Translator::open(&dir, "none.btb", "none.sct"), Err(EngineError::Read(_))));
        assert_eq!(encode_cp437("€"), Err('€'));
    }
}
