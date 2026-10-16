//! The tables shipped with BrailleCraft: a folder of table sources (.bmt) and its index (index.tsv), which names,
//! for each rules table code a document can carry, its scan table code, the table source that recreates the pair
//! and the language. A table is compiled the first time it is asked for and kept.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{read_file, EngineError, Translator};

/// One line of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    /// The rules table code a document names (small letters).
    pub rules: String,
    /// The scan table code that goes with it.
    pub scan: String,
    /// The table source's file name without ".bmt".
    pub table: String,
    pub language: String,
}

pub struct TableSet {
    folder: PathBuf,
    entries: Vec<TableEntry>,
    by_code: HashMap<String, usize>,
    compiled: RefCell<HashMap<usize, Rc<Translator>>>,
}

impl TableSet {
    /// Reads `folder`/index.tsv: lines of rules code, scan code, table name and language, separated by tabs;
    /// "#" starts a comment line.
    pub fn open(folder: impl AsRef<Path>) -> Result<TableSet, EngineError> {
        let folder = folder.as_ref().to_path_buf();
        let bytes = read_file(&folder.join("index.tsv"))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 3 {
                return Err(EngineError::BadTable(format!("index.tsv: {line}")));
            }
            entries.push(TableEntry {
                rules: f[0].trim().to_lowercase(),
                scan: f[1].trim().to_lowercase(),
                table: f[2].trim().to_string(),
                language: f.get(3).map_or("", |s| s.trim()).to_string(),
            });
        }
        // A rules code listed more than once (with another scan table) is found by its first line.
        let mut by_code = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            by_code.entry(e.rules.clone()).or_insert(i);
        }
        Ok(TableSet { folder, entries, by_code, compiled: RefCell::new(HashMap::new()) })
    }

    pub fn entries(&self) -> &[TableEntry] {
        &self.entries
    }

    /// The index line for a rules table code (as a document names it, whatever the case).
    pub fn entry(&self, rules_code: &str) -> Option<&TableEntry> {
        self.by_code.get(&rules_code.trim().to_lowercase()).map(|&i| &self.entries[i])
    }

    /// The translator for a rules table code, compiled from its table source the first time.
    pub fn translator(&self, rules_code: &str) -> Result<Rc<Translator>, EngineError> {
        let code = rules_code.trim().to_lowercase();
        let Some(&i) = self.by_code.get(&code) else {
            return Err(EngineError::Read(format!("no translation table {rules_code} in {}", self.folder.display())));
        };
        self.compiled_at(i)
    }

    /// The translator for a rules table code with a given scan table code (as an export step names the pair).
    pub fn translator_with_scan(&self, rules_code: &str, scan_code: &str) -> Result<Rc<Translator>, EngineError> {
        let (rules, scan) = (rules_code.trim().to_lowercase(), scan_code.trim().to_lowercase());
        let Some(i) = self.entries.iter().position(|e| e.rules == rules && e.scan == scan) else {
            return Err(EngineError::Read(format!("no translation table {rules_code}+{scan_code} in {}", self.folder.display())));
        };
        self.compiled_at(i)
    }

    fn compiled_at(&self, i: usize) -> Result<Rc<Translator>, EngineError> {
        if let Some(t) = self.compiled.borrow().get(&i) {
            return Ok(t.clone());
        }
        let path = self.folder.join(format!("{}.bmt", self.entries[i].table));
        let source = String::from_utf8(read_file(&path)?)
            .map_err(|e| EngineError::Read(format!("{}: {e}", path.display())))?;
        let t = Rc::new(Translator::from_source(&source)?);
        self.compiled.borrow_mut().insert(i, t.clone());
        Ok(t)
    }
}
