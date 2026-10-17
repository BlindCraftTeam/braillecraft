//! The editing area: the program-drawn editing window (vendor/wxdragon-sys wxd_doc_editor.h) showing a document's
//! editor model (bc_core::editor), as the reference program draws its own editing window (spec 27, spec 30). The
//! window draws the rows it is given, keeps the system caret and gives screen readers its text; every key, typed
//! character and click comes back here and is carried out on the model (documents.rs).

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
        )
    };
    Area(window)
}

/// What the window was last given, so a change gives it only what is new.
#[derive(Default)]
pub struct ShownRows {
    rows: Vec<String>,
    access: Vec<String>,
    kinds: Vec<c_int>,
    labels: Vec<String>,
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
        let count = ed.rows();
        let mut rows = Vec::with_capacity(count);
        let mut access = Vec::new();
        let separate = forms.paint.map(|e| &e.id) != forms.access.map(|e| &e.id);
        let mut kinds = Vec::with_capacity(count);
        let mut labels = Vec::with_capacity(if line_numbers { count } else { 0 });
        for r in 0..count {
            let row = ed.row(r);
            let text = ed.row_text(r);
            if separate {
                access.push(row_form(ed, r, &text, forms.access, false));
            }
            rows.push(row_form(ed, r, &text, forms.paint, true));
            kinds.push(c_int::from(matches!(row, Some(Row::Break { .. }))));
            if line_numbers {
                labels.push(match row {
                    Some(Row::Line { line, .. }) => (line + 1).to_string(),
                    _ => String::new(),
                });
            }
        }
        if rows != last.rows || kinds != last.kinds || access != last.access {
            let changed_from = rows
                .iter()
                .zip(&last.rows)
                .zip(kinds.iter().zip(&last.kinds))
                .enumerate()
                .take_while(|(k, ((a, b), (c, d)))| a == b && c == d && access.get(*k) == last.access.get(*k))
                .count();
            let cstrings = |v: &[String]| -> Vec<CString> { v.iter().map(|s| CString::new(s.replace('\0', " ")).unwrap_or_default()).collect() };
            let (c, ca) = (cstrings(&rows), cstrings(&access));
            let p: Vec<*const c_char> = c.iter().map(|s| s.as_ptr()).collect();
            let pa: Vec<*const c_char> = ca.iter().map(|s| s.as_ptr()).collect();
            // The screen reader text, when it is written otherwise than what is drawn.
            let access_ptr = if pa.is_empty() { std::ptr::null() } else { pa.as_ptr() };
            unsafe { ffi::wxd_DocEditor_SetRows(self.0, p.as_ptr(), kinds.as_ptr(), access_ptr, p.len() as c_int, changed_from as c_int) };
            last.rows = rows;
            last.access = access;
            last.kinds = kinds;
        }
        if labels != last.labels {
            let c: Vec<CString> = labels.iter().map(|s| CString::new(s.as_str()).unwrap_or_default()).collect();
            let p: Vec<*const c_char> = c.iter().map(|s| s.as_ptr()).collect();
            let width = if line_numbers { LINE_NUMBER_WIDTH } else { 0 };
            unsafe { ffi::wxd_DocEditor_SetLineNumbers(self.0, p.as_ptr(), p.len() as c_int, width) };
            last.labels = labels;
        }
        let (row, col) = ed.caret_place();
        let (arow, acol) = ed.place(ed.anchor);
        let keep = keep_from_top.map_or(-1, |k| k as c_int);
        unsafe { ffi::wxd_DocEditor_SetCaret(self.0, row as c_int, col as c_int, arow as c_int, acol as c_int, keep) };
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
