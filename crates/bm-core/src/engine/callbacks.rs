//! What the engine asks of the document around it: the codes a style stands for, and which tables a language code
//! switches to.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::chars::{latin1, split_lines, strip};
use super::EngineError;

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

/// Language codes to table pairs, from the templates in a folder: each template's TranslationTables line names
/// the print-to-braille pair and the braille-to-print pair ("P2B_BTB+P2B_SCT/B2P_BTB+B2P_SCT"); a language code is
/// found as the start of a template's file name, a print-to-braille template (name ending in "p") first.
#[derive(Debug, Clone)]
pub struct Languages {
    folder: PathBuf,
    /// True for a print document: the print-to-braille pair is used.
    pub print_document: bool,
    entries: Vec<(String, (Vec<String>, Vec<String>))>,
}

impl Languages {
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
        Ok(Languages { folder, print_document: true, entries })
    }

    fn find(&self, code: &str) -> Option<&(Vec<String>, Vec<String>)> {
        let code = code.to_lowercase().replace('-', "");
        let mut hits: Vec<&String> = self.entries.iter().map(|e| &e.0).filter(|k| k.starts_with(&code)).collect();
        hits.sort_by(|a, b| (!a.ends_with('p'), *a).cmp(&(!b.ends_with('p'), *b)));
        let first = hits.first()?;
        self.entries.iter().find(|e| &e.0 == *first).map(|e| &e.1)
    }

    /// The rule table and scan table for a language code ("qip", or "code,table" to name the rule table).
    pub fn lookup(&self, name: &[u8]) -> Option<(PathBuf, PathBuf)> {
        let text: String = latin1(name).chars().take(30).collect();
        let (code, after) = text.split_once(',').unwrap_or((&text, ""));
        let entry = self.find(code)?;
        let pair = if self.print_document { &entry.0 } else { &entry.1 };
        if pair.len() < 2 {
            return None;
        }
        let btb = if self.print_document && !after.is_empty() { after } else { &pair[0] };
        Some((
            super::find_file(&self.folder, &format!("{btb}.btb")),
            super::find_file(&self.folder, &format!("{}.sct", pair[1])),
        ))
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
