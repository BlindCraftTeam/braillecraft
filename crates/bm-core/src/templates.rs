//! The document templates shipped with BrailleMaker (templates/*.template and templates/index.tsv): a template's
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

/// The four table codes of a template's TranslationTables line ("A+B/C+D"): print-to-braille rules and scan table,
/// braille-to-print rules and scan table, in small letters.
pub fn tables(text: &str) -> Option<[String; 4]> {
    let v = setting(text, "TranslationTables")?;
    let (p2b, b2p) = v.split_once('/')?;
    let (a, b) = p2b.split_once('+')?;
    let (c, d) = b2p.split_once('+')?;
    Some([a, b, c, d].map(|s| s.trim().to_lowercase()))
}

/// A template's styles.
pub fn styles(text: &str) -> StyleTable {
    StyleTable::from_template_text(text)
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
}
