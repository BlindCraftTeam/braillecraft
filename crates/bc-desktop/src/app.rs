//! Program state: the main window, its documents, global settings, and the parts every command shares (status
//! line, translated line, view settings, menu check marks).
//!
//! The state lives in one thread-local cell. Commands never hold it while a dialog is open: a modal dialog runs the
//! event loop, and events that arrive meanwhile (text changes, focus) must be able to reach the state.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;

use bc_core::doctext::Shown;
use wxdragon::prelude::*;
use wxdragon_sys as ffi;

use crate::dlg::Memory;
use crate::encodings;
use crate::menu::Menus;
use crate::speech::ReaderSettings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocKind {
    Print,
    Braille,
}

impl DocKind {
    pub fn name(self) -> &'static str {
        match self {
            DocKind::Print => "Print Document",
            DocKind::Braille => "Braille Document",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewFont {
    Print,
    Braille,
    SimBraille,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hyphen {
    None,
    Dashes,
    DashesHyphens,
    French,
    French113,
}

#[derive(Clone, Debug, Hash)]
pub struct Style {
    pub name: String,
    pub nested: bool,
    /// The codes that begin and end the style, in running text (codes marked 0x1C ... 0x1F, as the document file
    /// stores them; a bracket is text).
    pub begin: String,
    pub end: String,
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub font: ViewFont,
    pub full_page: bool,
    pub codes: bool,
    pub translated: bool,
    pub six_key: bool,
}

pub struct Doc {
    pub key: u64,
    pub frame: MDIChildFrame,
    /// The editing area: the program-drawn editing window (editor_view), showing `ed`.
    pub area: crate::editor_view::Area,
    /// The document's status line, the window after the editing area (as in the reference program, where screen
    /// reader scripts find it there): current style, Page, Line, Column, the character at the caret, Keys.
    pub status: StaticText,
    pub line: TextCtrl,
    pub name: String,
    pub kind: DocKind,
    pub untitled: bool,
    pub saved: bool,
    pub dirty: bool,
    pub path: Option<PathBuf>,
    /// The document's tables by code: print-to-braille rules and scan table, braille-to-print rules and scan table.
    pub tables: [String; 4],
    /// Codes in force from the start of the document (Document, Learning Tables sets the grade code here).
    pub initial_codes: String,
    /// The template the document was made with (its name as File, New lists it); its styles were copied into the
    /// document's own style list.
    pub template: String,
    pub cells: u32,
    pub lines: u32,
    /// Page numbering, in the order the document file keeps it: braille page numbers on even pages and on odd pages,
    /// print page numbers on even pages and on odd pages (0 none, 1 upper left, 2 upper right, 3 lower left, 4 lower
    /// right), then the first page whose number is shown.
    pub page_numbers: [u16; 5],
    /// The document laid out into pages, with what it was laid out from (a hash of the text, page size, numbering
    /// and styles); laid out again when any of them changes.
    pub pages: Option<(u64, std::rc::Rc<Vec<bc_core::format::Page>>)>,
    /// The document and its caret (bc_core::editor): its text with the ranges that are codes (a code comes only
    /// from a program command: a menu, a style, the code list, a file, as in the reference program; brackets typed
    /// are text), laid out as the formatted view or the coded view shows it.
    pub ed: bc_core::editor::Editor,
    /// What the editor was laid out by (page size, numbering, styles); laid out again when it changes.
    pub ed_layout: u64,
    /// Document states to go back to (Edit, Undo) and forward to (Edit, Redo), each with its caret; at most 100 are
    /// kept, one per edit (spec 30).
    pub undo: Vec<(Shown, usize)>,
    pub redo: Vec<(Shown, usize)>,
    /// What the editing window was last given; the document changed from outside an edit (`area_stale`).
    pub area_rows: crate::editor_view::ShownRows,
    pub area_stale: bool,
    pub view: View,
    pub hyphen: Hyphen,
    pub styles: Vec<Style>,
    /// What the document file it was read from holds that BrailleCraft writes back as read: the 224-byte settings
    /// block (empty for a new document), and the style names in the file's order with the index the file gives each.
    pub file_settings: Vec<u8>,
    pub file_styles: Vec<(String, u16)>,
    pub last_style: Option<Style>,
    /// Edit, Highlight: while on, caret moves extend the selection from where it was turned on.
    pub highlight_anchor: Option<usize>,
    pub last_line_key: String,
    pub six_down: HashSet<usize>,
    pub chord: usize,
    /// Document, Printer Setup: the document's own printer, paper and margins; None prints with the global ones.
    pub printer: Option<crate::print::PrinterSetup>,
    /// Document, Embosser Setup: the name of the document's embosser setup (empty: no target embosser) and its
    /// embosser page layout (form, top and binding margins, interpoint, banner page, interline print).
    pub embosser: String,
    pub layout: crate::embosser::Layout,
    /// The optional sections of the document file it was read from that BrailleCraft does not use itself, written
    /// back as they were (their descriptor's place, kind and bytes).
    pub file_sections: Vec<(usize, u16, Vec<u8>)>,
}

impl Doc {
    /// The document: its text and which runs are codes.
    pub fn shown(&self) -> Shown {
        self.ed.doc.clone()
    }

    /// Puts `shown` in the document, codes and all (a translation, a file, a style renamed): the caret stays at its
    /// place where it can, and the editing window is given the new rows (App::show_area).
    pub fn set_shown(&mut self, shown: &Shown) {
        let caret = self.ed.caret;
        self.ed.set_doc(shown.clone(), caret);
        self.area_stale = true;
    }

    /// The caret's place in the document.
    pub fn caret(&self) -> usize {
        self.ed.caret
    }

    /// The selection's start and end in the document.
    pub fn selection(&self) -> (usize, usize) {
        self.ed.selection()
    }

    /// Selects document places `from` to `to` (the caret at `to`; no selection when they are equal); the editing
    /// window shows it at the next App::show_area.
    pub fn select_doc(&mut self, from: usize, to: usize) {
        self.ed.select(from, to);
        self.area_stale = true;
    }

    /// The window name: "Untitled Print Document 1"; "name -- New Braille Document" for a translated or opened
    /// document not yet saved in the program's own format; "name -- Braille Document" once saved.
    pub fn full_name(&self) -> String {
        if self.untitled {
            self.name.clone()
        } else {
            format!("{} -- {}{}", self.name, if self.saved { "" } else { "New " }, self.kind.name())
        }
    }
}

#[derive(Clone, Debug)]
pub struct DefaultViews {
    pub translated_font: ViewFont,
    pub print_full_page: bool,
    pub print_codes: bool,
    pub print_translated: bool,
    pub braille_font: ViewFont,
    pub braille_full_page: bool,
    pub braille_codes: bool,
    pub braille_translated: bool,
    pub six_key: bool,
}

#[derive(Clone, Debug)]
pub struct Shortcuts {
    pub grade0: String,
    pub flush_right_leader: String,
    pub cbc: String,
    pub new_line: String,
    pub new_page: String,
    pub new_ref_page: String,
    /// Page Up and Page Down, and with Control: 0 a screen, 1 a page, 2 to the top of a page.
    pub page_nav: usize,
    pub control_page_nav: usize,
}

impl Shortcuts {
    /// The settings section the shortcuts are kept in.
    pub const SECTION: &'static str = "Shortcuts";
    /// The kept words for the page-navigation choices, in the dialog's order.
    pub const PAGE_NAV: [&'static str; 3] = ["screen", "page", "pageTop"];

    pub fn page_nav_number(word: &str) -> Option<usize> {
        Self::PAGE_NAV.iter().position(|w| w.eq_ignore_ascii_case(word))
    }
}

#[derive(Clone, Debug)]
pub struct Fonts {
    pub print_face: String,
    pub print_size: i32,
    pub braille_size: i32,
    pub status_face: String,
    pub status_size: i32,
    /// Global, Fonts, for printed output: "Auto-determine point size", and the point sizes of the print font and of
    /// the braille and simulated braille fonts when it is off (spec 24, 2c).
    pub print_auto: bool,
    pub print_output_size: i32,
    pub braille_output_size: i32,
}

pub struct Settings {
    pub encoding: String,
    pub six_key_set: String,
    pub default_views: DefaultViews,
    pub shortcuts: Shortcuts,
    pub fonts: Fonts,
    pub reader: ReaderSettings,
    /// Global, View Preferences: "Use verbose labels for non-ANSI characters in coded view"; the status line then
    /// shows the character at the caret with its Unicode value, "c {U+XXXX}".
    pub verbose_labels: bool,
    /// Global, View Preferences: the margin bell position (cells before the end of the line; 0 off), line numbers in
    /// formatted braille documents, when the Cursor Locator is shown, and which codes the formatted view's translated
    /// line shows (spec 24 2d.2).
    pub view_prefs: ViewPrefs,
    /// Global, Printer Setup: the printer, paper and margins documents print with when they have none of their own.
    pub printer: crate::print::PrinterSetup,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            encoding: encodings::DEFAULT.to_string(),
            six_key_set: "fdsjkl".to_string(),
            default_views: DefaultViews {
                // The defaults are the reference program's (spec 18): translated line in the print font, braille
                // documents with the translated line and six-key entry on, status line Segoe UI 12. Braille
                // documents open in the braille font (owner direction, queue line T4).
                translated_font: ViewFont::Print,
                print_full_page: false,
                print_codes: false,
                print_translated: false,
                braille_font: ViewFont::Braille,
                braille_full_page: false,
                braille_codes: false,
                braille_translated: true,
                six_key: true,
            },
            shortcuts: Shortcuts {
                grade0: "[cz]".into(),
                flush_right_leader: "[fr;p~\"]".into(),
                cbc: "[cb]".into(),
                new_line: "[<]".into(),
                new_page: "[pg]".into(),
                new_ref_page: "[lea]".into(),
                page_nav: 0,
                control_page_nav: 1,
            },
            fonts: Fonts {
                print_face: "Courier New".into(),
                print_size: 12,
                braille_size: 18,
                status_face: "Segoe UI".into(),
                status_size: 12,
                print_auto: true,
                print_output_size: 12,
                braille_output_size: 18,
            },
            reader: ReaderSettings::default(),
            verbose_labels: false,
            view_prefs: ViewPrefs::default(),
            printer: crate::print::PrinterSetup::default(),
        }
    }
}

/// Which codes the formatted view's translated line shows (Display, TranslatedLineCodes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineCodes {
    HideAll,
    HideIdle,
    ShowAll,
}

impl LineCodes {
    /// The words the setting is kept as, in the order the dialog lists them.
    pub const WORDS: [&'static str; 3] = ["hide", "noidle", "show"];
    pub const ALL: [LineCodes; 3] = [LineCodes::HideAll, LineCodes::HideIdle, LineCodes::ShowAll];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(1)
    }
}

/// Global, View Preferences, with the reference program's defaults (spec 24 2d.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewPrefs {
    pub bell_position: i64,
    pub line_numbers: bool,
    pub locator_on_open: bool,
    pub locator_on_view_switch: bool,
    pub locator_on_shift: bool,
    pub line_codes: LineCodes,
}

impl Default for ViewPrefs {
    fn default() -> Self {
        ViewPrefs {
            bell_position: 0,
            line_numbers: false,
            locator_on_open: true,
            locator_on_view_switch: true,
            locator_on_shift: true,
            line_codes: LineCodes::HideIdle,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FindSpec {
    pub find: String,
    pub replace_with: String,
    pub match_case: bool,
    pub backward: bool,
    pub whole_words: bool,
}

pub struct App {
    pub frame: MDIParentFrame,
    pub menus: Menus,
    pub docs: Vec<Doc>,
    pub active: Option<usize>,
    pub next_key: u64,
    pub untitled: [u32; 2],
    pub settings: Settings,
    pub memory: Memory,
    pub last_find: Option<FindSpec>,
    pub busy: bool,
    /// Text taken out of a document or copied, with its codes, newest last: putting the same text back (paste,
    /// undo) puts its codes back.
    pub known: Vec<Shown>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

pub fn set(app: App) {
    APP.with(|a| *a.borrow_mut() = Some(app));
}

/// Runs `f` with the state. Panics if the state is already in use: commands call it only between dialogs.
pub fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| f(a.borrow_mut().as_mut().expect("program state")))
}

/// Runs `f` with the state when it is free; for event handlers, which may fire while a command is running.
pub fn try_with(f: impl FnOnce(&mut App)) {
    APP.with(|a| {
        if let Ok(mut guard) = a.try_borrow_mut() {
            if let Some(app) = guard.as_mut() {
                f(app);
            }
        }
    });
}

/// The main window, for parenting dialogs.
pub fn frame() -> MDIParentFrame {
    with(|a| a.frame)
}

// ------------------------------------------------------------------ text positions

/// The text of a control and the caret, selection and positions as character counts (wxWidgets positions count
/// characters; the controls are created with the rich text style on Windows so a line break counts as one).
pub fn char_to_byte(s: &str, chars: i64) -> usize {
    s.char_indices().nth(chars.max(0) as usize).map_or(s.len(), |(b, _)| b)
}

pub fn byte_to_char(s: &str, byte: usize) -> i64 {
    s[..byte.min(s.len())].chars().count() as i64
}

/// The line the caret is on, with its codes.
pub fn line_of(doc: &Shown, pos: i64) -> Shown {
    let text = &doc.text;
    let b = char_to_byte(text, pos);
    let start = text[..b].rfind('\n').map_or(0, |i| i + 1);
    let end = text[b..].find('\n').map_or(text.len(), |i| b + i);
    doc.slice(byte_to_char(text, start) as usize, byte_to_char(text, end) as usize)
}

/// The status line's parts are shared with the web build (bc_core::status).
pub use bc_core::status::{char_field, StatusLine};
#[cfg(test)]
pub use bc_core::status::open_styles;

// ------------------------------------------------------------------ fonts

pub fn font(face: &str, size: i32) -> Option<Font> {
    Font::builder().with_face_name(face).with_point_size(size).with_family(FontFamily::Modern).build()
}

impl App {
    pub fn doc(&self) -> Option<&Doc> {
        self.active.and_then(|i| self.docs.get(i))
    }

    pub fn doc_mut(&mut self) -> Option<&mut Doc> {
        self.active.and_then(move |i| self.docs.get_mut(i))
    }

    pub fn index_of(&self, key: u64) -> Option<usize> {
        self.docs.iter().position(|d| d.key == key)
    }

    pub fn encoding(&self) -> &'static encodings::Encoding {
        encodings::find(&self.settings.encoding)
    }

    pub fn face(&self, font: ViewFont) -> (String, i32) {
        match font {
            ViewFont::Print => (self.settings.fonts.print_face.clone(), self.settings.fonts.print_size),
            ViewFont::Braille => (self.encoding().font_family("Braille"), self.settings.fonts.braille_size),
            ViewFont::SimBraille => (self.encoding().font_family("SimBraille"), self.settings.fonts.braille_size),
        }
    }

    /// The title bar: "BrailleCraft - [name -- New Print Document]". On Windows the system writes it from the
    /// maximized document window's own title; elsewhere it is set here.
    pub fn set_title(&self) {
        let name = self.doc().map(|d| d.full_name());
        if cfg!(windows) {
            self.frame.set_label("BrailleCraft");
        } else {
            match &name {
                Some(n) => self.frame.set_label(&format!("BrailleCraft - [{n}]")),
                None => self.frame.set_label("BrailleCraft"),
            }
        }
    }

    /// Applies the active document's view settings: font, full page, translated line; then the menu marks.
    pub fn apply_view(&mut self) {
        let Some(i) = self.active else { return };
        let (face, mut size) = self.face(self.docs[i].view.font);
        let d = &self.docs[i];
        if d.view.full_page {
            // Full-Page: the font is sized so one whole page (lines by cells) fits the window.
            let area = d.area.get_client_size();
            let by_height = area.height * 72 / 96 / (d.lines as i32 + 1);
            let by_width = area.width * 72 / 96 * 10 / 6 / (d.cells as i32 + 1);
            size = by_height.min(by_width).max(4);
        }
        if let Some(f) = font(&face, size) {
            d.area.set_font(&f);
        }
        let (lface, lsize) = self.face(if d.kind == DocKind::Print { self.settings.default_views.translated_font } else { ViewFont::Print });
        if let Some(f) = font(&lface, lsize) {
            d.line.set_font(&f);
        }
        d.line.show(d.view.translated);
        d.frame.layout();
        self.show_area(i, None);
        self.refresh_menus();
        self.docs[i].last_line_key.clear();
        self.update_status();
    }

    pub fn refresh_menus(&mut self) {
        let names: Vec<String> = self.docs.iter().map(|d| d.full_name()).collect();
        self.menus.set_documents(&names, self.active);
        self.refresh_table_menus();
        let Some(d) = self.doc() else { return };
        let v = d.view;
        let hyphen = d.hyphen;
        let highlight = d.highlight_anchor.is_some();
        let m = &self.menus;
        m.check(1209, highlight);
        m.check(1301, v.full_page);
        m.check(match v.font { ViewFont::Print => 1303, ViewFont::Braille => 1304, ViewFont::SimBraille => 1305 }, true);
        m.check(1307, v.six_key);
        m.check(1309, v.codes);
        m.check(1310, v.translated);
        // Edit, Goto and View, Full-Page are used only in the formatted view (spec 27 2d).
        m.enable(1215, !v.codes);
        m.enable(1301, !v.codes);
        m.check(
            match hyphen {
                Hyphen::None => 2901,
                Hyphen::Dashes => 2902,
                Hyphen::DashesHyphens => 2903,
                Hyphen::French => 2904,
                Hyphen::French113 => 2905,
            },
            true,
        );
    }

    /// Document, Translation Tables: the region in force checked, then the languages of that region and those the
    /// open documents use, the active document's checked. Document, Learning Tables: the courses of the active
    /// document's language between Uncontracted and Fully Contracted; checked is the course whose step the document
    /// uses, else Uncontracted when its initial codes hold a grade 1 code, else Fully Contracted.
    pub fn refresh_table_menus(&mut self) {
        let region = crate::translation::region();
        let item = (0..5).find(|&i| bc_core::table_list::TableList::region_bit(i) == region).map_or(2706, |i| 2701 + i as i32);
        self.menus.check(item, true);
        let used: Vec<Option<(usize, Option<usize>)>> =
            self.docs.iter().map(|d| crate::translation::with_list(|l| l.entry_for(&d.tables))).collect();
        let active = self.active.and_then(|i| used.get(i).copied().flatten());
        let listed: Vec<(usize, String)> = crate::translation::with_list(|l| {
            l.listed(region, |e| used.iter().any(|u| u.map(|x| x.0) == Some(e)))
                .into_iter()
                .map(|e| (e, l.entries[e].name.clone()))
                .collect()
        });
        self.menus.set_languages(&listed, active.map(|a| a.0));
        let (courses, has_steps, step) = match active {
            Some((entry, step)) => crate::translation::with_list(|l| {
                (l.courses(entry), !l.entries[entry].steps.is_empty(), step)
            }),
            None => (Vec::new(), false, None),
        };
        let initial = self.doc().map(|d| d.initial_codes.clone()).unwrap_or_default();
        let checked = match step.and_then(|s| courses.iter().position(|c| c.steps.contains(&s))) {
            Some(c) => crate::menu::COURSE_BASE + c as i32,
            None if initial.contains("[g1") => 2801,
            None => 2802,
        };
        let names: Vec<String> = courses.into_iter().map(|c| c.name).collect();
        self.menus.set_courses(&names, has_steps, checked);
    }

    /// The active document's status line (current style, Page, Line, Column, the character at the caret, Keys);
    /// then its translated line.
    pub fn update_status(&mut self) {
        let Some(i) = self.active else { return };
        // The editing window is given the document again when it was set from outside an edit, or when what the
        // document is laid out by (page size, numbering, styles) changed.
        if self.docs[i].area_stale || self.docs[i].ed_layout != self.page_layout(i).0 {
            self.show_area(i, None);
        }
        let shown = self.docs[i].shown();
        let pos = self.docs[i].caret() as i64;
        // In the formatted view, the caret's page, line on the page and column (spec 27 1b; spec 19 section 4); the
        // coded view shows none.
        let ed = &self.docs[i].ed;
        let location = if ed.coded {
            None
        } else {
            let (row, col) = ed.caret_place();
            match ed.row(row) {
                Some(bc_core::editor::Row::Line { page, line }) => Some((page + 1, line + 1, col + 1)),
                _ => None,
            }
        };
        let d = &self.docs[i];
        let style = bc_core::status::style_text(&shown, pos);
        let status = StatusLine {
            style,
            location,
            character: char_field(&shown, pos, self.settings.verbose_labels),
            six_key: d.view.six_key,
            highlight: d.highlight_anchor.is_some(),
            // A braille keyboard connection; BrailleCraft has none, so the mark is never shown.
            keyboard: false,
        }
        .text();
        if d.status.get_label() != status {
            d.status.set_label(&status);
        }
        self.update_translated_line(&shown, pos);
    }

    /// After a character was typed in document `i`: the margin bell rings when the caret has come to the bell
    /// position, that many cells before the end of the line (spec 24 2d.2: not in coded view, a bell
    /// position above 0, the caret's column equal to the cells per line less the bell position).
    pub fn margin_bell(&mut self, i: usize) {
        let bell = self.settings.view_prefs.bell_position;
        if bell <= 0 || self.docs[i].view.codes {
            return;
        }
        let (_, col) = self.docs[i].ed.caret_place();
        if col as i64 + 1 == i64::from(self.docs[i].cells) - bell {
            crate::bell::ring();
        }
    }

    /// View, Translated Line: the line the caret is on, translated. A print document shows its braille in the font
    /// chosen in Global, Default Views; a braille document shows its print.
    fn update_translated_line(&mut self, shown: &Shown, pos: i64) {
        let Some(i) = self.active else { return };
        if !self.docs[i].view.translated {
            return;
        }
        let line = line_of(shown, pos);
        let d = &self.docs[i];
        let key = format!(
            "{}|{:?}|{}|{}|{}|{}|{}|{:?}|{}|{:?}",
            d.key,
            d.kind,
            d.tables.join("+"),
            d.initial_codes,
            d.template,
            self.settings.encoding,
            line.text,
            line.codes,
            d.view.codes,
            self.settings.view_prefs.line_codes
        );
        if key == self.docs[i].last_line_key {
            return;
        }
        let shown = self.translate_line(i, &line);
        let d = &self.docs[i];
        d.line.set_value(&shown);
        d.line.set_accessibility_value(&shown);
        self.docs[i].last_line_key = key;
    }

    /// One line of document `i` translated: a print line into braille (in the display encoding), a braille line
    /// into print, with the document's initial codes in force. The engine passes those codes through at the start
    /// of what it writes; the translated line leaves them off.
    pub fn translate_line(&self, i: usize, line: &Shown) -> String {
        let enc = self.encoding();
        let d = &self.docs[i];
        // A braille document with no braille-to-print table shows the reference program's text for message 0;
        // screen reader scripts read "Error message 0" on the translated line as "no back-translation".
        if d.kind == DocKind::Braille && !crate::translation::has_table(&d.tables[2]) {
            return "Error message 0".to_string();
        }
        let styles = crate::translation::style_table(&d.styles);
        let r = match d.kind {
            DocKind::Print => crate::translation::forward(&d.tables[0], line, &d.initial_codes, enc, &styles),
            DocKind::Braille => crate::translation::back(&d.tables[2], line, &d.initial_codes, enc, &styles),
        };
        // The codes the line shows (spec 24 2d.2): every code while the document shows its codes, else
        // as Global, View Preferences says: all, all but the idle code, or none.
        let codes = if d.view.codes { LineCodes::ShowAll } else { self.settings.view_prefs.line_codes };
        r.map(|s| match codes {
            LineCodes::ShowAll => s.text,
            LineCodes::HideIdle => s.text_keeping_codes(|c| !bc_core::doctext::is_idle_code(c)),
            LineCodes::HideAll => s.text_keeping_codes(|_| false),
        })
        .unwrap_or_else(|e| e)
    }

    /// Keeps text taken out or copied, with its codes (the last few).
    pub fn remember(&mut self, s: Shown) {
        if s.codes.is_empty() {
            return;
        }
        self.known.retain(|k| k.text != s.text);
        self.known.push(s);
        if self.known.len() > 32 {
            self.known.remove(0);
        }
    }

    /// Document `i` laid out into pages: its cells per line and lines per page, its styles, and for a braille
    /// document its braille page numbers. Kept until the text, its codes or any of those changes.
    pub fn laid_out(&mut self, i: usize, value: &Shown) -> std::rc::Rc<Vec<bc_core::format::Page>> {
        use std::hash::{Hash, Hasher};
        let (layout_key, layout, styles) = self.page_layout(i);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (&value.text, &value.codes, layout_key).hash(&mut h);
        let key = h.finish();
        let d = &self.docs[i];
        if let Some((k, pages)) = &d.pages {
            if *k == key {
                return pages.clone();
            }
        }
        // The editor's own pages, when it shows this document laid out the same way.
        let pages = if !d.ed.coded && d.ed_layout == layout_key && d.ed.doc == *value {
            std::rc::Rc::new(d.ed.pages.clone())
        } else {
            std::rc::Rc::new(bc_core::format::format(value, &layout, Some(&styles)))
        };
        self.docs[i].pages = Some((key, pages.clone()));
        pages
    }

    /// What document `i` is laid out by: its cells per line and lines per page, its braille page numbers (a braille
    /// document), the first page whose number is shown, and its styles; with a key that changes when any of them does.
    pub fn page_layout(&self, i: usize) -> (u64, bc_core::format::Layout, bc_core::engine::StyleTable) {
        use std::hash::{Hash, Hasher};
        let d = &self.docs[i];
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (d.cells, d.lines, d.page_numbers, d.kind == DocKind::Braille, &d.styles).hash(&mut h);
        let braille = d.kind == DocKind::Braille;
        let (cells, lines) = (d.cells.max(1) as usize, d.lines.max(1) as usize);
        let mut layout = if braille {
            bc_core::format::Layout::braille(cells, lines)
        } else {
            bc_core::format::Layout::print(cells, lines)
        };
        // INTERIM: a print document's own pages carry no number line until the print page layout is traced.
        if braille {
            layout.numbers_even = bc_core::format::Corner::from_code(d.page_numbers[0]);
            layout.numbers_odd = bc_core::format::Corner::from_code(d.page_numbers[1]);
        }
        layout.first_number_shown = d.page_numbers[4].max(1) as usize;
        (h.finish(), layout, crate::translation::style_table(&d.styles))
    }

    /// Gives document `i`'s editing window its rows, line numbers and caret; the document is laid out again first
    /// when what it is laid out by has changed. `keep_from_top` keeps the caret row that many rows from the top.
    pub fn show_area(&mut self, i: usize, keep_from_top: Option<usize>) {
        let (key, layout, styles) = self.page_layout(i);
        // Line numbers beside the formatted lines of a braille document, when Global, View Preferences asks for
        // them (spec 24 2d.2, spec 27 3b).
        let line_numbers =
            self.docs[i].kind == DocKind::Braille && self.settings.view_prefs.line_numbers && !self.docs[i].view.codes;
        let d = &mut self.docs[i];
        if d.ed_layout != key {
            d.ed.layout = layout;
            d.ed.styles = Some(styles);
            d.ed.relayout();
            d.ed_layout = key;
        }
        d.area_stale = false;
        let area = d.area;
        area.show(&d.ed, &mut d.area_rows, line_numbers, keep_from_top);
    }

    /// Edits document `i` by `change` on its editor (typing, Backspace, Delete, a code put in): the document before
    /// it is kept for Edit, Undo (one step per edit, at most 100, spec 30), what it took out is kept with its codes
    /// for pasting, and the editing window shows the result.
    pub fn edit_by(&mut self, i: usize, change: impl FnOnce(&mut bc_core::editor::Editor) -> Shown) {
        let d = &mut self.docs[i];
        let before = (d.ed.doc.clone(), d.ed.caret);
        let removed = change(&mut d.ed);
        if d.ed.doc == before.0 {
            self.show_area(i, None);
            return;
        }
        if d.undo.len() >= 100 {
            d.undo.remove(0);
        }
        d.undo.push(before);
        d.redo.clear();
        d.dirty = true;
        d.last_line_key.clear();
        self.remember(removed);
        self.show_area(i, None);
        if self.active == Some(i) {
            self.update_status();
        }
    }

    /// Edits document `i`: document places `from` to `to` become `put` (a code the range cuts into goes as a whole,
    /// spec 27 5a); the caret goes after `put`.
    pub fn edit(&mut self, i: usize, from: usize, to: usize, put: &Shown) {
        let put = put.clone();
        self.edit_by(i, move |e| e.replace(from, to, &put));
    }

    /// Edit, Undo (or Redo with `redo`): the document as it was before the last edit (or after the last one undone).
    /// False when there is none.
    pub fn undo(&mut self, i: usize, redo: bool) -> bool {
        let d = &mut self.docs[i];
        let Some((model, at)) = (if redo { d.redo.pop() } else { d.undo.pop() }) else { return false };
        let current = (d.ed.doc.clone(), d.ed.caret);
        if redo { d.undo.push(current) } else { d.redo.push(current) }
        d.ed.set_doc(model, at);
        d.dirty = true;
        d.last_line_key.clear();
        self.show_area(i, None);
        self.update_status();
        true
    }

    /// View, Codes for document `i` (spec 27 2c): the coded view shows the document's text with its codes in rows,
    /// the formatted view its laid-out lines. The caret keeps its place in the document and its row on the screen.
    pub fn set_view_codes(&mut self, i: usize, codes: bool) {
        let keep = self.docs[i].area.caret_screen_row();
        let d = &mut self.docs[i];
        d.view.codes = codes;
        d.ed.set_view(codes);
        self.show_area(i, Some(keep));
        self.docs[i].last_line_key.clear();
    }

    pub fn mark_dirty(&mut self) {
        if let Some(d) = self.doc_mut() {
            d.dirty = true;
        }
    }
}

/// The child window's frame functions that the MDI child wrapper does not offer (it is a frame underneath).
pub fn child_set_title(child: &MDIChildFrame, title: &str) {
    let c = std::ffi::CString::new(title).unwrap_or_default();
    unsafe { ffi::wxd_Frame_SetTitle(child.handle_ptr() as *mut ffi::wxd_Frame_t, c.as_ptr()) };
}

pub fn child_maximize(child: &MDIChildFrame) {
    unsafe { ffi::wxd_Frame_Maximize(child.handle_ptr() as *mut ffi::wxd_Frame_t, true) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(location: Option<(usize, usize, usize)>, six_key: bool, highlight: bool, keyboard: bool) -> String {
        StatusLine { style: "Current Style: None".into(), location, character: "a".into(), six_key, highlight, keyboard }
            .text()
    }

    #[test]
    fn status_line_fields_and_keys() {
        assert_eq!(line(Some((1, 2, 3)), false, false, false), "Current Style: None  Page:1  Line:2  Column:3  a  Keys:1");
        assert!(line(Some((1, 2, 3)), true, false, false).ends_with("Keys:6"));
        assert!(line(Some((1, 2, 3)), true, true, true).ends_with("Keys:6H"));
        assert!(line(Some((1, 2, 3)), false, false, true).ends_with("Keys:1+"));
        // Coded view: no Page, Line or Column.
        let coded = line(None, false, false, false);
        assert!(!coded.contains("Page:") && !coded.contains("Line:") && !coded.contains("Column:"));
    }

    #[test]
    fn character_field() {
        assert_eq!(char_field(&Shown::marked("ab"), 0, false), "a");
        assert_eq!(char_field(&Shown::marked("ab"), 2, false), "(end)");
        assert_eq!(char_field(&Shown::marked("é"), 0, true), "é {U+00E9}");
        assert_eq!(char_field(&Shown::marked("x[pg]y"), 1, false), "[pg]");
        assert_eq!(char_field(&Shown::marked("[es~h1.]T"), 0, false), "<h1.>");
        assert_eq!(char_field(&Shown::marked("[EE~h1.]"), 0, false), "</h1.>");
        assert_eq!(char_field(&Shown::marked("a\u{a0}b"), 1, false), "[']");
        // A line break is the new-line code; the one the editor adds after [p] is not in the document.
        assert_eq!(char_field(&Shown::marked("a\nb"), 1, false), "[<]");
        assert_eq!(char_field(&Shown::marked("[p]\nb"), 3, false), "b");
        // Brackets typed in the text are text, even when they look like a code.
        assert_eq!(char_field(&Shown::plain("x[pg]y"), 1, false), "[");
    }
}
