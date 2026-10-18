//! The headless session: a document held in this process over the core, with no window, answering the control
//! methods the tools call. The program's data (tables, templates, embossers) is read from the folders beside the
//! program or the build tree.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use bc_core::command_table::COMMANDS;
use bc_core::doctext::Shown;
use bc_core::dxfile::{self, DxDocument};
use bc_core::DocumentKind;
use bc_core::engine::{LanguageSource, Languages, StyleTable, TableSet, Translator};
use bc_core::filekind::FileKind;
use bc_core::format::{self, Layout, Page};
use bc_core::import_chain::{self, Files, Recipe};
use bc_core::rule_script::RuleScript;
use bc_core::table_list::TableList;
use bc_core::templates::{self, Templates};
use serde_json::{json, Value};

use crate::backend::Backend;
use crate::NEEDS_APP;

/// North American ASCII braille: the character at index p shows the cell with dots p (dot 1 = 1 ... dot 6 = 32).
pub const NABCC: &str = " a1b'k2l@cif/msp\"e3h9o6r^djg>ntq,*5<-u8v.%[$+x!&;:4\\0z7(_?w]#y)=";

/// The dots of a North American ASCII braille character (upper case as lower), 0 for a character outside the set.
pub fn dots_of(c: char) -> u8 {
    let c = c.to_ascii_lowercase();
    NABCC.find(c).map_or(0, |p| p as u8)
}

/// The Unicode braille pattern of a North American ASCII braille text.
pub fn unicode_braille(text: &str) -> String {
    text.chars().map(|c| if c == '\n' || c == '\r' { c } else { char::from_u32(0x2800 + u32::from(dots_of(c))).unwrap_or(c) }).collect()
}

/// North American ASCII braille from a text that may hold Unicode braille patterns.
pub fn ascii_braille(text: &str) -> String {
    text.chars()
        .map(|c| match c as u32 {
            0x2800..=0x283F => NABCC.chars().nth((c as u32 - 0x2800) as usize).unwrap_or(' '),
            _ => c,
        })
        .collect()
}

/// The folders the session reads: where the program's data is.
pub struct DataDirs {
    pub tables: PathBuf,
    pub templates: PathBuf,
    pub embossers: PathBuf,
}

impl DataDirs {
    /// The folders beside the running program (or the build tree); an error names what is missing.
    pub fn find() -> Result<DataDirs, String> {
        let find = |installed: &str, source: &str, marker: &str| bc_core::paths::resource_dir(installed, source, marker).ok_or_else(|| format!("the {installed} folder was not found beside the program"));
        Ok(DataDirs { tables: find("tables", "tables", "index.tsv")?, templates: find("templates", "templates", "index.tsv")?, embossers: find("embossers", "embossers", "embossers.txt")? })
    }

    /// The folders under a workspace root (tests and the build tree).
    pub fn under(root: &Path) -> DataDirs {
        DataDirs { tables: root.join("tables"), templates: root.join("templates"), embossers: root.join("embossers") }
    }
}

/// The folders an automation may read from and write to (`--automation-read-root`, `--automation-write-root`).
#[derive(Default, Clone)]
pub struct Roots {
    pub read: Vec<PathBuf>,
    pub write: Vec<PathBuf>,
}

impl Roots {
    fn allows(roots: &[PathBuf], path: &Path) -> bool {
        if roots.is_empty() {
            return true;
        }
        let full = path.canonicalize().or_else(|_| path.parent().map(|p| p.canonicalize().map(|c| c.join(path.file_name().unwrap_or_default()))).unwrap_or_else(|| Ok(path.to_path_buf()))).unwrap_or_else(|_| path.to_path_buf());
        roots.iter().any(|r| r.canonicalize().map_or(false, |r| full.starts_with(&r)))
    }
    fn check_read(&self, path: &Path) -> Result<(), String> {
        if Self::allows(&self.read, path) { Ok(()) } else { Err(format!("{} is outside the folders this session may read", path.display())) }
    }
    fn check_write(&self, path: &Path) -> Result<(), String> {
        if Self::allows(&self.write, path) { Ok(()) } else { Err(format!("{} is outside the folders this session may write", path.display())) }
    }
}

struct Shipped(PathBuf);

impl Files for Shipped {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.0.join("scan").join(name)).map_err(|e| format!("{name}: {e}"))
    }
    fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
        RuleScript::open(&self.0.join("rule-scripts").join(name)).map_err(|e| e.to_string())
    }
    fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        import_chain::UNIMAP_TABLES
            .iter()
            .map(|n| std::fs::read(self.0.join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| format!("{n}: {e}")))
            .collect()
    }
    fn word_map(&self, name: &str) -> Result<String, String> {
        std::fs::read(self.0.join("word-styles").join(format!("{name}.map"))).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("{name}: {e}"))
    }
}

/// A state of the document kept for undo and redo.
pub struct Snapshot {
    shown: Shown,
    caret: usize,
    anchor: usize,
    braille: bool,
    cells: u32,
    lines: u32,
    name: String,
    path: Option<PathBuf>,
}

/// The session's document.
pub struct Doc {
    pub braille: bool,
    pub path: Option<PathBuf>,
    pub name: String,
    pub shown: Shown,
    pub tables: [String; 4],
    pub template: String,
    pub styles: StyleTable,
    pub style_list: Vec<(String, String, String)>,
    pub numbering: [u32; 5],
    pub hyphenation: u16,
    pub initial: String,
    pub cells: u32,
    pub lines: u32,
    pub caret: usize,
    pub anchor: usize,
    pub dirty: bool,
}

/// An in-process session.
pub struct Headless {
    dirs: DataDirs,
    tables: Rc<TableSet>,
    list: TableList,
    templates: Templates,
    doc: Doc,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    roots: Roots,
}

impl Headless {
    /// A session with a new print document from the default template.
    pub fn open(dirs: DataDirs) -> Result<Headless, String> {
        let tables = Rc::new(TableSet::open(&dirs.tables).map_err(|e| format!("tables: {e:?}"))?);
        let list = TableList::read(dirs.tables.join("languages.tsv"))?;
        let templates = Templates::open(&dirs.templates)?;
        let mut s = Headless { dirs, tables, list, templates, doc: Doc::empty(false), undo: Vec::new(), redo: Vec::new(), roots: Roots::default() };
        s.new_document(false, None)?;
        Ok(s)
    }

    /// Confines the files the session reads and writes.
    pub fn with_roots(mut self, roots: Roots) -> Headless {
        self.roots = roots;
        self
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    fn files(&self) -> Shipped {
        Shipped(self.dirs.tables.clone())
    }

    fn template_text(&self, name: &str) -> Result<String, String> {
        let entry = self.templates.by_name(name).ok_or_else(|| format!("no template named {name}"))?;
        self.templates.text(entry)
    }

    fn new_document(&mut self, braille: bool, template: Option<&str>) -> Result<(), String> {
        let name = template.unwrap_or(templates::BUILT_IN_DEFAULT).to_string();
        let text = self.template_text(&name)?;
        let mut doc = Doc::empty(braille);
        doc.template = name;
        doc.apply_template(&text);
        self.doc = doc;
        self.undo.clear();
        self.redo.clear();
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        let d = &self.doc;
        Snapshot { shown: d.shown.clone(), caret: d.caret, anchor: d.anchor, braille: d.braille, cells: d.cells, lines: d.lines, name: d.name.clone(), path: d.path.clone() }
    }

    fn restore(&mut self, s: Snapshot) {
        let d = &mut self.doc;
        d.shown = s.shown;
        d.caret = s.caret;
        d.anchor = s.anchor;
        d.braille = s.braille;
        d.cells = s.cells;
        d.lines = s.lines;
        d.name = s.name;
        d.path = s.path;
        d.dirty = true;
    }

    fn remember(&mut self) {
        let snap = self.snapshot();
        self.undo.push(snap);
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.doc.dirty = true;
    }

    fn layout(&self, cells: Option<u32>, lines: Option<u32>) -> Layout {
        let d = &self.doc;
        let (c, l) = (cells.unwrap_or(d.cells).max(1) as usize, lines.unwrap_or(d.lines).max(1) as usize);
        let mut layout = if d.braille { Layout::braille(c, l) } else { Layout::print(c, l) };
        layout.numbers_even = format::Corner::from_code(d.numbering[0]);
        layout.numbers_odd = format::Corner::from_code(d.numbering[1]);
        layout.print_numbers_even = format::Corner::from_code(d.numbering[2]);
        layout.print_numbers_odd = format::Corner::from_code(d.numbering[3]);
        layout.first_number_shown = d.numbering[4].max(1) as usize;
        let file = bc_core::hyphen::file_name(d.hyphenation.min(4) as u8, !d.braille);
        if !file.is_empty() {
            if let Ok(text) = std::fs::read_to_string(self.dirs.tables.join("hyphenation").join(format!("{file}.hct"))) {
                layout.hyphen = bc_core::hyphen::HyphenTable::parse(&text).ok();
            }
        }
        layout
    }

    fn pages(&self, cells: Option<u32>, lines: Option<u32>) -> Vec<Page> {
        format::format(&self.doc.shown, &self.layout(cells, lines), Some(&self.doc.styles))
    }

    fn languages(&self, print: bool) -> Languages {
        let mut l = Languages::from_list(&self.list, LanguageSource::Shipped(self.tables.clone()));
        l.print_document = print;
        l
    }

    /// The table codes of a language by its name, when given.
    fn tables_for(&self, language: Option<&str>) -> Result<[String; 4], String> {
        let Some(name) = language else { return Ok(self.doc.tables.clone()) };
        let want = name.to_lowercase();
        let found = self.list.entries.iter().find(|e| e.name.to_lowercase() == want).or_else(|| self.list.entries.iter().find(|e| e.name.to_lowercase().contains(&want)));
        found.map(|e| e.tables.clone()).ok_or_else(|| format!("no language named {name} (see list_tables)"))
    }

    fn translator(&self, code: &str) -> Result<Rc<Translator>, String> {
        self.tables.translator(code).map_err(|e| format!("table {code}: {e:?}"))
    }

    /// Print text with codes to braille with the document's (or a language's) forward tables: the document's
    /// initial codes in force from the start, its styles looked up, the echo of the initial codes left off.
    fn forward(&self, text: &Shown, language: Option<&str>) -> Result<Shown, String> {
        let tables = self.tables_for(language)?;
        let tr = self.translator(&tables[0])?;
        let lead = Shown::marked(&self.doc.initial).to_running();
        let running = format!("{lead}{}", text.to_running());
        let out = tr.translate_marked(&running, Some(&self.doc.styles), Some(&self.languages(true))).map_err(|e| format!("Translation failed: {e:?}"))?;
        Ok(Shown::from_running(out.strip_prefix(lead.as_str()).unwrap_or(&out)))
    }

    fn backward(&self, text: &Shown, language: Option<&str>) -> Result<Shown, String> {
        let tables = self.tables_for(language)?;
        let tr = self.translator(&tables[2])?;
        let lead = Shown::marked(&self.doc.initial).to_running();
        let running = format!("{lead}{}", text.to_running());
        let out = tr.translate_marked(&running, Some(&self.doc.styles), Some(&self.languages(false))).map_err(|e| format!("Translation failed: {e:?}"))?;
        Ok(Shown::from_running(out.strip_prefix(lead.as_str()).unwrap_or(&out)))
    }

    /// Reads a file into a document of the kind its name and content say (or `kind`).
    fn read_file(&self, path: &Path, kind: Option<&str>, code_page: Option<&str>, template: Option<&str>) -> Result<(bool, Shown, Option<DxDocument>), String> {
        self.roots.check_read(path)?;
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let detected = bc_core::filekind::detect(&name, &bytes);
        let kind = match kind {
            Some(k) => FileKind::filters().iter().map(|f| f.0).chain([FileKind::Document, FileKind::Pef, FileKind::Markdown, FileKind::Sgml]).find(|f| format!("{f:?}").eq_ignore_ascii_case(k)).ok_or_else(|| format!("unknown import kind {k}"))?,
            None => detected,
        };
        let files = self.files();
        let unknown = 0u8;
        let page = code_page.unwrap_or("Windows-1252").to_string();
        let template_text = match template {
            Some(t) => Some(self.template_text(t)?),
            None => None,
        };
        let map = template_text.as_deref().map(|t| bc_core::word::map_file_name(&templates::styles(t).lookup(b"word:mws", 0).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default())).unwrap_or_else(|| "default".into());
        let chain = |recipe: Recipe, braille: bool| -> Result<(bool, Shown, Option<DxDocument>), String> {
            let got = import_chain::run(&bytes, &recipe, &files);
            if !got.errors.is_empty() {
                return Err(got.errors.join("\n"));
            }
            Ok((braille, Shown::from_running(&bc_core::internal::decode(&got.text)), None))
        };
        match kind {
            FileKind::Document => {
                let d = dxfile::read(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))?;
                let braille = d.kind == DocumentKind::Braille;
                Ok((braille, Shown::from_running(&d.text), Some(d)))
            }
            FileKind::Pef => {
                let xml = String::from_utf8_lossy(&bytes);
                let pages = bc_core::pef::read(&xml)?;
                let text: String = pages.iter().map(|p| p.iter().map(|r| ascii_braille(r)).collect::<Vec<_>>().join("\n")).collect::<Vec<_>>().join("\n[pg]\n");
                Ok((true, Shown::marked(&text), None))
            }
            FileKind::Pdf => {
                let styles: Vec<String> = template_text.as_deref().map(|t| templates::styles(t).entries().map(|(n, _, _)| n.to_string()).collect()).unwrap_or_default();
                let running = bc_core::pdf_import::import(&bytes, &Default::default(), &styles)?;
                Ok((false, Shown::from_running(&running), None))
            }
            FileKind::Docx => chain(Recipe::Docx(bc_core::word::Options::default(), map, unknown), false),
            FileKind::Markdown => chain(Recipe::Markdown(bc_core::word::Options::default(), unknown), false),
            FileKind::Sgml => chain(Recipe::Html(bc_core::word::Options::default(), unknown), false),
            FileKind::CodedText => chain(Recipe::CodedText(page, unknown), false),
            FileKind::FormattedText => chain(Recipe::FormattedText(unknown), false),
            FileKind::CodedBraille => chain(Recipe::CodedBraille, true),
            FileKind::Orbit => chain(Recipe::Orbit, true),
            FileKind::FormattedBraille => chain(Recipe::FormattedBraille(bc_core::braide::Settings::default()), true),
            FileKind::Rtf => Err("This document cannot be imported into BrailleCraft.  It appears to be in Rich Text Format (RTF).".into()),
            FileKind::Unsupported | FileKind::Music => Err("This document cannot be imported into BrailleCraft.  It appears to be in an unsupported file format.".into()),
            other => Err(format!("BrailleCraft cannot import {other:?} files yet.")),
        }
    }

    fn open_file(&mut self, params: &Value) -> Result<Value, String> {
        let path = PathBuf::from(params.get("path").and_then(Value::as_str).ok_or("missing `path`")?);
        let (braille, shown, file) = self.read_file(&path, params.get("kind").and_then(Value::as_str), params.get("code_page").and_then(Value::as_str), params.get("template").and_then(Value::as_str))?;
        let template = params.get("template").and_then(Value::as_str).unwrap_or(templates::BUILT_IN_DEFAULT).to_string();
        let text = self.template_text(&template)?;
        let mut doc = Doc::empty(braille);
        doc.template = template;
        doc.apply_template(&text);
        if let Some(f) = &file {
            doc.tables = f.tables.clone();
            doc.numbering = f.page_numbering;
            doc.hyphenation = f.hyphenation;
            if !f.styles.is_empty() {
                doc.style_list = f.styles.clone();
                doc.styles = style_table(&f.styles);
            }
        }
        doc.shown = shown;
        doc.path = Some(path.clone());
        doc.name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        doc.dirty = false;
        self.doc = doc;
        self.undo.clear();
        self.redo.clear();
        self.inspect()
    }

    fn insert_file(&mut self, params: &Value) -> Result<Value, String> {
        let path = PathBuf::from(params.get("path").and_then(Value::as_str).ok_or("missing `path`")?);
        let (braille, shown, _) = self.read_file(&path, params.get("kind").and_then(Value::as_str), params.get("code_page").and_then(Value::as_str), params.get("template").and_then(Value::as_str))?;
        if braille != self.doc.braille {
            return Err(if braille { "Braille files cannot be inserted into a print document.".into() } else { "Print files cannot be inserted into a braille document.".into() });
        }
        self.insert_shown(&shown);
        self.inspect()
    }

    fn insert_shown(&mut self, put: &Shown) {
        self.remember();
        let (from, to) = (self.doc.caret.min(self.doc.anchor), self.doc.caret.max(self.doc.anchor));
        let len = self.doc.shown.text.chars().count();
        let (from, to) = (from.min(len), to.min(len));
        let before = self.doc.shown.slice(0, from);
        let after = self.doc.shown.slice(to, len);
        let mut out = before;
        out.push(put);
        let caret = out.text.chars().count();
        out.push(&after);
        self.doc.shown = out;
        self.doc.caret = caret;
        self.doc.anchor = caret;
    }

    fn dx_document(&self, version: u16) -> DxDocument {
        let d = &self.doc;
        let tables = if version == 0 { dxfile::older_table_names(&d.tables, |_| false) } else { d.tables.clone() };
        DxDocument {
            kind: if d.braille { DocumentKind::Braille } else { DocumentKind::Print },
            version,
            page_numbering: d.numbering,
            translated_line: false,
            six_key: false,
            tables,
            text: d.shown.to_running(),
            styles: d.style_list.clone(),
            style_index: Vec::new(),
            coded_view: false,
            hyphenation: d.hyphenation,
            view_font: 0,
            settings: Vec::new(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
        }
    }

    fn save(&mut self, params: &Value) -> Result<Value, String> {
        let path = match params.get("path").and_then(Value::as_str) {
            Some(p) => PathBuf::from(p),
            None => self.doc.path.clone().ok_or("the document has no file yet: give `path`")?,
        };
        let path = if path.extension().is_none() { path.with_extension(if self.doc.braille { "dxb" } else { "dxp" }) } else { path };
        self.roots.check_write(&path)?;
        let bytes = dxfile::write(&self.dx_document(1));
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        self.doc.path = Some(path.clone());
        self.doc.name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.doc.dirty = false;
        Ok(json!({"path": path.display().to_string(), "bytes": bytes.len()}))
    }

    fn script(&self, name: &str) -> Result<RuleScript, String> {
        self.files().rule_script(name)
    }

    /// The text through the text export table (the LaTeX and MathJax exports), or the body as the file stores it.
    fn text_export(&self) -> Vec<u8> {
        let running = self.doc.shown.to_running();
        match self.tables.translator_with_scan("txxtxa", "binary").ok().and_then(|t| t.translate_marked(&running, None, None).ok()) {
            Some(s) => s.chars().map(|c| c as u32 as u8).collect(),
            None => dxfile::body_bytes(&running),
        }
    }

    fn export(&mut self, params: &Value) -> Result<Value, String> {
        let path = PathBuf::from(params.get("path").and_then(Value::as_str).ok_or("missing `path`")?);
        self.roots.check_write(&path)?;
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let kind = params.get("type").and_then(Value::as_str).map(|t| t.to_lowercase()).unwrap_or(ext.clone());
        let cells = params.get("cells").and_then(Value::as_u64).map(|c| c as u32);
        let lines = params.get("lines").and_then(Value::as_u64).map(|l| l as u32);
        let braille = self.doc.braille;
        let running = self.doc.shown.to_running();
        let bytes: Vec<u8> = match kind.as_str() {
            "dxp" | "dxb" | "document" => dxfile::write(&self.dx_document(1)),
            "older" => dxfile::write(&self.dx_document(0)),
            "brf" | "brl" => {
                if !braille {
                    return Err("formatted braille is written from a braille document: translate first".into());
                }
                let mut out = Vec::new();
                for page in self.pages(cells, lines) {
                    for line in &page.lines {
                        out.extend_from_slice(line.text.trim_end_matches(' ').as_bytes());
                        out.extend_from_slice(b"\r\n");
                    }
                    out.push(0x0C);
                }
                out
            }
            "pef" => {
                if !braille {
                    return Err("PEF is written from a braille document: translate first".into());
                }
                let pages = self.pages(cells, lines);
                let (c, l) = (cells.unwrap_or(self.doc.cells) as usize, lines.unwrap_or(self.doc.lines) as usize);
                bc_core::pef::write(&pages, c, l, false, &self.doc.name, &format!("braillecraft-{}", self.doc.name), "", dots_of).into_bytes()
            }
            "prf" | "formatted" => {
                let pages = self.pages(cells, lines);
                format::formatted_text_file(&pages, params.get("code_page").and_then(Value::as_str).unwrap_or("ISO-8859-1"))
            }
            "txt" | "bru" | "coded" => {
                let body = dxfile::body_bytes(&running);
                let mut scripts = [self.script(if braille { "coded-braille.rules" } else { "coded-text.rules" })?, self.script("coded-wrap.rules")?];
                bc_core::rule_script::run_all(&mut scripts, &body).map_err(|e| e.to_string())?
            }
            "lbf" | "refreshable" => bc_core::lbf::write(&running, &self.doc.style_list),
            "htm" | "html" => {
                let names = bc_core::html_export::SCRIPTS;
                let mut scripts = [self.script(names[0])?, self.script(names[1])?, self.script(names[2])?, self.script(names[3])?];
                bc_core::html_export::html(&dxfile::body_bytes(&running), &mut scripts)?
            }
            "mathjax" => {
                let middle = self.text_export();
                let body = self.script("mathjax.rules")?.run(&middle).map_err(|e| e.to_string())?;
                bc_core::html_export::with_mathjax(&body)
            }
            "tex" | "latex" => {
                let middle = self.text_export();
                self.script("latex.rules")?.run(&middle).map_err(|e| e.to_string())?
            }
            "md" | "markdown" => {
                if braille {
                    let pages: Vec<Vec<String>> = self.pages(cells, lines).iter().map(|p| p.lines.iter().map(|l| unicode_braille(&l.text)).collect()).collect();
                    bc_core::markdown::braille_pages(&pages).into_bytes()
                } else {
                    let names = bc_core::html_export::SCRIPTS;
                    let mut scripts = [self.script(names[0])?, self.script(names[1])?, self.script(names[2])?, self.script(names[3])?];
                    let page = bc_core::html_export::html(&dxfile::body_bytes(&running), &mut scripts)?;
                    bc_core::markdown::from_html(&page).into_bytes()
                }
            }
            other => return Err(format!("unknown export type {other}; types: dxp, dxb, older, brf, pef, prf, txt, bru, lbf, htm, mathjax, tex, md")),
        };
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(json!({"path": path.display().to_string(), "type": kind, "bytes": bytes.len()}))
    }

    fn inspect(&self) -> Result<Value, String> {
        let d = &self.doc;
        let pages = self.pages(None, None);
        let pos = d.caret;
        let (page, line, column) = format::locate(&pages, pos);
        let open = bc_core::status::open_styles(&d.shown, pos as i64);
        let style = format::table_at(&pages, pos).map_or_else(|| bc_core::status::style_text(&d.shown, pos as i64), |t| t.status(pos));
        let status = bc_core::status::StatusLine { style, location: Some((page, line, column)), character: bc_core::status::char_field(&d.shown, pos as i64, false), six_key: false, highlight: false, keyboard: false }.text();
        Ok(json!({
            "kind": if d.braille { "braille" } else { "print" },
            "name": d.name,
            "path": d.path.as_ref().map(|p| p.display().to_string()),
            "template": d.template,
            "tables": d.tables,
            "characters": d.shown.text.chars().count(),
            "codes": d.shown.codes.len(),
            "cells": d.cells, "lines": d.lines, "pages": pages.len(),
            "caret": d.caret, "selection": [d.caret.min(d.anchor), d.caret.max(d.anchor)],
            "page": page, "line": line, "column": column,
            "open_styles": open,
            "status": status,
            "styles": d.styles.entries().map(|(n, _, _)| n.to_string()).collect::<Vec<_>>(),
            "dirty": d.dirty,
        }))
    }

    fn select(&mut self, params: &Value) -> Result<Value, String> {
        let len = self.doc.shown.text.chars().count();
        if let Some(text) = params.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            let n = params.get("occurrence").and_then(Value::as_u64).unwrap_or(1).max(1) as usize;
            let hay: Vec<char> = self.doc.shown.text.chars().collect();
            let needle: Vec<char> = text.chars().collect();
            let mut seen = 0;
            let mut found = None;
            for start in 0..hay.len().saturating_sub(needle.len() - 1) {
                if hay[start..start + needle.len()] == needle[..] {
                    seen += 1;
                    if seen == n {
                        found = Some(start);
                        break;
                    }
                }
            }
            let start = found.ok_or_else(|| format!("occurrence {n} of {text:?} not found"))?;
            self.doc.anchor = start;
            self.doc.caret = start + needle.len();
        } else {
            let from = params.get("from").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(self.doc.anchor).min(len);
            let to = params.get("to").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(from).min(len);
            self.doc.anchor = from;
            self.doc.caret = to;
        }
        Ok(json!({"selection": [self.doc.caret.min(self.doc.anchor), self.doc.caret.max(self.doc.anchor)], "text": self.doc.shown.slice(self.doc.caret.min(self.doc.anchor), self.doc.caret.max(self.doc.anchor)).text}))
    }

    fn execute(&mut self, params: &Value) -> Result<Value, String> {
        let id = params.get("command").and_then(Value::as_i64).ok_or("missing `command` (a command id)")? as i32;
        let p = params.get("params").cloned().unwrap_or(json!({}));
        if bc_core::command_table::by_id(id).is_none() {
            return Err(format!("no command with id {id}"));
        }
        match id {
            1101 => {
                let braille = p.get("braille").and_then(Value::as_bool).unwrap_or(false);
                let template = p.get("template").and_then(Value::as_str).map(str::to_string);
                self.new_document(braille, template.as_deref())?;
                self.inspect()
            }
            1102 => self.open_file(&p),
            1105 => self.save(&p),
            1106 => self.export(&p),
            1109 => self.translate_document(),
            1201 => self.undo_redo(false, 1),
            1202 => self.undo_redo(true, 1),
            1206 => {
                if self.doc.caret == self.doc.anchor {
                    let len = self.doc.shown.text.chars().count();
                    if self.doc.caret < len {
                        self.doc.anchor = self.doc.caret + 1;
                    }
                }
                self.insert_shown(&Shown::default());
                self.inspect()
            }
            1207 => {
                self.doc.anchor = 0;
                self.doc.caret = self.doc.shown.text.chars().count();
                self.inspect()
            }
            1213 => self.insert_file(&p),
            1701..=2404 => {
                use bc_core::menu_codes::{Dynamic, Insert};
                let Some(what) = bc_core::menu_codes::insert(id) else { return Err(format!("command {id} inserts nothing")) };
                let put = match what {
                    Insert::Text(t) => Shown::marked(t),
                    Insert::Pair(a, b) => {
                        let (from, to) = (self.doc.caret.min(self.doc.anchor), self.doc.caret.max(self.doc.anchor));
                        let middle = self.doc.shown.slice(from, to);
                        let mut s = Shown::marked(a);
                        s.push(&middle);
                        s.push(&Shown::marked(b));
                        s
                    }
                    Insert::Setting(d) => match d {
                        Dynamic::Grade0 => Shown::marked("[cz]"),
                        Dynamic::Cbc => Shown::marked("[cb]"),
                        Dynamic::FlushRightLeader => Shown::marked("[fr;p~\"]"),
                        Dynamic::NewPage => Shown::marked("[pg]"),
                        Dynamic::NewReferencePage => Shown::marked("[lea]"),
                        Dynamic::TerminationSign => Shown::plain(if self.doc.braille { ",'" } else { "&@" }),
                        Dynamic::HardSpace => Shown::plain("\u{a0}"),
                    },
                };
                self.insert_shown(&put);
                self.inspect()
            }
            _ => Err(format!("command {id} {NEEDS_APP}")),
        }
    }

    fn translate_document(&mut self) -> Result<Value, String> {
        if self.doc.shown.text.trim().is_empty() {
            return Err("The file that you are trying to translate is empty.".into());
        }
        let out = if self.doc.braille { self.backward(&self.doc.shown, None)? } else { self.forward(&self.doc.shown, None)? };
        let to_braille = !self.doc.braille;
        self.remember();
        self.doc.braille = to_braille;
        self.doc.shown = out;
        self.doc.caret = 0;
        self.doc.anchor = 0;
        self.doc.path = None;
        let stem = self.doc.name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_else(|| self.doc.name.clone());
        self.doc.name = if stem.is_empty() { String::new() } else { format!("{stem}.{}", if to_braille { "brf" } else { "txt" }) };
        if to_braille {
            self.doc.cells = 40;
            self.doc.lines = 25;
        }
        self.inspect()
    }

    fn undo_redo(&mut self, redo: bool, steps: usize) -> Result<Value, String> {
        for _ in 0..steps.max(1) {
            let (from, to) = if redo { (&mut self.redo, &mut self.undo) } else { (&mut self.undo, &mut self.redo) };
            let Some(snap) = from.pop() else { break };
            let now = Snapshot { shown: self.doc.shown.clone(), caret: self.doc.caret, anchor: self.doc.anchor, braille: self.doc.braille, cells: self.doc.cells, lines: self.doc.lines, name: self.doc.name.clone(), path: self.doc.path.clone() };
            to.push(now);
            self.restore(snap);
        }
        self.inspect()
    }

    fn commands(&self) -> Value {
        Value::Array(
            COMMANDS
                .iter()
                .map(|c| {
                    let headless = matches!(c.id, 1101 | 1102 | 1105 | 1106 | 1109 | 1201 | 1202 | 1206 | 1207 | 1213 | 1701..=2404);
                    json!({"id": c.id, "label": c.label, "menu": c.menu, "shortcut": c.shortcut, "headless": headless})
                })
                .collect(),
        )
    }

    fn answer(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let q = params.get("query").and_then(Value::as_str).map(|q| q.to_lowercase());
        let matches = |s: &str| q.as_ref().is_none_or(|q| s.to_lowercase().contains(q.as_str()));
        match method {
            "ping" => Ok(json!("pong")),
            "engine.commands" => Ok(self.commands()),
            "engine.execute" => self.execute(&params),
            "document.new" => {
                let braille = params.get("braille").and_then(Value::as_bool).unwrap_or(false);
                let template = params.get("template").and_then(Value::as_str).map(str::to_string);
                self.new_document(braille, template.as_deref())?;
                self.inspect()
            }
            "document.open" => self.open_file(&params),
            "document.insert_file" => self.insert_file(&params),
            "document.save" => self.save(&params),
            "document.export" => self.export(&params),
            "document.text" => Ok(json!({"text": self.doc.shown.text, "kind": if self.doc.braille { "braille" } else { "print" }})),
            "document.inspect" => self.inspect(),
            "text.insert" => {
                let text = params.get("text").and_then(Value::as_str).ok_or("missing `text`")?;
                if let Some(at) = params.get("at").and_then(Value::as_u64) {
                    let at = (at as usize).min(self.doc.shown.text.chars().count());
                    self.doc.caret = at;
                    self.doc.anchor = at;
                }
                let put = if self.doc.braille { Shown::marked(&ascii_braille(text)) } else { Shown::marked(text) };
                self.insert_shown(&put);
                self.inspect()
            }
            "select" => self.select(&params),
            "translate" => {
                let language = params.get("language").and_then(Value::as_str);
                match params.get("text").and_then(Value::as_str) {
                    Some(text) => {
                        let out = self.forward(&Shown::marked(text), language)?;
                        Ok(json!({"braille": out.text, "unicode": unicode_braille(&out.text)}))
                    }
                    None => {
                        if self.doc.braille {
                            return Err("the open document is braille already; give `text`, or use back_translate".into());
                        }
                        let v = self.translate_document()?;
                        Ok(json!({"document": v, "braille": self.doc.shown.text, "unicode": unicode_braille(&self.doc.shown.text)}))
                    }
                }
            }
            "back_translate" => {
                let language = params.get("language").and_then(Value::as_str);
                match params.get("text").and_then(Value::as_str) {
                    Some(text) => {
                        let out = self.backward(&Shown::marked(&ascii_braille(text)), language)?;
                        Ok(json!({"print": out.text}))
                    }
                    None => {
                        if !self.doc.braille {
                            return Err("the open document is print; give `text`, or use translate".into());
                        }
                        let v = self.translate_document()?;
                        Ok(json!({"document": v, "print": self.doc.shown.text}))
                    }
                }
            }
            "document.format" => {
                let cells = params.get("cells").and_then(Value::as_u64).map(|c| c as u32);
                let lines = params.get("lines").and_then(Value::as_u64).map(|l| l as u32);
                let pages = self.pages(cells, lines);
                let first = params.get("first_page").and_then(Value::as_u64).unwrap_or(1).max(1) as usize - 1;
                let count = params.get("pages").and_then(Value::as_u64).unwrap_or(5) as usize;
                let shown: Vec<Value> = pages.iter().enumerate().skip(first).take(count).map(|(k, p)| json!({"page": k + 1, "lines": p.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>()})).collect();
                Ok(json!({"pages": pages.len(), "cells": cells.unwrap_or(self.doc.cells), "lines": lines.unwrap_or(self.doc.lines), "shown": shown}))
            }
            "emboss.bytes" => {
                if !self.doc.braille {
                    return Err("an emboss job is made from a braille document: translate first".into());
                }
                let cells = params.get("cells").and_then(Value::as_u64).map_or(self.doc.cells, |c| c as u32);
                let lines = params.get("lines").and_then(Value::as_u64).map_or(self.doc.lines, |l| l as u32);
                let copies = params.get("copies").and_then(Value::as_u64).unwrap_or(1).max(1) as u32;
                let pages = self.pages(Some(cells), Some(lines));
                let sides: Vec<Option<Vec<String>>> = pages.iter().map(|p| Some(p.lines.iter().map(|l| l.text.clone()).collect())).collect();
                let options = bc_core::emboss::Options { cells, lines, ..bc_core::emboss::Options::default() };
                let bytes = bc_core::emboss::job(&options, &sides, copies);
                let mut result = json!({"bytes": bytes.len(), "pages": pages.len(), "base64": crate::base64(&bytes)});
                if let Some(port) = params.get("port").and_then(Value::as_str).filter(|p| !p.is_empty()) {
                    let path = Path::new(port);
                    self.roots.check_write(path)?;
                    std::fs::write(path, &bytes).map_err(|e| format!("{port}: {e}"))?;
                    result["sent_to"] = json!(port);
                }
                Ok(result)
            }
            "tables.list" => Ok(Value::Array(self.list.entries.iter().filter(|e| matches(&e.name)).map(|e| json!({"language": e.name, "tables": e.tables})).collect())),
            "templates.list" => Ok(Value::Array(self.templates.names_sorted().into_iter().filter(|n| matches(n)).map(Value::String).collect())),
            "embossers.list" => {
                let catalog = bc_core::embossers::Catalog::read(&self.dirs.embossers.join("embossers.txt"), None)?;
                Ok(Value::Array(catalog.names().into_iter().filter(|n| matches(n)).map(Value::String).collect()))
            }
            "undo" => self.undo_redo(false, params.get("steps").and_then(Value::as_u64).unwrap_or(1) as usize),
            "redo" => self.undo_redo(true, params.get("steps").and_then(Value::as_u64).unwrap_or(1) as usize),
            m if m.starts_with("ui.") => Err(format!("`{m}` {NEEDS_APP}")),
            other => Err(format!("unknown method `{other}`")),
        }
    }
}

/// A document's style list as the engine looks it up.
fn style_table(styles: &[(String, String, String)]) -> StyleTable {
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
    StyleTable::new(styles.iter().map(|(n, b, e)| (n.clone(), bytes(b), bytes(e))).collect())
}

impl Doc {
    fn empty(braille: bool) -> Doc {
        Doc {
            braille,
            path: None,
            name: String::new(),
            shown: Shown::default(),
            tables: Default::default(),
            template: String::new(),
            styles: StyleTable::default(),
            style_list: Vec::new(),
            numbering: [0; 5],
            hyphenation: 2,
            initial: String::new(),
            cells: if braille { 40 } else { 80 },
            lines: if braille { 25 } else { 50 },
            caret: 0,
            anchor: 0,
            dirty: false,
        }
    }

    /// The template's tables, styles, page numbering and hyphenation.
    fn apply_template(&mut self, text: &str) {
        if let Some(t) = templates::tables(text) {
            self.tables = t;
        }
        self.styles = templates::styles(text);
        self.style_list = self.styles.entries().map(|(n, b, e)| (n.to_string(), b, e)).collect();
        self.numbering = templates::page_numbers(text);
        self.hyphenation = templates::hyphenation(text);
    }
}

impl Backend for Headless {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let params = if params.is_null() { json!({}) } else { params };
        self.answer(method, params)
    }
    fn has_ui(&self) -> bool {
        false
    }
    fn describe(&self) -> String {
        "headless session (no window)".into()
    }
}
