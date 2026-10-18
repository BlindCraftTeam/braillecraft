//! The editing area: the program-drawn editing window (vendor/wxdragon-sys wxd_doc_editor.h) showing a document's
//! editor model (bc_core::editor), as the reference program draws its own editing window (spec 27, spec 30). The
//! window is told how many rows there are and asks for a row's text as it draws, measures or reads it (so a book's
//! rows are not held a second time in the window, plan line L1); it keeps the system caret and gives screen readers
//! its text; every key, typed character and click comes back here and is carried out on the model (documents.rs).

use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int};

use bc_core::editor::{Editor, Row};
use wxdragon::prelude::*;
use wxdragon_sys as ffi;

/// The editing window of one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area(*mut ffi::wxd_Window_t);

impl WxWidget for Area {
    fn handle_ptr(&self) -> *mut ffi::wxd_Window_t {
        self.0
    }
}

// As wxDragon's own widgets are: the window is only used on the main thread, where wxdragon::call_after runs what it
// is given.
unsafe impl Send for Area {}
unsafe impl Sync for Area {}

/// The reference's line number column is 20 pixels wide (spec 27 section 0).
const LINE_NUMBER_WIDTH: c_int = 20;

fn key_of(data: *mut c_void) -> u64 {
    unsafe { *(data as *const u64) }
}

extern "C" fn on_key(data: *mut c_void, code: c_int, unicode: c_int, modifiers: c_int) -> bool {
    crate::documents::area_key(key_of(data), code, unicode, modifiers)
}

extern "C" fn on_char(data: *mut c_void, unicode: c_int, modifiers: c_int) {
    crate::documents::area_char(key_of(data), unicode, modifiers);
}

extern "C" fn on_mouse(data: *mut c_void, kind: c_int, row: c_int, col: c_int, modifiers: c_int) {
    crate::documents::area_mouse(key_of(data), kind, row.max(0) as usize, col.max(0) as usize, modifiers);
}

extern "C" fn on_focus(data: *mut c_void, focused: bool) {
    crate::documents::area_focus(key_of(data), focused);
}

extern "C" fn free_key(data: *mut c_void) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(data as *mut u64) });
    }
}

thread_local! {
    /// The last row given to a window: the window copies it at once, so one is kept at a time.
    static ROW_ANSWER: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

/// A row the window asks for: `which` 0 as drawn, 1 as screen readers get it, 2 its line number label. NULL while
/// the program state is in use (the window asked during a call of the program's own: it asks again later).
extern "C" fn on_row(data: *mut c_void, row: c_int, which: c_int) -> *const c_char {
    let key = key_of(data);
    let row = row.max(0) as usize;
    let answer = crate::app::try_with_value(|a| {
        let i = a.index_of(key)?;
        let d = &a.docs[i];
        let ed = &d.ed;
        Some(match which {
            2 => match ed.row(row) {
                Some(Row::Line { line, .. }) => (line + 1).to_string(),
                _ => String::new(),
            },
            1 => row_form(ed, row, &ed.row_text(row), d.area_rows.forms.access, false),
            _ => row_form(ed, row, &ed.row_text(row), d.area_rows.forms.paint, true),
        })
    });
    match answer.flatten() {
        Some(text) => ROW_ANSWER.with(|c| {
            let mut c = c.borrow_mut();
            c.insert(CString::new(text.replace('\0', " ")).unwrap_or_default()).as_ptr()
        }),
        None => std::ptr::null(),
    }
}

/// Makes the editing window of document `key` in `parent`.
pub fn create(parent: &impl WxWidget, key: u64) -> Area {
    let data = Box::into_raw(Box::new(key)) as *mut c_void;
    let window = unsafe {
        ffi::wxd_DocEditor_Create(
            parent.handle_ptr(),
            data,
            Some(on_key),
            Some(on_char),
            Some(on_mouse),
            Some(on_focus),
            Some(free_key),
            Some(on_row),
        )
    };
    Area(window)
}

/// What the window was last given, so a change gives it only what is new: the text screen readers get (each row
/// and a line feed; the next text is compared with it to tell them the stretch that changed), the rows' kinds, the
/// forms the rows are written in (the window asks for rows in them), and whether the line number column is shown.
#[derive(Default)]
pub struct ShownRows {
    access_text: String,
    kinds: Vec<u8>,
    paint_id: Option<String>,
    pub forms: CellForms,
    line_numbers: Option<bool>,
}

/// Where `old` and `new` differ: the offset in characters of the first difference, and the stretch of each from
/// there that is not shared with the other's end.
fn changed_stretch(old: &str, new: &str) -> (usize, String, String) {
    let mut p = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(p) {
        p -= 1;
    }
    let (ot, nt) = (&old[p..], &new[p..]);
    let mut s = ot.bytes().rev().zip(nt.bytes().rev()).take_while(|(a, b)| a == b).count();
    while !ot.is_char_boundary(ot.len() - s) || !nt.is_char_boundary(nt.len() - s) {
        s -= 1;
    }
    (old[..p].chars().count(), ot[..ot.len() - s].to_string(), nt[..nt.len() - s].to_string())
}

/// How a braille document's cells are written in the window (Global, Internationalization): as the document holds
/// them in the display encoding (None), or as Unicode braille patterns read through that encoding: `paint` for what
/// is drawn, `access` for the text given to screen readers.
#[derive(Clone, Copy, Default)]
pub struct CellForms {
    pub paint: Option<&'static crate::encodings::Encoding>,
    pub access: Option<&'static crate::encodings::Encoding>,
}

/// The characters the window reads as the start and end of a code in a row's text (wxd_doc_editor.h): the code
/// between them is drawn in the code font inside a box (spec 27 4a).
const CODE_START: char = '\u{e000}';
const CODE_END: char = '\u{e001}';

/// Row `r`'s text for the window: its cells (never its code text) written as Unicode braille patterns through `enc`
/// when one is given, and, with `mark`, each code (its display form) between the window's code markers.
fn row_form(ed: &Editor, r: usize, text: &str, enc: Option<&crate::encodings::Encoding>, mark: bool) -> String {
    let codes = ed.row_codes(r);
    let chars: Vec<char> = text.chars().collect();
    // The document's own text never carries the markers.
    let cells = |run: &str| enc.map_or_else(|| run.to_string(), |e| e.text_to_cells(run)).replace([CODE_START, CODE_END], "\u{fffd}");
    let mut out = String::with_capacity(text.len() + 2 * codes.len());
    let mut at = 0;
    for &(s, e) in &codes {
        let (s, e) = (s.min(chars.len()), e.min(chars.len()));
        let run: String = chars[at..s.max(at)].iter().collect();
        out.push_str(&cells(&run));
        if mark {
            out.push(CODE_START);
        }
        out.extend(chars[s.max(at)..e].iter().map(|&c| if c == CODE_START || c == CODE_END { '\u{fffd}' } else { c }));
        if mark {
            out.push(CODE_END);
        }
        at = e.max(at);
    }
    let run: String = chars[at..].iter().collect();
    out.push_str(&cells(&run));
    out
}

impl Area {
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    /// Gives the window `ed`'s rows (when they changed), the line numbers (`line_numbers`: the formatted view of a
    /// braille document with them on, spec 24 2d.2) and the caret and selection; `keep_from_top` keeps the caret
    /// row that many rows from the top (a view switch, spec 27 2c).
    pub fn show(&self, ed: &Editor, last: &mut ShownRows, line_numbers: bool, keep_from_top: Option<usize>, forms: CellForms) {
        if self.0.is_null() {
            return;
        }
        last.forms = forms;
        // The rows' kinds and lengths, and the text screen readers get, row by row: the window holds none of the
        // rows (it asks for each as it needs it, on_row), so a long document costs it only a few bytes a row.
        let count = ed.rows();
        let mut kinds: Vec<u8> = Vec::with_capacity(count);
        let mut lengths: Vec<c_int> = Vec::with_capacity(count);
        let mut now = String::with_capacity(last.access_text.len());
        let mut longest = ed.row_width();
        for r in 0..count {
            match ed.row(r) {
                Some(Row::Line { .. }) | Some(Row::Coded { .. }) => {
                    let access = row_form(ed, r, &ed.row_text(r), forms.access, false);
                    let len = access.chars().count();
                    kinds.push(0);
                    lengths.push(len as c_int);
                    longest = longest.max(len);
                    now.push_str(&access);
                    now.push('\n');
                }
                _ => {
                    kinds.push(1);
                    lengths.push(0);
                }
            }
        }
        let paint_id = forms.paint.map(|e| e.id.clone());
        if now != last.access_text || kinds != last.kinds || paint_id != last.paint_id || last.line_numbers.is_none() {
            // Screen readers are told the stretch that changed.
            let (at, removed, inserted) = changed_stretch(&last.access_text, &now);
            let removed = CString::new(removed.replace('\0', " ")).unwrap_or_default();
            let inserted = CString::new(inserted.replace('\0', " ")).unwrap_or_default();
            unsafe {
                ffi::wxd_DocEditor_SetRows(
                    self.0,
                    count as c_int,
                    kinds.as_ptr(),
                    lengths.as_ptr(),
                    longest as c_int,
                    at as c_int,
                    removed.as_ptr(),
                    inserted.as_ptr(),
                )
            };
            last.access_text = now;
            last.kinds = kinds;
            last.paint_id = paint_id;
        }
        if last.line_numbers != Some(line_numbers) {
            let width = if line_numbers { LINE_NUMBER_WIDTH } else { 0 };
            unsafe { ffi::wxd_DocEditor_SetLineNumbers(self.0, width) };
            last.line_numbers = Some(line_numbers);
        }
        let (row, col) = ed.caret_place();
        let (arow, acol) = ed.place(ed.anchor);
        let keep = keep_from_top.map_or(-1, |k| k as c_int);
        // The caret row's text goes with the caret: the window cannot ask for it while the program is in here.
        let caret_row = CString::new(row_form(ed, row, &ed.row_text(row), forms.paint, true).replace('\0', " ")).unwrap_or_default();
        unsafe {
            ffi::wxd_DocEditor_SetCaret(self.0, row as c_int, col as c_int, arow as c_int, acol as c_int, keep, caret_row.as_ptr())
        };
    }

    /// The first row shown and the number of whole rows shown.
    pub fn top_row(&self) -> usize {
        unsafe { ffi::wxd_DocEditor_GetTopRow(self.0).max(0) as usize }
    }

    pub fn visible_rows(&self) -> usize {
        unsafe { ffi::wxd_DocEditor_GetVisibleRows(self.0).max(1) as usize }
    }

    /// The caret row's place from the top of the window.
    pub fn caret_screen_row(&self) -> usize {
        unsafe { ffi::wxd_DocEditor_GetCaretScreenRow(self.0).max(0) as usize }
    }

    pub fn set_top_row(&self, row: usize) {
        unsafe { ffi::wxd_DocEditor_SetTopRow(self.0, row as c_int) };
    }

    /// The Cursor Locator (spec 24 2d.2).
    pub fn show_cursor_locator(&self) {
        unsafe { ffi::wxd_DocEditor_ShowCursorLocator(self.0) };
    }

    /// The name screen readers give the window.
    pub fn set_accessible_name(&self, name: &str) {
        if let Ok(c) = CString::new(name) {
            unsafe { ffi::wxd_DocEditor_SetAccessibleName(self.0, c.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bc_core::doctext::Shown;

    #[test]
    fn cells_become_braille_patterns_and_codes_stay() {
        let enc = crate::encodings::find(crate::encodings::DEFAULT);
        let ed = Editor::new(Shown::marked("ab[g1]c"), bc_core::format::Layout::braille(40, 25), None, true, 42);
        let text = ed.row_text(0);
        assert_eq!(text, "ab[g1]c");
        assert_eq!(row_form(&ed, 0, &text, Some(enc), false), "\u{2801}\u{2803}[g1]\u{2809}");
        // The window gets each code between its markers; a hard space is shown as the code ['].
        assert_eq!(row_form(&ed, 0, &text, Some(enc), true), "\u{2801}\u{2803}\u{e000}[g1]\u{e001}\u{2809}");
        let ed = Editor::new(Shown::marked("a\u{a0}[es~h1.]b"), bc_core::format::Layout::braille(40, 25), None, true, 42);
        assert_eq!(row_form(&ed, 0, &ed.row_text(0), None, true), "a\u{e000}[']\u{e001}");
        assert_eq!(row_form(&ed, 1, &ed.row_text(1), None, true), "\u{e000}<h1.>\u{e001}b");
    }
}
