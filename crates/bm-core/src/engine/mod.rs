//! The BrailleMaker translation engine. It translates with BrailleMaker's own table sources (tables/*.bmt, compiled
//! by `compile`, found by code through `TableSet`), or with binary rule tables (.btb) and scan tables (.sct) read
//! from a folder at run time.
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
mod tableset;
mod writer;

use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use callbacks::{LanguageSource, Languages, LoadedTables, StyleTable};
pub use scan_table::{Font, ScanTable};
pub use table::RuleTable;
pub use tableset::{TableEntry, TableSet};
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

    /// The rule table, shared (for a language switch to it).
    pub fn rules(&self) -> Rc<RuleTable> {
        self.rules.clone()
    }

    /// Translates a text.
    pub fn translate(&self, text: &str) -> Result<String, EngineError> {
        self.translate_with(text, self.styles.as_ref(), self.languages.as_ref())
    }

    /// Translates a text with a document's styles and language table switches in place of the translator's own;
    /// one compiled table serves every document that uses it.
    pub fn translate_with(
        &self,
        text: &str,
        styles: Option<&StyleTable>,
        languages: Option<&Languages>,
    ) -> Result<String, EngineError> {
        let brackets = self.bracket_codes.unwrap_or(self.rules.writer_mode() != 2);
        self.run(text, styles, languages, brackets)
    }

    /// Translates a document's running text, whose codes are marked as the reference program keeps them, 0x1C code
    /// 0x1F; "[" and "]" are text (bm_core::doctext).
    pub fn translate_marked(
        &self,
        text: &str,
        styles: Option<&StyleTable>,
        languages: Option<&Languages>,
    ) -> Result<String, EngineError> {
        self.run(text, styles, languages, false)
    }

    fn run(
        &self,
        text: &str,
        styles: Option<&StyleTable>,
        languages: Option<&Languages>,
        brackets: bool,
    ) -> Result<String, EngineError> {
        let out = self.engine_output(text, styles, languages, brackets)?;
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
    fn engine_output(
        &self,
        text: &str,
        style: Option<&StyleTable>,
        languages: Option<&Languages>,
        brackets: bool,
    ) -> Result<Vec<i64>, EngineError> {
        let text = if brackets { text.replace('[', "\u{1c}").replace(']', "\u{1f}") } else { text.to_string() };
        let codes = encode_cp437(&text).map_err(EngineError::NotEncodable)?;
        // The scanner gets the document's styles too, as the reference hands its scanner the same style lookup.
        let codes: Vec<i64> = scanner::scan(&self.scan, codes, style)?.into_iter().filter(|&c| c != EOF).collect();
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
                let found = match languages {
                    Some(l) => l.rule_table(name)?,
                    None => None,
                };
                if stack.len() < MAX_TABLE_DEPTH {
                    let next = found.unwrap_or_else(|| stack.last().unwrap().clone());
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

    /// Emphasis fonts (the older American scan table): the italic sign in front of each word of a short stretch, two
    /// in front of the first word and one in front of the last of a stretch of 4 words or more; bold its own sign;
    /// no sign when the next word starts with "_".
    #[test]
    fn emphasis_fonts() {
        let Some(f) = folder() else { return };
        let t = Translator::open(&f, "amerc.btb", "amedxp.sct").unwrap();
        let cases = [
            ("a [fts~i]big[fte~i] dog", "a .big dog"),
            ("a [fts~i]one two three[fte~i] dog", "a .\"o .two .?ree dog"),
            ("a [fts~i]one two three four five[fte~i] dog", "a ..\"o two ?ree f\\r .five dog"),
            ("[fts~b]Bold words here[fte~b] end", "_.,bold _.^ws _.\"h 5d"),
            ("THE [fts~i]BIG RED DOG RAN[fte~i] home", ",,! ..,,big ,,r$ ,,dog .,,ran home"),
            ("[fts~i]_under score[fte~i] x", ".\"u score ;x"),
        ];
        for (text, braille) in cases {
            assert_eq!(t.translate(text).unwrap(), braille, "{text}");
        }
    }

    /// Doubled words (the Indonesian scan table): the scanner marks the two copies and the rule table writes the
    /// doubling sign.
    #[test]
    fn doubled_words() {
        let Some(f) = folder() else { return };
        let t = Translator::open(&f, "idaacp.btb", "idndxp.sct").unwrap();
        let cases = [
            ("anak-anak main bola", ">ak< m*n bola"),
            ("Anak-Anak", ",>ak-,>ak"),
            ("buah-buahan", "b+h<>"),
            ("lari-lari-lari", "lari<-lari"),
            ("anak anak", ">ak >ak"),
        ];
        for (text, braille) in cases {
            assert_eq!(t.translate(text).unwrap(), braille, "{text}");
        }
    }

    /// A sample document translated with its own tables (the older American pair, which uses fonts) and its own
    /// styles, when BM_DX_SAMPLES names the sample folder; with BM_DX_EXPECTED naming a folder of expected results
    /// (FILE.txt, from the reference port), the result must be the same.
    #[test]
    fn a_sample_document_with_fonts_translates() {
        let (Some(f), Ok(dir)) = (folder(), std::env::var("BM_DX_SAMPLES")) else { return };
        let path = std::path::Path::new(&dir).join("dochow.dxp");
        let d = crate::dxfile::read(&std::fs::read(&path).unwrap()).unwrap();
        let bytes = |s: &str| s.chars().map(|c| c as u32 as u8).collect::<Vec<u8>>();
        let styles = StyleTable::new(d.styles.iter().map(|(n, b, e)| (n.clone(), bytes(b), bytes(e))).collect());
        let t = Translator::open(&f, &format!("{}.btb", d.tables[0]), &format!("{}.sct", d.tables[1])).unwrap();
        let out = t.translate_marked(&d.text, Some(&styles), Some(&Languages::from_folder(&f).unwrap())).unwrap();
        assert!(out.starts_with("\u{1c}svpnp2:2:0:0\u{1f}\u{1c}es~h1.\u{1f},,topical ,,how-to ,,guide"), "{}", &out[..80]);
        if let Ok(expected) = std::env::var("BM_DX_EXPECTED") {
            let want = std::fs::read_to_string(std::path::Path::new(&expected).join("dochow.txt")).unwrap();
            assert!(out == want, "differs from the reference port at {:?}", out.chars().zip(want.chars()).position(|(a, b)| a != b));
        }
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

    fn shipped_set() -> TableSet {
        TableSet::open(format!("{}/../../tables", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// The shipped template's styles and the table list's language codes, with the shipped tables: the same
    /// results as the binary tables give (styles_and_language_switch).
    #[test]
    fn shipped_template_styles_and_language_switches() {
        let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
        let set = Rc::new(shipped_set());
        let list = crate::table_list::TableList::read(format!("{root}/tables/languages.tsv")).unwrap();
        let templates = crate::templates::Templates::open(format!("{root}/templates")).unwrap();
        let text = templates.text(templates.by_name(crate::templates::BUILT_IN_DEFAULT).unwrap()).unwrap();
        let styles = crate::templates::styles(&text);
        let languages = Languages::from_list(&list, LanguageSource::Shipped(set.clone()));
        let t = set.translator("enuncp").unwrap();
        let cases = [
            (
                "the cat [es~G1Passage]the cat[ee~G1Passage] the cat",
                "! cat \u{1c}es~g1passage\u{1f}the cat\u{1c}ee~g1passage\u{1f} ! cat",
            ),
            ("a cat [lnb~qip]ab[lnb] the cat", "a cat \u{1c}lnb~qip\u{1f}\";2ab;;\u{1c}lnb\u{1f} ! cat"),
        ];
        for (text, braille) in cases {
            assert_eq!(t.translate_with(text, Some(&styles), Some(&languages)).unwrap(), braille, "{text}");
        }
        // Without them the style and the switch do nothing.
        assert_eq!(t.translate("a cat [lnb~qip]ab[lnb] the cat").unwrap(), "a cat \u{1c}lnb~qip\u{1f};ab\u{1c}lnb\u{1f} ! cat");
    }

    #[test]
    fn shipped_tables_are_found_by_the_codes_a_document_names() {
        let set = shipped_set();
        assert_eq!(set.entry("ENUNCP").map(|e| e.table.as_str()), Some("en-ueb-g2-forward"));
        assert_eq!(set.translator("enuncp").unwrap().translate("The cat sat.").unwrap(), ",! cat sat4");
        assert_eq!(set.translator("enuncb").unwrap().translate(",! cat sat4").unwrap(), "The cat sat.");
        assert!(set.translator("nosuch").is_err());
    }

    #[test]
    fn every_shipped_table_compiles() {
        let set = shipped_set();
        // The 656 tables a document can name, and the text export table with its other scan table.
        assert_eq!(set.entries().len(), 657);
        for e in set.entries() {
            set.translator_with_scan(&e.rules, &e.scan).unwrap_or_else(|err| panic!("{}: {err}", e.table));
        }
        // A rules code listed twice is found by its first line.
        assert_eq!(set.entry("txxtxa").map(|e| e.scan.as_str()), Some("gendxp"));
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
