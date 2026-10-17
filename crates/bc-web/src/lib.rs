//! BrailleCraft in the browser (queue line 11): the core's translation, document files, braille page layout and
//! embossing, called from the web interface. The browser has no folder of tables to open, so the page fetches the
//! shipped table sources and hands them in as text.

use bc_core::doctext::Shown;
use bc_core::dxfile::{self, DxDocument};
use bc_core::engine::{LanguageSource, Languages, Translator};
use bc_core::printer_setup::{parse_margin, PrinterSetup, PAPERS};
use bc_core::DocumentKind;
use wasm_bindgen::prelude::*;

fn error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

// ------------------------------------------------------------------ the editor's codes

/// The editor's text with its codes kept apart from it, as the desktop program keeps them (bc_core::doctext): a code
/// is a code because the program put it in (a menu code, a style, a document file's codes), never because the text
/// has brackets in it. `known` holds text taken out or copied with codes, `taken` codes that went, as the desktop's
/// own. The browser counts positions in UTF-16 units; they are converted here.
#[derive(Default)]
struct Tracker {
    shown: Shown,
    known: Vec<Shown>,
    taken: Vec<(usize, Shown)>,
    /// The editor's text converted cell for cell (`editor_alias`).
    alias: Option<String>,
    /// Texts made from the editor's since its last change, with its codes: a line of it (`editor_slice`) and braille
    /// conversions of a known text (`convert_braille`), newest last.
    derived: Vec<Shown>,
}

thread_local! {
    static EDITOR: std::cell::RefCell<Tracker> = std::cell::RefCell::new(Tracker::default());
}

/// A UTF-16 position in `text` as a character position.
fn char_pos(text: &str, utf16: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in text.chars().enumerate() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16() as u32;
    }
    text.chars().count()
}

/// A character position in `text` as a UTF-16 position.
fn utf16_pos(text: &str, chars: usize) -> u32 {
    text.chars().take(chars).map(|c| c.len_utf16() as u32).sum()
}

/// `text` (editor form) with the editor's codes when it is the editor's text or the conversion of it declared with
/// `editor_alias`; else every bracketed run in it is a code (text the program wrote itself).
fn shown_of(text: &str) -> Shown {
    tracked(text).unwrap_or_else(|| Shown::marked(text))
}

/// `text` with the editor's codes, when it is the editor's text, its declared conversion, or a text made from it
/// (`derived`).
fn tracked(text: &str) -> Option<Shown> {
    EDITOR.with(|e| {
        let e = e.borrow();
        let ours = &e.shown;
        if ours.text == text {
            Some(ours.clone())
        } else if e.alias.as_deref() == Some(text) {
            Some(Shown { text: text.to_string(), codes: ours.codes.clone(), inner: ours.inner.clone() })
        } else {
            e.derived.iter().rev().find(|d| d.text == text).cloned()
        }
    })
}

/// Keeps a text made from the editor's, with its codes (the last few).
fn derive(s: Shown) {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        e.derived.retain(|d| d.text != s.text);
        e.derived.push(s);
        if e.derived.len() > 8 {
            e.derived.remove(0);
        }
    })
}

// ------------------------------------------------------------------ importing files

thread_local! {
    /// The files the import chains read, as the page fetched them, by name.
    static IMPORT_FILES: std::cell::RefCell<std::collections::HashMap<String, Vec<u8>>> = Default::default();
}

/// The files the import chains read, for the page to fetch and hand in with `import_add_file`: "scan/<name>",
/// "rule-scripts/<name>" and "unimap/<name>" under tables/.
#[wasm_bindgen]
pub fn import_file_names() -> Vec<String> {
    let scan = bc_core::import_chain::SCAN_TABLES.iter().map(|n| format!("scan/{n}"));
    let maps = bc_core::import_chain::UNIMAP_TABLES.iter().map(|n| format!("unimap/{n}"));
    scan.chain(bc_core::import_chain::RULE_SCRIPTS.iter().map(|n| format!("rule-scripts/{n}"))).chain(maps).collect()
}

/// A file the import chains read (a name from `import_file_names`, without its folder) and its bytes.
#[wasm_bindgen]
pub fn import_add_file(name: &str, bytes: &[u8]) {
    let name = name.rsplit('/').next().unwrap_or(name).to_string();
    IMPORT_FILES.with(|f| f.borrow_mut().insert(name, bytes.to_vec()));
}

struct Fetched;

impl bc_core::import_chain::Files for Fetched {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
        IMPORT_FILES.with(|f| f.borrow().get(name).cloned()).ok_or_else(|| format!("{name} was not found."))
    }
    fn rule_script(&self, name: &str) -> Result<bc_core::rule_script::RuleScript, String> {
        let bytes = self.scan_table(name)?;
        bc_core::rule_script::RuleScript::from_bytes(&bytes, None).map_err(|e| e.to_string())
    }
    fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        bc_core::import_chain::UNIMAP_TABLES.iter().map(|n| self.scan_table(n).map(|b| (n.to_string(), b))).collect()
    }
}

/// The import filters in the order the import dialog lists them, one per line: the kind (as `detect_file` names it),
/// a tab, the name, a tab, and its marks: "c" asks for a code page, "e" for a braille encoding, "b" makes a braille
/// document.
#[wasm_bindgen]
pub fn import_filters() -> String {
    use bc_core::filekind::FileKind;
    FileKind::filters()
        .iter()
        .map(|(k, name)| {
            let marks = [(k.asks_code_page(), 'c'), (k.asks_braille_encoding(), 'e'), (k.is_braille(), 'b')];
            format!("{k:?}\t{name}\t{}", marks.iter().filter(|m| m.0).map(|m| m.1).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The code page a text file is imported in when none has been chosen: ISO-8859-1, as for a system that is not
/// one of the four the original program names.
#[wasm_bindgen]
pub fn default_import_code_page() -> String {
    bc_core::codepages::default_import(None).to_string()
}

/// The kind of a file from its name and its first bytes, as File, Open finds it ("Document", "Pef", "CodedText",
/// "FormattedBraille", "Rtf" and so on).
#[wasm_bindgen]
pub fn detect_file(name: &str, bytes: &[u8]) -> String {
    format!("{:?}", bc_core::filekind::detect(name, bytes))
}

/// A file read through the import chain of its kind.
#[wasm_bindgen]
pub struct Imported {
    running: String,
    errors: String,
    long_page: bool,
    wide_line: bool,
}

#[wasm_bindgen]
impl Imported {
    /// The document text in running form (codes marked 0x1C ... 0x1F).
    #[wasm_bindgen(getter)]
    pub fn running(&self) -> String {
        self.running.clone()
    }
    /// The steps that failed, one per line (empty when none did).
    #[wasm_bindgen(getter)]
    pub fn errors(&self) -> String {
        self.errors.clone()
    }
    /// A formatted braille page had more lines than the importer's lines per page.
    #[wasm_bindgen(getter)]
    pub fn long_page(&self) -> bool {
        self.long_page
    }
    /// A formatted braille line had more cells than the importer's cells per line.
    #[wasm_bindgen(getter)]
    pub fn wide_line(&self) -> bool {
        self.wide_line
    }
}

/// A coded text, coded braille, formatted braille or Orbit file through its import chain (the files from
/// `import_file_names` handed in first). `kind` is the kind as `detect_file` names it; `code_page` and `unknown` (0 "*", 1
/// nothing, 2 "{U+XXXX}") are for coded text; formatted braille comes in North American braille ASCII with
/// `braide` = odd and even main page corners, odd and even reference page corners, header, footer, lines, cells and
/// 1 to keep its formatting.
#[wasm_bindgen]
pub fn import_file(bytes: &[u8], kind: &str, code_page: &str, unknown: u8, braide: Vec<i32>) -> Result<Imported, JsError> {
    use bc_core::filekind::FileKind;
    use bc_core::import_chain::Recipe;
    let filter = kind;
    let kind = FileKind::filters().iter().find(|f| format!("{:?}", f.0) == kind).map(|f| f.0);
    let recipe = match kind {
        Some(FileKind::CodedText) => Recipe::CodedText(code_page.to_string(), unknown),
        Some(FileKind::FormattedText) => Recipe::FormattedText(unknown),
        Some(FileKind::CodedBraille) => Recipe::CodedBraille,
        Some(FileKind::Orbit) => Recipe::Orbit,
        Some(FileKind::FormattedBraille) => {
            let at = |k: usize, d: i32| braide.get(k).copied().unwrap_or(d);
            let mut s = bc_core::braide::Settings::default();
            s.odd_main = at(0, s.odd_main);
            s.even_main = at(1, s.even_main);
            s.odd_reference = at(2, s.odd_reference);
            s.even_reference = at(3, s.even_reference);
            s.header = at(4, s.header);
            s.footer = at(5, s.footer);
            s.lines = at(6, s.lines as i32).max(1) as usize;
            s.cells = at(7, s.cells as i32).max(1) as usize;
            s.preserve = at(8, 0) != 0;
            Recipe::FormattedBraille(s)
        }
        _ => return Err(JsError::new(&format!("{filter} files are not read through an import chain."))),
    };
    let out = bc_core::import_chain::run(bytes, &recipe, &Fetched);
    Ok(Imported {
        running: bc_core::internal::decode(&out.text),
        errors: out.errors.join("\n"),
        long_page: out.warnings.long_page,
        wide_line: out.warnings.wide_line,
    })
}

/// The program's version, for the About box.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Running text (a translation's result) as the editor shows it, its codes kept for the calls given it (a braille
/// conversion for display), so braille brackets in it stay cells.
#[wasm_bindgen]
pub fn running_text(running: &str) -> String {
    let s = Shown::from_running(running);
    let text = s.text.clone();
    derive(s);
    text
}

/// Text with no codes (a laid-out line), kept as such for the calls given it.
#[wasm_bindgen]
pub fn plain_text(text: &str) -> String {
    derive(Shown::plain(text));
    text.to_string()
}

/// The editor's text from UTF-16 `from` to `to` (a line, for the translated line), with its codes kept for the
/// calls given it.
#[wasm_bindgen]
pub fn editor_slice(from: u32, to: u32) -> String {
    let part = EDITOR.with(|e| {
        let e = e.borrow();
        let t = &e.shown.text;
        e.shown.slice(char_pos(t, from), char_pos(t, to))
    });
    let text = part.text.clone();
    derive(part);
    text
}

/// Whether `text` is the editor's text with only cells outside its codes changed (braille converted between
/// encodings, cell for cell).
fn converts(ours: &Shown, text: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (ours.text.chars().collect(), text.chars().collect());
    a.len() == b.len() && ours.codes.iter().all(|&(s, t)| a[s..t] == b[s..t])
}

/// `text` is the editor's text converted cell for cell (braille in North American braille for the core, while the
/// editor shows another encoding): calls given it read the editor's codes. Answers whether it is such a conversion.
#[wasm_bindgen]
pub fn editor_alias(text: &str) -> bool {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        let ok = converts(&e.shown, text);
        e.alias = ok.then(|| text.to_string());
        ok
    })
}

/// The editor shows `text`, its text converted cell for cell (another braille encoding): the codes stay where they
/// are. Answers whether it was such a conversion; otherwise nothing changes.
#[wasm_bindgen]
pub fn editor_convert(text: &str) -> bool {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        if !converts(&e.shown, text) {
            return false;
        }
        e.shown.text = text.to_string();
        e.alias = None;
        e.derived.clear();
        true
    })
}

fn remember(t: &mut Tracker, s: Shown) {
    if s.codes.is_empty() {
        return;
    }
    t.known.retain(|k| k.text != s.text);
    t.known.push(s);
    if t.known.len() > 32 {
        t.known.remove(0);
    }
}

/// The editor takes a document's running text (codes marked 0x1C ... 0x1F, as a document file keeps them); answers
/// the text as the editor shows it.
#[wasm_bindgen]
pub fn editor_load(running: &str) -> String {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        e.shown = Shown::from_running(running);
        e.taken.clear();
        e.alias = None;
        e.derived.clear();
        e.shown.text.clone()
    })
}

/// A PDF imported as running text with codes (bc_core::pdf_import, the reference's PDF front end); `styles` are the
/// style names of the document it goes into, one per line.
#[wasm_bindgen]
pub fn import_pdf(bytes: &[u8], styles: &str) -> Result<String, JsError> {
    let styles: Vec<String> = styles.lines().map(str::to_string).collect();
    bc_core::pdf_import::import(bytes, &bc_core::pdf_import::PdfOptions::default(), &styles).map_err(error)
}

/// The editor takes text with no codes (a text or braille file, a new document).
#[wasm_bindgen]
pub fn editor_load_plain(text: &str) {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        e.shown = Shown::plain(text);
        e.taken.clear();
        e.alias = None;
        e.derived.clear();
    })
}

/// The editor's text became `text` (an edit): its codes follow it (bc_core::doctext::after_edit). Answers what is
/// left of codes the edit broke, as UTF-16 [from, to] pairs, last first: they are taken out, since a code goes as a
/// whole.
#[wasm_bindgen]
pub fn editor_edit(text: &str) -> Vec<u32> {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        if e.shown.text == text {
            return Vec::new();
        }
        let taken = std::mem::take(&mut e.taken);
        let edited = bc_core::doctext::after_edit(&e.shown, text, &e.known, &taken);
        e.shown = edited.shown;
        e.taken = edited.taken;
        e.alias = None;
        e.derived.clear();
        if let Some(r) = edited.removed {
            remember(&mut e, r);
        }
        edited.remains.iter().flat_map(|&(s, t)| [utf16_pos(text, s), utf16_pos(text, t)]).collect()
    })
}

/// Text put in at UTF-16 position `at` from running text (an inserted file): `shown` is what the editor got (the
/// running text's editor form, converted cell for cell for a braille display), and the running text's codes become
/// codes at the same places. Nothing is marked when the editor does not hold `shown` there.
#[wasm_bindgen]
pub fn editor_mark_running(at: u32, running: &str, shown: &str) {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        let from = char_pos(&e.shown.text, at);
        let put = Shown::from_running(running);
        let len = shown.chars().count();
        if put.text.chars().count() != len || e.shown.text.chars().skip(from).take(len).ne(shown.chars()) {
            return;
        }
        let codes: Vec<(usize, usize)> = put.codes.iter().map(|c| (c.0 + from, c.1 + from)).collect();
        let inner: Vec<(usize, usize)> = put.inner.iter().map(|c| (c.0 + from, c.1 + from)).collect();
        e.shown.codes.extend(codes);
        e.shown.inner.extend(inner);
        e.shown.codes.sort_unstable();
        e.shown.codes.dedup();
        e.shown.inner.sort_unstable();
        e.shown.inner.dedup();
        e.alias = None;
        e.derived.clear();
    })
}

/// The text the program put in at UTF-16 position `at` (editor form, `marked`: its bracketed runs are codes) is
/// known as codes; the editor's text already holds it.
#[wasm_bindgen]
pub fn editor_mark(at: u32, marked: &str) {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        let from = char_pos(&e.shown.text, at);
        let put = Shown::marked(marked);
        let len = put.text.chars().count();
        if e.shown.text.chars().skip(from).take(len).ne(put.text.chars()) {
            return;
        }
        let codes: Vec<(usize, usize)> = put.codes.iter().map(|c| (c.0 + from, c.1 + from)).collect();
        let inner: Vec<(usize, usize)> = put.inner.iter().map(|c| (c.0 + from, c.1 + from)).collect();
        e.shown.codes.extend(codes);
        e.shown.inner.extend(inner);
        e.shown.codes.sort_unstable();
        e.shown.codes.dedup();
        e.shown.inner.sort_unstable();
        e.shown.inner.dedup();
        e.alias = None;
        e.derived.clear();
    })
}

/// Code text the program puts in ("[p]", "[hds2]") as the editor shows it: a [p] or [l] code takes the editor's own
/// line break after it, as the desktop program's editor puts it in.
#[wasm_bindgen]
pub fn marked_text(codes: &str) -> String {
    Shown::marked(codes).text
}

/// Copy or cut of UTF-16 `from` to `to`: the part is kept with its codes, so pasting it brings them back.
#[wasm_bindgen]
pub fn editor_remember(from: u32, to: u32) {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        let (f, t) = (char_pos(&e.shown.text, from), char_pos(&e.shown.text, to));
        let part = e.shown.slice(f, t);
        remember(&mut e, part);
    })
}

/// The editor's codes, as UTF-16 [from, to] pairs (Change Case leaves them as they are).
#[wasm_bindgen]
pub fn editor_codes() -> Vec<u32> {
    EDITOR.with(|e| {
        let e = e.borrow();
        e.shown.codes.iter().flat_map(|&(s, t)| [utf16_pos(&e.shown.text, s), utf16_pos(&e.shown.text, t)]).collect()
    })
}

/// Edit, Change Case on the editor's UTF-16 range `from`..`to` (the word at the caret when they are equal), as the
/// desktop program: [range start, range end, then place and new character code for each letter that changes], all in
/// UTF-16 units; empty when nothing changes.
#[wasm_bindgen]
pub fn editor_change_case(from: u32, to: u32) -> Vec<u32> {
    EDITOR.with(|e| {
        let e = e.borrow();
        let t = &e.shown.text;
        let Some((s, end, changes)) = e.shown.change_case(char_pos(t, from), char_pos(t, to)) else { return Vec::new() };
        let mut out = vec![utf16_pos(t, s), utf16_pos(t, end)];
        for (p, c) in changes {
            out.push(utf16_pos(t, p));
            out.push(c as u32);
        }
        out
    })
}

/// Layout, Apply Last Style: the paragraph style ended last before UTF-16 position `at` in the editor, read from its
/// codes; empty when there is none.
#[wasm_bindgen]
pub fn editor_last_paragraph_style(at: u32) -> String {
    EDITOR.with(|e| {
        let e = e.borrow();
        e.shown.last_paragraph_style_before(char_pos(&e.shown.text, at)).unwrap_or_default()
    })
}

/// The editor's text in running form (codes marked 0x1C ... 0x1F), as a document file keeps it.
#[wasm_bindgen]
pub fn editor_running() -> String {
    EDITOR.with(|e| e.borrow().shown.to_running())
}

/// Document, Modify Style, Rename with "everywhere": the style's codes are renamed in the editor's text, where
/// codes are marked; text that only reads like the code is left as it is. Answers the new editor text.
#[wasm_bindgen]
pub fn editor_rename_style(old: &str, new: &str) -> String {
    EDITOR.with(|e| {
        let mut e = e.borrow_mut();
        let (o, c) = (bc_core::doctext::OPEN, bc_core::doctext::CLOSE);
        let running = e.shown.to_running().replace(&format!("{o}es~{old}{c}"), &format!("{o}es~{new}{c}")).replace(&format!("{o}ee~{old}{c}"), &format!("{o}ee~{new}{c}"));
        e.shown = Shown::from_running(&running);
        e.taken.clear();
        e.alias = None;
        e.derived.clear();
        e.shown.text.clone()
    })
}

/// A translator made from one table source (a shipped .bmt file's text).
#[wasm_bindgen]
pub struct Engine(Translator);

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<Engine, JsError> {
        Translator::from_source(source).map(Engine).map_err(error)
    }

    /// Translates a text (codes written in square brackets, as the editor shows them), giving the result in the
    /// same form: codes in brackets, a line break after each new-line and paragraph code.
    pub fn translate(&self, text: &str) -> Result<String, JsError> {
        self.0.translate(&shown_of(text).to_running()).map(|out| Shown::from_running(&out).text).map_err(error)
    }

    /// Translates a document's text (editor form) as the desktop program does: the document's initial codes in force
    /// from the start and its styles looked up by the engine; the echo of the initial codes is left off the result.
    pub fn translate_document(&self, text: &str, doc: &Document, initial: &str) -> Result<String, JsError> {
        translate_document(&self.0, text, &doc.0, initial, None).map(|r| Shown::from_running(&r).text).map_err(error)
    }

    /// As `translate_document`, the result in running form (codes marked 0x1C ... 0x1F).
    pub fn translate_document_running(&self, text: &str, doc: &Document, initial: &str) -> Result<String, JsError> {
        translate_document(&self.0, text, &doc.0, initial, None).map_err(error)
    }

    /// As `translate_document`, with the language switches of the table list, from the tables the list has loaded
    /// (`LanguageList::add_table`); a table asked for and not loaded is given by `LanguageList::missing`, to be
    /// loaded before translating again.
    /// With `running`, the result is in running form (codes marked 0x1C ... 0x1F), for the editor to take with its
    /// codes (`editor_load`).
    #[allow(clippy::too_many_arguments)]
    pub fn translate_document_with(&self, text: &str, doc: &Document, initial: &str, list: &LanguageList, braille_document: bool, running: bool) -> Result<String, JsError> {
        let mut languages = Languages::from_list(&list.0, LanguageSource::Loaded(list.1.clone()));
        languages.print_document = !braille_document;
        let r = translate_document(&self.0, text, &doc.0, initial, Some(&languages)).map_err(error)?;
        Ok(if running { r } else { Shown::from_running(&r).text })
    }
}

fn translate_document(tr: &Translator, text: &str, doc: &DxDocument, initial: &str, languages: Option<&Languages>) -> Result<String, bc_core::engine::EngineError> {
    let lead = Shown::marked(initial).to_running();
    let running = format!("{lead}{}", shown_of(text).to_running());
    let out = tr.translate_marked(&running, Some(&style_table(doc)), languages)?;
    Ok(out.strip_prefix(lead.as_str()).unwrap_or(&out).to_string())
}

/// A document's styles as the engine looks them up (the desktop program's own).
fn style_table(doc: &DxDocument) -> bc_core::engine::StyleTable {
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
    bc_core::engine::StyleTable::new(doc.styles.iter().map(|s| (s.0.clone(), bytes(&s.1), bytes(&s.2))).collect())
}

/// The initial codes with every grade code taken out and `code` put at their end (Document, Learning Tables).
#[wasm_bindgen]
pub fn regrade(initial: &str, code: &str) -> String {
    bc_core::table_list::regrade(initial, code)
}

/// The translation table sets by language (tables/languages.tsv), for Document, Translation Tables and Learning
/// Tables.
#[wasm_bindgen]
pub struct LanguageList(bc_core::table_list::TableList, std::rc::Rc<std::cell::RefCell<bc_core::engine::LoadedTables>>);

#[wasm_bindgen]
impl LanguageList {
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> Result<LanguageList, JsError> {
        bc_core::table_list::TableList::parse(text).map(|l| LanguageList(l, Default::default())).map_err(|e| JsError::new(&e))
    }

    /// The region bit of a region item of Document, Translation Tables (0 Western Hemisphere to 4 Pacific); all
    /// regions for any other.
    pub fn region_bit(item: usize) -> u32 {
        if item < 5 {
            bc_core::table_list::TableList::region_bit(item)
        } else {
            bc_core::table_list::ALL_REGIONS
        }
    }

    /// What the two menus show for a document with `tables` and `initial` codes, as the desktop program fills them,
    /// as JSON: "languages" [[entry, name]...] listed for `region` (and the document's own), "active" its entry (or
    /// null), "courses" the names of its language's learning courses, "hasSteps", and "checked" the item to mark in
    /// Learning Tables: a course number, or 2801 (Uncontracted, when the initial codes hold a grade 1 code) or 2802.
    pub fn menus(&self, region: u32, tables: Vec<String>, initial: &str) -> String {
        let l = &self.0;
        let used = <[String; 4]>::try_from(tables).ok().and_then(|t| l.entry_for(&t));
        let languages: Vec<(usize, &str)> = l.listed(region, |e| used.map(|u| u.0) == Some(e)).into_iter().map(|e| (e, l.entries[e].name.as_str())).collect();
        let (courses, has_steps, step) = match used {
            Some((entry, step)) => (l.courses(entry), !l.entries[entry].steps.is_empty(), step),
            None => (Vec::new(), false, None),
        };
        let checked = match step.and_then(|s| courses.iter().position(|c| c.steps.contains(&s))) {
            Some(c) => serde_json::json!(c),
            None if initial.contains("[g1") => serde_json::json!(2801),
            None => serde_json::json!(2802),
        };
        let names: Vec<&str> = courses.iter().map(|c| c.name.as_str()).collect();
        serde_json::json!({"languages": languages, "active": used.map(|u| u.0), "courses": names, "hasSteps": has_steps, "checked": checked})
            .to_string()
    }

    /// Loads the table a rules table code is recreated as (a shipped .bmt file's text), for language switches.
    pub fn add_table(&self, code: &str, source: &str) -> Result<(), JsError> {
        let table = Translator::from_source(source).map_err(error)?.rules();
        self.1.borrow_mut().add(code, table);
        Ok(())
    }

    /// Notes that a rules table code has no table here; a switch to it stays on the current table.
    pub fn add_absent(&self, code: &str) {
        self.1.borrow_mut().add_absent(code);
    }

    /// The rules table codes language switches asked for, since the last call, that are not loaded.
    pub fn missing(&self) -> Vec<String> {
        self.1.borrow_mut().take_missing()
    }

    /// The four tables of entry `entry`.
    pub fn tables(&self, entry: usize) -> Vec<String> {
        self.0.entries.get(entry).map(|e| e.tables.to_vec()).unwrap_or_default()
    }

    /// The learning steps of course `course` of the language of a document with `tables`, as JSON: "title"
    /// ("course - Learning Tables"), "names", "tables" (each step's print-to-braille rules table) and "selected" (the
    /// step the document uses, else the first); empty when there is no such course.
    pub fn steps(&self, tables: Vec<String>, course: usize) -> String {
        let l = &self.0;
        let Some((entry, step)) = <[String; 4]>::try_from(tables).ok().and_then(|t| l.entry_for(&t)) else { return String::new() };
        let courses = l.courses(entry);
        let Some(c) = courses.get(course) else { return String::new() };
        let e = &l.entries[entry];
        let names: Vec<&str> = c.steps.iter().map(|&s| e.steps[s].name.as_str()).collect();
        let step_tables: Vec<&str> = c.steps.iter().map(|&s| e.steps[s].table.as_str()).collect();
        let selected = step.and_then(|s| c.steps.iter().position(|&x| x == s)).unwrap_or(0);
        serde_json::json!({"title": format!("{} - Learning Tables", c.name), "names": names, "tables": step_tables, "selected": selected, "language": e.tables})
            .to_string()
    }
}

/// A print or braille document file (.dxp, .dxb), as the web interface shows it.
#[wasm_bindgen]
pub struct Document(DxDocument);

#[wasm_bindgen]
impl Document {
    /// True for a braille document, false for a print document.
    #[wasm_bindgen(getter)]
    pub fn braille(&self) -> bool {
        self.0.kind == DocumentKind::Braille
    }

    /// The text as the editor shows it: codes in square brackets.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        Shown::from_running(&self.0.text).text
    }

    /// The text in running form (codes marked 0x1C ... 0x1F), for the editor to take with its codes.
    #[wasm_bindgen(getter)]
    pub fn running(&self) -> String {
        self.0.text.clone()
    }

    /// The four table names: print to braille, its scan table, braille to print, its scan table.
    #[wasm_bindgen(getter)]
    pub fn tables(&self) -> Vec<String> {
        self.0.tables.to_vec()
    }

    /// The names of the document's styles, in its order.
    #[wasm_bindgen(getter)]
    pub fn style_names(&self) -> Vec<String> {
        self.0.styles.iter().map(|s| s.0.clone()).collect()
    }

    /// Whether the document's translated line shows, as the file keeps it.
    #[wasm_bindgen(getter)]
    pub fn translated_line(&self) -> bool {
        self.0.translated_line
    }

    /// The name of the document's embosser setup, empty for none.
    #[wasm_bindgen(getter)]
    pub fn embosser(&self) -> String {
        self.0.embosser.clone()
    }

    /// The file's bytes with `text` (editor form) in place of the document's text; everything else as read.
    pub fn save(&self, text: &str) -> Vec<u8> {
        let doc = DxDocument { text: shown_of(text).to_running(), ..self.0.clone() };
        dxfile::write(&doc)
    }

    /// The document's embosser page layout as the file keeps it (spec 25 section 10), as JSON
    /// {"layout": {form, top_margin, binding, interpoint, banner, interline}, "cells", "lines"}; empty when the file
    /// holds none.
    #[wasm_bindgen(getter)]
    pub fn embosser_layout(&self) -> String {
        let Some(l) = &self.0.layout else { return String::new() };
        let layout = Layout {
            form: l.form.clone(),
            top_margin: u32::from(l.top_margin),
            binding: u32::from(l.binding),
            interpoint: l.interpoint,
            banner: l.banner != 0,
            interline: u32::from(l.interline),
        };
        serde_json::json!({"layout": layout, "cells": l.cells, "lines": l.lines}).to_string()
    }

    /// As `save`, with the document's embosser setup name and page layout (JSON as `embosser_layout` gives it, the
    /// layout written only when it names a form), as the desktop program saves them.
    pub fn save_with(&self, text: &str, embosser: &str, layout: &str, cells: u32, lines: u32) -> Result<Vec<u8>, JsError> {
        self.save_version(text, embosser, layout, cells, lines, 1)
    }

    /// As `save_with`, in file format `version`: 1, or 0 for the older format, which names the print-to-braille
    /// tables by their older names where they have them (spec 04d-C; a web document has no initial codes in force).
    pub fn save_version(&self, text: &str, embosser: &str, layout: &str, cells: u32, lines: u32, version: u16) -> Result<Vec<u8>, JsError> {
        let tables = if version == 0 { dxfile::older_table_names(&self.0.tables, |_| false) } else { self.0.tables.clone() };
        let mut doc = DxDocument { text: shown_of(text).to_running(), embosser: embosser.to_string(), version, tables, ..self.0.clone() };
        if !layout.is_empty() {
            let l: Layout = serde_json::from_str(layout).map_err(error)?;
            if !l.form.is_empty() {
                let short = |n: u32| n.min(u32::from(u16::MAX)) as u16;
                doc.layout = Some(dxfile::DxLayout {
                    lines: short(lines),
                    top_margin: short(l.top_margin),
                    cells: short(cells),
                    binding: short(l.binding),
                    interpoint: l.interpoint,
                    banner: u8::from(l.banner),
                    interline: l.interline.min(2) as u8,
                    unknown: self.0.settings.get(108 + 11).copied().unwrap_or(0),
                    form: l.form,
                });
            }
        }
        Ok(dxfile::write(&doc))
    }

    /// Save As refreshable braille (.lbf): `text` (editor form) with the document's styles (bc_core::lbf).
    pub fn refreshable(&self, text: &str) -> Vec<u8> {
        bc_core::lbf::write(&shown_of(text).to_running(), &self.0.styles)
    }

    /// A style's start and end codes as the style dialogs show them (each code in brackets), as JSON [begin, end];
    /// empty when the document has no style of that name.
    pub fn style_codes(&self, name: &str) -> String {
        match self.0.styles.iter().find(|s| s.0 == name) {
            Some(s) => serde_json::json!([codes_for_dialog(&s.1), codes_for_dialog(&s.2)]).to_string(),
            None => String::new(),
        }
    }

    /// Document, Add Style (dialog 1025): a style named `name`, its codes typed in the dialog, or those of the style
    /// `base` it is based on where a field is left empty. The reference message when the name cannot be used, else
    /// an empty string.
    pub fn add_style(&mut self, name: &str, base: &str, begin: &str, end: &str) -> String {
        if let Err(m) = check_style_name(name, &self.style_names()) {
            return m;
        }
        let based = self.0.styles.iter().find(|s| s.0 == base).cloned();
        let take = |typed: &str, kept: Option<&String>| if typed.is_empty() { kept.cloned().unwrap_or_default() } else { Shown::marked(typed).to_running() };
        let style = (name.to_string(), take(begin, based.as_ref().map(|s| &s.1)), take(end, based.as_ref().map(|s| &s.2)));
        self.0.styles.push(style);
        String::new()
    }

    /// Document, Modify Style (dialog 1026): the style's codes from the dialog fields. A field is taken back only
    /// when it was changed, so text in a style that reads like a bracket is not made into a code by showing it.
    pub fn set_style_codes(&mut self, name: &str, begin: &str, end: &str) {
        if let Some(s) = self.0.styles.iter_mut().find(|s| s.0 == name) {
            if begin != codes_for_dialog(&s.1) {
                s.1 = Shown::marked(begin).to_running();
            }
            if end != codes_for_dialog(&s.2) {
                s.2 = Shown::marked(end).to_running();
            }
        }
    }

    /// Modify Style's Rename (dialog 1027): the style takes the name `new`.
    pub fn rename_style(&mut self, old: &str, new: &str) {
        if let Some(s) = self.0.styles.iter_mut().find(|s| s.0 == old) {
            s.0 = new.to_string();
        }
    }

    /// The document's page numbering (Document, Page Numbering): the position of braille page numbers on even and
    /// odd pages and of print page numbers on even and odd pages (0 none, then upper left, upper right, lower left,
    /// lower right), then the first page number.
    #[wasm_bindgen(getter)]
    pub fn page_numbers(&self) -> Vec<u32> {
        self.0.page_numbering.to_vec()
    }

    /// The document takes four tables (Document, Translation Tables and Learning Tables).
    pub fn set_tables(&mut self, tables: Vec<String>) {
        if let Ok(t) = <[String; 4]>::try_from(tables) {
            self.0.tables = t;
        }
    }

    /// File, Translate's new document: this document's tables, styles and page numbering, as a braille document or
    /// a print document, with `text` (editor form).
    pub fn as_kind(&self, braille: bool, text: &str) -> Document {
        Document(DxDocument {
            kind: if braille { DocumentKind::Braille } else { DocumentKind::Print },
            text: shown_of(text).to_running(),
            embosser: String::new(),
            layout: None,
            ..self.0.clone()
        })
    }

    pub fn set_page_numbers(&mut self, numbers: Vec<u32>) {
        if let Ok(n) = <[u32; 5]>::try_from(numbers) {
            self.0.page_numbering = n;
        }
    }

    /// Document, Delete Style (dialog 1047): the style leaves the document.
    pub fn remove_style(&mut self, name: &str) {
        self.0.styles.retain(|s| s.0 != name);
    }
}

/// Style codes in running text as a dialog shows them, each code in brackets (the desktop program's own).
fn codes_for_dialog(running: &str) -> String {
    running
        .chars()
        .map(|c| match c {
            bc_core::doctext::OPEN => '[',
            bc_core::doctext::CLOSE => ']',
            c => c,
        })
        .collect()
}

/// The checks Add Style makes on a new style's name, with the reference messages.
fn check_style_name(name: &str, names: &[String]) -> Result<(), String> {
    if name.is_empty() {
        return Err("The style name is empty.".into());
    }
    if name == "New Style Name" {
        return Err("You must give the style a name other than \"New Style Name\".".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return Err("The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore.".into());
    }
    if names.iter().any(|n| n == name) {
        return Err(format!("A style named {name} already exists."));
    }
    Ok(())
}

/// A new document from a template's text, as File, New makes one in the desktop program: the template's four
/// tables, page numbering and styles, `text` (editor form) as its text. The built-in default template's tables when
/// the template names none.
#[wasm_bindgen]
pub fn new_document(braille: bool, text: &str, template: &str) -> Document {
    use bc_core::templates;
    let styles = templates::styles(template)
        .entries()
        .map(|(name, begin, end)| (name.to_string(), Shown::marked(&begin).to_running(), Shown::marked(&end).to_running()))
        .collect();
    Document(DxDocument {
        kind: if braille { DocumentKind::Braille } else { DocumentKind::Print },
        version: 1,
        page_numbering: templates::page_numbers(template),
        translated_line: true,
        six_key: false,
        tables: templates::tables(template).unwrap_or_else(|| ["enuncp", "enudxp", "enuncb", "gendxb"].map(String::from)),
        text: shown_of(text).to_running(),
        styles,
        style_index: Vec::new(),
        coded_view: false,
        hyphenation: templates::hyphenation(template),
        view_font: if braille { 1203 } else { 1201 },
        settings: Vec::new(),
        embosser: String::new(),
        sections: Vec::new(),
        layout: None,
    })
}

/// The code pages Save As offers for formatted text, the first chosen every time (spec 04d-E).
#[wasm_bindgen]
pub fn code_pages() -> Vec<String> {
    bc_core::codepages::NAMES.iter().map(|s| s.to_string()).collect()
}

/// Save As formatted text (.prf): a print document laid out `cells` by `lines`, its formatted text in `code_page`; a
/// character the code page cannot hold is written '#', a hard space as a space (spec 04d-B, 04d-F).
#[wasm_bindgen]
pub fn formatted_text(text: &str, cells: u32, lines: u32, code_page: &str) -> Vec<u8> {
    let pages = laid_out(text, cells, lines, false);
    bc_core::format::formatted_text_file(&pages, code_page)
}

/// Save As formatted braille (.brf): a braille document laid out `cells` by `lines` (spec 04d-B). Each line loses its
/// trailing spaces; each character goes to its cell through `shown_cells` (JSON: the 64 cells' characters as the
/// shown encoding lists them, "aA" for a cell shown as a or A), a space and a hard space being the blank cell and a
/// character with no cell written '*'; each cell is written as `codes` gives it (one code point per cell, written as
/// one byte when it is 0 to 255 and '#' otherwise), or as Unicode braille in UTF-8 when `codes` is empty; CR LF ends
/// each line and a form feed each page.
#[wasm_bindgen]
pub fn formatted_braille(text: &str, cells: u32, lines: u32, shown_cells: &str, codes: Vec<u32>) -> Result<Vec<u8>, JsError> {
    let shown: Vec<String> = serde_json::from_str(shown_cells).map_err(error)?;
    let cell_of = |c: char| shown.iter().position(|s| s.contains(c));
    let mut out = Vec::new();
    for page in laid_out(text, cells, lines, true) {
        for line in &page.lines {
            for c in line.text.trim_end_matches(' ').chars() {
                let cell = if c == ' ' || c == '\u{a0}' { Some(0) } else { cell_of(c) };
                match cell {
                    Some(p) if codes.is_empty() => {
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(char::from_u32(0x2800 + p as u32).unwrap_or(' ').encode_utf8(&mut buf).as_bytes());
                    }
                    Some(p) => out.push(codes.get(p).copied().filter(|c| *c <= 0xFF).map_or(b'#', |c| c as u8)),
                    None => out.push(b'*'),
                }
            }
            out.extend_from_slice(b"\r\n");
        }
        out.push(0x0C);
    }
    Ok(out)
}

/// Save As coded braille (.bru) or coded text (.txt): the body as the document file stores it through the coding
/// script (`script`, coded-braille.rules or coded-text.rules) and the line wrapping script (`wrap`,
/// coded-wrap.rules), as the reference program makes these files (spec 04d-C, spec 20).
#[wasm_bindgen]
pub fn coded(text: &str, script: &[u8], wrap: &[u8]) -> Result<Vec<u8>, JsError> {
    use bc_core::rule_script::{run_all, RuleScript};
    let body = dxfile::body_bytes(&shown_of(text).to_running());
    let mut scripts = [RuleScript::from_bytes(script, None).map_err(error)?, RuleScript::from_bytes(wrap, None).map_err(error)?];
    run_all(&mut scripts, &body).map_err(error)
}

/// Save As LaTeX (.tex) for a print document (spec 22): the text through the text export table (`table`, the
/// source of table-txxtxa-binary-forward.bmt; when it cannot be used, the body as the document file stores it goes
/// on unchanged), then the LaTeX rule script (`script`, latex.rules).
#[wasm_bindgen]
pub fn latex(text: &str, table: &str, script: &[u8]) -> Result<Vec<u8>, JsError> {
    let running = shown_of(text).to_running();
    let translated = Translator::from_source(table).ok().and_then(|t| t.translate_marked(&running, None, None).ok());
    // The engine writes bytes; its text holds one character per byte.
    let middle: Vec<u8> = match translated {
        Some(s) => s.chars().map(|c| c as u32 as u8).collect(),
        None => dxfile::body_bytes(&running),
    };
    bc_core::rule_script::RuleScript::from_bytes(script, None).map_err(error)?.run(&middle).map_err(error)
}

/// Braille text (editor form) from one encoding to another, cell for cell, as Global, Internationalization changes
/// the encoding of braille documents; the codes keep their text. `from` and `to` are JSON lists of the 64 cells'
/// characters as each encoding lists them ("aA" for a cell shown as a or A); a Unicode braille cell is a cell too, and
/// a cell with no character in `to` is written as Unicode braille.
#[wasm_bindgen]
pub fn convert_braille(text: &str, from: &str, to: &str) -> Result<String, JsError> {
    let from: Vec<String> = serde_json::from_str(from).map_err(error)?;
    let to: Vec<String> = serde_json::from_str(to).map_err(error)?;
    let convert = |c: char| {
        let cell = match c as u32 {
            n @ 0x2800..=0x283F => Some((n - 0x2800) as usize),
            _ => from.iter().position(|s| s.contains(c)),
        };
        match cell {
            Some(p) => to.get(p).and_then(|s| s.chars().next()).unwrap_or_else(|| char::from_u32(0x2800 + p as u32).unwrap_or(' ')),
            None => c,
        }
    };
    let known = tracked(text);
    let shown = known.clone().unwrap_or_else(|| Shown::marked(text));
    let chars: Vec<char> = shown.text.chars().collect();
    let mut out = String::with_capacity(shown.text.len());
    let mut at = 0;
    for &(start, end) in &shown.codes {
        out.extend(chars[at.min(start)..start].iter().map(|&c| convert(c)));
        out.extend(&chars[start..end.min(chars.len())]);
        at = end;
    }
    out.extend(chars[at.min(chars.len())..].iter().map(|&c| convert(c)));
    // A conversion of the editor's text (or of a text made from it) keeps its codes for the calls given it.
    if let Some(k) = known {
        derive(Shown { text: out.clone(), codes: k.codes, inner: k.inner });
    }
    Ok(out)
}

/// The status line for the caret at `position` of a document (editor form) laid out `cells` by `lines`, as the
/// desktop program shows it: the current style, Page, Line and Column on the laid-out pages (left out in coded view,
/// `codes`), the character at the caret (with its Unicode value when `verbose`), and Keys (with H when highlighting).
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn status_line(text: &str, cells: u32, lines: u32, braille: bool, position: usize, codes: bool, verbose: bool, six_key: bool, highlight: bool) -> String {
    use bc_core::status::{char_field, style_text, StatusLine};
    let shown = shown_of(text);
    let pos = position as i64;
    StatusLine {
        style: style_text(&shown, pos),
        location: (!codes).then(|| bc_core::format::locate(&laid_out(text, cells, lines, braille), position)),
        character: char_field(&shown, pos, verbose),
        six_key,
        highlight,
        keyboard: false,
    }
    .text()
}

/// Edit, Goto: the character position of a page, line and column (from 1) on a document's laid-out pages, as the
/// status line counts them; -1 when the document has no such place.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn position_of(text: &str, cells: u32, lines: u32, braille: bool, page: usize, line: usize, column: usize) -> i64 {
    bc_core::format::position_of(&laid_out(text, cells, lines, braille), page, line, column).map_or(-1, |p| p as i64)
}

/// What a Layout menu code item inserts (bc_core::menu_codes), as JSON: {"text": codes}, {"pair": [before, after]}
/// around the selection, or {"setting": name} for one taken from Shortcut Preferences or the document (grade0, cbc,
/// flush_right_leader, new_page, new_ref_page, termination_sign, hard_space); empty for no code item.
#[wasm_bindgen]
pub fn menu_code(id: i32) -> String {
    use bc_core::menu_codes::{insert, Dynamic, Insert};
    match insert(id) {
        Some(Insert::Text(s)) => serde_json::json!({ "text": s }).to_string(),
        Some(Insert::Pair(a, b)) => serde_json::json!({ "pair": [a, b] }).to_string(),
        Some(Insert::Setting(d)) => {
            let name = match d {
                Dynamic::Grade0 => "grade0",
                Dynamic::Cbc => "cbc",
                Dynamic::FlushRightLeader => "flush_right_leader",
                Dynamic::NewPage => "new_page",
                Dynamic::NewReferencePage => "new_ref_page",
                Dynamic::TerminationSign => "termination_sign",
                Dynamic::HardSpace => "hard_space",
            };
            serde_json::json!({ "setting": name }).to_string()
        }
        None => String::new(),
    }
}

/// The code vocabulary of Layout, Code List, as JSON [[code, description], ...].
#[wasm_bindgen]
pub fn code_list() -> String {
    serde_json::to_string(bc_core::menu_codes::CODE_LIST).unwrap_or_default()
}

/// The page (from 1) on which a character position of a document laid out `cells` by `lines` falls.
#[wasm_bindgen]
pub fn page_of(text: &str, cells: u32, lines: u32, braille: bool, position: usize) -> usize {
    bc_core::format::locate(&laid_out(text, cells, lines, braille), position).0
}

/// The lines of a document's chosen pages (indexes, -1 for a blank page; empty for every page), as JSON
/// [[line, ...] or null for a blank page], for File Print.
#[wasm_bindgen]
pub fn page_lines(text: &str, cells: u32, lines: u32, braille: bool, pages: Vec<i32>) -> String {
    let laid = laid_out(text, cells, lines, braille);
    let side = |p: &bc_core::format::Page| serde_json::json!(p.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>());
    let out: Vec<serde_json::Value> = if pages.is_empty() {
        laid.iter().map(side).collect()
    } else {
        pages.iter().map(|&i| usize::try_from(i).ok().and_then(|i| laid.get(i)).map_or(serde_json::Value::Null, side)).collect()
    };
    serde_json::Value::Array(out).to_string()
}

/// Reads a .dxp or .dxb file.
#[wasm_bindgen]
pub fn read_document(bytes: &[u8]) -> Result<Document, JsError> {
    dxfile::read(bytes).map(Document).map_err(error)
}

/// Braille text (North American ASCII braille, codes in square brackets) laid out in pages of `cells` by `lines`:
/// the lines of each page with CR LF, a form feed after every page (the formatted text of a braille file).
#[wasm_bindgen]
pub fn layout_braille(text: &str, cells: usize, lines: usize) -> String {
    let pages = bc_core::format::format(&shown_of(text), &bc_core::format::Layout::braille(cells, lines), None);
    bc_core::format::formatted_text(&pages)
}

/// The bytes of an embossing job on the plain (Generic) driver: braille text laid out in pages of `cells` by
/// `lines`, `copies` times, as Write to File gives them.
#[wasm_bindgen]
pub fn emboss_job(text: &str, cells: u32, lines: u32, copies: u32) -> Vec<u8> {
    let pages = bc_core::format::format(&shown_of(text), &bc_core::format::Layout::braille(cells as usize, lines as usize), None);
    let sides: Vec<Option<Vec<String>>> = pages.iter().map(|p| Some(p.lines.iter().map(|l| l.text.clone()).collect())).collect();
    let options = bc_core::emboss::Options { cells, lines, ..bc_core::emboss::Options::default() };
    bc_core::emboss::job(&options, &sides, copies)
}

thread_local! {
    /// The active document's page numbering (the five values of Document, Page Numbering), which its pages are laid
    /// out with.
    static NUMBERING: std::cell::Cell<[u32; 5]> = const { std::cell::Cell::new([0; 5]) };
}

/// Sets the page numbering the active document's pages are laid out with: braille page numbers on even pages and
/// on odd pages, print page numbers on even and odd pages, and the first page whose number is shown.
#[wasm_bindgen]
pub fn set_page_numbering(numbers: Vec<u32>) {
    let mut n = [0u32; 5];
    for (k, v) in numbers.into_iter().take(5).enumerate() {
        n[k] = v;
    }
    NUMBERING.with(|c| c.set(n));
}

/// A document's pages: a braille document in braille pages, a print document in print pages, with the active
/// document's page numbering (as the desktop program lays them out for the view, File Print and File Emboss).
fn laid_out(text: &str, cells: u32, lines: u32, braille: bool) -> Vec<bc_core::format::Page> {
    let (c, l) = (cells.max(1) as usize, lines.max(1) as usize);
    let mut layout = if braille { bc_core::format::Layout::braille(c, l) } else { bc_core::format::Layout::print(c, l) };
    let n = NUMBERING.with(|c| c.get());
    layout.numbers_even = bc_core::format::Corner::from_code(n[0]);
    layout.numbers_odd = bc_core::format::Corner::from_code(n[1]);
    layout.print_numbers_even = bc_core::format::Corner::from_code(n[2]);
    layout.print_numbers_odd = bc_core::format::Corner::from_code(n[3]);
    layout.first_number_shown = n[4].max(1) as usize;
    bc_core::format::format(&shown_of(text), &layout, None)
}

/// How many pages a document takes laid out `cells` by `lines`.
#[wasm_bindgen]
pub fn page_count(text: &str, cells: u32, lines: u32, braille: bool) -> usize {
    laid_out(text, cells, lines, braille).len()
}

/// A page range as the Print and Emboss dialogs take it ("1-7,b,9"), for a document of `page_count` pages, as JSON:
/// {"kind": "pages" | "malformed" | "empty" | "none", "pages": [index or -1 for a blank page], "missing", "repeated"}.
#[wasm_bindgen]
pub fn parse_range(text: &str, page_count: usize) -> String {
    use bc_core::range::Range;
    let (range, missing, repeated) = bc_core::range::parse_range(text, page_count);
    let (kind, pages) = match range {
        Range::Pages(p) => ("pages", p.iter().map(|x| x.map_or(-1, |n| n as i64)).collect()),
        Range::Malformed => ("malformed", Vec::new()),
        Range::Empty => ("empty", Vec::new()),
        Range::NoneFound => ("none", Vec::new()),
    };
    serde_json::json!({"kind": kind, "pages": pages, "missing": missing, "repeated": repeated}).to_string()
}

/// The embosser models (embossers.txt, and the user's own models if any), for Embosser Setup and File Emboss.
#[wasm_bindgen]
pub struct Embossers(bc_core::embossers::Catalog);

#[wasm_bindgen]
impl Embossers {
    #[wasm_bindgen(constructor)]
    pub fn new(shipped: &str, user: Option<String>) -> Result<Embossers, JsError> {
        bc_core::embossers::Catalog::from_texts(shipped, user.as_deref()).map(Embossers).map_err(error)
    }

    /// Every model as the setup lists it, sort-to-top models first.
    pub fn names(&self) -> Vec<String> {
        self.0.names()
    }

    /// A new setup of a listed model with the model's own values, as JSON (the shape the desktop program keeps).
    pub fn new_setup(&self, label: &str, european: bool) -> String {
        serde_json::to_string(&Setup::for_model(&self.0, label, european)).unwrap_or_default()
    }

    /// The forms of a listed model as JSON: [{"id", "name", "cells", "lines"}], cells and lines being the most the
    /// form takes.
    pub fn forms(&self, label: &str) -> String {
        let forms: Vec<_> = find_model(&self.0, label)
            .map(|m| m.forms.iter().map(|f| serde_json::json!({"id": f.id, "name": form_name(&f.id), "cells": f.maximum.0, "lines": f.maximum.1})).collect())
            .unwrap_or_default();
        serde_json::Value::Array(forms).to_string()
    }

    /// A model's driver name (Generic, Index, Enabling and the like).
    pub fn driver(&self, label: &str) -> String {
        find_model(&self.0, label).map(|m| m.driver().to_string()).unwrap_or_default()
    }

    /// Whether a model has a flag (CanDoInterpoint, TranSend and the like).
    pub fn flag(&self, label: &str, key: &str) -> bool {
        find_model(&self.0, label).is_some_and(|m| m.flag(key))
    }

    /// A setup given as JSON after a change of its model to `label` on the setup sheet (spec 25 18.3), as JSON.
    pub fn with_model(&self, setup: &str, label: &str, european: bool) -> Result<String, JsError> {
        let s: Setup = serde_json::from_str(setup).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(serde_json::to_string(&s.with_model(&self.0, label, european)).unwrap_or_default())
    }

    /// The ports Write to Port lists for a model (spec 25 16.1); none for an unknown model.
    pub fn ports(&self, label: &str) -> Vec<String> {
        bc_core::emboss_setup::ports(find_model(&self.0, label))
    }

    /// The output encodings a model offers.
    pub fn encodings(&self, label: &str) -> Vec<String> {
        find_model(&self.0, label).map(|m| m.encodings().iter().map(|s| s.to_string()).collect()).unwrap_or_default()
    }

    /// The checks before an embossing job (spec 25 9.1) for a setup and a document layout given as JSON: "" when the
    /// job may go ahead, else "stop:" or "ask:" and the message (2364 stops the job, 2362 is a Yes/No question).
    pub fn check(&self, setup: &str, layout: &str, cells: u32, lines: u32) -> Result<String, JsError> {
        use bc_core::emboss_setup::{check, does_not_fit_message, form_not_defined_message, Check};
        let (s, l) = parse(setup, layout)?;
        let Some(model) = find_model(&self.0, &s.model) else { return Ok("stop:Embosser not found.".into()) };
        let (setup_name, form) = (s.shown_name(), form_name(&l.form));
        Ok(match check(model, &l, cells, lines) {
            Check::Ready => String::new(),
            Check::FormNotDefined => format!("stop:{}", form_not_defined_message(&setup_name, &form, cells, lines)),
            Check::DoesNotFit => format!("ask:{}", does_not_fit_message(&setup_name, &form, cells, lines)),
        })
    }

    /// The bytes of an embossing job: a document (`braille` or print) laid out `cells` by `lines`, the chosen pages (indexes, -1 for a
    /// blank page; empty for every page), `copies` times, through a setup and layout given as JSON. `encoding` is the
    /// byte for each of the 64 cells in the setup's output encoding, or empty for North American English. `pass_sides` is
    /// the braille pass's sides (spec 25 19.2): 0 every side, 1 the fronts, 2 the backs.
    #[allow(clippy::too_many_arguments)]
    pub fn job(&self, setup: &str, layout: &str, text: &str, braille: bool, cells: u32, lines: u32, copies: u32, pages: Vec<i32>, encoding: Vec<u8>, pass_sides: u8) -> Result<Vec<u8>, JsError> {
        let (s, l) = parse(setup, layout)?;
        let model = find_model(&self.0, &s.model).ok_or_else(|| JsError::new("Embosser not found."))?;
        let laid = laid_out(text, cells, lines, braille);
        let side = |p: &bc_core::format::Page| p.lines.iter().map(|l| l.text.clone()).collect::<Vec<String>>();
        let sides: Vec<Option<Vec<String>>> = if pages.is_empty() {
            laid.iter().map(|p| Some(side(p))).collect()
        } else {
            pages.iter().map(|&i| usize::try_from(i).ok().and_then(|i| laid.get(i)).map(side)).collect()
        };
        let encoding = <[u8; 64]>::try_from(encoding.as_slice()).ok();
        let options = bc_core::emboss_setup::job_options(&s, model, &l, cells, lines, copies, encoding);
        let which = match pass_sides {
            1 => bc_core::emboss::Sides::Fronts,
            2 => bc_core::emboss::Sides::Backs,
            _ => bc_core::emboss::Sides::All,
        };
        // The Typewriter driver's messages go as their bytes: a browser has no FTDI library (spec 25 21).
        if options.typewriter {
            return Ok(bc_core::emboss::typewriter_job(&options, &sides, copies, which).concat());
        }
        Ok(bc_core::emboss::job_sides(&options, &sides, copies, which))
    }

    /// Whether a job through this setup and layout is embossed in two passes, fronts then backs (spec 25 19.1): a
    /// ManualInterpoint model with interpoint on (the web File Emboss dialog leaves every pass flag set).
    pub fn manual_interpoint(&self, setup: &str, layout: &str) -> Result<bool, JsError> {
        let (s, l) = parse(setup, layout)?;
        let Some(model) = find_model(&self.0, &s.model) else { return Ok(false) };
        let options = bc_core::emboss_setup::job_options(&s, model, &l, 0, 0, 1, None);
        Ok(model.flag("ManualInterpoint") && options.interpoint)
    }
}

use bc_core::emboss_setup::{find_model, form_name, Layout, Setup};

fn parse(setup: &str, layout: &str) -> Result<(Setup, Layout), JsError> {
    Ok((serde_json::from_str(setup).map_err(error)?, serde_json::from_str(layout).map_err(error)?))
}

/// Message 2892: no embosser setup chosen for the document.
#[wasm_bindgen]
pub fn no_setup_message() -> String {
    bc_core::emboss_setup::NO_SETUP_MESSAGE.to_string()
}

/// A form's description as the setup lists it.
#[wasm_bindgen(js_name = formName)]
pub fn form_name_of(id: &str) -> String {
    form_name(id)
}

/// Whether region `region` (ISO 3166 two letters) takes the European order of default forms (spec 25 12.3).
#[wasm_bindgen(js_name = a4Region)]
pub fn a4_region_of(region: &str) -> bool {
    bc_core::emboss_setup::a4_region(region)
}

/// The choice lists of the embosser setup sheet, as JSON: bauds, parities, handshaking, versions, line ends, page
/// ends, letter cases and the TranSend SE printer choices (the ports follow the model: the catalog's `ports`).
#[wasm_bindgen]
pub fn setup_lists() -> String {
    use bc_core::emboss_setup::*;
    serde_json::json!({
        "bauds": BAUDS, "parities": PARITIES, "handshaking": HANDSHAKING, "versions": VERSIONS,
        "lineEnds": LINE_ENDS, "pageEnds": PAGE_ENDS, "cases": CASES, "transendFilters": TRANSEND_FILTERS,
    })
    .to_string()
}

/// A kept Printer Setup (its settings text; empty for the defaults).
fn printer_setup_of(text: &str) -> PrinterSetup {
    PrinterSetup::from_text(text).unwrap_or_default()
}

/// Printer Setup (dialog 1071) filled from a kept setup, as JSON: the papers in the box's sorted order (a paper not
/// in the list is added by its size), the chosen one, the orientation, the four margins as shown (left, top, right,
/// bottom), and the formatted page size.
#[wasm_bindgen]
pub fn printer_setup_view(setup: &str) -> String {
    let s = printer_setup_of(setup);
    let mut papers: Vec<String> = PAPERS.iter().map(|p| p.0.to_string()).collect();
    let name = s.paper_name();
    if !papers.contains(&name) {
        papers.push(name.clone());
    }
    papers.sort();
    let margins: Vec<String> = s.margins.iter().map(|m| s.margin_text(*m)).collect();
    serde_json::json!({"papers": papers, "paper": name, "landscape": s.landscape, "margins": margins, "lines": s.lines, "chars": s.chars})
        .to_string()
}

/// The setup as Printer Setup's OK reads it: the paper by name, the orientation, and the margins as typed (the unit
/// becomes millimetres when the left margin is typed in them). The new settings text, or the reference message when
/// a margin is not a number of 0 or more.
#[wasm_bindgen]
pub fn printer_setup_read(setup: &str, paper: &str, landscape: bool, margins: Vec<String>) -> Result<String, JsError> {
    read_printer_setup(setup, paper, landscape, &margins).map_err(|m| JsError::new(m))
}

fn read_printer_setup(setup: &str, paper: &str, landscape: bool, margins: &[String]) -> Result<String, &'static str> {
    let mut s = printer_setup_of(setup);
    if let Some(p) = PAPERS.iter().find(|p| p.0 == paper) {
        s.paper = (p.1, p.2);
    }
    s.landscape = landscape;
    let read: Option<Vec<i32>> = margins.iter().map(|m| parse_margin(m, s.metric)).collect();
    match read {
        Some(m) if m.len() == 4 => {
            s.margins = [m[0], m[1], m[2], m[3]];
            s.metric = margins[0].to_lowercase().contains("mm");
            Ok(s.to_text())
        }
        _ => Err("Each margin must be a number of 0 or more, in inches (for example 1\") or in millimetres (for example 25 mm)."),
    }
}

/// Keeps the formatted page size in a setup (Global, Printer Setup): lines 5 to 160 and characters 10 to 100;
/// a value out of range is left as it was.
#[wasm_bindgen]
pub fn printer_setup_page(setup: &str, lines: u32, chars: u32) -> String {
    let mut s = printer_setup_of(setup);
    if (5..=160).contains(&lines) {
        s.lines = lines;
    }
    if (10..=100).contains(&chars) {
        s.chars = chars;
    }
    s.to_text()
}

/// The page rule the browser prints with for a setup: the paper size turned by the orientation, and the margins.
#[wasm_bindgen]
pub fn printer_page_css(setup: &str) -> String {
    let s = printer_setup_of(setup);
    let (w, h) = if s.landscape { (s.paper.1, s.paper.0) } else { (s.paper.0, s.paper.1) };
    let m = s.margins.map(|t| format!("{}in", t as f64 / 1000.0));
    format!("@page {{ size: {}mm {}mm; margin: {} {} {} {}; }}", w as f64 / 10.0, h as f64 / 10.0, m[1], m[2], m[3], m[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_emboss_on_two_pages() {
        // Three lines on pages of two lines: two pages, each ended by a form feed.
        let text = "ab[l]cd[l]ef";
        let laid = layout_braille(text, 10, 2);
        assert_eq!(laid.matches('\x0c').count(), 2);
        let job = emboss_job(text, 10, 2, 1);
        assert_eq!(job.iter().filter(|&&b| b == 0x0c).count(), 2);
        assert!(job.starts_with(b"ab\r\n"));
    }

    #[test]
    fn a_document_reads_and_saves() {
        let doc = DxDocument {
            kind: DocumentKind::Braille,
            version: 1,
            page_numbering: [0; 5],
            translated_line: false,
            six_key: false,
            tables: ["amerc".into(), "AMEDXP".into(), "AMEBPC".into(), "AMEDXB".into()],
            text: ",! cat".into(),
            styles: Vec::new(),
            style_index: Vec::new(),
            coded_view: false,
            hyphenation: 2,
            view_font: 1203,
            settings: Vec::new(),
            embosser: "Index Everest-D V5".into(),
            sections: Vec::new(),
            layout: None,
        };
        let read = read_document(&dxfile::write(&doc)).ok().unwrap();
        assert!(read.braille());
        assert_eq!((read.text(), read.embosser()), (",! cat".to_string(), "Index Everest-D V5".to_string()));
        let again = read_document(&read.save(",! dog")).ok().unwrap();
        assert_eq!(again.text(), ",! dog");
    }

    fn shipped(path: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn coded_and_latex_match_the_desktop_path() {
        use bc_core::rule_script::{run_all, RuleScript};
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables");
        let text = "[p]The cat [hds2]sat[hde] here.[l]\nNext line.";
        for (braille, script) in [(true, "coded-braille.rules"), (false, "coded-text.rules")] {
            let web = coded(text, &shipped(&format!("tables/rule-scripts/{script}")), &shipped("tables/rule-scripts/coded-wrap.rules")).ok().unwrap();
            let mut scripts = [RuleScript::open(&folder.join("rule-scripts").join(script)).unwrap(), RuleScript::open(&folder.join("rule-scripts/coded-wrap.rules")).unwrap()];
            let desktop = run_all(&mut scripts, &dxfile::body_bytes(&Shown::marked(text).to_running())).unwrap();
            assert_eq!(web, desktop, "braille {braille}");
            assert!(!web.is_empty());
        }
        let table = String::from_utf8(shipped("tables/table-txxtxa-binary-forward.bmt")).unwrap();
        let web = latex(text, &table, &shipped("tables/rule-scripts/latex.rules")).ok().unwrap();
        let set = bc_core::engine::TableSet::open(&folder).unwrap();
        let running = Shown::marked(text).to_running();
        let middle: Vec<u8> = set.translator_with_scan("txxtxa", "binary").unwrap().translate_marked(&running, None, None).unwrap().chars().map(|c| c as u32 as u8).collect();
        let desktop = RuleScript::open(&folder.join("rule-scripts/latex.rules")).unwrap().run(&middle).unwrap();
        assert_eq!(web, desktop);
        assert!(String::from_utf8_lossy(&web).contains("cat"), "{}", String::from_utf8_lossy(&web));
    }

    #[test]
    fn formatted_braille_writes_cells_in_the_chosen_encoding() {
        let na: Vec<String> = bc_core::emboss::NA_CELLS.iter().map(|&b| format!("{}{}", (b as char).to_ascii_lowercase(), b as char)).collect();
        let shown = serde_json::to_string(&na).unwrap();
        let caps: Vec<u32> = bc_core::emboss::NA_CELLS.iter().map(|&b| u32::from(b)).collect();
        // A page is written whole: its 25 lines, blank ones too, then a form feed.
        let out = formatted_braille("ab c", 40, 25, &shown, caps).ok().unwrap();
        assert!(out.starts_with(b"AB C\r\n\r\n"));
        assert_eq!((out.windows(2).filter(|w| w == b"\r\n").count(), out.last()), (25, Some(&0x0c)));
        let unicode = String::from_utf8(formatted_braille("ab", 40, 25, &shown, Vec::new()).ok().unwrap()).unwrap();
        assert!(unicode.starts_with("\u{2801}\u{2803}\r\n"), "{unicode:?}");
    }

    #[test]
    fn a_new_document_from_the_default_template_saves_in_both_formats() {
        let template = String::from_utf8(shipped("templates/english-ueb-basic.template")).unwrap_or_default();
        let doc = new_document(false, "The cat.", &template);
        for version in [1, 0] {
            let bytes = doc.save_version("The dog.", "Everest", "", 40, 25, version).ok().unwrap();
            let back = read_document(&bytes).ok().unwrap();
            assert_eq!((back.text(), back.embosser(), back.braille()), ("The dog.".to_string(), "Everest".to_string(), false));
        }
    }

    #[test]
    fn the_status_line_reads_as_the_desktop_one() {
        let text = "[es~h1.]ab[ee~h1.]\ncd";
        assert_eq!(status_line(text, 40, 25, true, 8, false, false, false, false), "Current Style: h1.  Page:1  Line:1  Column:1  a  Keys:1");
        assert_eq!(status_line(text, 40, 25, true, 8, true, true, true, true), "Current Style: h1.  a {U+0061}  Keys:6H");
        assert!(status_line(text, 40, 25, true, 21, false, false, false, false).ends_with("(end)  Keys:1"));
    }

    #[test]
    fn menu_codes_and_goto() {
        assert_eq!(menu_code(1904), r#"{"pair":["[hds]","[hde]"]}"#);
        assert_eq!(menu_code(1910), r#"{"text":"[p]"}"#);
        assert_eq!(menu_code(1701), r#"{"setting":"grade0"}"#);
        assert_eq!(menu_code(1), "");
        assert!(code_list().starts_with(r#"[["atx","ASCII is all text"]"#));
        let text = "ab[l]\ncd[l]\nef";
        assert_eq!(position_of(text, 40, 2, true, 2, 1, 1), 12);
        assert_eq!(position_of(text, 40, 2, true, 9, 1, 1), -1);
    }

    #[test]
    fn translation_gives_the_editor_form() {
        let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tables/en-ueb-g2-forward.bmt")).unwrap();
        let e = Engine::new(&source).ok().unwrap();
        let out = e.translate("The cat.\nThe mat.").ok().unwrap();
        assert!(!out.chars().any(|c| ('\u{1c}'..='\u{1f}').contains(&c)), "{out:?}");
        assert_eq!(out.lines().count(), 2, "{out:?}");
    }

    #[test]
    fn an_index_setup_checks_and_embosses_the_chosen_pages() {
        let shipped = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../embossers/embossers.txt")).unwrap();
        let emb = Embossers::new(&shipped, None).ok().unwrap();
        assert!(emb.names().iter().any(|n| n == "Index Everest-D V5"));
        let setup = emb.new_setup("Index Everest-D V5", false);
        let layout = r#"{"form":"11_w","top_margin":0,"binding":0,"interpoint":false,"banner":false,"interline":0}"#;
        assert_eq!(emb.check(&setup, layout, 40, 25).ok().unwrap(), "");
        assert!(emb.check(&setup, layout, 60, 25).ok().unwrap().starts_with("ask:"));
        let range: serde_json::Value = serde_json::from_str(&parse_range("2,b", 2)).unwrap();
        assert_eq!(range["pages"], serde_json::json!([1, -1]));
        let text = "ab[l]cd[l]ef";
        assert_eq!(page_count(text, 40, 2, true), 2);
        // Positions are the editor's: a line break follows each [l], so "e" is at 12.
        let shown = "ab[l]\ncd[l]\nef";
        assert_eq!(page_of(shown, 40, 2, true, 12), 2);
        assert_eq!(page_of(shown, 40, 2, true, 1), 1);
        let job = emb.job(&setup, layout, text, true, 40, 2, 1, vec![1], Vec::new(), 0).ok().unwrap();
        assert!(job.starts_with(b"\x1bD"), "{:?}", String::from_utf8_lossy(&job));
        assert!(String::from_utf8_lossy(&job).contains("ef"));
        assert!(!String::from_utf8_lossy(&job).contains("ab"));
    }

    #[test]
    fn a_document_translates_with_its_initial_codes_and_lists_its_language() {
        let source = String::from_utf8(shipped("tables/en-ueb-g2-forward.bmt")).unwrap();
        let tr = Translator::from_source(&source).unwrap();
        let doc = new_document(false, "", "");
        let contracted = translate_document(&tr, "The cat sat with the dog.", &doc.0, "", None).unwrap();
        let uncontracted = translate_document(&tr, "The cat sat with the dog.", &doc.0, "[g1l]", None).unwrap();
        assert_eq!(contracted, Engine(Translator::from_source(&source).unwrap()).translate("The cat sat with the dog.").ok().unwrap());
        assert_ne!(contracted, uncontracted);
        assert!(!uncontracted.contains("g1l"), "{uncontracted:?}");
        let braille = doc.as_kind(true, ",! cat");
        assert!(braille.braille() && braille.tables() == doc.tables() && braille.text() == ",! cat");
        let list = LanguageList::new(&String::from_utf8(shipped("tables/languages.tsv")).unwrap()).ok().unwrap();
        let menus: serde_json::Value = serde_json::from_str(&list.menus(LanguageList::region_bit(0), doc.tables(), "")).unwrap();
        let active = menus["active"].as_u64().unwrap() as usize;
        assert!(menus["languages"].as_array().unwrap().iter().any(|l| l[0].as_u64() == Some(active as u64)));
        assert_eq!(menus["checked"], 2802);
        let graded: serde_json::Value = serde_json::from_str(&list.menus(1, doc.tables(), &regrade("", "[g1l]"))).unwrap();
        assert_eq!(graded["checked"], 2801);
        assert_eq!(list.tables(active).len(), 4);
    }

    #[test]
    fn the_editor_keeps_codes_apart_from_brackets_in_the_text() {
        use bc_core::doctext::{CLOSE, OPEN};
        // Typed text with brackets is text; a code the program puts in is a code.
        editor_load_plain("");
        assert!(editor_edit("a [b] c").is_empty());
        // A [p] code is put in with the editor's own line break after it.
        assert_eq!(marked_text("[p]"), "[p]\n");
        let typed = "a [b] c[p]\n";
        assert!(editor_edit(typed).is_empty());
        editor_mark(7, "[p]");
        assert_eq!(editor_running(), format!("a [b] c{OPEN}p{CLOSE}"));
        assert_eq!(editor_codes(), vec![7, 10]);
        // Saved, the bracket stays text and the code a code.
        let doc = new_document(false, "", "");
        let back = read_document(&doc.save(typed)).ok().unwrap();
        assert_eq!(back.running(), format!("a [b] c{OPEN}p{CLOSE}"));
        // Taking one bracket of the code: what is left of it goes as a whole, with its line break.
        assert_eq!(editor_edit("a [b] cp]\n"), vec![9, 10, 7, 9]);
        assert_eq!(editor_edit("a [b] c"), Vec::<u32>::new());
        assert!(editor_codes().is_empty());
        // Cut and paste bring the codes back.
        editor_load(&format!("x{OPEN}hds2{CLOSE}y"));
        editor_remember(0, 8);
        assert!(editor_edit("").is_empty());
        assert!(editor_edit("x[hds2]y").is_empty());
        assert_eq!(editor_codes(), vec![1, 7]);
        // A conversion declared cell for cell reads the same codes.
        assert!(editor_alias("X[hds2]Y"));
        assert_eq!(shown_of("X[hds2]Y").codes, vec![(1, 7)]);
        assert!(!editor_alias("X[hds3]Y"));
        assert_eq!(editor_rename_style("q", "r"), "x[hds2]y");
        // A line of the editor, and a braille conversion of it, keep its codes; the code's text is not converted.
        let line = editor_slice(0, 8);
        assert_eq!(line, "x[hds2]y");
        let converted = convert_braille(&line, r#"["x","h"]"#, r#"["z","q"]"#).ok().unwrap();
        assert_eq!(converted, "z[hds2]y");
        assert_eq!(shown_of(&converted).codes, vec![(1, 7)]);
        // Brackets typed as text are converted as cells.
        editor_load_plain("[h]");
        assert_eq!(convert_braille("[h]", r#"["x","h"]"#, r#"["z","q"]"#).ok().unwrap(), "[q]");
    }

    #[test]
    fn a_language_switch_asks_for_its_table_then_uses_it() {
        let source = String::from_utf8(shipped("tables/en-ueb-g2-forward.bmt")).unwrap();
        let e = Engine(Translator::from_source(&source).unwrap());
        let doc = new_document(false, "", "");
        let list = LanguageList::new(&String::from_utf8(shipped("tables/languages.tsv")).unwrap()).ok().unwrap();
        let text = "the [lnb~qip]the[lnb] the";
        let first = translate_document(&e.0, text, &doc.0, "", Some(&web_languages(&list, false))).unwrap();
        assert_eq!(list.missing(), vec!["qipacp".to_string()]);
        list.add_table("qipacp", &String::from_utf8(shipped("tables/ipa-basic-forward.bmt")).unwrap()).ok().unwrap();
        let second = translate_document(&e.0, text, &doc.0, "", Some(&web_languages(&list, false))).unwrap();
        assert!(list.missing().is_empty());
        assert_ne!(first, second, "{first:?}");
        // A code with no table is noted once; then the switch stays on the current table without asking again.
        let fresh = LanguageList::new(&String::from_utf8(shipped("tables/languages.tsv")).unwrap()).ok().unwrap();
        fresh.add_absent("qipacp");
        let stayed = translate_document(&e.0, text, &doc.0, "", Some(&web_languages(&fresh, false))).unwrap();
        assert!(fresh.missing().is_empty());
        assert_eq!(stayed, first);
    }

    fn web_languages(list: &LanguageList, braille: bool) -> Languages {
        let mut l = Languages::from_list(&list.0, LanguageSource::Loaded(list.1.clone()));
        l.print_document = !braille;
        l
    }

    #[test]
    fn styles_are_added_changed_renamed_and_removed() {
        let mut doc = new_document(false, "", "");
        let n = doc.style_names().len();
        assert_eq!(doc.add_style("New Style Name", "", "", ""), "You must give the style a name other than \"New Style Name\".");
        assert!(doc.add_style("bad name", "", "", "").starts_with("The style name contains invalid characters."));
        assert_eq!(doc.add_style("quote", "", "[ind4][rm4]", "[p]"), "");
        assert_eq!(doc.add_style("quote", "", "", ""), "A style named quote already exists.");
        assert_eq!(doc.add_style("quote2", "quote", "", "[l]"), "");
        assert_eq!(doc.style_codes("quote2"), r#"["[ind4][rm4]","[l]"]"#);
        doc.set_style_codes("quote2", "[ind4][rm4]", "[p]");
        assert_eq!(doc.style_codes("quote2"), r#"["[ind4][rm4]","[p]"]"#);
        doc.rename_style("quote2", "block");
        doc.remove_style("quote");
        assert_eq!(doc.style_names().len(), n + 1);
        assert!(doc.style_names().contains(&"block".to_string()) && doc.style_codes("quote").is_empty());
        doc.set_page_numbers(vec![0, 2, 0, 4, 7]);
        assert_eq!(doc.page_numbers(), vec![0, 2, 0, 4, 7]);
    }

    #[test]
    fn printer_setup_fills_reads_and_prints() {
        let view: serde_json::Value = serde_json::from_str(&printer_setup_view("")).unwrap();
        assert_eq!((view["paper"].as_str(), view["margins"][0].as_str(), view["lines"].as_u64()), (Some("Letter (8.5 x 11 inches)"), Some("1\""), Some(54)));
        let m = |v: [&str; 4]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let a4 = read_printer_setup("", "A4 (210 x 297 mm)", true, &m(["25.4 mm", "1", "1", "0.5\""])).unwrap();
        let s = PrinterSetup::from_text(&a4).unwrap();
        assert_eq!((s.paper, s.landscape, s.margins, s.metric), ((2100, 2970), true, [1000, 1000, 1000, 500], true));
        assert!(read_printer_setup("", "A4 (210 x 297 mm)", false, &m(["x", "1", "1", "1"])).is_err());
        assert_eq!(printer_page_css(&a4), "@page { size: 297mm 210mm; margin: 1in 1in 0.5in 1in; }");
        let paged = PrinterSetup::from_text(&printer_setup_page(&a4, 40, 200)).unwrap();
        assert_eq!((paged.lines, paged.chars), (40, 65));
    }
}
