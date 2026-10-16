//! What the engine asks of the document around it: the codes a style stands for, and which tables a language code
//! switches to.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::chars::{latin1, split_lines, strip};
use super::{EngineError, RuleTable, TableSet};
use crate::table_list::TableList;

/// Template code text ("[l][kps]") to engine codes: "[" is 0x1C and "]" is 0x1F.
fn codes_from_text(text: &str) -> Vec<u8> {
    text.chars()
        .filter(|&c| (c as u32) < 256)
        .map(|c| match c {
            '[' => 0x1c,
            ']' => 0x1f,
            c => c as u8,
        })
        .collect()
}

/// Engine codes back to template code text: 0x1C is "[" and 0x1F is "]".
fn text_from_codes(codes: &[u8]) -> String {
    codes
        .iter()
        .map(|&b| match b {
            0x1c => '[',
            0x1f => ']',
            b => b as char,
        })
        .collect()
}

/// The styles of a document or template: per style name, the codes that begin it and the codes that end it.
#[derive(Debug, Clone, Default)]
pub struct StyleTable {
    styles: Vec<(String, Vec<u8>, Vec<u8>)>,
    /// The same, by name in small letters: the scanner writes code words in small letters, so a name is found
    /// whatever its case.
    folded: HashMap<String, usize>,
}

impl StyleTable {
    pub fn new(styles: Vec<(String, Vec<u8>, Vec<u8>)>) -> StyleTable {
        let mut t = StyleTable::default();
        for (name, begin, end) in styles {
            match t.styles.iter_mut().find(|s| s.0 == name) {
                Some(s) => {
                    s.1 = begin;
                    s.2 = end;
                }
                None => t.styles.push((name, begin, end)),
            }
        }
        for (i, s) in t.styles.iter().enumerate() {
            t.folded.insert(s.0.to_lowercase(), i);
        }
        t
    }

    /// Styles given as name, begin codes and end codes, the codes written as a template writes them ("[l][kps]").
    pub fn from_text<'a>(styles: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> StyleTable {
        StyleTable::new(
            styles
                .into_iter()
                .map(|(name, begin, end)| (name.to_string(), codes_from_text(begin), codes_from_text(end)))
                .collect(),
        )
    }

    /// Each style in order: its name, begin codes and end codes, written as a template writes them.
    pub fn entries(&self) -> impl Iterator<Item = (&str, String, String)> + '_ {
        self.styles.iter().map(|(name, begin, end)| (name.as_str(), text_from_codes(begin), text_from_codes(end)))
    }

    /// The Styles section of a template file: after the line "Styles:", a name line and then two indented lines,
    /// the begin codes and the end codes. Lines starting with "|" are comments.
    pub fn from_template_text(text: &str) -> StyleTable {
        let lines = split_lines(text);
        let Some(start) = lines.iter().position(|l| strip(l) == "Styles:") else {
            return StyleTable::default();
        };
        let mut styles: Vec<(String, Vec<String>)> = Vec::new();
        let mut name: Option<String> = None;
        let mut codes: Vec<String> = Vec::new();
        let finish = |name: &Option<String>, codes: &[String], styles: &mut Vec<(String, Vec<String>)>| {
            if let Some(n) = name {
                let mut c = codes.to_vec();
                c.resize(2.max(c.len()), String::new());
                c.truncate(2);
                styles.push((n.clone(), c));
            }
        };
        for line in &lines[start + 1..] {
            if line.starts_with('|') || strip(line).is_empty() {
                continue;
            }
            if line.starts_with(' ') || line.starts_with('\t') {
                codes.push(strip(line).to_string());
                continue;
            }
            finish(&name, &codes, &mut styles);
            name = Some(strip(line).to_string());
            codes.clear();
        }
        finish(&name, &codes, &mut styles);
        StyleTable::new(
            styles.into_iter().map(|(n, c)| (n, codes_from_text(&c[0]), codes_from_text(&c[1]))).collect(),
        )
    }

    pub fn from_template(path: impl AsRef<Path>) -> Result<StyleTable, EngineError> {
        Ok(StyleTable::from_template_text(&latin1(&super::read_file(path.as_ref())?)))
    }

    /// The begin (`which` 0) or end (`which` 1) codes of a style; None for an unknown style or no codes.
    pub fn lookup(&self, name: &[u8], which: i64) -> Option<Vec<u8>> {
        let text = latin1(name);
        let entry = self.styles.iter().find(|s| s.0 == text).or_else(|| {
            self.folded.get(&text.to_lowercase()).and_then(|&i| self.styles.get(i))
        })?;
        let codes = match which {
            0 => &entry.1,
            1 => &entry.2,
            _ => return None,
        };
        if codes.is_empty() {
            None
        } else {
            Some(codes.clone())
        }
    }
}

/// Where the rule table a language code names is taken from.
#[derive(Clone)]
pub enum LanguageSource {
    /// Binary rule tables (.btb) in a folder.
    Folder(PathBuf),
    /// The tables shipped with BrailleMaker, compiled from their sources.
    Shipped(Rc<TableSet>),
    /// Rule tables already loaded, by rules table code (the web build, which fetches tables as they are needed). A
    /// code asked for and not loaded is noted, so the caller can load it and translate again.
    Loaded(Rc<RefCell<LoadedTables>>),
}

/// The rule tables of `LanguageSource::Loaded`.
#[derive(Default)]
pub struct LoadedTables {
    tables: HashMap<String, Rc<RuleTable>>,
    /// Codes known to have no table; they are not noted again.
    absent: HashSet<String>,
    missing: Vec<String>,
}

impl LoadedTables {
    /// Keeps the rule table for a rules table code.
    pub fn add(&mut self, code: &str, table: Rc<RuleTable>) {
        self.tables.insert(code.to_lowercase(), table);
    }

    /// Notes that a rules table code has no table: a switch to it stays on the current table, as when the code is
    /// unknown.
    pub fn add_absent(&mut self, code: &str) {
        self.absent.insert(code.to_lowercase());
    }

    /// The codes asked for since the last call that are neither loaded nor known to be absent.
    pub fn take_missing(&mut self) -> Vec<String> {
        std::mem::take(&mut self.missing)
    }

    fn get(&mut self, code: &str) -> Option<Rc<RuleTable>> {
        let key = code.to_lowercase();
        if let Some(t) = self.tables.get(&key) {
            return Some(t.clone());
        }
        if !self.absent.contains(&key) && !self.missing.contains(&key) {
            self.missing.push(key);
        }
        None
    }
}

/// Language codes to table pairs. The program's own way (`from_list`): the code is looked up exactly in the
/// translation table list; a print document takes the entry's print-to-braille rules table, a braille document its
/// braille-to-print one; "code,TABLE" names the rules table for a print document. `from_folder` is the earlier
/// reading from template file names, kept for comparisons.
#[derive(Clone)]
pub struct Languages {
    source: LanguageSource,
    /// True for a print document: the print-to-braille pair is used.
    pub print_document: bool,
    /// Codes are matched exactly (the table list) rather than as the start of a template file name.
    exact: bool,
    entries: Vec<(String, (Vec<String>, Vec<String>))>,
}

impl Languages {
    /// From the translation table list: each entry's language code with its two pairs.
    pub fn from_list(list: &TableList, source: LanguageSource) -> Languages {
        let entries = list
            .entries
            .iter()
            .map(|e| {
                let t = &e.tables;
                (e.code.to_lowercase(), (vec![t[0].clone(), t[1].clone()], vec![t[2].clone(), t[3].clone()]))
            })
            .collect();
        Languages { source, print_document: true, exact: true, entries }
    }

    pub fn from_folder(folder: impl AsRef<Path>) -> Result<Languages, EngineError> {
        let folder = folder.as_ref().to_path_buf();
        let mut names: Vec<String> = std::fs::read_dir(&folder)
            .map_err(|e| EngineError::Read(format!("{}: {e}", folder.display())))?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        let mut entries: Vec<(String, (Vec<String>, Vec<String>))> = Vec::new();
        for f in names {
            if !f.to_lowercase().ends_with(".dxt") {
                continue;
            }
            let Ok(bytes) = std::fs::read(folder.join(&f)) else { continue };
            let text = latin1(&bytes).replace("\r\n", "\n").replace('\r', "\n");
            for line in text.split('\n') {
                if let Some(rest) = line.strip_prefix("TranslationTables:") {
                    let rest = strip(rest);
                    let (p2b, b2p) = rest.split_once('/').unwrap_or((rest, ""));
                    let key = f[..f.len() - 4].to_lowercase();
                    let value = (p2b.split('+').map(String::from).collect(), b2p.split('+').map(String::from).collect());
                    match entries.iter_mut().find(|e| e.0 == key) {
                        Some(e) => e.1 = value,
                        None => entries.push((key, value)),
                    }
                    break;
                }
            }
        }
        Ok(Languages { source: LanguageSource::Folder(folder), print_document: true, exact: false, entries })
    }

    fn find(&self, code: &str) -> Option<&(Vec<String>, Vec<String>)> {
        if self.exact {
            let code = code.to_lowercase();
            return self.entries.iter().find(|e| e.0 == code).map(|e| &e.1);
        }
        let code = code.to_lowercase().replace('-', "");
        let mut hits: Vec<&String> = self.entries.iter().map(|e| &e.0).filter(|k| k.starts_with(&code)).collect();
        hits.sort_by(|a, b| (!a.ends_with('p'), *a).cmp(&(!b.ends_with('p'), *b)));
        let first = hits.first()?;
        self.entries.iter().find(|e| &e.0 == *first).map(|e| &e.1)
    }

    /// The rule table code and scan table code for a language code ("qip", or "code,table" to name the rule
    /// table).
    pub fn lookup(&self, name: &[u8]) -> Option<(String, String)> {
        let text: String = latin1(name).chars().take(30).collect();
        let (code, after) = text.split_once(',').unwrap_or((&text, ""));
        let entry = self.find(code)?;
        let pair = if self.print_document { &entry.0 } else { &entry.1 };
        if pair.len() < 2 {
            return None;
        }
        let btb = if self.print_document && !after.is_empty() { after } else { &pair[0] };
        Some((btb.to_string(), pair[1].clone()))
    }

    /// The rule table a language code switches to; None when the code is not known (the engine then stays on the
    /// current table).
    pub fn rule_table(&self, name: &[u8]) -> Result<Option<Rc<RuleTable>>, EngineError> {
        let Some((btb, _)) = self.lookup(name) else { return Ok(None) };
        match &self.source {
            LanguageSource::Folder(folder) => {
                let path = super::find_file(folder, &format!("{btb}.btb"));
                Ok(Some(Rc::new(RuleTable::read(&super::read_file(&path)?)?)))
            }
            LanguageSource::Shipped(set) => Ok(Some(set.translator(&btb)?.rules.clone())),
            LanguageSource::Loaded(loaded) => Ok(loaded.borrow_mut().get(&btb)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_from_a_template() {
        let t = StyleTable::from_template_text(
            "Name: x\nStyles:\n| a comment\nh1.\n  [l][kps]\n  [pg]\nG1Passage\n  [g1]\n  [g2]\nempty\n",
        );
        assert_eq!(t.lookup(b"h1.", 0), Some(vec![0x1c, b'l', 0x1f, 0x1c, b'k', b'p', b's', 0x1f]));
        assert_eq!(t.lookup(b"g1passage", 1), Some(vec![0x1c, b'g', b'2', 0x1f]));
        assert_eq!(t.lookup(b"empty", 0), None);
        assert_eq!(t.lookup(b"nothing", 0), None);
    }
}
