//! The open documents as tabs (a BrailleCraft addition the owner asked for), as a browser shows its pages: each tab
//! carries the document's full name and a close button; Ctrl+Tab and Ctrl+Shift+Tab move between the tabs, Ctrl+W
//! (Cmd+W on macOS) closes one, tabs are dragged to reorder, and Ctrl+Shift+Page Up and Page Down move the current
//! tab. The tabs are the system's own, so screen readers read them as a tab list with every tab named by its
//! document and the active one marked.
//!
//! - Windows and Linux: the tab bar across the top of the main window (vendor/wxdragon-sys wxd_doc_tabs.h), the
//!   system's tab control with a page per document. On Linux a real close button sits in each tab; on Windows the
//!   tabs are drawn by the program with the close sign at the right of the text, and a tab has a context menu (Close
//!   Tab, Move Tab Left, Move Tab Right). The tabs are kept in the order of the program's document list
//!   (app::App::docs): tab `i` is document `i`.
//! - macOS: each document is a window of its own, and the document windows are the tabs of one window, as in the
//!   system's own programs (vendor/wxdragon-sys wxd_window_tabs.h): AppKit's tab bar, its close buttons and plus
//!   button, Cmd+Shift+] and Cmd+Shift+[, and the Window menu's tab items. The program's own frame holds the menu bar
//!   and is never shown. AppKit tells nobody when a tab is dragged, so the program's list is put in the tabs' order
//!   (reconcile) whenever a document window becomes active, the View menu opens, or the list is used.

use std::ffi::{c_void, CString};
use std::os::raw::c_int;

use wxdragon::prelude::*;
use wxdragon_sys as ffi;

/// Whether the documents are window tabs (macOS) rather than the pages of the tab bar.
pub const WINDOW_TABS: bool = cfg!(target_os = "macos");

/// The tab bar (Windows and Linux). Copied freely; the window lives as long as the main window. On macOS there is
/// none (Tabs::none): every call on it does nothing and answers no tab.
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
    /// No tab bar (macOS, where the documents are window tabs).
    pub fn none() -> Tabs {
        Tabs(std::ptr::null_mut())
    }

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

    /// The keyboard focus on the tab bar with the focus rectangle shown (Windows), for the tab pictures.
    pub fn show_focus(&self) {
        unsafe { ffi::wxd_DocTabs_ShowFocus(self.0) };
    }
}

// ------------------------------------------------------------------ window tabs (macOS)
//
// The functions do nothing on the other systems (vendor/wxdragon-sys cpp/src/window_tabs.cpp), so the program calls
// them where WINDOW_TABS says so and the code is the same on every system.

/// The plus button of a document window's tab bar: File, New, as the menu gives it.
extern "C" fn on_new_tab(_data: *mut c_void) {
    wxdragon::call_after(Box::new(|| crate::commands::run(1101, false)));
}

/// Window tabbing on for the program, the program frame `host` (which holds the menu bar and is never shown) kept
/// out of it.
pub fn enable(host: &Frame) {
    unsafe { ffi::wxd_WindowTabs_Enable(host.handle_ptr()) };
}

/// Makes `window` a document window, before it is first shown.
pub fn prepare(window: &Frame) {
    unsafe { ffi::wxd_WindowTabs_Prepare(window.handle_ptr(), std::ptr::null_mut(), Some(on_new_tab)) };
}

/// Adds `window` as the last tab of `into`'s window; false when it opens as a window of its own.
pub fn add(into: &Frame, window: &Frame) -> bool {
    unsafe { ffi::wxd_WindowTabs_Add(into.handle_ptr(), window.handle_ptr()) }
}

/// Shows `window`'s tab bar when it is hidden.
pub fn show_tab_bar(window: &Frame) {
    unsafe { ffi::wxd_WindowTabs_ShowTabBar(window.handle_ptr()) };
}

/// Selects `window`'s tab and brings the window to the front.
pub fn select(window: &Frame) {
    unsafe { ffi::wxd_WindowTabs_Select(window.handle_ptr()) };
}

/// The next or previous tab of `window`'s group; false when the group has one tab.
pub fn step(window: &Frame, forward: bool) -> bool {
    unsafe { ffi::wxd_WindowTabs_Step(window.handle_ptr(), forward) }
}

/// Moves `window`'s tab to place `to` in its group.
pub fn move_to(window: &Frame, to: usize) -> bool {
    unsafe { ffi::wxd_WindowTabs_Move(window.handle_ptr(), to as c_int) }
}

/// The windows of `window`'s tab group in tab order (the window alone when it has no group).
pub fn group_order(window: &Frame) -> Vec<*mut ffi::wxd_Window_t> {
    let n = unsafe { ffi::wxd_WindowTabs_GroupOrder(window.handle_ptr(), std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let mut out = vec![std::ptr::null_mut(); n as usize];
    let got = unsafe { ffi::wxd_WindowTabs_GroupOrder(window.handle_ptr(), out.as_mut_ptr(), n) };
    out.truncate(got.clamp(0, n) as usize);
    out
}

/// The edited mark in the close button of `window` and of its tab.
pub fn set_edited(window: &Frame, edited: bool) {
    unsafe { ffi::wxd_WindowTabs_SetEdited(window.handle_ptr(), edited) };
}

/// Whether `window` is the key window.
pub fn is_key(window: &Frame) -> bool {
    unsafe { ffi::wxd_WindowTabs_IsKey(window.handle_ptr()) }
}

/// Every item of the main menu, a "Menu > Item" line each, for the tab pictures' report (macOS; empty elsewhere).
pub fn menu_dump() -> String {
    let n = unsafe { ffi::wxd_WindowTabs_MenuDump(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return String::new();
    }
    let mut buf = vec![0u8; n as usize + 1];
    unsafe { ffi::wxd_WindowTabs_MenuDump(buf.as_mut_ptr() as *mut std::os::raw::c_char, buf.len() as c_int) };
    buf.truncate(n as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

/// The program's document list (document keys) put in the order of the tab groups: `groups` is each tab group's
/// documents in tab order. Each group's documents keep the places in the list its documents already have, filled in
/// the group's tab order; a document in no group keeps its place, and a document given in no group or twice counts
/// once. So a tab dragged inside a group reorders only that group's documents, a tab moved to a window of its own
/// changes nothing, and merging every window into one takes the tab order of the merged window.
pub fn reconcile(docs: &[u64], groups: &[Vec<u64>]) -> Vec<u64> {
    let mut out = docs.to_vec();
    let mut placed: Vec<u64> = Vec::new();
    for group in groups {
        let mut members: Vec<u64> = Vec::new();
        for k in group {
            if docs.contains(k) && !placed.contains(k) && !members.contains(k) {
                members.push(*k);
            }
        }
        let mut slots: Vec<usize> = members.iter().filter_map(|k| docs.iter().position(|d| d == k)).collect();
        slots.sort_unstable();
        for (slot, key) in slots.into_iter().zip(members.iter()) {
            out[slot] = *key;
        }
        placed.extend(members);
    }
    out
}

/// What a key does to the tabs (the editing area hands every key to tab_key first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKey {
    Next,
    Previous,
    Close,
    MoveLeft,
    MoveRight,
}

// wxWidgets key codes and modifier bits.
const TAB: i32 = 9;
const PAGE_UP: i32 = 366;
const PAGE_DOWN: i32 = 367;
const MOD_ALT: i32 = 1;
const MOD_CONTROL: i32 = 2;
const MOD_SHIFT: i32 = 4;
const MOD_META: i32 = 8;
/// The Control key itself on macOS, where MOD_CONTROL is the Command key (wxMOD_RAW_CONTROL).
const MOD_RAW_CONTROL: i32 = 16;

/// The tab keys: Control+Tab and Control+Shift+Tab (the Control key itself on macOS too) the next and previous
/// document; on macOS Cmd+Shift+] and Cmd+Shift+[ as well, as the key code comes with Shift (] and [) or without it
/// (} and {); Ctrl+W (Cmd+W on macOS) close; Control+Shift+Page Up and Page Down move the tab left and right. Not with
/// Alt (Option).
pub fn tab_key(code: i32, mods: i32, macos: bool) -> Option<TabKey> {
    let control = mods & MOD_CONTROL != 0;
    let raw_control = if macos { mods & MOD_RAW_CONTROL != 0 } else { control };
    let shift = mods & MOD_SHIFT != 0;
    if mods & (MOD_ALT | MOD_META) != 0 {
        return None;
    }
    if raw_control && code == TAB {
        return Some(if shift { TabKey::Previous } else { TabKey::Next });
    }
    if macos && control && shift && !raw_control {
        match code {
            93 | 125 => return Some(TabKey::Next),
            91 | 123 => return Some(TabKey::Previous),
            _ => {}
        }
    }
    if control && !shift && (code == i32::from(b'W') || code == i32::from(b'w')) {
        return Some(TabKey::Close);
    }
    if raw_control && shift && (code == PAGE_UP || code == PAGE_DOWN) {
        return Some(if code == PAGE_UP { TabKey::MoveLeft } else { TabKey::MoveRight });
    }
    None
}

/// Ctrl+[ ('[': the Code prompt) and Ctrl+] (']': the Character prompt), Cmd on macOS; with Shift up, so that
/// Cmd+Shift+] and Cmd+Shift+[ stay the tab keys.
pub fn bracket_prompt(code: i32, mods: i32) -> Option<char> {
    let control = mods & MOD_CONTROL != 0;
    let shift = mods & MOD_SHIFT != 0;
    let alt = mods & (MOD_ALT | MOD_META) != 0;
    if !control || shift || alt {
        return None;
    }
    match code {
        91 => Some('['),
        93 => Some(']'),
        _ => None,
    }
}

// ------------------------------------------------------------------ the tab pictures of the build checks

/// The tab pictures (BC_LAYOUT_TABS=<report file>, for the build checks): three documents are opened, Untitled Print
/// Document 1, Untitled Braille Document 1 (one character typed, so it has unsaved changes) and Untitled Print
/// Document 2 (the active one), then one step a second and a half apart, each writing what it found to the report;
/// the program is left running for the picture and is stopped by the check. `stage` (BC_LAYOUT_TABS_STAGE) picks
/// what is done after the report: nothing (the picture of the three tabs), "hover" (the pointer on the second tab's
/// close sign), "focus" (the keyboard focus on the tab bar), "close" (the active document closed), "click" (a click
/// at the left end of the first tab, which must not close it, then on its close sign, which must).
pub fn picture_run(path: String, stage: String) {
    use crate::app::{self, DocKind};
    use bc_core::doctext::Shown;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::Write;
    use std::rc::Rc;

    let _ = std::fs::write(&path, "");
    let report = {
        let path = path.clone();
        move |line: String| {
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&path) {
                let _ = writeln!(f, "{line}");
            }
        }
    };
    let open = |kind: DocKind| {
        let key = crate::documents::new_document(kind, None, &Shown::default());
        app::with(|a| {
            if let Some(i) = a.index_of(key) {
                a.docs[i].dirty = false;
            }
        });
        key
    };
    open(DocKind::Print);
    let braille = open(DocKind::Braille);
    app::with(|a| {
        if let Some(i) = a.index_of(braille) {
            crate::documents::type_in(a, i, &Shown::plain("a"));
        }
    });
    let last = open(DocKind::Print);
    crate::documents::bring_to_front(app::with(|a| a.index_of(last)).unwrap_or(0));

    // What the program and the tabs say: the document list, the active document, the tabs as the system has them.
    let state = {
        let report = report.clone();
        move |when: &str| {
            let lines = app::with(|a| {
                a.reconcile_tabs();
                let mut lines = Vec::new();
                let names: Vec<String> = a.docs.iter().map(|d| d.full_name()).collect();
                lines.push(format!("{when}: documents: {}", names.join(" | ")));
                lines.push(format!("{when}: active: {}", a.doc().map(|d| d.full_name()).unwrap_or_default()));
                lines.push(format!("{when}: dirty: {:?}", a.docs.iter().map(|d| d.dirty).collect::<Vec<_>>()));
                if WINDOW_TABS {
                    let name_of = |p: *mut ffi::wxd_Window_t| {
                        a.docs
                            .iter()
                            .find(|d| d.window.is_some_and(|w| w.handle_ptr() == p))
                            .map(|d| d.full_name())
                            .unwrap_or_else(|| "(not a document)".into())
                    };
                    if let Some(w) = a.doc().and_then(|d| d.window) {
                        let group: Vec<String> = group_order(&w).into_iter().map(name_of).collect();
                        lines.push(format!("{when}: tab group: {}", group.join(" | ")));
                        lines.push(format!("{when}: tab group agrees with the documents: {}", group == names));
                    }
                    let key: Vec<String> =
                        a.docs.iter().filter(|d| d.window.is_some_and(|w| is_key(&w))).map(|d| d.full_name()).collect();
                    lines.push(format!("{when}: key window: {}", key.join(" | ")));
                    lines.push(format!(
                        "{when}: active is the key window: {}",
                        a.doc().and_then(|d| d.window).is_some_and(|w| is_key(&w))
                    ));
                } else {
                    lines.push(format!("{when}: tabs: {} selected {:?}", a.tabs.count(), a.tabs.selection()));
                    for i in 0..a.docs.len() {
                        lines.push(format!(
                            "{when}: tab {i}: {:?} close sign {:?}",
                            a.tabs.tab_rect(i),
                            a.tabs.close_button_rect(i)
                        ));
                    }
                }
                lines
            });
            for l in lines {
                report(l);
            }
        }
    };

    let steps: Rc<RefCell<VecDeque<Box<dyn FnOnce()>>>> = Rc::default();
    let mut list: Vec<Box<dyn FnOnce()>> = Vec::new();
    {
        let state = state.clone();
        let report = report.clone();
        list.push(Box::new(move || {
            state("opened");
            if WINDOW_TABS {
                for line in menu_dump().lines() {
                    report(format!("menu: {line}"));
                }
            }
            // The active (third) tab one place left and back: the list follows the tabs both times.
            crate::documents::move_tab(last, true);
            state("moved left");
            crate::documents::move_tab(last, false);
            state("moved back");
        }));
    }
    match stage.as_str() {
        "hover" => list.push(Box::new(move || {
            let rect = app::with(|a| a.tabs.close_button_rect(1));
            if let Some(r) = rect {
                UIActionSimulator::new().mouse_move(r.x + r.width / 2, r.y + r.height / 2);
            }
        })),
        "focus" => list.push(Box::new(|| app::with(|a| a.tabs).show_focus())),
        "close" => {
            list.push(Box::new(move || crate::commands::close_document(Some(last))));
            let state = state.clone();
            list.push(Box::new(move || state("closed")));
        }
        "click" => {
            let r1 = report.clone();
            list.push(Box::new(move || {
                // The left end of the first tab, where the close sign used to be: it must not close the tab.
                let rect = app::with(|a| a.tabs.tab_rect(0));
                if let Some(r) = rect {
                    let sim = UIActionSimulator::new();
                    sim.mouse_move(r.x + 10, r.y + r.height / 2);
                    sim.mouse_click(MouseButton::Left);
                }
                r1(format!("clicked the left end of the first tab: {rect:?}"));
            }));
            let state1 = state.clone();
            let r2 = report.clone();
            list.push(Box::new(move || {
                state1("after the left end click");
                let rect = app::with(|a| a.tabs.close_button_rect(0));
                if let Some(r) = rect {
                    let sim = UIActionSimulator::new();
                    sim.mouse_move(r.x + r.width / 2, r.y + r.height / 2);
                    sim.mouse_click(MouseButton::Left);
                }
                r2(format!("clicked the first tab's close sign: {rect:?}"));
            }));
            let state2 = state.clone();
            list.push(Box::new(move || state2("after the close sign click")));
        }
        _ => {}
    }
    {
        let report = report.clone();
        list.push(Box::new(move || report("done".into())));
    }
    steps.borrow_mut().extend(list);
    let timer = wxdragon::timer::Timer::new(&crate::app::host());
    timer.on_tick(move |_| {
        let next = steps.borrow_mut().pop_front();
        if let Some(step) = next {
            step();
        }
    });
    timer.start(1500, false);
    // Kept for the life of the program, which the check ends by stopping it.
    std::mem::forget(timer);
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
    use super::{bracket_prompt, moved_index, reconcile, tab_key, TabKey};

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

    const CONTROL: i32 = 2;
    const SHIFT: i32 = 4;
    const ALT: i32 = 1;
    const RAW_CONTROL: i32 = 16;

    #[test]
    fn reconcile_keeps_an_unchanged_order() {
        assert_eq!(reconcile(&[1, 2, 3], &[vec![1, 2, 3]]), [1, 2, 3]);
        assert_eq!(reconcile(&[1, 2, 3], &[]), [1, 2, 3]);
    }

    #[test]
    fn reconcile_follows_a_tab_dragged_in_one_window() {
        // Documents a b c in one window; c dragged to the front.
        assert_eq!(reconcile(&[1, 2, 3], &[vec![3, 1, 2]]), [3, 1, 2]);
    }

    #[test]
    fn reconcile_reorders_only_the_window_whose_tabs_moved() {
        // a b in one window, c d in another, whose tabs are now d c.
        assert_eq!(reconcile(&[1, 2, 3, 4], &[vec![1, 2], vec![4, 3]]), [1, 2, 4, 3]);
    }

    #[test]
    fn reconcile_keeps_the_list_when_a_tab_moves_to_a_window_of_its_own() {
        // b moved out to a new window: groups a c and b.
        assert_eq!(reconcile(&[1, 2, 3], &[vec![1, 3], vec![2]]), [1, 2, 3]);
    }

    #[test]
    fn reconcile_takes_the_order_of_merged_windows() {
        assert_eq!(reconcile(&[1, 2, 3, 4], &[vec![3, 1, 2, 4]]), [3, 1, 2, 4]);
    }

    #[test]
    fn reconcile_leaves_a_document_in_no_group_in_its_place() {
        // c is in no group (its window closing); a and d swapped round it.
        assert_eq!(reconcile(&[1, 2, 3, 4], &[vec![4, 2, 1]]), [4, 2, 3, 1]);
        // A window that is no document, and a document named twice, are passed over.
        assert_eq!(reconcile(&[1, 2], &[vec![9, 2, 1], vec![1]]), [2, 1]);
    }

    #[test]
    fn reconcile_lets_the_active_document_be_found_again_by_key() {
        let docs = [10, 20, 30, 40];
        let active = docs[2];
        let order = reconcile(&docs, &[vec![40, 30, 20, 10]]);
        assert_eq!(order.iter().position(|k| *k == active), Some(1));
    }

    #[test]
    fn tab_key_control_tab_moves_between_documents_on_every_system() {
        assert_eq!(tab_key(9, CONTROL, false), Some(TabKey::Next));
        assert_eq!(tab_key(9, CONTROL | SHIFT, false), Some(TabKey::Previous));
        // On macOS the Control key itself (Command+Tab belongs to the system).
        assert_eq!(tab_key(9, RAW_CONTROL, true), Some(TabKey::Next));
        assert_eq!(tab_key(9, RAW_CONTROL | SHIFT, true), Some(TabKey::Previous));
        assert_eq!(tab_key(9, CONTROL, true), None);
        assert_eq!(tab_key(9, 0, false), None);
    }

    #[test]
    fn tab_key_command_shift_brackets_on_macos_only() {
        for code in [93, 125] {
            assert_eq!(tab_key(code, CONTROL | SHIFT, true), Some(TabKey::Next));
            assert_eq!(tab_key(code, CONTROL | SHIFT, false), None);
        }
        for code in [91, 123] {
            assert_eq!(tab_key(code, CONTROL | SHIFT, true), Some(TabKey::Previous));
            assert_eq!(tab_key(code, CONTROL | SHIFT, false), None);
        }
        // With Option it is not a tab key.
        assert_eq!(tab_key(93, CONTROL | SHIFT | ALT, true), None);
    }

    #[test]
    fn tab_key_close_and_moves() {
        assert_eq!(tab_key(87, CONTROL, false), Some(TabKey::Close));
        assert_eq!(tab_key(87, CONTROL, true), Some(TabKey::Close));
        assert_eq!(tab_key(87, CONTROL | SHIFT, false), None);
        assert_eq!(tab_key(87, 0, false), None);
        assert_eq!(tab_key(366, CONTROL | SHIFT, false), Some(TabKey::MoveLeft));
        assert_eq!(tab_key(367, CONTROL | SHIFT, false), Some(TabKey::MoveRight));
        assert_eq!(tab_key(366, RAW_CONTROL | SHIFT, true), Some(TabKey::MoveLeft));
        assert_eq!(tab_key(367, CONTROL, false), None);
    }

    #[test]
    fn tab_key_leaves_the_bracket_prompts_alone() {
        // Ctrl+[ and Ctrl+] without Shift are the Code and Character prompts, not tab keys.
        assert_eq!(tab_key(91, CONTROL, true), None);
        assert_eq!(tab_key(93, CONTROL, false), None);
        assert_eq!(bracket_prompt(91, CONTROL), Some('['));
        assert_eq!(bracket_prompt(93, CONTROL), Some(']'));
        // Cmd+Shift+] and Cmd+Shift+[ are not prompts.
        assert_eq!(bracket_prompt(93, CONTROL | SHIFT), None);
        assert_eq!(bracket_prompt(91, CONTROL | SHIFT), None);
        assert_eq!(bracket_prompt(93, CONTROL | ALT), None);
        assert_eq!(bracket_prompt(93, 0), None);
    }
}
