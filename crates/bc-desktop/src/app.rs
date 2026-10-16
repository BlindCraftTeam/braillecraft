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
    pub text: TextCtrl,
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
    /// Which bracketed runs of the text are codes (bc_core::doctext::Shown): a code comes only from a program
    /// command (a menu, a style, the code list, a file), as in the reference program; brackets typed are text.
    pub codes: Vec<(usize, usize)>,
    pub inner: Vec<(usize, usize)>,
    /// The text as it was when the codes were last brought up to date.
    pub last_value: String,
    /// Codes that have gone, with their places, so that an undo putting their text back brings them back.
    pub taken: Vec<(usize, Shown)>,
    pub view: View,
    pub hyphen: Hyphen,
    pub styles: Vec<Style>,
    /// What the document file it was read from holds that BrailleCraft writes back as read: the 224-byte settings
    /// block (empty for a new document), and the style names in the file's order with the index the file gives each.
    pub file_settings: Vec<u8>,
    pub file_styles: Vec<(String, u16)>,
    pub last_style: Option<Style>,
    pub highlight_anchor: Option<i64>,
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
    /// The document as the editor shows it: its text and which runs are codes.
    pub fn shown(&self) -> Shown {
        Shown { text: self.text.get_value(), codes: self.codes.clone(), inner: self.inner.clone() }
    }

    /// Puts `shown` in the editing area, codes and all.
    pub fn set_shown(&mut self, shown: &Shown) {
        // The codes are set first: the change handler then finds nothing changed.
        self.codes = shown.codes.clone();
        self.inner = shown.inner.clone();
        self.last_value = shown.text.clone();
        self.taken.clear();
        self.text.set_value(&shown.text);
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
            printer: crate::print::PrinterSetup::default(),
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
            let area = d.text.get_client_size();
            let by_height = area.height * 72 / 96 / (d.lines as i32 + 1);
            let by_width = area.width * 72 / 96 * 10 / 6 / (d.cells as i32 + 1);
            size = by_height.min(by_width).max(4);
        }
        if let Some(f) = font(&face, size) {
            // wxGTK applies a font to a multi-line control by setting its whole text again, which moves the caret
            // to the end; the caret and selection are kept across the change.
            let (from, to) = d.text.get_selection();
            let caret = d.text.get_insertion_point();
            d.text.set_font(&f);
            if from != to {
                d.text.set_selection(from, to);
            } else {
                d.text.set_insertion_point(caret);
            }
            d.text.show_position(caret);
        }
        let (lface, lsize) = self.face(if d.kind == DocKind::Print { self.settings.default_views.translated_font } else { ViewFont::Print });
        if let Some(f) = font(&lface, lsize) {
            d.line.set_font(&f);
        }
        d.line.show(d.view.translated);
        d.frame.layout();
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
        let shown = self.docs[i].shown();
        let pos = self.docs[i].text.get_insertion_point();
        // Page, line and column of the caret on the laid-out pages (spec 19 section 4): the ordinal page, the line on
        // that page and the cell on that line.
        let location = (!self.docs[i].view.codes)
            .then(|| bc_core::format::locate(&self.laid_out(i, &shown), pos.max(0) as usize));
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
            "{}|{:?}|{}|{}|{}|{}|{}|{:?}",
            d.key,
            d.kind,
            d.tables.join("+"),
            d.initial_codes,
            d.template,
            self.settings.encoding,
            line.text,
            line.codes
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
        r.map(|s| s.text).unwrap_or_else(|e| e)
    }

    /// Brings document `i`'s codes up to date with its text after an edit (bc_core::doctext::after_edit). Returns
    /// what is left of codes the edit broke, to be removed (last first).
    pub fn sync_codes(&mut self, i: usize) -> Vec<(usize, usize)> {
        let d = &mut self.docs[i];
        let text = d.text.get_value();
        if text == d.last_value {
            return Vec::new();
        }
        let before = Shown { text: std::mem::take(&mut d.last_value), codes: std::mem::take(&mut d.codes), inner: std::mem::take(&mut d.inner) };
        let taken = std::mem::take(&mut d.taken);
        let e = bc_core::doctext::after_edit(&before, &text, &self.known, &taken);
        let d = &mut self.docs[i];
        d.codes = e.shown.codes;
        d.inner = e.shown.inner;
        d.taken = e.taken;
        d.last_value = text;
        if let Some(r) = e.removed {
            self.remember(r);
        }
        e.remains
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
        let d = &self.docs[i];
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (&value.text, &value.codes, d.cells, d.lines, d.page_numbers, d.kind == DocKind::Braille, &d.styles).hash(&mut h);
        let key = h.finish();
        if let Some((k, pages)) = &d.pages {
            if *k == key {
                return pages.clone();
            }
        }
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
        let styles = crate::translation::style_table(&d.styles);
        let pages = std::rc::Rc::new(bc_core::format::format(value, &layout, Some(&styles)));
        self.docs[i].pages = Some((key, pages.clone()));
        pages
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
