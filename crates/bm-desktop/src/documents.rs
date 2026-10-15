//! Document windows: each open document is a child window holding the editing area (a native multi-line text
//! control), under it the one-line translated line on yellow, and at the bottom the document's status line. The
//! child windows sit maximized in the main window, as in the reference screenshots.

use std::collections::HashSet;

use bm_core::doctext::Shown;
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
        last_style: None,
        highlight_anchor: None,
        last_line_key: String::new(),
        six_down: HashSet::new(),
        chord: 0,
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
    let edited = *text;
    text.on_text_updated(move |_| {
        app::try_with(|a| {
            if let Some(i) = a.index_of(key) {
                // The codes follow the edit; a code that lost a bracket goes as a whole, as codes do in the
                // reference program, once this change has been handled.
                let remains = a.sync_codes(i);
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
                }
            }
        });
    });

    let t = *text;
    text.on_key_down(move |e| {
        let mut consumed = false;
        if let WindowEventData::Keyboard(k) = &e {
            app::try_with(|a| {
                let Some(i) = a.index_of(key) else { return };
                // Copy and cut keep the selection with its codes, so pasting it brings them back.
                let code = k.get_key_code().unwrap_or(0);
                if k.control_down() && (code == 'C' as i32 || code == 'X' as i32 || code == 'c' as i32 || code == 'x' as i32) {
                    let (from, to) = a.docs[i].text.get_selection();
                    let part = a.docs[i].shown().slice(from.max(0) as usize, to.max(0) as usize);
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
                t.write_text(&c.to_string());
            }
            k.event.skip(true);
            caret_moved(key);
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

fn six_key_dot(set: &str, key_code: Option<i32>) -> Option<usize> {
    let code = key_code?;
    let c = char::from_u32(code as u32)?.to_ascii_lowercase();
    set.chars().position(|k| k == c)
}
