//! Document windows: each open document is a page of the tab bar across the top of the main window (Windows, Linux),
//! or a window of its own that is one of the window tabs (macOS) (tabs.rs, a BrailleCraft addition in place of
//! stacked document windows), holding the editing area (the program-drawn editing window, editor_view), under it the
//! one-line translated line on yellow, and at the bottom the document's status line.
//!
//! Every key, typed character and click in the editing area comes here and is carried out on the document's editor
//! model (bc_core::editor; spec 27 section 5, spec 30).

use std::collections::HashSet;

use bc_core::doctext::Shown;
use bc_core::editor::{Editor, Move, Row};
use wxdragon::prelude::*;

use crate::app::{self, Doc, DocKind, Hyphen, View};
use crate::editor_view;
use crate::speech;

/// Opens a new document in a tab of its own with `shown` (its text and codes). `name` None makes an untitled one
/// ("Untitled Print Document 1").
pub fn new_document(kind: DocKind, name: Option<String>, shown: &Shown) -> u64 {
    let (tabs, key, name, untitled, view, into, last_frame) = app::with(|a| {
        a.next_key += 1;
        let untitled = name.is_none();
        let name = name.unwrap_or_else(|| {
            let n = &mut a.untitled[if kind == DocKind::Braille { 1 } else { 0 }];
            *n += 1;
            format!("Untitled {} Document {n}", if kind == DocKind::Braille { "Braille" } else { "Print" })
        });
        let v = &a.settings.default_views;
        let braille = kind == DocKind::Braille;
        let view = View {
            font: if braille { v.braille_font } else { app::ViewFont::Print },
            full_page: if braille { v.braille_full_page } else { v.print_full_page },
            codes: if braille { v.braille_codes } else { v.print_codes },
            translated: if braille { v.braille_translated } else { v.print_translated },
            six_key: braille && v.six_key,
        };
        (a.tabs, a.next_key, name, untitled, view, a.doc().and_then(|d| d.window), a.last_frame)
    });

    // macOS: the document's own window, one of the window tabs; its page fills it.
    let window = crate::tabs::WINDOW_TABS.then(|| document_window(key, &name));
    let page = match window {
        Some(w) => Panel::builder(&w).build(),
        None => Panel::builder(&tabs).build(),
    };
    let area = editor_view::create(&page, key);
    // The status line is made right after the editing area and the translated line after it: screen reader scripts
    // find them as the next window and the one after (spec 17).
    let status = StaticText::builder(&page).with_label("Current Style: None").build();
    app::with(|a| crate::apply_status_font(a, &status));
    let line = TextCtrl::builder(&page).with_style(TextCtrlStyle::ReadOnly).build();
    line.set_background_color(Colour::new(255, 255, 0, 255));
    line.set_foreground_color(Colour::new(0, 0, 0, 255));
    line.set_accessibility_label("Translated line");
    speech::set_name(line.get_handle(), "Translated line");
    // The editing area's accessible name.
    area.set_accessible_name("Document Editing Window");
    line.show(view.translated);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&area, 1, SizerFlag::Expand, 0);
    sizer.add(&line, 0, SizerFlag::Expand, 0);
    sizer.add(&status, 0, SizerFlag::Expand | SizerFlag::All, 2);
    page.set_sizer(sizer, true);

    let braille = kind == DocKind::Braille;
    // The coded view's rows are 42 characters for a braille document and 65 for a print one (spec 27 4b); the page
    // size is set from the document's own when it is first shown (App::show_area).
    let template = crate::translation::default_template();
    let page_numbers = crate::translation::template_page_numbers(&template);
    let styles = crate::translation::template_style_list(&template);
    let (cells, lines) = if braille { (40, 25) } else { (80, 50) };
    // Laid out once, with the document's own layout, so a book is not laid out again at its first show (L1).
    let (ed_layout, layout, style_table) = app::layout_for(cells, lines, page_numbers, braille, &styles, Hyphen::Dashes);
    let ed = Editor::new(shown.clone(), layout, Some(style_table), view.codes, if braille { 42 } else { 65 });
    // A new document's print page is the Global, Printer Setup page size.
    let print_page = app::with(|a| (a.settings.printer.lines, a.settings.printer.chars));
    let doc = Doc {
        key,
        page,
        window,
        edited_shown: false,
        area,
        status,
        line,
        name,
        kind,
        untitled,
        saved: false,
        document_file: false,
        dirty: false,
        path: None,
        // A new document is made with the default template: its tables and its styles.
        tables: crate::translation::template_tables(&crate::translation::default_template())
            .unwrap_or_else(|| crate::translation::DEFAULT_TABLES.map(str::to_string)),
        template,
        cells,
        lines,
        emb_page: { let (_, c, l) = crate::embosser::default_layout(); (c, l) },
        print_page,
        page_numbers,
        pages: None,
        ed,
        ed_layout,
        undo: Vec::new(),
        redo: Vec::new(),
        area_rows: editor_view::ShownRows::default(),
        area_stale: true,
        view,
        hyphen: Hyphen::Dashes,
        styles,
        file_settings: Vec::new(),
        file_styles: Vec::new(),
        highlight_anchor: None,
        find: Default::default(),
        replace: Default::default(),
        replace_with: String::new(),
        last_line_key: String::new(),
        line_lead: Default::default(),
        six_down: HashSet::new(),
        chord: Default::default(),
        last_applied: String::new(),
        printer: None,
        embosser: crate::embosser::default_setup().unwrap_or_default(),
        layout: crate::embosser::default_layout().0,
        file_sections: Vec::new(),
        ebraille: Default::default(),
    };
    let title = doc.full_name();
    app::with(|a| {
        a.docs.push(doc);
        // The document opens at its start, in the view Global, Default Views sets: the formatted view with View,
        // Codes off, else the coded view (spec 27 2a).
        let i = a.docs.len() - 1;
        a.docs[i].select_doc(0, 0);
        a.set_view_codes(i, view.codes);
    });
    // The document's tab, last in the row and selected (the tab bar does not report the selection an add makes;
    // the document is made the active one here). On macOS the window becomes the last tab of the window in front,
    // or a window of its own where the last document window was (else 1000 by 720 in the middle of the screen).
    match window {
        // A command line that runs without the editor shows no document window (the main window is not shown then
        // either).
        Some(w) if !SHOW_WINDOWS.with(|s| s.get()) => w.set_label(&title),
        Some(w) => {
            w.set_label(&title);
            if !into.is_some_and(|into| crate::tabs::add(&into, &w)) {
                match last_frame {
                    Some((at, size)) => w.set_size_with_pos(at.x, at.y, size.width, size.height),
                    None => {
                        w.set_size(Size::new(1000, 720));
                        w.centre();
                    }
                }
            }
            w.show(true);
            let first = app::with(|a| !std::mem::replace(&mut a.tab_bar_shown, true));
            if first {
                crate::tabs::show_tab_bar(&w);
            }
        }
        None => {
            tabs.add_page(&page, &title, true);
        }
    }
    activate_key(key);
    area.set_focus();
    // A dialog that was open (File, Open) gives focus back to the control it came from when it finishes closing;
    // the new document takes it once that is done.
    wxdragon::call_after(Box::new(move || {
        // The document may have been closed before this runs: its windows are then gone.
        let Some(area) = live_area(key) else { return };
        area.set_focus();
        activate_key(key);
    }));
    // The memory the open freed on the way goes back to the system (plan line L1).
    bc_core::memory::release();
    key
}

/// macOS: a document's own window (no menu bar of its own: the program frame's is shown while it is active). Its
/// activation makes the document the active one (a tab chosen, a window brought to the front); its close (the close
/// button of the window or of its tab, Cmd+W from the Window menu) is File, Close for the document, with the question
/// about unsaved changes.
fn document_window(key: u64, title: &str) -> Frame {
    let window = Frame::builder().with_title(title).with_size(Size::new(1000, 720)).build();
    crate::tabs::prepare(&window);
    window.on_activate(move |e| {
        if let WindowEventData::Activate(a) = &e {
            if a.is_active() {
                wxdragon::call_after(Box::new(move || window_activated(key)));
                run_start_hook();
            }
            // wxWidgets puts the menu bar up as the window becomes active.
            a.event.skip(true);
        }
    });
    window.on_close(move |e| {
        if let WindowEventData::General(ev) = &e {
            if ev.can_veto() {
                ev.veto();
            }
        }
        wxdragon::call_after(Box::new(move || crate::commands::close_document(Some(key))));
    });
    window
}

/// macOS: document `key`'s window became active (its tab chosen, the window clicked, a neighbour's tab closed): the
/// document list follows the tabs' order, the document becomes the active one and its editing area takes the
/// keyboard.
pub fn window_activated(key: u64) {
    app::try_with(|a| {
        a.reconcile_tabs();
    });
    activate_key(key);
    if let Some(area) = live_area(key) {
        area.set_focus();
    }
}

thread_local! {
    /// Whether a new document's window is shown (macOS): not while a command line runs without the editor.
    static SHOW_WINDOWS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Document windows are not shown from here on (macOS; a command line that runs without the editor).
pub fn hide_new_windows() {
    SHOW_WINDOWS.with(|s| s.set(false));
}

thread_local! {
    /// What runs once at start-up when the program's first window is active (Embosser Setup when none is set up, or
    /// the command of a picture run), taken by the first to come: the main window's activation (Windows, Linux), a
    /// document window's (macOS), the start with no document (macOS), or a picture run's timer.
    static START_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

pub fn set_start_hook(hook: Box<dyn FnOnce()>) {
    START_HOOK.with(|h| *h.borrow_mut() = Some(hook));
}

/// Runs the start-up hook if it has not run.
pub fn run_start_hook() {
    let hook = START_HOOK.with(|h| h.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

/// macOS: the Dock icon clicked. With no document open a new untitled print document is opened (the program has no
/// window without one); else the active document's window comes to the front.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn reopen() {
    let (empty, active) = app::with(|a| (a.docs.is_empty(), a.active.unwrap_or(0)));
    if empty {
        let key = new_document(DocKind::Print, None, &Shown::default());
        app::with(|a| {
            if let Some(i) = a.index_of(key) {
                a.docs[i].dirty = false;
            }
        });
    } else {
        bring_to_front(active);
    }
}

/// The editing area of the document with this key, while it is open. A call put off until later
/// (wxdragon::call_after) looks it up here, never keeps it: a document closed meanwhile has no windows left.
fn live_area(key: u64) -> Option<editor_view::Area> {
    let mut found = None;
    app::try_with(|a| found = a.index_of(key).map(|i| a.docs[i].area));
    found
}

/// The key of the document at `index` in the document list (the order of the tabs), while the state is free.
fn key_at(index: usize) -> Option<u64> {
    let mut key = None;
    app::try_with(|a| key = a.docs.get(index).map(|d| d.key));
    key
}

/// The key code of the Shift key (WXK_SHIFT).
const SHIFT_KEY: i32 = 306;

/// Shows the Cursor Locator over document `key`'s editing area (spec 24 2d.2): a rectangle closing in on the caret.
pub fn show_cursor_locator(key: u64) {
    let mut area = None;
    app::try_with(|a| area = a.index_of(key).map(|i| a.docs[i].area));
    if let Some(area) = area {
        area.show_cursor_locator();
    }
}

/// Makes the document with this key the active one; its tab follows.
pub fn activate_key(key: u64) {
    app::try_with(|a| {
        if let Some(i) = a.index_of(key) {
            a.active = Some(i);
            if a.tabs.selection() != Some(i) {
                a.tabs.change_selection(i);
            }
            a.set_title();
            a.apply_view();
        }
    });
}

/// Brings document `index` to the front (View menu list, the control channel): its tab is selected and its editing
/// area takes the keyboard.
pub fn bring_to_front(index: usize) {
    let target = app::with(|a| a.docs.get(index).map(|d| (a.tabs, d.area, d.key, d.window)));
    if let Some((tabs, area, key, window)) = target {
        match window {
            Some(w) => crate::tabs::select(&w),
            None => {
                if tabs.selection() != Some(index) {
                    tabs.set_selection(index);
                }
            }
        }
        activate_key(key);
        area.set_focus();
    }
}

/// Closes the document with this key without asking (the caller has asked): its tab and its page go, and the
/// document the tab bar selects in its place becomes the active one. On macOS the document's window goes (its tab
/// with it, or the window when it was the last tab); the document whose window the system makes active then becomes
/// the active one (window_activated), the neighbour in the list until then.
pub fn destroy(key: u64) {
    let removed = app::with(|a| {
        let i = a.index_of(key)?;
        let d = a.docs.remove(i);
        a.active = None;
        if let Some(w) = d.window {
            a.last_frame = Some((w.get_position(), w.get_size()));
        }
        Some((a.tabs, d.page, d.window, i))
    });
    let Some((tabs, page, window, i)) = removed else { return };
    tabs.remove_page(i);
    page.destroy();
    if let Some(w) = window {
        w.destroy();
    }
    let next = app::with(|a| {
        let n = a.docs.len();
        let at = if window.is_some() { i.min(n.saturating_sub(1)) } else { tabs.selection().filter(|s| *s < n).unwrap_or(n.saturating_sub(1)) };
        (n > 0).then(|| a.docs[at].key)
    });
    match next {
        Some(k) => {
            activate_key(k);
            if let Some(area) = live_area(k) {
                area.set_focus();
            }
        }
        None => app::with(|a| a.set_title()),
    }
    app::with(|a| {
        a.refresh_menus();
        a.update_status();
    });
}

/// The tab bar selected tab `index` (a click on it, the keys in the tab bar): its document becomes the active one
/// and its editing area takes the keyboard once the tab bar has finished switching.
pub fn tab_changed(index: usize) {
    let Some(key) = key_at(index) else { return };
    activate_key(key);
    wxdragon::call_after(Box::new(move || {
        if let Some(area) = live_area(key) {
            area.set_focus();
        }
    }));
}

/// The close button of tab `index` (or a middle click on it, or Ctrl+W in the tab bar): as File, Close, with the
/// question about unsaved changes.
pub fn tab_close(index: usize) {
    if let Some(key) = key_at(index) {
        wxdragon::call_after(Box::new(move || crate::commands::close_document(Some(key))));
    }
}

/// Tab `from` was moved to `to` (a drag, or Ctrl+Shift+Page Up or Page Down): the document list follows, so the
/// View menu's list and every index agree with the tabs.
pub fn tab_moved(from: usize, to: usize) {
    app::try_with(|a| {
        if from >= a.docs.len() || to >= a.docs.len() {
            return;
        }
        let d = a.docs.remove(from);
        a.docs.insert(to, d);
        if let Some(active) = a.active {
            a.active = Some(crate::tabs::moved_index(active, from, to));
        }
        a.refresh_menus();
    });
}

/// Ctrl+Tab (forward) and Ctrl+Shift+Tab in the editing area (Cmd+Shift+] and Cmd+Shift+[ on macOS too): the next
/// or previous document, wrapping. On macOS the next tab of the window as the tabs stand; for a window with one tab,
/// the next document in the list (in another window).
pub fn switch_tab(forward: bool) {
    let (tabs, n, active, window) = app::with(|a| {
        a.reconcile_tabs();
        (a.tabs, a.docs.len(), a.active, a.doc().and_then(|d| d.window))
    });
    if n < 2 {
        return;
    }
    if let Some(w) = window {
        if crate::tabs::step(&w, forward) {
            return;
        }
    }
    let at = tabs.selection().or(active).unwrap_or(0);
    bring_to_front(if forward { (at + 1) % n } else { (at + n - 1) % n });
}

/// Ctrl+Shift+Page Up and Page Down in the editing area: the document's tab one place left or right. On macOS the
/// tab moves in its window, and the document list follows the tabs.
pub fn move_tab(key: u64, up: bool) {
    let window = app::with(|a| a.index_of(key).and_then(|i| a.docs[i].window));
    if let Some(w) = window {
        let order = crate::tabs::group_order(&w);
        let Some(i) = order.iter().position(|p| *p == w.handle_ptr()) else { return };
        let to = if up { i.checked_sub(1) } else { (i + 1 < order.len()).then_some(i + 1) };
        if let Some(to) = to {
            crate::tabs::move_to(&w, to);
            app::with(|a| {
                a.reconcile_tabs();
            });
        }
        return;
    }
    let Some((tabs, i, n)) = app::with(|a| a.index_of(key).map(|i| (a.tabs, i, a.docs.len()))) else { return };
    let to = if up { i.checked_sub(1) } else { (i + 1 < n).then_some(i + 1) };
    if let Some(to) = to {
        tabs.move_page(i, to);
    }
}

thread_local! {
    /// Shift was pressed with no other key since: letting it go shows the Cursor Locator when Global, View
    /// Preferences asks for it (spec 24 2d.2).
    static SHIFT_ALONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// wxWidgets key codes and modifier bits.
const BACK: i32 = 8;
const TAB: i32 = 9;
const SPACE: i32 = 32;
const ENTER: i32 = 13;
const ESCAPE: i32 = 27;
const DELETE: i32 = 127;
const END: i32 = 312;
const HOME: i32 = 313;
const LEFT: i32 = 314;
const UP: i32 = 315;
const RIGHT: i32 = 316;
const DOWN: i32 = 317;
const INSERT: i32 = 322;
const PAGE_UP: i32 = 366;
const PAGE_DOWN: i32 = 367;
const NUMPAD_ENTER: i32 = 370;
const NUMPAD_HOME: i32 = 375;
const NUMPAD_LEFT: i32 = 376;
const NUMPAD_UP: i32 = 377;
const NUMPAD_RIGHT: i32 = 378;
const NUMPAD_DOWN: i32 = 379;
const NUMPAD_PAGE_UP: i32 = 380;
const NUMPAD_PAGE_DOWN: i32 = 381;
const NUMPAD_END: i32 = 382;
const NUMPAD_INSERT: i32 = 384;
const NUMPAD_DELETE: i32 = 385;
const F9: i32 = 348;
const MOD_ALT: i32 = 1;
const MOD_CONTROL: i32 = 2;
const MOD_SHIFT: i32 = 4;
const MOD_META: i32 = 8;

/// A key in the editing area of document `key` (spec 27 5a, spec 30): pressed (`code` above 0) or let go (`code`
/// negated). True when it was carried out here; the others go on to the menus and the window.
pub fn area_key(key: u64, code: i32, _unicode: i32, mods: i32) -> bool {
    if code < 0 {
        key_up(key, -code);
        return false;
    }
    SHIFT_ALONE.with(|s| s.set(code == SHIFT_KEY));
    let control = mods & MOD_CONTROL != 0;
    let alt = mods & (MOD_ALT | MOD_META) != 0;
    let shift = mods & MOD_SHIFT != 0;
    let mut done = false;
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        // Six-key entry (bc_core::sixkey), with Control and Alt up: the six keys make a braille cell.
        if !control && !alt {
            if let Some(taken) = six_key_down(a, i, code, shift) {
                done = taken;
                return;
            }
        }
        // The tab keys (tabs::tab_key): Ctrl+Tab and Ctrl+Shift+Tab (and Cmd+Shift+] and Cmd+Shift+[ on macOS) the
        // next and previous document, Ctrl+W (Cmd+W on macOS) close the document, Ctrl+Shift+Page Up and Page Down
        // move its tab. Each runs once the key has been dealt with, outside the state.
        if let Some(t) = crate::tabs::tab_key(code, mods, cfg!(target_os = "macos")) {
            use crate::tabs::TabKey;
            match t {
                TabKey::Next | TabKey::Previous => {
                    let forward = t == TabKey::Next;
                    wxdragon::call_after(Box::new(move || switch_tab(forward)));
                }
                TabKey::Close => wxdragon::call_after(Box::new(move || crate::commands::close_document(Some(key)))),
                TabKey::MoveLeft | TabKey::MoveRight => {
                    let up = t == TabKey::MoveLeft;
                    wxdragon::call_after(Box::new(move || move_tab(key, up)));
                }
            }
            done = true;
            return;
        }
        // Ctrl+U (spec 30 section 12 unknown 7): the character at the caret becomes its capital form, one undo step.
        if control && !shift && !alt && code == i32::from(b'U') {
            a.edit_by(i, |ed| ed.capitalize_at_caret().unwrap_or_default());
            done = true;
            return;
        }
        // The one-line prompts (dialog 30070): F9 or Ctrl+[ a code, Ctrl+F9 or Ctrl+< a style, Alt+F9 or Ctrl+] a
        // character by its number; each runs once the key has been dealt with, outside the state.
        let prompt = match code {
            F9 if !control && !alt && !shift => Some(crate::commands::Prompt::Code),
            F9 if control && !alt => Some(crate::commands::Prompt::Style),
            F9 if alt && !control => Some(crate::commands::Prompt::Char),
            _ if crate::tabs::bracket_prompt(code, mods) == Some('[') => Some(crate::commands::Prompt::Code),
            _ if control && !alt && (code == i32::from(b'<') || (shift && code == i32::from(b','))) => {
                Some(crate::commands::Prompt::Style)
            }
            _ if crate::tabs::bracket_prompt(code, mods) == Some(']') => Some(crate::commands::Prompt::Char),
            _ => None,
        };
        if let Some(p) = prompt {
            wxdragon::call_after(Box::new(move || crate::commands::prompt(p)));
            done = true;
            return;
        }
        // Alt with a key belongs to the menus; Alt with the arrows moves between table cells (handled with the
        // arrows below).
        if alt && !control {
            return;
        }
        let highlight = a.docs[i].highlight_anchor.is_some();
        // In Highlight, Shift with a move starts the selection again from the caret (spec 27 section 9, unknown 7).
        if shift && highlight {
            let caret = a.docs[i].ed.caret;
            a.docs[i].ed.select(caret, caret);
            a.docs[i].highlight_anchor = Some(caret);
        }
        let extend = shift || highlight;
        let moved = |m: Move| Some((m, extend));
        let movement = match code {
            LEFT | NUMPAD_LEFT => moved(if control { Move::WordLeft } else { Move::Left }),
            RIGHT | NUMPAD_RIGHT => moved(if control { Move::WordRight } else { Move::Right }),
            UP | NUMPAD_UP => moved(if control { Move::ParagraphUp } else { Move::Up }),
            DOWN | NUMPAD_DOWN => moved(if control { Move::ParagraphDown } else { Move::Down }),
            HOME | NUMPAD_HOME => moved(if control { Move::DocStart } else { Move::Home }),
            END | NUMPAD_END => moved(if control { Move::DocEnd } else { Move::End }),
            _ => None,
        };
        if let Some((m, extend)) = movement {
            a.docs[i].ed.move_caret(m, extend);
            caret_moved(a, i);
            done = true;
            return;
        }
        done = match code {
            PAGE_UP | PAGE_DOWN | NUMPAD_PAGE_UP | NUMPAD_PAGE_DOWN => {
                page_move(a, i, matches!(code, PAGE_DOWN | NUMPAD_PAGE_DOWN), control, extend);
                true
            }
            BACK => {
                a.edit_by(i, |e| e.backspace(control));
                true
            }
            DELETE | NUMPAD_DELETE => {
                a.edit_by(i, |e| e.delete(control));
                true
            }
            // Enter puts in the New Line shortcut code (spec 27 5a).
            ENTER | NUMPAD_ENTER if !control => {
                let code = Shown::marked(&a.settings.shortcuts.new_line);
                type_in(a, i, &code);
                true
            }
            // Insert does nothing in the editing area; there is no overwrite mode (spec 30).
            INSERT | NUMPAD_INSERT => true,
            TAB | ESCAPE => false,
            _ => false,
        };
    });
    done
}

/// A key let go: the end of a six-key chord enters its cell; Shift let go alone shows the Cursor Locator.
fn key_up(key: u64, code: i32) {
    let mut cell: Option<char> = None;
    let mut locator = false;
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        let set = a.settings.six_key_set.clone();
        let enc = a.encoding();
        let d = &mut a.docs[i];
        // Six-key entry: the first chord key let go types the cell (or the space owed).
        if let Some(place) = d.area.raw_key_place() {
            d.six_down.remove(&place);
            let dot = bc_core::sixkey::dot_of(&set, place);
            if let bc_core::sixkey::KeyResult::Type(t) = d.chord.key_up(dot, six_key_letter(place)) {
                cell = Some(typed_char(enc, t));
            }
        }
        if let Some(c) = cell {
            type_in(a, i, &Shown::plain(&c.to_string()));
        }
        locator = code == SHIFT_KEY && SHIFT_ALONE.with(|s| s.replace(false)) && a.settings.view_prefs.locator_on_shift;
    });
    if locator {
        wxdragon::call_after(Box::new(move || show_cursor_locator(key)));
    }
}

/// A character typed in the editing area: put in at the caret, over the selection.
pub fn area_char(key: u64, unicode: i32, _mods: i32) {
    let Some(c) = char::from_u32(unicode as u32) else { return };
    SHIFT_ALONE.with(|s| s.set(false));
    app::try_with(|a| {
        if let Some(i) = a.index_of(key) {
            type_in(a, i, &Shown::plain(&c.to_string()));
        }
    });
}

/// The mouse in the editing area (spec 27 5c): a click puts the caret at the place under it (Shift extends the
/// selection to it), a drag selects, a double click selects the word.
pub fn area_mouse(key: u64, kind: i32, row: usize, col: usize, mods: i32) {
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        let ed = &mut a.docs[i].ed;
        let rows = ed.rows();
        let pos = ed.pos_at(row.min(rows.saturating_sub(1)), col);
        match kind {
            0 if mods & MOD_SHIFT != 0 => ed.select(ed.anchor, pos),
            0 => ed.set_caret(pos),
            1 => ed.select(ed.anchor, pos),
            // The double click selects the word under it (spec 30 section 12 unknown 9: the word's characters, up to
            // a space or a code on either side).
            3 => ed.select_word_at(pos),
            _ => return,
        }
        caret_moved(a, i);
    });
}

/// The editing area got or lost the keyboard focus.
pub fn area_focus(key: u64, focused: bool) {
    // Keys let go while the window had no focus are not seen: the six-key keys held start again.
    if !focused {
        app::try_with(|a| {
            if let Some(i) = a.index_of(key) {
                a.docs[i].six_down.clear();
            }
        });
    }
    if focused {
        app::try_with(|a| {
            if let Some(i) = a.index_of(key) {
                if a.active == Some(i) {
                    a.update_status();
                }
            }
        });
    }
}

/// After the caret moved in document `i`: the editing window shows it, and the status line follows it.
pub(crate) fn caret_moved(a: &mut app::App, i: usize) {
    a.show_area(i, None);
    if a.active == Some(i) {
        a.update_status();
    }
}

/// Puts `s` in place of the selection of document `i`; the margin bell rings for a character typed (spec 24 2d.2).
pub fn type_in(a: &mut app::App, i: usize, s: &Shown) {
    let s = s.clone();
    let typed = s.codes.is_empty() && s.text.chars().count() == 1;
    a.edit_by(i, move |e| e.insert(&s));
    if typed {
        a.margin_bell(i);
    }
}

/// Page Up and Page Down (spec 27 section 7): by the setting Shortcuts PageNav (without Control) or ControlPageNav
/// (with it): a screen (the caret moves up or down line by line until its row has moved by the window's rows less
/// one), a page (the same line and column on the previous or next page) or to the top of a page (its first cell that
/// shows document text). In the coded view, a screen.
fn page_move(a: &mut app::App, i: usize, down: bool, control: bool, extend: bool) {
    let mode = if control { a.settings.shortcuts.control_page_nav } else { a.settings.shortcuts.page_nav };
    // A page move (a page, or the top of a page) in the formatted view keeps the caret's screen row when the caret
    // was on screen (spec 27 section 9, unknown 7).
    let keep = if mode != 0 && !a.docs[i].ed.coded { a.kept_caret_row(i) } else { None };
    let area = a.docs[i].area;
    let ed = &mut a.docs[i].ed;
    let (row, col) = ed.caret_place();
    let target = match ed.row(row) {
        Some(Row::Line { page, line }) if mode != 0 && !ed.coded => {
            let last = ed.pages.len().saturating_sub(1);
            let p = if down { (page + 1).min(last) } else { page.saturating_sub(1) };
            if mode == 1 {
                let lines = ed.pages[p].lines.len();
                Some(ed.pos_at(ed.page_row(p) + line.min(lines.saturating_sub(1)), col))
            } else {
                let first = ed.pages[p].lines.iter().flat_map(|l| l.sources.iter().flatten()).next();
                Some(first.unwrap_or_else(|| ed.pos_at(ed.page_row(p), 0)))
            }
        }
        _ => None,
    };
    match target {
        Some(pos) => {
            if extend {
                ed.select(ed.anchor, pos);
            } else {
                ed.set_caret(pos);
            }
        }
        None => {
            let rows = area.visible_rows().saturating_sub(1).max(1);
            for _ in 0..rows {
                let before = ed.caret_place().0;
                ed.move_caret(if down { Move::Down } else { Move::Up }, extend);
                if ed.caret_place().0 == before {
                    break;
                }
            }
        }
    }
    a.show_area(i, keep);
    if a.active == Some(i) {
        a.update_status();
    }
}

/// Edit, Undo, Redo, Cut, Copy, Paste, Clear and Select All, on the document (their keys reach them through the Edit
/// menu). Copy and Cut keep the selection with its codes, so pasting it brings them back. True when done.
pub fn edit_command(a: &mut app::App, i: usize, id: i32) -> bool {
    let model = a.docs[i].shown();
    let (from, to) = a.docs[i].selection();
    match id {
        1203 | 1204 => {
            let part = model.slice(from, to);
            Clipboard::get().set_text(&part.text);
            a.remember(part);
            if id == 1203 && from != to {
                a.edit(i, from, to, &Shown::default());
                // Cut ends Highlight.
                if a.docs[i].highlight_anchor.take().is_some() {
                    a.update_status();
                }
            }
        }
        1205 => {
            let Some(text) = Clipboard::get().get_text() else { return true };
            let s = a.known.iter().find(|k| k.text == text).cloned().unwrap_or_else(|| Shown::plain(&text));
            a.edit(i, from, to, &s);
        }
        1206 if from != to => a.edit(i, from, to, &Shown::default()),
        // With no selection Clear is the Delete key.
        1206 => a.edit_by(i, |e| e.delete(false)),
        1201 | 1202 => {
            a.undo(i, id == 1202);
        }
        1207 => {
            let len = a.docs[i].ed.len();
            a.docs[i].ed.select(0, len);
            caret_moved(a, i);
        }
        _ => return false,
    }
    true
}

/// A key pressed in document `i` with Control and Alt up, for six-key entry (bc_core::sixkey): Some(true) when it
/// was taken (typed a cell or did nothing), Some(false) when it goes on as typing, None when six-key entry has
/// nothing to do with it.
fn six_key_down(a: &mut app::App, i: usize, code: i32, shift: bool) -> Option<bool> {
    use bc_core::sixkey::{KeyResult, Mode};
    let set = a.settings.six_key_set.clone();
    let enc = a.encoding();
    let d = &mut a.docs[i];
    let mode = Mode::of(d.kind == DocKind::Braille, d.view.six_key);
    if mode == Mode::Off {
        return None;
    }
    let result = if code == SPACE {
        d.chord.space_down()
    } else {
        let place = d.area.raw_key_place()?;
        let repeat = !d.six_down.insert(place);
        d.chord.key_down(bc_core::sixkey::dot_of(&set, place), six_key_letter(place), shift, repeat, mode)
    };
    match result {
        KeyResult::Pass => None,
        KeyResult::Swallow => Some(true),
        KeyResult::Type(t) => {
            let c = typed_char(enc, t);
            type_in(a, i, &Shown::plain(&c.to_string()));
            Some(true)
        }
    }
}

/// Six-key entry in text field `cid` of dialog `d`: Edit, Find and the Header and Footer dialogs take the keys as
/// the active document's editing area does (bc_core::sixkey, with the document's mode when the dialog opens); every
/// other dialog types as usual. INFERENCE: a text field gives no physical key, so a key is named by the character
/// its key code stands for (its US place on a US layout).
pub fn six_key_field(d: &crate::dlg::Dlg, cid: i32) {
    use bc_core::sixkey::{Chord, KeyResult, Mode};
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(field) = d.win(cid).and_then(|w| w.as_widget::<TextCtrl>()) else { return };
    let Some((mode, set, enc)) = app::with(|a| {
        let set = a.settings.six_key_set.clone();
        let enc = a.encoding();
        a.doc().map(|doc| (Mode::of(doc.kind == DocKind::Braille, doc.view.six_key), set, enc))
    }) else {
        return;
    };
    if mode == Mode::Off {
        return;
    }
    let place_of = |code: i32| {
        char::from_u32(code as u32)
            .map(|c| c.to_ascii_lowercase())
            .filter(|c| c.is_ascii_lowercase() || ";,./[\\]'".contains(*c))
    };
    let state: Rc<RefCell<(Chord, HashSet<char>)>> = Rc::default();
    let (down_state, down_set, down_field) = (state.clone(), set.clone(), field.clone());
    field.on_key_down(move |e| {
        let WindowEventData::Keyboard(k) = &e else { return };
        let code = k.get_key_code().unwrap_or(0);
        if k.control_down() || k.alt_down() || k.meta_down() {
            k.event.skip(true);
            return;
        }
        let mut st = down_state.borrow_mut();
        let result = if code == SPACE {
            st.0.space_down()
        } else {
            match place_of(code) {
                Some(p) => {
                    let repeat = !st.1.insert(p);
                    let dot = bc_core::sixkey::dot_of(&down_set, p);
                    st.0.key_down(dot, six_key_letter(p), k.shift_down(), repeat, mode)
                }
                None => KeyResult::Pass,
            }
        };
        drop(st);
        match result {
            KeyResult::Pass => k.event.skip(true),
            KeyResult::Swallow => {}
            KeyResult::Type(t) => down_field.write_text(&typed_char(enc, t).to_string()),
        }
    });
    let up_field = field.clone();
    field.on_key_up(move |e| {
        let WindowEventData::Keyboard(k) = &e else { return };
        let code = k.get_key_code().unwrap_or(0);
        let Some(p) = place_of(code) else {
            k.event.skip(true);
            return;
        };
        let mut st = state.borrow_mut();
        st.1.remove(&p);
        let result = st.0.key_up(bc_core::sixkey::dot_of(&set, p), six_key_letter(p));
        drop(st);
        match result {
            KeyResult::Type(t) => up_field.write_text(&typed_char(enc, t).to_string()),
            KeyResult::Pass => k.event.skip(true),
            KeyResult::Swallow => {}
        }
    });
}

/// What a chord types: its cell in the shown braille encoding, or a space.
fn typed_char(enc: &crate::encodings::Encoding, t: bc_core::sixkey::Typed) -> char {
    match t {
        bc_core::sixkey::Typed::Cell(dots) => enc.cell_char(usize::from(dots)),
        bc_core::sixkey::Typed::Space => ' ',
    }
}

/// Whether the key at US place `place` is a letter key for six-key entry: A to Z by place, but on a French keyboard
/// layout the key at the US "M" place types as usual.
pub(crate) fn six_key_letter(place: char) -> bool {
    place.is_ascii_lowercase() && !(place == 'm' && french_layout())
}

/// Whether the keyboard layout in use is French (language 0x40c, 0x80c, 0xc0c, 0x100c, 0x140c or 0x180c). Known
/// on Windows; INFERENCE: elsewhere the layout is taken as not French, as BrailleCraft cannot read it there.
fn french_layout() -> bool {
    #[cfg(windows)]
    {
        type GetLayout = unsafe extern "system" fn(u32) -> isize;
        // SAFETY: user32 is loaded in every windowed program; GetKeyboardLayout takes a thread id (0 for this one).
        let id = unsafe {
            let Ok(lib) = libloading::Library::new("user32.dll") else { return false };
            let Ok(f) = lib.get::<GetLayout>(b"GetKeyboardLayout\0") else { return false };
            (f(0) as usize & 0xffff) as u32
        };
        [0x40c, 0x80c, 0xc0c, 0x100c, 0x140c, 0x180c].contains(&id)
    }
    #[cfg(not(windows))]
    {
        false
    }
}
