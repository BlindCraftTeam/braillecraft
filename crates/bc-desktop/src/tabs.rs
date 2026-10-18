//! The tab bar across the top of the main window (a BrailleCraft addition the owner asked for): the open documents
//! as tabs of one notebook (vendor/wxdragon-sys wxd_doc_tabs.h), a page per document, in place of document
//! windows stacked in the main window. The tab carries the document's full name and a close
//! button (Linux and Windows; a middle click closes everywhere); Ctrl+Tab and Ctrl+Shift+Tab move between the tabs,
//! Ctrl+W (Cmd+W on macOS) closes, tabs are dragged to reorder, and Ctrl+Shift+Page Up and Page Down move the
//! current tab. The notebook is the system's tab control, so screen readers read it as a tab list with the active
//! tab marked.
//!
//! The tabs are kept in the order of the program's document list (app::App::docs): tab `i` is document `i`.

use std::ffi::{c_void, CString};
use std::os::raw::c_int;

use wxdragon::prelude::*;
use wxdragon_sys as ffi;

/// The tab bar. Copied freely; the window lives as long as the main window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tabs(*mut ffi::wxd_Window_t);

impl WxWidget for Tabs {
    fn handle_ptr(&self) -> *mut ffi::wxd_Window_t {
        self.0
    }
}

// As the editing window (editor_view): used on the main thread only.
unsafe impl Send for Tabs {}
unsafe impl Sync for Tabs {}

thread_local! {
    /// The one tab bar, for the places that know a document's page but not its index.
    static TABS: std::cell::Cell<*mut ffi::wxd_Window_t> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

extern "C" fn on_changed(_data: *mut c_void, index: c_int) {
    if index >= 0 {
        crate::documents::tab_changed(index as usize);
    }
}

extern "C" fn on_close(_data: *mut c_void, index: c_int) {
    if index >= 0 {
        crate::documents::tab_close(index as usize);
    }
}

extern "C" fn on_moved(_data: *mut c_void, from: c_int, to: c_int) {
    if from >= 0 && to >= 0 {
        crate::documents::tab_moved(from as usize, to as usize);
    }
}

extern "C" fn on_free(_data: *mut c_void) {}

/// Makes the tab bar in `parent` (the main window, whose only child it is, so it fills the window).
pub fn create(parent: &impl WxWidget) -> Tabs {
    let window = unsafe {
        ffi::wxd_DocTabs_Create(parent.handle_ptr(), std::ptr::null_mut(), Some(on_changed), Some(on_close), Some(on_moved), Some(on_free))
    };
    TABS.with(|t| t.set(window));
    Tabs(window)
}

/// Sets the tab text of the document whose page is `page` (its full name).
pub fn set_title(page: &impl WxWidget, title: &str) {
    let tabs = Tabs(TABS.with(|t| t.get()));
    if tabs.0.is_null() {
        return;
    }
    let index = unsafe { ffi::wxd_DocTabs_FindPage(tabs.0, page.handle_ptr()) };
    if index >= 0 {
        tabs.set_page_text(index as usize, title);
    }
}

/// A screen rectangle, as the window reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Tabs {
    /// Adds `page` as the last tab, selected when `select`; its index.
    pub fn add_page(&self, page: &impl WxWidget, text: &str, select: bool) -> usize {
        let c = CString::new(text).unwrap_or_default();
        let index = unsafe { ffi::wxd_DocTabs_AddPage(self.0, page.handle_ptr(), c.as_ptr(), select) };
        index.max(0) as usize
    }

    pub fn set_page_text(&self, index: usize, text: &str) {
        let c = CString::new(text).unwrap_or_default();
        unsafe { ffi::wxd_DocTabs_SetPageText(self.0, index as c_int, c.as_ptr()) };
    }

    /// The selected tab, if any.
    pub fn selection(&self) -> Option<usize> {
        let index = unsafe { ffi::wxd_DocTabs_GetSelection(self.0) };
        (index >= 0).then_some(index as usize)
    }

    /// Selects a tab; the change is reported (documents::tab_changed).
    pub fn set_selection(&self, index: usize) {
        unsafe { ffi::wxd_DocTabs_SetSelection(self.0, index as c_int) };
    }

    /// Selects a tab without reporting it.
    pub fn change_selection(&self, index: usize) {
        unsafe { ffi::wxd_DocTabs_ChangeSelection(self.0, index as c_int) };
    }

    pub fn count(&self) -> usize {
        unsafe { ffi::wxd_DocTabs_GetPageCount(self.0) }.max(0) as usize
    }

    /// Takes a tab out; its page window is kept for the caller to destroy.
    pub fn remove_page(&self, index: usize) -> bool {
        unsafe { ffi::wxd_DocTabs_RemovePage(self.0, index as c_int) }
    }

    /// Moves a tab; the move is reported (documents::tab_moved).
    pub fn move_page(&self, from: usize, to: usize) -> bool {
        unsafe { ffi::wxd_DocTabs_MovePage(self.0, from as c_int, to as c_int) }
    }

    /// The tab's rectangle on the screen, where the system reports it.
    pub fn tab_rect(&self, index: usize) -> Option<Rect> {
        let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
        unsafe { ffi::wxd_DocTabs_TabRect(self.0, index as c_int, &mut x, &mut y, &mut w, &mut h) }
            .then_some(Rect { x, y, width: w, height: h })
    }

    /// The close button's rectangle on the screen, where the tab has one.
    pub fn close_button_rect(&self, index: usize) -> Option<Rect> {
        let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
        unsafe { ffi::wxd_DocTabs_CloseButtonRect(self.0, index as c_int, &mut x, &mut y, &mut w, &mut h) }
            .then_some(Rect { x, y, width: w, height: h })
    }
}

/// Where a document list's active index lands after the element at `from` moves to `to` (the other elements shift
/// to make room): the same document stays active.
pub fn moved_index(active: usize, from: usize, to: usize) -> usize {
    if active == from {
        to
    } else if from < active && to >= active {
        active - 1
    } else if from > active && to <= active {
        active + 1
    } else {
        active
    }
}

#[cfg(test)]
mod tests {
    use super::moved_index;

    #[test]
    fn the_active_document_keeps_its_place_through_a_move() {
        // The active tab is the one moved.
        assert_eq!(moved_index(0, 0, 2), 2);
        assert_eq!(moved_index(2, 2, 0), 0);
        // A tab before the active one moves past it: the active one shifts left.
        assert_eq!(moved_index(2, 0, 2), 1);
        assert_eq!(moved_index(2, 1, 3), 1);
        // A tab after the active one moves before it: the active one shifts right.
        assert_eq!(moved_index(1, 3, 0), 2);
        assert_eq!(moved_index(1, 2, 1), 2);
        // A move on the far side leaves it alone.
        assert_eq!(moved_index(0, 1, 3), 0);
        assert_eq!(moved_index(3, 0, 1), 3);
    }

    #[test]
    fn a_list_moved_the_same_way_agrees() {
        let mut names = vec!["a", "b", "c", "d"];
        let active = 2;
        let (from, to) = (0, 3);
        let moved = names.remove(from);
        names.insert(to, moved);
        assert_eq!(names, ["b", "c", "d", "a"]);
        assert_eq!(names[moved_index(active, from, to)], "c");
    }
}
