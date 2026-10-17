//! Document windows: each open document is a child window holding the editing area (a native multi-line text
//! control), under it the one-line translated line on yellow, and at the bottom the document's status line. The
//! child windows sit maximized in the main window, as in the reference screenshots.

use std::collections::HashSet;

use bc_core::doctext::Shown;
use wxdragon::prelude::*;

use crate::app::{self, child_maximize, child_set_title, Doc, DocKind, Hyphen, View};
use crate::speech;

/// Opens a new document window with `shown` (its text and codes). `name` None makes an untitled one ("Untitled
/// Print Document 1").
pub fn new_document(kind: DocKind, name: Option<String>, shown: &Shown) -> u64 {
    let text = shown.text.as_str();
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
    // The rich text style on Windows makes positions count a line break as one character, as on the other systems.
    let style = if cfg!(windows) { TextCtrlStyle::MultiLine | TextCtrlStyle::Rich2 } else { TextCtrlStyle::MultiLine };
    // Control id 0, as the reference program's editing area has: screen reader scripts read the last line of the
    // document window (the status line) directly when the focused window has id 0.
    let text_ctrl = TextCtrl::builder(&child).with_id(0).with_style(style).build();
    text_ctrl.set_value(text);
    // A document opens at its start (GTK leaves the caret after the text).
    text_ctrl.set_insertion_point(0);
    text_ctrl.show_position(0);
    text_ctrl.set_modified(false);
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
    text_ctrl.set_accessibility_label("Document Editing Window");
    speech::set_name(text_ctrl.get_handle(), "Document Editing Window");
    line.show(view.translated);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&text_ctrl, 1, SizerFlag::Expand, 0);
    sizer.add(&line, 0, SizerFlag::Expand, 0);
    sizer.add(&status, 0, SizerFlag::Expand | SizerFlag::All, 2);
    child.set_sizer(sizer, true);

    wire(key, &child, &text_ctrl);

    let doc = Doc {
        key,
        frame: child,
        text: text_ctrl,
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
        cells: if kind == DocKind::Braille { 40 } else { 80 },
        lines: if kind == DocKind::Braille { 25 } else { 50 },
        page_numbers: crate::translation::template_page_numbers(&crate::translation::default_template()),
        pages: None,
        codes: shown.codes.clone(),
        inner: shown.inner.clone(),
        last_value: shown.text.clone(),
        taken: Vec::new(),
        view,
        hyphen: Hyphen::Dashes,
        styles: crate::translation::template_style_list(&crate::translation::default_template()),
        file_settings: Vec::new(),
        file_styles: Vec::new(),
        last_style: None,
        highlight_anchor: None,
        last_line_key: String::new(),
        six_down: HashSet::new(),
        chord: 0,
        printer: None,
        embosser: crate::embosser::default_setup().unwrap_or_default(),
        layout: crate::embosser::default_layout().0,
        file_sections: Vec::new(),
        formatted: None,
    };
    app::with(|a| {
        child_set_title(&doc.frame, &doc.full_name());
        a.docs.push(doc);
        // With View, Codes off (the Global, Default Views setting) the document opens in the formatted view
        // (spec 27 2a).
        if !view.codes {
            let i = a.docs.len() - 1;
            a.set_view_codes(i, false);
            a.docs[i].select_doc(0, 0);
        }
    });
    child_maximize(&child);
    child.show(true);
    activate_key(key);
    text_ctrl.set_focus();
    // A dialog that was open (File, Open) gives focus back to the control it came from when it finishes closing;
    // the new document takes it once that is done.
    // On macOS each document is a window of its own, and the system gives activation back to the window that had it
    // before the dialog (the untitled document of a plain start), whose activation would make it the active document;
    // the new document's window is brought to the front, as the View menu's list does.
    wxdragon::call_after(Box::new(move || {
        child.raise();
        child_maximize(&child);
        text_ctrl.set_focus();
        activate_key(key);
    }));
    key
}

/// The key code of the Shift key (WXK_SHIFT).
const SHIFT_KEY: i32 = 306;

/// Shows the Cursor Locator over document `key`'s editing area (spec 24 2d.2): a rectangle closing in on the caret.
pub fn show_cursor_locator(key: u64) {
    let mut text = None;
    app::try_with(|a| text = a.index_of(key).map(|i| a.docs[i].text));
    if let Some(t) = text {
        unsafe { wxdragon_sys::wxd_TextCtrl_ShowCursorLocator(t.handle_ptr() as *mut wxdragon_sys::wxd_Window_t) };
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
    let target = app::with(|a| a.docs.get(index).map(|d| (d.frame, d.text, d.key)));
    if let Some((frame, text, key)) = target {
        frame.raise();
        child_maximize(&frame);
        text.set_focus();
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
            let (f, t) = (d.frame, d.text);
            child_maximize(&f);
            t.set_focus();
        }
        a.refresh_menus();
        a.update_status();
    });
}

fn wire(key: u64, child: &MDIChildFrame, text: &TextCtrl) {
    let edited = *text;
    text.on_text_updated(move |_| {
        app::try_with(|a| {
            if let Some(i) = a.index_of(key) {
                // In the formatted view the area's text is the laid-out lines: what was typed or pasted into them is
                // put in the document at the place it shows, and the lines are shown again (formatted_typed).
                if a.docs[i].formatted.is_some() {
                    formatted_typed(a, i);
                    return;
                }
                // The codes follow the edit; a code that lost a bracket goes as a whole, as codes do in the
                // reference program, once this change has been handled.
                let before = a.docs[i].last_value.chars().count();
                let remains = a.sync_codes(i);
                // One character more: typed (or one pasted), which is when the reference rings the margin bell.
                let typed = remains.is_empty() && a.docs[i].last_value.chars().count() == before + 1;
                if !remains.is_empty() {
                    wxdragon::call_after(Box::new(move || {
                        for (from, to) in remains {
                            edited.replace(from as i64, to as i64, "");
                        }
                    }));
                }
                a.docs[i].dirty = true;
                if a.active == Some(i) {
                    a.update_status();
                    if typed {
                        a.margin_bell(i);
                    }
                }
            }
        });
    });

    let t = *text;
    // Shift pressed and let go with no other key between: the Cursor Locator, when Global, View Preferences asks for
    // it (spec 24 2d.2).
    let shift_alone = std::rc::Rc::new(std::cell::Cell::new(false));
    let shift_down = shift_alone.clone();
    text.on_key_down(move |e| {
        let mut consumed = false;
        if let WindowEventData::Keyboard(k) = &e {
            shift_down.set(k.get_key_code() == Some(SHIFT_KEY));
            app::try_with(|a| {
                let Some(i) = a.index_of(key) else { return };
                // Copy and cut keep the selection with its codes, so pasting it brings them back.
                let code = k.get_key_code().unwrap_or(0);
                if let Some(f) = a.docs[i].formatted.as_mut() {
                    f.echo = None;
                }
                if a.docs[i].formatted.is_some() && formatted_key(a, i, code, k.control_down(), k.alt_down() || k.meta_down()) {
                    let caret = a.docs[i].text.get_insertion_point();
                    if let Some(f) = a.docs[i].formatted.as_mut() {
                        f.echo = Some(caret);
                    }
                    consumed = true;
                    return;
                }
                if k.control_down() && (code == 'C' as i32 || code == 'X' as i32 || code == 'c' as i32 || code == 'x' as i32) {
                    let (from, to) = a.docs[i].selection();
                    let part = a.docs[i].shown().slice(from, to);
                    a.remember(part);
                }
                let d = &mut a.docs[i];
                if d.kind != DocKind::Braille || !d.view.six_key || k.control_down() || k.alt_down() || k.meta_down() {
                    return;
                }
                if let Some(dot) = six_key_dot(&a.settings.six_key_set, k.get_key_code()) {
                    let d = &mut a.docs[i];
                    d.six_down.insert(dot);
                    d.chord |= 1 << dot;
                    let caret = d.text.get_insertion_point();
                    if let Some(f) = d.formatted.as_mut() {
                        f.echo = Some(caret);
                    }
                    consumed = true;
                }
            });
            if !consumed {
                k.event.skip(true);
            }
        }
    });

    text.on_key_up(move |e| {
        if let WindowEventData::Keyboard(k) = &e {
            let mut insert: Option<char> = None;
            app::try_with(|a| {
                let Some(i) = a.index_of(key) else { return };
                let set = a.settings.six_key_set.clone();
                let enc = a.encoding();
                let d = &mut a.docs[i];
                if d.kind == DocKind::Braille && d.view.six_key {
                    if let Some(dot) = six_key_dot(&set, k.get_key_code()) {
                        d.six_down.remove(&dot);
                        // The cell is entered when the last key of the chord is released.
                        if d.six_down.is_empty() && d.chord != 0 {
                            insert = Some(enc.cell_char(d.chord));
                            d.chord = 0;
                        }
                    }
                }
            });
            if let Some(c) = insert {
                let mut done = false;
                app::try_with(|a| {
                    if let Some(i) = a.index_of(key).filter(|&i| a.docs[i].formatted.is_some()) {
                        type_in(a, i, &Shown::plain(&c.to_string()));
                        done = true;
                    }
                });
                if !done {
                    t.write_text(&c.to_string());
                }
            }
            k.event.skip(true);
            caret_moved(key);
            if k.get_key_code() == Some(SHIFT_KEY) && shift_alone.replace(false) {
                let mut on = false;
                app::try_with(|a| on = a.settings.view_prefs.locator_on_shift);
                if on {
                    wxdragon::call_after(Box::new(move || show_cursor_locator(key)));
                }
            }
        }
    });

    text.on_mouse_left_up(move |e| {
        if let WindowEventData::MouseButton(m) = &e {
            m.event.skip(true);
        }
        caret_moved(key);
    });

    let editor = *text;
    child.on_activate(move |e| {
        if let WindowEventData::Activate(a) = &e {
            if a.is_active() {
                activate_key(key);
                // The editing area takes the keyboard when its document becomes active. On Linux the documents are
                // pages of a notebook, which takes focus as it switches pages; focus is given once that is done.
                wxdragon::call_after(Box::new(move || editor.set_focus()));
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

/// Highlight mode and the status line follow the caret.
fn caret_moved(key: u64) {
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        let d = &a.docs[i];
        if let Some(anchor) = d.highlight_anchor {
            let (from, to) = d.text.get_selection();
            if from == to && from != anchor {
                d.text.set_selection(from.min(anchor), from.max(anchor));
            }
        }
        if a.active == Some(i) {
            a.update_status();
        }
    });
}

/// Puts `s` in place of the selection of document `i`, which shows the formatted view.
pub fn type_in(a: &mut app::App, i: usize, s: &Shown) {
    let (from, to) = a.docs[i].selection();
    a.edit_formatted(i, from, to, s);
    if s.codes.is_empty() && s.text.chars().count() == 1 {
        a.margin_bell(i);
    }
}

/// A key pressed in the formatted view that edits the document (spec 27 5a): Backspace and Delete (a code next to
/// the caret, which the view does not show, goes as a whole; with Control, a word), Enter (the New Line shortcut
/// code), and the clipboard, undo and redo keys. True when the key was used.
pub fn formatted_key(a: &mut app::App, i: usize, code: i32, control: bool, alt: bool) -> bool {
    const BACK: i32 = 8;
    const ENTER: i32 = 13;
    const NUMPAD_ENTER: i32 = 370;
    const DELETE: i32 = 127;
    const NUMPAD_DELETE: i32 = 385;
    if alt {
        return false;
    }
    let model = a.docs[i].shown();
    let chars: Vec<char> = model.text.chars().collect();
    let (from, to) = a.docs[i].selection();
    match code {
        BACK | DELETE | NUMPAD_DELETE if from != to => a.edit_formatted(i, from, to, &Shown::default()),
        BACK => {
            let start = if control {
                word_start(&chars, from)
            } else if let Some(c) = model.codes.iter().find(|c| c.1 == from) {
                c.0
            } else if from > 0 && model.is_own_break(from - 1) {
                model.codes.iter().find(|c| c.1 == from - 1).map_or(from - 1, |c| c.0)
            } else {
                from.saturating_sub(1)
            };
            if start < from {
                a.edit_formatted(i, start, from, &Shown::default());
            }
        }
        DELETE | NUMPAD_DELETE => {
            let end = if control {
                word_end(&chars, from)
            } else if let Some(c) = model.codes.iter().find(|c| c.0 == from) {
                if model.is_own_break(c.1) { c.1 + 1 } else { c.1 }
            } else {
                (from + 1).min(chars.len())
            };
            if end > from {
                a.edit_formatted(i, from, end, &Shown::default());
            }
        }
        ENTER | NUMPAD_ENTER if !control => {
            let code = Shown::marked(&a.settings.shortcuts.new_line);
            type_in(a, i, &code);
        }
        _ => return false,
    }
    true
}

/// Edit, Undo, Redo, Cut, Copy and Paste in the formatted view, on the document (their keys reach them through
/// the Edit menu). True when done.
pub fn formatted_command(a: &mut app::App, i: usize, id: i32) -> bool {
    let model = a.docs[i].shown();
    let (from, to) = a.docs[i].selection();
    match id {
        1203 | 1204 => {
            let part = model.slice(from, to);
            Clipboard::get().set_text(&part.text);
            a.remember(part);
            if id == 1203 && from != to {
                a.edit_formatted(i, from, to, &Shown::default());
            }
        }
        1205 => {
            let Some(text) = Clipboard::get().get_text() else { return true };
            let s = a.known.iter().find(|k| k.text == text).cloned().unwrap_or_else(|| Shown::plain(&text));
            a.edit_formatted(i, from, to, &s);
        }
        1206 if from != to => a.edit_formatted(i, from, to, &Shown::default()),
        1201 | 1202 => {
            a.undo_formatted(i, id == 1202);
        }
        _ => return false,
    }
    true
}

/// The editing area of document `i`, in the formatted view, no longer holds the lines it was given: the stretch
/// that changed (characters typed or pasted, replacing what was selected) is made in the document at the places the
/// lines show (spec 27 5a and 5c), and the lines are shown again. The area inserts typed characters itself on every
/// system, so they are taken from it here rather than from the key events.
fn formatted_typed(a: &mut app::App, i: usize) {
    let now = a.docs[i].text.get_value();
    let d = &mut a.docs[i];
    let Some(f) = d.formatted.as_mut() else { return };
    if now == f.view.text {
        return;
    }
    // The control carried out a key the program had already carried out: its change is put back.
    if let Some(caret) = f.echo.take() {
        let text = f.view.text.clone();
        let t = d.text;
        t.set_value(&text);
        t.set_insertion_point(caret);
        wxdragon::call_after(Box::new(move || t.set_insertion_point(caret)));
        return;
    }
    let Some(f) = d.formatted.as_ref() else { return };
    let old: Vec<char> = f.view.text.chars().collect();
    let new: Vec<char> = now.chars().collect();
    let p = old.iter().zip(&new).take_while(|(x, y)| x == y).count();
    let s = old.iter().rev().zip(new.iter().rev()).take(old.len().min(new.len()) - p).take_while(|(x, y)| x == y).count();
    let (old_end, new_end) = (old.len() - s, new.len() - s);
    let put: String = new[p..new_end].iter().collect();
    let from = f.view.to_document(&f.pages, p);
    let to = if old_end > p { f.view.to_document(&f.pages, old_end).max(from) } else { from };
    a.edit_formatted(i, from, to, &Shown::plain(&put));
    if old_end == p && put.chars().count() == 1 {
        a.margin_bell(i);
    }
}

/// The start of the word before character `at` (spaces before it included), for Control+Backspace.
fn word_start(chars: &[char], at: usize) -> usize {
    let mut p = at.min(chars.len());
    while p > 0 && chars[p - 1].is_whitespace() {
        p -= 1;
    }
    while p > 0 && !chars[p - 1].is_whitespace() {
        p -= 1;
    }
    p
}

/// The end of the word at character `at` (the spaces after it included), for Control+Delete.
fn word_end(chars: &[char], at: usize) -> usize {
    let mut p = at;
    while p < chars.len() && !chars[p].is_whitespace() {
        p += 1;
    }
    while p < chars.len() && chars[p].is_whitespace() {
        p += 1;
    }
    p
}

fn six_key_dot(set: &str, key_code: Option<i32>) -> Option<usize> {
    let code = key_code?;
    let c = char::from_u32(code as u32)?.to_ascii_lowercase();
    set.chars().position(|k| k == c)
}
