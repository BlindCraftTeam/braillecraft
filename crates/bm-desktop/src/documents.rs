//! Document windows: each open document is a child window holding the editing area (a native multi-line text
//! control) and, under it, the one-line translated line on yellow. The child windows sit maximized in the main
//! window, as in the reference screenshots.

use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use wxdragon::prelude::*;

use crate::app::{self, child_maximize, child_set_title, Doc, DocKind, Hyphen, View};
use crate::speech::{self, BrailleLines};

/// Opens a new document window. `name` None makes an untitled one ("Untitled Print Document 1").
pub fn new_document(kind: DocKind, name: Option<String>, text: &str) -> u64 {
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
    let text_ctrl = TextCtrl::builder(&child).with_style(style).build();
    text_ctrl.set_value(text);
    // A document opens at its start (GTK leaves the caret after the text).
    text_ctrl.set_insertion_point(0);
    text_ctrl.show_position(0);
    text_ctrl.set_modified(false);
    let line = TextCtrl::builder(&child).with_style(TextCtrlStyle::ReadOnly).build();
    line.set_background_color(Colour::new(255, 255, 0, 255));
    line.set_foreground_color(Colour::new(0, 0, 0, 255));
    line.set_accessibility_label("Translated line");
    speech::set_name(line.get_handle(), "Translated line");
    text_ctrl.set_accessibility_label("BrailleMaker editing window");
    speech::set_name(text_ctrl.get_handle(), "BrailleMaker editing window");
    line.show(view.translated);

    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    sizer.add(&text_ctrl, 1, SizerFlag::Expand, 0);
    sizer.add(&line, 0, SizerFlag::Expand, 0);
    child.set_sizer(sizer, true);

    wire(key, &child, &text_ctrl);
    wire_line(&line, &text_ctrl);

    let doc = Doc {
        key,
        frame: child,
        text: text_ctrl,
        line,
        name,
        kind,
        untitled,
        saved: false,
        dirty: false,
        path: None,
        table: "en-ueb-g2.ctb".to_string(),
        cells: if kind == DocKind::Braille { 40 } else { 80 },
        lines: if kind == DocKind::Braille { 25 } else { 50 },
        view,
        hyphen: Hyphen::Dashes,
        styles: Vec::new(),
        last_style: None,
        highlight_anchor: None,
        last_line_key: String::new(),
        six_down: HashSet::new(),
        chord: 0,
        reader_at: None,
    };
    app::with(|a| {
        child_set_title(&doc.frame, &doc.full_name());
        a.docs.push(doc);
    });
    child_maximize(&child);
    child.show(true);
    activate_key(key);
    text_ctrl.set_focus();
    // A dialog that was open (File, Open) gives focus back to the control it came from when it finishes closing;
    // the new document takes it once that is done.
    wxdragon::call_after(Box::new(move || {
        text_ctrl.set_focus();
        activate_key(key);
    }));
    key
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
    text.on_text_updated(move |_| {
        app::try_with(|a| {
            if let Some(i) = a.index_of(key) {
                a.docs[i].dirty = true;
                if a.active == Some(i) {
                    a.update_status();
                }
            }
        });
    });

    let t = *text;
    text.on_key_down(move |e| {
        let mut consumed = false;
        if let WindowEventData::Keyboard(k) = &e {
            if k.alt_down() && !k.control_down() && !k.shift_down() {
                if let Some(code) = k.get_key_code() {
                    if reader_key(key, code) {
                        return;
                    }
                }
            }
            app::try_with(|a| {
                let Some(i) = a.index_of(key) else { return };
                let d = &mut a.docs[i];
                if d.kind != DocKind::Braille || !d.view.six_key || k.control_down() || k.alt_down() || k.meta_down() {
                    return;
                }
                if let Some(dot) = six_key_dot(&a.settings.six_key_set, k.get_key_code()) {
                    let d = &mut a.docs[i];
                    d.six_down.insert(dot);
                    d.chord |= 1 << dot;
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
            let mut six_mode = false;
            app::try_with(|a| {
                let Some(i) = a.index_of(key) else { return };
                let set = a.settings.six_key_set.clone();
                let enc = a.encoding();
                let d = &mut a.docs[i];
                six_mode = d.kind == DocKind::Braille && d.view.six_key;
                if six_mode {
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
                t.write_text(&c.to_string());
                echo_entry(key, Some(c));
            } else if !k.alt_down() && !k.control_down() {
                if let Some(code) = k.get_key_code() {
                    if (32..127).contains(&code) && (!six_mode || code == 32) {
                        echo_entry(key, None);
                    }
                }
            }
            k.event.skip(true);
            caret_moved(key, k.get_key_code());
        }
    });

    text.on_mouse_left_up(move |e| {
        if let WindowEventData::MouseButton(m) = &e {
            m.event.skip(true);
        }
        caret_moved(key, None);
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

/// Key codes of the movement keys (wxWidgets WXK_ values).
const KEY_END: i32 = 312;
const KEY_HOME: i32 = 313;
const KEY_UP: i32 = 315;
const KEY_DOWN: i32 = 317;
const KEY_PAGE_UP: i32 = 366;
const KEY_PAGE_DOWN: i32 = 367;

/// Highlight mode and the status line follow the caret. After a movement by key, the screen reader hears what the
/// reading settings ask for: a new page, a skipped line, a change of style, and in a braille document the line
/// that was moved to (translated or as braille cells).
fn caret_moved(key: u64, moved_by: Option<i32>) {
    let mut said: Option<(*mut c_void, String)> = None;
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
        let (page, _, _, line_no, line, style) = a.caret_place(i);
        let before = a.docs[i].reader_at.replace((page, line_no, style.clone()));
        let (Some(code), Some((old_page, old_line, old_style))) = (moved_by, before) else { return };
        let r = &a.settings.reader;
        let vertical = matches!(code, KEY_UP | KEY_DOWN | KEY_PAGE_UP | KEY_PAGE_DOWN | KEY_HOME | KEY_END);
        let mut parts: Vec<String> = Vec::new();
        if page != old_page {
            parts.push(format!("Page {page}"));
        }
        if r.skipped_lines && page == old_page && matches!(code, KEY_UP | KEY_DOWN) && line_no.abs_diff(old_line) > 1 {
            parts.push("line skip".to_string());
        }
        if r.style_changes && style != old_style {
            parts.push(style);
        }
        if a.docs[i].kind == DocKind::Braille && vertical && line_no != old_line {
            match r.braille_lines {
                BrailleLines::Translated => {
                    let t = a.translate_line(i, &line);
                    parts.push(if t.trim().is_empty() { speech::BLANK.to_string() } else { t });
                }
                BrailleLines::Cells => parts.push(speech::speak_line_as_cells(&line, r.local_signs)),
                BrailleLines::Silent => {}
            }
        }
        if !parts.is_empty() {
            said = Some((a.docs[i].text.get_handle(), parts.join(". ")));
        }
    });
    if let Some((widget, text)) = said {
        speech::announce(widget, &text, false);
    }
}

thread_local! {
    /// The last reading key and when it was pressed, for the keys that do more when pressed twice quickly.
    static LAST_READING_KEY: Cell<(i32, Option<Instant>)> = const { Cell::new((0, None)) };
}

fn pressed_twice(code: i32) -> bool {
    LAST_READING_KEY.with(|c| {
        let (last, at) = c.get();
        let now = Instant::now();
        c.set((code, Some(now)));
        last == code && at.is_some_and(|t| now.duration_since(t) < Duration::from_millis(500))
    })
}

/// The reading keys of the document window (Alt with 7, 8, 9, comma, period, End, U, I, P, W, semicolon); see
/// speech.rs. Returns true when the key was one of them.
fn reader_key(key: u64, code: i32) -> bool {
    let code = if (97..=122).contains(&code) { code - 32 } else { code };
    const KEYS: [i32; 11] = [55, 56, 57, 44, 46, 59, KEY_END, 85, 73, 80, 87];
    if !KEYS.contains(&code) {
        return false;
    }
    let twice = pressed_twice(code);
    let mut said: Option<(*mut c_void, String)> = None;
    let mut to_line: Option<TextCtrl> = None;
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        let widget = a.docs[i].text.get_handle();
        let (page, line_on_page, column, _, line, style) = a.caret_place(i);
        let braille = a.docs[i].kind == DocKind::Braille;
        let coded = a.docs[i].view.codes;
        let signs = a.settings.reader.local_signs;
        let mut turned_on = false;
        if matches!(code, 55 | 59) && !a.docs[i].view.translated {
            a.docs[i].view.translated = true;
            a.apply_view();
            turned_on = true;
        }
        let text = match code {
            55 => {
                let t = a.translate_line(i, &line);
                let t = if braille {
                    if t.trim().is_empty() { speech::BLANK.to_string() } else { t }
                } else {
                    speech::speak_line_as_cells(&t, signs)
                };
                if turned_on { format!("{}. {t}", speech::TRANSLATED_LINE_ON) } else { t }
            }
            56 => style,
            57 if coded => speech::NO_LOCATION.to_string(),
            57 => speech::location(page, line_on_page, column),
            44 => speech::line_position(&line, column, twice),
            46 if coded => speech::CODED_VIEW.to_string(),
            46 => speech::line_length(&line, twice),
            KEY_END if coded => speech::NO_LOCATION.to_string(),
            KEY_END => {
                let word = speech::last_word(line.trim_end());
                if word.is_empty() {
                    speech::BLANK.to_string()
                } else if braille {
                    speech::speak_cells(word, signs)
                } else {
                    word.to_string()
                }
            }
            85 => a.settings.reader.cycle_braille_lines().to_string(),
            73 => a.settings.reader.cycle_style_announcements(),
            80 => a.settings.reader.cycle_entry_echo(),
            87 => a.settings.reader.toggle_completed_words().to_string(),
            _ => {
                // Alt+semicolon: the focus goes to the start of the translated line; the screen reader speaks it.
                to_line = Some(a.docs[i].line);
                if turned_on { speech::TURNING_ON_TRANSLATED_LINE.to_string() } else { String::new() }
            }
        };
        said = Some((widget, text));
    });
    if let Some(line) = to_line {
        line.set_focus();
        line.set_insertion_point(0);
    }
    if let Some((widget, text)) = said {
        speech::announce(widget, &text, true);
    }
    true
}

/// On the translated line, Alt+semicolon gives the focus back to the editing area.
fn wire_line(line: &TextCtrl, editor: &TextCtrl) {
    let editor = *editor;
    line.on_key_down(move |e| {
        if let WindowEventData::Keyboard(k) = &e {
            if k.alt_down() && !k.control_down() && k.get_key_code() == Some(59) {
                editor.set_focus();
                return;
            }
            k.event.skip(true);
        }
    });
}

/// The echo for braille entry: a cell entered (with six keys, or typed) is spoken by its name when the echo is
/// characters; the word just completed is spoken as braille cells when a space is typed and the echo is words.
fn echo_entry(key: u64, entered: Option<char>) {
    let mut said: Option<(*mut c_void, String)> = None;
    app::try_with(|a| {
        let Some(i) = a.index_of(key) else { return };
        let d = &a.docs[i];
        if d.kind != DocKind::Braille {
            return;
        }
        let r = &a.settings.reader;
        let value = d.text.get_value();
        let pos = d.text.get_insertion_point();
        let before = &value[..app::char_to_byte(&value, pos)];
        let typed = entered.or_else(|| before.chars().last());
        let Some(c) = typed else { return };
        let text = if c == ' ' {
            if !r.echo_words() {
                return;
            }
            let line = before.rsplit('\n').next().unwrap_or("").trim_end();
            let word = line.rsplit(' ').next().unwrap_or("");
            if word.is_empty() {
                return;
            }
            speech::speak_cells(word, r.local_signs)
        } else {
            if !r.echo_characters() {
                return;
            }
            speech::speak_cells(&c.to_string(), r.local_signs)
        };
        said = Some((d.text.get_handle(), text));
    });
    if let Some((widget, text)) = said {
        speech::announce(widget, &text, true);
    }
}

fn six_key_dot(set: &str, key_code: Option<i32>) -> Option<usize> {
    let code = key_code?;
    let c = char::from_u32(code as u32)?.to_ascii_lowercase();
    set.chars().position(|k| k == c)
}
