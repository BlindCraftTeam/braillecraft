//! Document windows: each open document is a child window holding the editing area (the program-drawn editing
//! window, editor_view, as the reference program draws its own), under it the one-line translated line on yellow,
//! and at the bottom the document's status line. The child windows sit maximized in the main window, as in the
//! reference screenshots.
//!
//! Every key, typed character and click in the editing area comes here and is carried out on the document's editor
//! model (bc_core::editor), as the reference program's key handler does (spec 27 section 5, spec 30).

use std::collections::HashSet;

use bc_core::doctext::Shown;
use bc_core::editor::{Editor, Move, Row};
use wxdragon::prelude::*;

use crate::app::{self, child_maximize, child_set_title, Doc, DocKind, Hyphen, View};
use crate::editor_view;
use crate::speech;

/// Opens a new document window with `shown` (its text and codes). `name` None makes an untitled one ("Untitled
/// Print Document 1").
pub fn new_document(kind: DocKind, name: Option<String>, shown: &Shown) -> u64 {
    let (frame, key, name, untitled, view) = app::with(|a| {
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
        (a.frame, a.next_key, name, untitled, view)
    });

    let child = MDIChildFrame::builder(&frame).with_title(&name).build();
    let area = editor_view::create(&child, key);
    // The status line is made right after the editing area and the translated line after it: screen reader scripts
    // find them as the next window and the one after (spec 17).
    let status = StaticText::builder(&child).with_label("Current Style: None").build();
    app::with(|a| crate::apply_status_font(a, &status));
    let line = TextCtrl::builder(&child).with_style(TextCtrlStyle::ReadOnly).build();
    line.set_background_color(Colour::new(255, 255, 0, 255));
    line.set_foreground_color(Colour::new(0, 0, 0, 255));
    line.set_accessibility_label("Translated line");
    speech::set_name(line.get_handle(), "Translated line");
    // The name the reference program's document provider gives its editing area.
    area.set_accessible_name("Document Editing Window");
    line.show(view.translated);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&area, 1, SizerFlag::Expand, 0);
    sizer.add(&line, 0, SizerFlag::Expand, 0);
    sizer.add(&status, 0, SizerFlag::Expand | SizerFlag::All, 2);
    child.set_sizer(sizer, true);

    wire(key, &child, area);

    let braille = kind == DocKind::Braille;
    // The coded view's rows are 42 characters for a braille document and 65 for a print one (spec 27 4b); the page
    // size is set from the document's own when it is first shown (App::show_area).
    let ed = Editor::new(shown.clone(), bc_core::format::Layout::braille(40, 25), None, view.codes, if braille { 42 } else { 65 });
    let doc = Doc {
        key,
        frame: child,
        area,
        status,
        line,
        name,
        kind,
        untitled,
        saved: false,
        dirty: false,
        path: None,
        // A new document is made with the default template: its tables and its styles.
        tables: crate::translation::template_tables(&crate::translation::default_template())
            .unwrap_or_else(|| crate::translation::DEFAULT_TABLES.map(str::to_string)),
        initial_codes: String::new(),
        template: crate::translation::default_template(),
        cells: if braille { 40 } else { 80 },
        lines: if braille { 25 } else { 50 },
        page_numbers: crate::translation::template_page_numbers(&crate::translation::default_template()),
        pages: None,
        ed,
        ed_layout: 0,
        undo: Vec::new(),
        redo: Vec::new(),
        area_rows: editor_view::ShownRows::default(),
        area_stale: true,
        view,
        hyphen: Hyphen::Dashes,
        styles: crate::translation::template_style_list(&crate::translation::default_template()),
        file_settings: Vec::new(),
        file_styles: Vec::new(),
        highlight_anchor: None,
        last_line_key: String::new(),
        six_down: HashSet::new(),
        chord: 0,
        printer: None,
        embosser: crate::embosser::default_setup().unwrap_or_default(),
        layout: crate::embosser::default_layout().0,
        file_sections: Vec::new(),
    };
    app::with(|a| {
        child_set_title(&doc.frame, &doc.full_name());
        a.docs.push(doc);
        // The document opens at its start, in the view Global, Default Views sets: the formatted view with View,
        // Codes off, else the coded view (spec 27 2a).
        let i = a.docs.len() - 1;
        a.docs[i].select_doc(0, 0);
        a.set_view_codes(i, view.codes);
    });
    child_maximize(&child);
    child.show(true);
    activate_key(key);
    area.set_focus();
    // A dialog that was open (File, Open) gives focus back to the control it came from when it finishes closing;
    // the new document takes it once that is done.
    // On macOS each document is a window of its own, and the system gives activation back to the window that had it
    // before the dialog (the untitled document of a plain start), whose activation would make it the active document;
    // the new document's window is brought to the front, as the View menu's list does.
    wxdragon::call_after(Box::new(move || {
        child.raise();
        child_maximize(&child);
        area.set_focus();
        activate_key(key);
    }));
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

/// Makes the document with this key the active one.
pub fn activate_key(key: u64) {
    app::try_with(|a| {
        if let Some(i) = a.index_of(key) {
            a.active = Some(i);
            a.set_title();
            a.apply_view();
        }
    });
}

/// Brings document `index` to the front (View menu list).
pub fn bring_to_front(index: usize) {
    let target = app::with(|a| a.docs.get(index).map(|d| (d.frame, d.area, d.key)));
    if let Some((frame, area, key)) = target {
        frame.raise();
        child_maximize(&frame);
        area.set_focus();
        activate_key(key);
    }
}

/// Closes the document with this key without asking (the caller has asked).
pub fn destroy(key: u64) {
    let frame = app::with(|a| {
        let i = a.index_of(key)?;
        let d = a.docs.remove(i);
        a.active = if a.docs.is_empty() { None } else { Some(a.docs.len() - 1) };
        Some(d.frame)
    });
    if let Some(f) = frame {
        f.destroy();
    }
    app::with(|a| {
        a.set_title();
        if let Some(d) = a.doc() {
            let (f, area) = (d.frame, d.area);
            child_maximize(&f);
            area.set_focus();
        }
        a.refresh_menus();
        a.update_status();
    });
}

thread_local! {
    /// Shift was pressed with no other key since: letting it go shows the Cursor Locator when Global, View
    /// Preferences asks for it (spec 24 2d.2).
    static SHIFT_ALONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn wire(key: u64, child: &MDIChildFrame, area: editor_view::Area) {
    child.on_activate(move |e| {
        if let WindowEventData::Activate(a) = &e {
            if a.is_active() {
                activate_key(key);
                // The editing area takes the keyboard when its document becomes active. On Linux the documents are
                // pages of a notebook, which takes focus as it switches pages; focus is given once that is done.
                wxdragon::call_after(Box::new(move || area.set_focus()));
            }
            a.event.skip(true);
        }
    });

    child.on_close(move |e| {
        // Closing a document window asks about unsaved changes, as File, Close does.
        if let WindowEventData::General(ev) = &e {
            if ev.can_veto() {
                ev.veto();
            }
        }
        wxdragon::call_after(Box::new(move || crate::commands::close_document(Some(key))));
    });
}

// wxWidgets key codes and modifier bits.
const BACK: i32 = 8;
const TAB: i32 = 9;
const ENTER: i32 = 13;
const ESCAPE: i32 = 27;
const DELETE: i32 = 127;
const END: i32 = 312;
const HOME: i32 = 313;
const LEFT: i32 = 314;
const UP: i32 = 315;
const RIGHT: i32 = 316;
const DOWN: i32 = 317;
const INSERT: i32 = 324;
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
        // Six-key entry: the six keys make a braille cell, entered when the last of them is let go.
        let d = &mut a.docs[i];
        if d.kind == DocKind::Braille && d.view.six_key && !control && !alt {
            if let Some(dot) = six_key_dot(&a.settings.six_key_set, code) {
                let d = &mut a.docs[i];
                d.six_down.insert(dot);
                d.chord |= 1 << dot;
                done = true;
                return;
            }
        }
        // Alt with a key belongs to the menus (Alt with the arrows moves between table cells in the reference, not
        // built: BrailleCraft has no tables yet).
        if alt && !control {
            return;
        }
        let extend = shift || a.docs[i].highlight_anchor.is_some();
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
        if d.kind == DocKind::Braille && d.view.six_key {
            if let Some(dot) = six_key_dot(&set, code) {
                d.six_down.remove(&dot);
                // The cell is entered when the last key of the chord is released.
                if d.six_down.is_empty() && d.chord != 0 {
                    cell = Some(enc.cell_char(d.chord));
                    d.chord = 0;
                }
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
            3 => {
                // INFERENCE (spec 27 5c: the double click sets the reference's word select, not traced further).
                ed.set_caret(pos);
                ed.move_caret(Move::WordLeft, false);
                let start = ed.caret;
                ed.move_caret(Move::WordRight, true);
                ed.anchor = start;
            }
            _ => return,
        }
        caret_moved(a, i);
    });
}

/// The editing area got or lost the keyboard focus.
pub fn area_focus(key: u64, focused: bool) {
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
fn caret_moved(a: &mut app::App, i: usize) {
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
                let first = ed.pages[p].lines.iter().flat_map(|l| l.sources.iter().flatten()).next().copied();
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
    caret_moved(a, i);
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
            }
        }
        1205 => {
            let Some(text) = Clipboard::get().get_text() else { return true };
            let s = a.known.iter().find(|k| k.text == text).cloned().unwrap_or_else(|| Shown::plain(&text));
            a.edit(i, from, to, &s);
        }
        1206 if from != to => a.edit(i, from, to, &Shown::default()),
        1206 => {}
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

fn six_key_dot(set: &str, code: i32) -> Option<usize> {
    let c = char::from_u32(code as u32)?.to_ascii_lowercase();
    set.chars().position(|k| k == c)
}
