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
    kinds: Vec<c_int>,
    labels: Vec<String>,
}

impl Area {
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    /// Gives the window `ed`'s rows (when they changed), the line numbers (`line_numbers`: the formatted view of a
    /// braille document with them on, spec 24 2d.2) and the caret and selection; `keep_from_top` keeps the caret
    /// row that many rows from the top (a view switch, spec 27 2c).
    pub fn show(&self, ed: &Editor, last: &mut ShownRows, line_numbers: bool, keep_from_top: Option<usize>) {
        if self.0.is_null() {
            return;
        }
        let count = ed.rows();
        let mut rows = Vec::with_capacity(count);
        let mut kinds = Vec::with_capacity(count);
        let mut labels = Vec::with_capacity(if line_numbers { count } else { 0 });
        for r in 0..count {
            let row = ed.row(r);
            rows.push(ed.row_text(r));
            kinds.push(c_int::from(matches!(row, Some(Row::Break { .. }))));
            if line_numbers {
                labels.push(match row {
                    Some(Row::Line { line, .. }) => (line + 1).to_string(),
                    _ => String::new(),
                });
            }
        }
        if rows != last.rows || kinds != last.kinds {
            let changed_from = rows.iter().zip(&last.rows).zip(kinds.iter().zip(&last.kinds)).take_while(|((a, b), (c, d))| a == b && c == d).count();
            let c: Vec<CString> = rows.iter().map(|s| CString::new(s.replace('\0', " ")).unwrap_or_default()).collect();
            let p: Vec<*const c_char> = c.iter().map(|s| s.as_ptr()).collect();
            unsafe { ffi::wxd_DocEditor_SetRows(self.0, p.as_ptr(), kinds.as_ptr(), p.len() as c_int, changed_from as c_int) };
            last.rows = rows;
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
