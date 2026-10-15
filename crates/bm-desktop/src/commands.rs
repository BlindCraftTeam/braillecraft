//! Every menu command, by command id. Dialogs are opened by resource id (ui-native/dialogs.xrc) and their
//! controls read by control id; the ids are listed in crates/bm-desktop/src/dialog_table.rs.
//!
//! The state is never held while a dialog or message is open (see app.rs), so each command reads what it needs,
//! shows its dialog, then writes back.

use std::path::Path;

use wxdragon::prelude::*;

use crate::app::{self, DocKind, FindSpec, Hyphen, Style, ViewFont};
use crate::codes::{self, Dynamic, Insert};
use crate::dlg::Dlg;
use crate::documents;
use crate::encodings;
use crate::settings_store;
use crate::translation;
use bm_core::doctext::Shown;
use bm_core::table_list::TableList;

// Messages for parts of the program that later build steps bring. They say what is missing and where it comes.
const PLANNED_EBRAILLE: &str = "eBraille output is built after the formatted braille, document and PEF outputs.";
const PLANNED_PRINT: &str = "Printing is built in step 9 of the plan.";
const PLANNED_EMBOSS: &str = "Embossing (Index and Enabling embossers, and emboss to file) is built in step 10 of the plan.";
const PLANNED_SPELL: &str = "The spell checker (Hunspell dictionaries) is built after the core features of version 1.";
const PLANNED_HELP: &str = "The BrailleMaker help is written with the features it describes; it is not in this build yet.";
const PLANNED_TEMPLATES: &str = "Templates are built with the BrailleMaker document format in step 7 of the plan.";
const PLANNED_PICTURES: &str = "Pictures are built after the core features of version 1.";
const PLANNED_FORMATTER: &str = "This needs the page formatter, built in step 8 of the plan.";

const FORMS: &[&str] = &["11 x 11.5 inches", "11 x 8.5 inches (Letter)", "A4 (297 x 210 mm)", "11.5 x 11 inches (tractor)"];
const STYLE_SHOW: &[&str] = &["Recommended and In-Use Styles", "All Styles", "Styles In-Use", "Only Recommended Styles"];

// ------------------------------------------------------------------ messages

pub fn message(text: &str) {
    let f = app::frame();
    MessageDialog::builder(&f, text, "BrailleMaker")
        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
        .build()
        .show_modal();
}

fn yes_no(text: &str) -> bool {
    let f = app::frame();
    MessageDialog::builder(&f, text, "BrailleMaker")
        .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion)
        .build()
        .show_modal()
        == wxdragon::id::ID_YES
}

/// Yes, No or Cancel: Some(true), Some(false), None.
fn yes_no_cancel(text: &str) -> Option<bool> {
    let f = app::frame();
    let r = MessageDialog::builder(&f, text, "BrailleMaker")
        .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::Cancel | MessageDialogStyle::IconWarning)
        .build()
        .show_modal();
    if r == wxdragon::id::ID_YES {
        Some(true)
    } else if r == wxdragon::id::ID_NO {
        Some(false)
    } else {
        None
    }
}

fn open_dialog(id: i32, title: Option<&str>) -> Dlg {
    Dlg::open(&app::frame(), id, title)
}

/// Document, Page Numbering: the document's own page numbering (from its template or its file), changed on OK. The
/// four groups are braille page numbers on even and on odd pages, print page numbers on even and on odd pages, each
/// none, upper left, upper right, lower left, lower right; then the first page number shown.
fn page_numbering() {
    const GROUPS: [[i32; 5]; 4] = [[7, 8, 9, 10, 11], [13, 14, 15, 16, 17], [19, 20, 21, 22, 23], [25, 26, 27, 28, 29]];
    let Some(numbers) = app::with(|a| a.doc().map(|d| d.page_numbers)) else { return };
    let d = open_dialog(1031, None);
    for (g, n) in GROUPS.iter().zip(numbers) {
        d.set_radio(g, n as usize);
    }
    d.set_text(31, &numbers[4].to_string());
    let ok = d.show() == 1;
    let mut chosen = numbers;
    for (k, g) in GROUPS.iter().enumerate() {
        if let Some(i) = d.radio(g) {
            chosen[k] = i as u16;
        }
    }
    let first = d.number(31);
    d.destroy();
    if !ok {
        return;
    }
    // A first page number that is not a number keeps the one the document had.
    if let Some(n) = first.filter(|n| (1..=9999).contains(n)) {
        chosen[4] = n as u16;
    }
    app::with(|a| {
        if let Some(doc) = a.doc_mut() {
            if doc.page_numbers != chosen {
                doc.page_numbers = chosen;
                doc.dirty = true;
            }
        }
        a.update_status();
    });
}

/// A settings dialog: shown with its last values; the values are kept on OK. Returns the dialog on OK.
fn settings_dialog(id: i32, key: &'static str, title: Option<&str>, setup: impl FnOnce(&Dlg)) -> Option<Dlg> {
    let d = open_dialog(id, title);
    setup(&d);
    if let Some(v) = app::with(|a| a.memory.get(id, key).cloned()) {
        d.restore(&v);
    }
    if d.show() == 1 {
        let values = d.values();
        app::with(|a| a.memory.put(id, key, values));
        Some(d)
    } else {
        d.destroy();
        None
    }
}

// ------------------------------------------------------------------ editing helpers

fn active_text() -> Option<TextCtrl> {
    app::with(|a| a.doc().map(|d| d.text))
}

fn insert_text(text: &str) {
    insert_shown(&Shown::plain(text));
}

/// Puts `s` (text and codes) in place of the selection, its codes known as codes; the caret goes after it.
fn insert_shown(s: &Shown) {
    if let Some(t) = active_text() {
        let (from, to) = t.get_selection();
        put_at(t, from, to, s);
        t.set_insertion_point(from + s.text.chars().count() as i64);
        t.set_focus();
    }
}

/// Replaces characters `from` to `to` with `s`, and marks its codes in the document.
fn put_at(t: TextCtrl, from: i64, to: i64, s: &Shown) {
    t.replace(from, to, &s.text);
    let at = from.max(0) as usize;
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            d.codes.extend(s.codes.iter().map(|c| (c.0 + at, c.1 + at)));
            d.inner.extend(s.inner.iter().map(|c| (c.0 + at, c.1 + at)));
            d.codes.sort_unstable();
            d.codes.dedup();
            d.inner.sort_unstable();
            d.inner.dedup();
            d.pages = None;
            d.last_line_key.clear();
        }
        a.update_status();
    });
}

/// Puts `before` at the start of the selection and `after` at its end (codes, as a pair); the selection itself is
/// left as it is.
fn wrap_selection(before: &Shown, after: &Shown) {
    if let Some(t) = active_text() {
        let (from, to) = t.get_selection();
        put_at(t, to, to, after);
        put_at(t, from, from, before);
        let caret = to + before.text.chars().count() as i64;
        t.set_insertion_point(caret);
        t.set_focus();
    }
}

fn select_range(from: i64, to: i64) {
    if let Some(t) = active_text() {
        t.set_selection(from, to);
        t.show_position(from);
        t.set_focus();
    }
    app::with(|a| a.update_status());
}

fn insert_code(id: i32) {
    let Some(code) = codes::insert(id) else { return };
    // What a code item puts in is codes, except the signs and the hard space, which are text.
    match code {
        Insert::Text(s) => insert_shown(&Shown::marked(s)),
        Insert::Pair(a, b) => wrap_selection(&Shown::marked(a), &Shown::marked(b)),
        Insert::Setting(which) => {
            let s = app::with(|a| {
                let sc = &a.settings.shortcuts;
                match which {
                    Dynamic::Grade0 => Shown::marked(&sc.grade0),
                    Dynamic::Cbc => Shown::marked(&sc.cbc),
                    Dynamic::FlushRightLeader => Shown::marked(&sc.flush_right_leader),
                    Dynamic::NewPage => Shown::marked(&sc.new_page),
                    Dynamic::NewReferencePage => Shown::marked(&sc.new_ref_page),
                    Dynamic::TerminationSign => {
                        Shown::plain(if a.doc().is_some_and(|d| d.kind == DocKind::Braille) { ",'" } else { "&@" })
                    }
                    Dynamic::HardSpace => Shown::plain("\u{a0}"),
                }
            });
            insert_shown(&s);
        }
    }
}

// ------------------------------------------------------------------ the command table

/// Runs the command with this id. `checked` is the new state of a check item.
pub fn run(id: i32, checked: bool) {
    if (crate::menu::DOCUMENT_BASE..crate::menu::DOCUMENT_BASE + 100).contains(&id) {
        documents::bring_to_front((id - crate::menu::DOCUMENT_BASE) as usize);
        return;
    }
    // Menu commands wait while one is running (its dialog is modal).
    if app::with(|a| std::mem::replace(&mut a.busy, true)) {
        return;
    }
    let has_doc = app::with(|a| a.active.is_some());
    match id {
        1101 => file_new(),
        1102 => open_file(),
        1103 => close_document(None),
        1105 if has_doc => {
            save(None, false);
        }
        1106 if has_doc => {
            save(None, true);
        }
        1107 => message(PLANNED_EBRAILLE),
        1109 => translate_document(),
        1111 => print(),
        1112 => emboss(),
        1114 => exit_program(),

        1201..=1207 => edit_standard(id),
        1209 => toggle_highlight(),
        1211 => change_case(),
        1213 => insert_file(),
        1215 => goto_position(),
        1216 => find(false),
        1217 => find(true),
        1218 => replace(),
        1220 | 1221 => message(PLANNED_SPELL),

        1301 => set_view(|v| v.full_page = checked),
        1303 => set_font(ViewFont::Print),
        1304 => set_font(ViewFont::Braille),
        1305 => set_font(ViewFont::SimBraille),
        1307 => six_key(checked),
        1309 => set_view(|v| v.codes = checked),
        1310 => set_view(|v| v.translated = checked),

        1401 => apply_style(),
        1402 => apply_last_style(),
        1403 | 1404 | 1406 => message(PLANNED_FORMATTER),
        1407 => join_lines(),
        1409 => set_page_number(),
        1410 => header_footer(1029, "[tls]", "[tle]", "[tld]"),
        1411 => header_footer(1030, "[rfs]", "[rfe]", "[rfd]"),
        1413 => message(PLANNED_FORMATTER),
        1416 => code_list(),
        1701..=2404 if has_doc => insert_code(id),
        2501..=2511 => message(PLANNED_PICTURES),

        1501 => {
            if settings_dialog(1062, "", None, fill_table_properties).is_some() {
                wrap_selection(&Shown::marked("[htbs]"), &Shown::marked("[htbe]"));
            }
        }
        1503 => {
            settings_dialog(1062, "", None, fill_table_properties);
        }

        2601 => embosser_setup(false),
        2602 => printer_setup(false),
        2604 => page_numbering(),
        2701..=2705 => set_region(TableList::region_bit((id - 2701) as usize)),
        2706 => set_region(bm_core::table_list::ALL_REGIONS),
        9000..=9399 => choose_language((id - crate::menu::LANGUAGE_BASE) as usize),
        9500..=9509 => choose_learning_step((id - crate::menu::COURSE_BASE) as usize),
        2801 => set_grade("[g1l]"),
        2802 => set_grade("[g2u]"),
        2901..=2905 => set_hyphen(id),
        2606 => message(PLANNED_EBRAILLE),
        2608 => add_style(),
        2609 => modify_style(),
        2610 => delete_style(),
        2612 => create_template(),
        2613 => message(PLANNED_TEMPLATES),

        3101 => embosser_setup(true),
        3102 => printer_setup(true),
        3104 => {
            settings_dialog(1060, "", None, |d| {
                d.set_checked(10, true);
                d.set_text(12, "");
                d.set_text(14, "05:00");
            });
        }
        3105 => default_views(),
        3106 => internationalization(),
        3107 => global_fonts(),
        3108 => view_preferences(),
        3109 => shortcut_preferences(),
        3111 => {
            let d = open_dialog(1023, None);
            d.show();
            d.destroy();
        }
        3113 => {
            settings_dialog(1037, "", None, |d| {
                d.set_items(7, &["Text Only", "URLs Only", "Both Text and URLs", "Structured Hyperlinks"], Some(0));
                d.set_items(23, &["Mark with asterisk (*)", "Ignore", "Output Unicode value"], Some(0));
                d.set_checked(26, true);
            });
        }
        3114 => {
            settings_dialog(1035, "", None, |d| {
                d.set_checked(13, true);
                d.set_text(16, "0.25");
                d.set_checked(18, true);
            });
        }
        3115 => {
            settings_dialog(1036, "", None, |d| {
                d.set_items(
                    10,
                    &["Ignore", "Mark from Word document", "Treat hard page breaks as print page breaks", "Use publishers' <ipp no=#> reference page tags"],
                    Some(0),
                );
            });
        }
        3116 => message("The PDF importer and its options are built in step 9 of the plan."),
        3117 => {
            settings_dialog(1045, "", None, |d| {
                d.set_radio(&[8, 9, 10, 11, 12], 2);
                d.set_radio(&[14, 15, 16, 17, 18], 4);
                d.set_radio(&[20, 21, 22, 23, 24], 0);
                d.set_radio(&[26, 27, 28, 29, 30], 0);
                d.set_radio(&[32, 33, 34, 35], 0);
                d.set_radio(&[37, 38, 39, 40], 0);
                d.set_text(42, "25");
                d.set_text(44, "40");
                // "When this is checked, most of the other controls on the dialog are disabled" (the manual).
                let (id, dialog) = (d.id, d.dialog);
                let apply = move || {
                    let d = Dlg { id, dialog };
                    let raw = d.checked(6);
                    for c in 7..=44 {
                        d.enable(c, !raw);
                    }
                };
                if let Some(cb) = d.win(6).and_then(|w| w.as_widget::<CheckBox>()) {
                    cb.on_toggled(move |_| apply());
                }
                // Applied once the remembered values are in place and the dialog is showing.
                wxdragon::call_after(Box::new(apply));
            });
        }

        3202 | 3204 | 3205 => message(PLANNED_HELP),
        3206 => message("BrailleMaker 0.1: the main window, menus, dialogs, editing, find and replace, six-key entry, the code vocabulary, braille fonts for 18 encodings, and translation with the BrailleMaker engine and its tables. The file format, printing and embossing follow in the plan."),
        3210 => message("Updates are published on the BrailleMaker GitHub page."),
        3211 => about(),
        _ => {}
    }
    app::with(|a| {
        a.busy = false;
        a.refresh_menus();
        a.update_status();
    });
}

// ------------------------------------------------------------------ File

/// What File, New shows: the templates in alphabetical order, less the hidden ones (unless Show Hidden Templates is
/// checked) and those of other regions (Select Region...); the default template is selected.
struct NewDialogState {
    hidden: Vec<String>,
    default: String,
    show_hidden: bool,
    region: u32,
    /// The names listed now, and the one the list cursor is on. The template list allows several selections, as
    /// the reference has it; the widget gives no way to read a selection there, so the item is followed from the
    /// list's selection events.
    names: Vec<String>,
    current: Option<String>,
}

const REGION_NAMES: [&str; 6] = ["Western Hemisphere", "Europe", "Asia", "Africa", "Pacific", "All Regions/All Languages"];

fn region_of(i: usize) -> u32 {
    if i >= 5 {
        bm_core::table_list::ALL_REGIONS
    } else {
        TableList::region_bit(i)
    }
}

/// The templates listed: a template whose tables are in the table list belongs to that language's regions; one
/// whose tables are not listed is shown in every region.
fn new_dialog_names(s: &NewDialogState) -> Vec<String> {
    let names = translation::with_templates(|t| t.map(|t| t.names_sorted()).unwrap_or_default());
    names
        .into_iter()
        .filter(|n| s.show_hidden || !s.hidden.contains(n))
        .filter(|n| {
            let Some(tables) = translation::template_tables(n) else { return true };
            translation::with_list(|l| l.entry_for(&tables).map_or(true, |(i, _)| l.entries[i].regions & s.region != 0))
        })
        .collect()
}

fn fill_new_dialog(d: &Dlg, s: &mut NewDialogState, keep: Option<String>) {
    let names = new_dialog_names(s);
    let items: Vec<&str> = names.iter().map(String::as_str).collect();
    let want = keep.unwrap_or_else(|| s.default.clone());
    let sel = names.iter().position(|n| *n == want).or(if names.is_empty() { None } else { Some(0) });
    d.set_items(10, &items, sel.map(|i| i as u32));
    s.current = sel.map(|i| names[i].clone());
    s.names = names;
    update_hide_button(d, s);
}

/// Hide Template and Show Template Always share a place: the second shows for a hidden template.
fn update_hide_button(d: &Dlg, s: &NewDialogState) {
    let hidden = s.current.as_ref().is_some_and(|n| s.hidden.contains(n));
    if let Some(w) = d.win(12) {
        w.show(!hidden);
    }
    if let Some(w) = d.win(13) {
        w.show(hidden);
    }
}

fn file_new() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let state = Rc::new(RefCell::new(NewDialogState {
        hidden: settings_store::load_hidden_templates(),
        default: translation::default_template(),
        show_hidden: false,
        region: bm_core::table_list::ALL_REGIONS,
        names: Vec::new(),
        current: None,
    }));
    let d = open_dialog(1017, None);
    d.set_radio(&[7, 8], 0);
    d.set_checked(15, false);
    fill_new_dialog(&d, &mut state.borrow_mut(), None);
    let copy = |d: &Dlg| Dlg { id: d.id, dialog: d.dialog };

    let (dc, st) = (copy(&d), state.clone());
    d.on_button(11, move || {
        let Some(name) = st.borrow().current.clone() else { return };
        if st.borrow().hidden.contains(&name) {
            message("This template is normally hidden, so it cannot be made the default.  You must first use the \"Show Template Always\" button.");
            return;
        }
        settings_store::save_default_template(&name);
        st.borrow_mut().default = name.clone();
        fill_new_dialog(&dc, &mut st.borrow_mut(), Some(name));
    });
    let (dc, st) = (copy(&d), state.clone());
    d.on_button(12, move || {
        let Some(name) = st.borrow().current.clone() else { return };
        if st.borrow().default == name {
            message("This is the default template.  It cannot be hidden unless you first choose another default.");
            return;
        }
        st.borrow_mut().hidden.push(name.clone());
        settings_store::save_hidden_templates(&st.borrow().hidden);
        fill_new_dialog(&dc, &mut st.borrow_mut(), Some(name));
    });
    let (dc, st) = (copy(&d), state.clone());
    d.on_button(13, move || {
        let Some(name) = st.borrow().current.clone() else { return };
        st.borrow_mut().hidden.retain(|n| *n != name);
        settings_store::save_hidden_templates(&st.borrow().hidden);
        fill_new_dialog(&dc, &mut st.borrow_mut(), Some(name));
    });
    let (dc, st) = (copy(&d), state.clone());
    d.on_button(14, move || {
        let current = (0..REGION_NAMES.len()).position(|i| region_of(i) == st.borrow().region);
        let r = open_dialog(1018, None);
        r.set_items(7, &REGION_NAMES, current.map(|i| i as u32));
        let ok = r.show() == 1;
        let chosen = r.selection(7);
        r.destroy();
        if let (true, Some(i)) = (ok, chosen) {
            st.borrow_mut().region = region_of(i as usize);
            let keep = st.borrow().current.clone();
            fill_new_dialog(&dc, &mut st.borrow_mut(), keep);
        }
    });
    if let Some(c) = d.win(15).and_then(|w| w.as_widget::<CheckBox>()) {
        let (dc, st) = (copy(&d), state.clone());
        c.on_toggled(move |e| {
            st.borrow_mut().show_hidden = e.is_checked();
            let keep = st.borrow().current.clone();
            fill_new_dialog(&dc, &mut st.borrow_mut(), keep);
        });
    }
    if let Some(l) = d.win(10).and_then(|w| w.as_widget::<ListBox>()) {
        let (dc, st) = (copy(&d), state.clone());
        l.on_selection_changed(move |e| {
            let mut s = st.borrow_mut();
            s.current = e.get_selection().and_then(|i| s.names.get(i as usize).cloned());
            update_hide_button(&dc, &s);
        });
    }
    let ok = d.show() == 1;
    let braille = d.radio(&[7, 8]) == Some(1);
    let template = state.borrow().current.clone();
    d.destroy();
    if ok {
        let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, None, &Shown::default());
        if let Some(name) = template {
            let tables = translation::template_tables(&name);
            let styles = translation::template_style_list(&name);
            let numbers = translation::template_page_numbers(&name);
            app::with(|a| {
                if let Some(i) = a.index_of(key) {
                    if let Some(t) = tables {
                        a.docs[i].tables = t;
                    }
                    a.docs[i].styles = styles;
                    a.docs[i].page_numbers = numbers;
                    a.docs[i].template = name;
                    a.docs[i].last_line_key.clear();
                }
                a.refresh_menus();
            });
        }
    }
}

fn read_text_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        // Not UTF-8: read as Windows Latin 1, the usual code page of braille and text files from Windows.
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    };
    Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn pick_file(message_text: &str) -> Option<std::path::PathBuf> {
    let f = app::frame();
    let dialog = FileDialog::builder(&f)
        .with_message(message_text)
        .with_wildcard("All supported files|*.txt;*.brf;*.brl;*.bru;*.dxp;*.dxb|Print and braille documents (*.dxp, *.dxb)|*.dxp;*.dxb|Braille files (*.brf, *.brl, *.bru)|*.brf;*.brl;*.bru|Text files (*.txt)|*.txt|All files|*")
        .with_style(FileDialogStyle::Open | FileDialogStyle::FileMustExist)
        .build();
    if dialog.show_modal() == wxdragon::id::ID_OK {
        dialog.get_path().map(Into::into)
    } else {
        None
    }
}

/// A .dxp (print) or .dxb (braille) document.
fn open_dx_document(path: &Path) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return message(&format!("{}: {e}", path.display())),
    };
    let doc = match bm_core::dxfile::read(&bytes) {
        Ok(d) => d,
        Err(e) => return message(&e.to_string()),
    };
    let braille = doc.kind == bm_core::DocumentKind::Braille;
    // The file's codes are codes and its brackets text. Braille is stored in North American ASCII braille; it is
    // shown in the encoding chosen in Internationalization.
    let shown = Shown::from_running(&doc.text);
    let shown = if braille {
        let to = app::with(|a| a.encoding());
        translation::convert_braille(&shown, encodings::find(encodings::DEFAULT), to)
    } else {
        shown
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, name, &shown);
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            let d = &mut a.docs[i];
            d.dirty = false;
            d.saved = true;
            d.path = Some(path.to_path_buf());
            // The document's own styles, as the reference program loads them from the file (spec 05-B (c)).
            if !doc.styles.is_empty() {
                d.styles = doc
                    .styles
                    .iter()
                    .map(|(name, begin, end)| Style { name: name.clone(), nested: !name.ends_with('.'), begin: begin.clone(), end: end.clone() })
                    .collect();
            }
            d.file_styles = doc.styles.iter().map(|s| s.0.clone()).zip(doc.style_index.iter().copied()).collect();
            d.file_settings = doc.settings.clone();
            d.view.translated = doc.translated_line;
            d.view.six_key = braille && doc.six_key;
            d.page_numbers = doc.page_numbering;
            // The document's own tables, as the file names them, matched to the table list: a set the list holds
            // stays; when only the print-to-braille rules table (or a learning step) is listed, the other three are
            // that language's; a table the list does not hold gives the tables of a new document.
            let stored: [String; 4] = doc.tables.clone().map(|t| t.to_lowercase());
            d.tables = match translation::with_list(|l| l.entry_for(&stored).map(|(e, _)| l.entries[e].tables.clone())) {
                Some(listed) => {
                    let mut t = stored.clone();
                    t[1..].clone_from_slice(&listed[1..]);
                    t
                }
                None => translation::DEFAULT_TABLES.map(str::to_string),
            };
        }
        a.apply_view();
    });
}

fn open_file() {
    let Some(path) = pick_file("Open") else { return };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "dxp" || ext == "dxb" {
        open_dx_document(&path);
        return;
    }
    match read_text_file(&path) {
        Ok(text) => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let braille = matches!(ext.as_str(), "brf" | "brl" | "bru");
            let text = if braille { text.replace('\u{c}', "\n") } else { text };
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            // A text or braille file holds no codes: all of it is text.
            let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, name, &Shown::plain(&text));
            app::with(|a| {
                if let Some(i) = a.index_of(key) {
                    a.docs[i].dirty = false;
                    a.docs[i].path = Some(path.clone());
                }
            });
        }
        Err(e) => message(&e),
    }
}

fn insert_file() {
    if active_text().is_none() {
        return;
    }
    let Some(path) = pick_file("Insert File") else { return };
    match read_text_file(&path) {
        Ok(text) => insert_text(&text),
        Err(e) => message(&e),
    }
}

/// Asks about an unsaved document. Returns false when the user cancels.
fn confirm_discard(key: u64) -> bool {
    let Some((dirty, name)) = app::with(|a| a.index_of(key).map(|i| (a.docs[i].dirty, a.docs[i].name.clone()))) else {
        return true;
    };
    if !dirty {
        return true;
    }
    match yes_no_cancel(&format!("Document \"{name}\" not saved!  Save changes?")) {
        None => false,
        Some(true) => save(Some(key), false),
        Some(false) => true,
    }
}

// ------------------------------------------------------------------ File, Save and Save As

/// What a Save As file type writes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveType {
    Document,
    /// The older document file format (version 0, spec 04d-C).
    OlderDocument,
    Formatted,
}

/// The Save As file types, in the reference program's order for each kind of document (spec 04d-B), as far as
/// BrailleMaker writes them; the first is chosen when the dialog opens, every time.
fn save_types(braille: bool) -> &'static [(&'static str, &'static str, SaveType)] {
    if braille {
        &[
            ("Braille document (*.dxb)", "dxb", SaveType::Document),
            ("Braille document, older format (*.dxb)", "dxb", SaveType::OlderDocument),
            ("Formatted braille (*.brf)", "brf", SaveType::Formatted),
        ]
    } else {
        &[
            ("Print document (*.dxp)", "dxp", SaveType::Document),
            ("Print document, older format (*.dxp)", "dxp", SaveType::OlderDocument),
            ("Formatted text (*.prf)", "prf", SaveType::Formatted),
        ]
    }
}

/// File, Save (`as_new` false) and File, Save As (true) for the document with this key (the active one when None).
/// A document read from or saved to a document file (.dxp, .dxb) is saved there again; any other is asked about.
/// Returns false when nothing was saved.
fn save(key: Option<u64>, as_new: bool) -> bool {
    let Some(key) = key.or_else(|| app::with(|a| a.doc().map(|d| d.key))) else { return false };
    let Some((kind, path, saved, name)) =
        app::with(|a| a.index_of(key).map(|i| (a.docs[i].kind, a.docs[i].path.clone(), a.docs[i].saved, a.docs[i].name.clone())))
    else {
        return false;
    };
    let braille = kind == DocKind::Braille;
    let own = if braille { "dxb" } else { "dxp" };
    if !as_new && saved {
        if let Some(p) = path.as_ref().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(own))) {
            return write_document(key, p, 1);
        }
    }
    let types = save_types(braille);
    let wildcard = types.iter().map(|(label, ext, _)| format!("{label}|*.{ext}")).collect::<Vec<_>>().join("|");
    // The name proposed: the document's own, with the extension of the first type.
    let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
    let f = app::frame();
    let dialog = FileDialog::builder(&f)
        .with_message("Save As")
        .with_wildcard(&wildcard)
        .with_default_file(&format!("{stem}.{own}"))
        .with_style(FileDialogStyle::Save | FileDialogStyle::OverwritePrompt)
        .build();
    if let Some(dir) = path.as_ref().and_then(|p| p.parent()) {
        dialog.set_directory(&dir.to_string_lossy());
    }
    dialog.set_filter_index(0);
    if dialog.show_modal() != wxdragon::id::ID_OK {
        return false;
    }
    let Some(chosen) = dialog.get_path().map(std::path::PathBuf::from) else { return false };
    // The chosen type, unless the name is typed with the extension of another type, which then gives the first
    // type with that extension.
    let filter = types.get(dialog.get_filter_index().max(0) as usize).copied().unwrap_or(types[0]);
    let typed = chosen.extension().and_then(|e| e.to_str()).map(str::to_string);
    let kind = match typed {
        Some(e) if !filter.1.eq_ignore_ascii_case(&e) => types.iter().find(|t| t.1.eq_ignore_ascii_case(&e)).copied().unwrap_or(filter),
        _ => filter,
    };
    let chosen = if chosen.extension().is_some_and(|e| e.eq_ignore_ascii_case(kind.1)) { chosen } else { chosen.with_extension(kind.1) };
    match kind.2 {
        SaveType::Formatted => write_formatted(key, &chosen, braille),
        SaveType::OlderDocument => write_document(key, &chosen, 0),
        SaveType::Document => write_document(key, &chosen, 1),
    }
}

/// Writes the document as a document file (.dxp or .dxb) in format `version` (1, or 0 for the older format), then
/// takes that file as its own.
fn write_document(key: u64, path: &Path, version: u16) -> bool {
    let Some(doc) = app::with(|a| {
        let i = a.index_of(key)?;
        let d = &a.docs[i];
        let braille = d.kind == DocKind::Braille;
        let shown = d.shown();
        // Braille is stored in North American ASCII braille, whatever encoding it is shown in.
        let shown = if braille { translation::convert_braille(&shown, a.encoding(), encodings::find(encodings::DEFAULT)) } else { shown };
        let styles: Vec<(String, String, String)> = d.styles.iter().map(|s| (s.name.clone(), s.begin.clone(), s.end.clone())).collect();
        // The file's style order and index values are kept while the style names are the ones it was read with.
        let same = d.file_styles.len() == styles.len() && d.file_styles.iter().zip(&styles).all(|(f, s)| f.0 == s.0);
        Some(bm_core::dxfile::DxDocument {
            kind: if braille { bm_core::DocumentKind::Braille } else { bm_core::DocumentKind::Print },
            version,
            page_numbering: d.page_numbers,
            translated_line: d.view.translated,
            six_key: braille && d.view.six_key,
            tables: {
                // A table is named as the file it was read from names it (the program matches names in small letters).
                let named: [String; 4] = std::array::from_fn(|i| {
                    let stored = d.file_settings.get(32 + i * 8..40 + i * 8).map(|b| {
                        b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>()
                    });
                    stored.filter(|s| s.eq_ignore_ascii_case(&d.tables[i])).unwrap_or_else(|| d.tables[i].clone())
                });
                // The older format gives older names where its code is among the document's initial codes.
                if version == 0 {
                    bm_core::dxfile::older_table_names(&named, |code| d.initial_codes.contains(&format!("[{code}]")))
                } else {
                    named
                }
            },
            text: shown.to_running(),
            style_index: if same { d.file_styles.iter().map(|f| f.1).collect() } else { Vec::new() },
            styles,
            settings: d.file_settings.clone(),
        })
    }) else {
        return false;
    };
    let bytes = bm_core::dxfile::write(&doc);
    if let Err(e) = std::fs::write(path, &bytes) {
        message(&format!("{}: {e}", path.display()));
        return false;
    }
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            let d = &mut a.docs[i];
            d.path = Some(path.to_path_buf());
            d.name = path.file_name().map_or_else(|| d.name.clone(), |n| n.to_string_lossy().into_owned());
            d.untitled = false;
            d.saved = true;
            d.dirty = false;
            d.file_settings = bytes[10..234].to_vec();
            // As written: read back, the file gives its own order and index values.
            if let Ok(back) = bm_core::dxfile::read(&bytes) {
                d.file_styles = back.styles.iter().map(|s| s.0.clone()).zip(back.style_index.iter().copied()).collect();
                d.styles = back
                    .styles
                    .iter()
                    .map(|(name, begin, end)| Style { name: name.clone(), nested: !name.ends_with('.'), begin: begin.clone(), end: end.clone() })
                    .collect();
            }
            child_title_update(d);
        }
        a.set_title();
    });
    true
}

fn child_title_update(d: &app::Doc) {
    app::child_set_title(&d.frame, &d.full_name());
}

/// The encodings Save As offers for formatted braille, in the reference program's order (spec 04d-B (a), (f)):
/// "12.4 Compatible (CAPS)", the national encodings, "Unicode Braille Patterns"; the first is chosen every time.
fn brf_encodings() -> Vec<String> {
    let mut list = vec!["12.4 Compatible (CAPS)".to_string()];
    list.extend(encodings::all().iter().map(|e| e.name.clone()));
    list.push("Unicode Braille Patterns".to_string());
    list
}

/// One cell (dot pattern) as the chosen encoding writes it into a formatted braille file (spec 04d-B (f)): CAPS is
/// North American ASCII braille in capitals; a national encoding writes its character as one byte when it is 0 to
/// 255 and '#' otherwise (the reference writes a character its code page cannot hold as '#'); Unicode braille writes
/// the cell in UTF-8.
fn brf_cell(out: &mut Vec<u8>, choice: usize, pattern: usize) {
    let all = encodings::all();
    if choice == 0 {
        let c = encodings::find(encodings::DEFAULT).cell_char(pattern).to_ascii_uppercase();
        out.push(if c.is_ascii() { c as u8 } else { b'#' });
    } else if let Some(e) = all.get(choice - 1) {
        let c = e.cell_char(pattern) as u32;
        out.push(if c <= 0xFF { c as u8 } else { b'#' });
    } else {
        let mut buf = [0u8; 4];
        out.extend_from_slice(char::from_u32(0x2800 + pattern as u32).unwrap_or(' ').encode_utf8(&mut buf).as_bytes());
    }
}

/// The bytes of a formatted braille file from a braille document's pages (spec 04d-B): each line without its
/// trailing spaces, every character of it (spaces too) through the cell table into the chosen encoding, CR LF after
/// each line and a form feed after each page. A character with no cell is written '*'. The pages hold the document's
/// characters in the encoding it is shown in (`shown_in`); page numbers the formatter adds are North American ASCII
/// braille.
fn brf_bytes(pages: &[bm_core::format::Page], shown_in: &encodings::Encoding, choice: usize) -> Vec<u8> {
    let numbers = encodings::find(encodings::DEFAULT);
    let mut out = Vec::new();
    for page in pages {
        for line in &page.lines {
            let text = line.text.trim_end_matches(' ');
            for (c, source) in text.chars().zip(&line.sources) {
                // A space and a hard space are the blank cell.
                let cell = match c {
                    ' ' | '\u{a0}' => Some(0),
                    c if source.is_none() => numbers.cell_of(c),
                    c => shown_in.cell_of(c),
                };
                match cell {
                    Some(p) => brf_cell(&mut out, choice, p),
                    None => out.push(b'*'),
                }
            }
            out.extend_from_slice(b"\r\n");
        }
        out.push(0x0C);
    }
    out
}

/// The bytes of a formatted text file from a print document's pages (spec 04d-B): bm_core::format::formatted_text
/// in the code page the reference program chooses first, Western European (ISO-8859-1); a character it cannot hold is
/// written '#', and a hard space is a space.
/// INTERIM: the reference offers a list of code pages for formatted text; until that list is traced, the first is
/// used.
fn prf_bytes(pages: &[bm_core::format::Page]) -> Vec<u8> {
    bm_core::format::formatted_text(pages)
        .chars()
        .map(|c| match c {
            '\u{a0}' => b' ',
            c if (c as u32) <= 0xFF => c as u32 as u8,
            _ => b'#',
        })
        .collect()
}

/// Save As formatted braille (.brf) for a braille document or formatted text (.prf) for a print document: the
/// document laid out into pages and written as the reference program's page writer writes them (spec 04d-B). The
/// document itself keeps its own file and name.
fn write_formatted(key: u64, path: &Path, braille: bool) -> bool {
    let choice = if braille {
        let list = brf_encodings();
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        let f = app::frame();
        let d = SingleChoiceDialog::builder(&f, "Braille encoding of the file:", "Formatted braille", &refs).build();
        d.set_selection(0);
        if d.show_modal() != wxdragon::id::ID_OK {
            return false;
        }
        d.get_selection().max(0) as usize
    } else {
        0
    };
    let Some((pages, shown_in)) = app::with(|a| {
        let i = a.index_of(key)?;
        let shown = a.docs[i].shown();
        Some((a.laid_out(i, &shown), a.encoding()))
    }) else {
        return false;
    };
    let out = if braille { brf_bytes(&pages, shown_in, choice) } else { prf_bytes(&pages) };
    if let Err(e) = std::fs::write(path, &out) {
        message(&format!("{}: {e}", path.display()));
        return false;
    }
    true
}

/// File, Close (and the close box of a document window).
pub fn close_document(key: Option<u64>) {
    let Some(key) = key.or_else(|| app::with(|a| a.doc().map(|d| d.key))) else { return };
    if confirm_discard(key) {
        documents::destroy(key);
    }
}

pub fn exit_program() {
    let keys: Vec<u64> = app::with(|a| a.docs.iter().map(|d| d.key).collect());
    for key in keys {
        documents::bring_to_front(app::with(|a| a.index_of(key)).unwrap_or(0));
        if !confirm_discard(key) {
            return;
        }
    }
    app::with(|a| {
        for d in a.docs.drain(..) {
            d.frame.destroy();
        }
        a.frame.destroy();
    });
}

fn print() {
    let d = open_dialog(1040, None);
    d.set_radio(&[9, 10, 11, 12, 14], 0);
    let ok = d.show() == 1;
    d.destroy();
    if ok {
        message(PLANNED_PRINT);
    }
}

fn emboss() {
    let form = app::with(|a| a.memory.get(1032, "document").and_then(|v| {
        v.iter().find_map(|(c, val)| match (c, val) {
            (13, crate::dlg::Value::Choice(Some(i))) => FORMS.get(*i as usize).map(|s| s.to_string()),
            _ => None,
        })
    }));
    let d = open_dialog(1039, None);
    d.set_radio(&[9, 10, 11, 12, 13], 0);
    d.set_text(23, "(No Target Embosser)");
    d.set_text(25, &form.unwrap_or_else(|| FORMS[0].to_string()));
    let ok = d.show() == 1;
    d.destroy();
    if ok {
        message(PLANNED_EMBOSS);
    }
}

/// File, Translate: a NEW document made from the open one, which is left untouched. A print document becomes a
/// braille document (forward translation), a braille document a print document (back-translation). The whole
/// document goes through the engine in one run, as in the reference program; the result keeps the codes, and its
/// lines and pages are laid out from them.
fn translate_document() {
    let Some((text, kind, tables, initial, template, styles, numbers, name, untitled)) = app::with(|a| {
        a.doc().map(|d| {
            let (codes, template, styles) = (d.initial_codes.clone(), d.template.clone(), d.styles.clone());
            let (name, numbers) = (d.name.clone(), d.page_numbers);
            (d.shown(), d.kind, d.tables.clone(), codes, template, styles, numbers, name, d.untitled)
        })
    }) else {
        return;
    };
    let style_table = translation::style_table(&styles);
    if text.text.trim().is_empty() {
        message("The file that you are trying to translate is empty.");
        return;
    }
    let enc = app::with(|a| a.encoding());
    let to_braille = kind == DocKind::Print;
    // The document's initial codes are in force from the start; the new document carries them too.
    let r = if to_braille {
        translation::forward(&tables[0], &text, &initial, enc, &style_table)
    } else {
        translation::back(&tables[2], &text, &initial, enc, &style_table)
    };
    let out = match r {
        Ok(s) => s,
        Err(e) => {
            message(&format!("Translation failed: {e}"));
            return;
        }
    };
    let new_name = (!untitled).then(|| {
        let stem = ["dxp", "dxb", "txt", "brf", "brl", "bru", "prf"]
            .iter()
            .find_map(|ext| name.strip_suffix(&format!(".{ext}")).or_else(|| name.strip_suffix(&format!(".{}", ext.to_uppercase()))))
            .unwrap_or(&name)
            .to_string();
        format!("{stem}{}", if to_braille { ".brf" } else { ".txt" })
    });
    let key = documents::new_document(if to_braille { DocKind::Braille } else { DocKind::Print }, new_name, &out);
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            a.docs[i].tables = tables.clone();
            a.docs[i].initial_codes = initial.clone();
            a.docs[i].template = template.clone();
            a.docs[i].styles = styles.clone();
            a.docs[i].page_numbers = numbers;
            a.docs[i].dirty = true;
            if to_braille {
                a.docs[i].cells = 40;
            }
        }
    });
}

// ------------------------------------------------------------------ Edit

fn edit_standard(id: i32) {
    // The native text control carries out undo, redo and the clipboard commands itself. Copy and cut keep the
    // selection with its codes, so pasting it brings them back.
    if let Some(t) = active_text() {
        // Edit, Cut and Edit, Copy.
        if matches!(id, 1203 | 1204) {
            let (from, to) = t.get_selection();
            app::with(|a| {
                if let Some(part) = a.doc().map(|d| d.shown().slice(from.max(0) as usize, to.max(0) as usize)) {
                    a.remember(part);
                }
            });
        }
        t.process_menu_command(crate::menu::menu_id(id));
        t.set_focus();
    }
}

fn toggle_highlight() {
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            d.highlight_anchor = match d.highlight_anchor {
                None => Some(d.text.get_insertion_point()),
                Some(_) => None,
            };
        }
        // The Keys field shows highlighting ("H"), as the reference rewrites it on this command.
        a.update_status();
    });
}

fn change_case() {
    let Some(t) = active_text() else { return };
    let (from, to) = t.get_selection();
    if from == to {
        return;
    }
    let text = t.get_string_selection();
    let upper = text != text.to_uppercase();
    // The text between the codes changes case, piece by piece; the codes are left as they are.
    let Some(doc) = app::with(|a| a.doc().map(|d| d.shown())) else { return };
    let (from_c, to_c) = (from.max(0) as usize, to.max(0) as usize);
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    let mut at = from_c;
    for &(s, e) in doc.codes.iter().filter(|c| c.1 > from_c && c.0 < to_c) {
        if s > at {
            pieces.push((at, s));
        }
        at = at.max(e);
    }
    if at < to_c {
        pieces.push((at, to_c));
    }
    for &(s, e) in pieces.iter().rev() {
        let part: String = doc.text.chars().skip(s).take(e - s).collect();
        let changed = if upper { part.to_uppercase() } else { part.to_lowercase() };
        if changed != part && changed.chars().count() == part.chars().count() {
            t.replace(s as i64, e as i64, &changed);
        }
    }
    t.set_selection(from, to);
}

/// Edit, Goto: a page, line and column of the laid-out pages, as the status line counts them.
fn goto_position() {
    let Some((pages, pos)) = app::with(|a| {
        let i = a.active?;
        let shown = a.docs[i].shown();
        Some((a.laid_out(i, &shown), a.docs[i].text.get_insertion_point()))
    }) else {
        return;
    };
    let (page, line, column) = bm_core::format::locate(&pages, pos.max(0) as usize);
    let d = open_dialog(1019, None);
    d.set_text(7, &page.to_string());
    d.set_text(10, &line.to_string());
    d.set_text(12, &column.to_string());
    let ok = d.show() == 1;
    let (page, l, c) = (d.number(7), d.number(10), d.number(12));
    d.destroy();
    if !ok {
        return;
    }
    let (Some(page), Some(l), Some(c)) = (page, l, c) else {
        message("You cannot go to that location in the document.");
        return;
    };
    if page < 1 || l < 1 || c < 1 {
        message("You cannot go to pages with numbers less than 1.");
        return;
    }
    match bm_core::format::position_of(&pages, page as usize, l as usize, c as usize) {
        Some(offset) => select_range(offset as i64, offset as i64),
        None => message("You cannot go to that location in the document."),
    }
}

fn find_next(spec: &FindSpec, text: &str, from: i64, backward: bool) -> Option<i64> {
    let fold = |s: &str| if spec.match_case { s.to_string() } else { s.to_lowercase() };
    let hay: Vec<char> = fold(text).chars().collect();
    let needle: Vec<char> = fold(&spec.find).chars().collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let is_word = |i: i64| i >= 0 && (i as usize) < hay.len() && (hay[i as usize].is_alphanumeric() || hay[i as usize] == '_');
    let fits = |i: usize| hay[i..i + needle.len()] == needle[..] && !(spec.whole_words && (is_word(i as i64 - 1) || is_word((i + needle.len()) as i64)));
    let last = hay.len() - needle.len();
    if backward {
        let start = (from - 1).min(last as i64);
        (0..=start.max(-1)).rev().filter(|i| *i >= 0).map(|i| i as usize).find(|&i| fits(i)).map(|i| i as i64)
    } else {
        ((from.max(0) as usize)..=last).find(|&i| fits(i)).map(|i| i as i64)
    }
}

fn read_find(d: &Dlg, replace: bool) -> FindSpec {
    if replace {
        FindSpec { find: d.text(7), replace_with: d.text(9), match_case: d.checked(10), backward: d.checked(11), whole_words: d.checked(12) }
    } else {
        FindSpec { find: d.text(7), replace_with: String::new(), match_case: d.checked(8), backward: d.checked(9), whole_words: d.checked(10) }
    }
}

fn find(again: bool) {
    let Some(t) = active_text() else { return };
    let mut spec = app::with(|a| a.last_find.clone());
    if !again || spec.is_none() {
        let d = open_dialog(1020, None);
        d.set_radio(&[12, 13, 14], 0);
        match &spec {
            Some(s) => {
                d.set_text(7, &s.find);
                d.set_checked(8, s.match_case);
                d.set_checked(9, s.backward);
                d.set_checked(10, s.whole_words);
            }
            None => {
                let sel = t.get_string_selection();
                if !sel.is_empty() {
                    d.set_text(7, &sel);
                }
            }
        }
        let ok = d.show() == 1;
        let s = read_find(&d, false);
        d.destroy();
        if !ok {
            return;
        }
        if s.find.is_empty() {
            message("The search string is empty.");
            return;
        }
        app::with(|a| a.last_find = Some(s.clone()));
        spec = Some(s);
    }
    let spec = spec.expect("find spec");
    let value = t.get_value();
    let (sel_from, sel_to) = t.get_selection();
    let from = if spec.backward { sel_from } else { sel_to };
    let len = value.chars().count() as i64;
    let mut found = find_next(&spec, &value, from, spec.backward);
    let at_edge = if spec.backward { from == len } else { from == 0 };
    if found.is_none() && !at_edge {
        // Offer to continue from the other end of the document.
        let q = if spec.backward {
            "BrailleMaker has finished searching to the beginning of the document.  Would you like to continue searching at the end?"
        } else {
            "BrailleMaker has finished searching to the end of the document.  Would you like to continue searching at the beginning?"
        };
        if !yes_no(q) {
            return;
        }
        found = find_next(&spec, &value, if spec.backward { len } else { 0 }, spec.backward);
    }
    match found {
        Some(i) => select_range(i, i + spec.find.chars().count() as i64),
        None => message(if again { "The text does not appear again in the document." } else { "Text was not found in the document." }),
    }
}

fn replace() {
    let Some(t) = active_text() else { return };
    let last = app::with(|a| a.last_find.clone());
    let d = open_dialog(1021, None);
    d.set_radio(&[14, 15, 16], 0);
    if let Some(s) = &last {
        d.set_text(7, &s.find);
        d.set_checked(10, s.match_case);
        d.set_checked(12, s.whole_words);
    }
    let ok = d.show() == 1;
    let spec = read_find(&d, true);
    d.destroy();
    if !ok {
        return;
    }
    if spec.find.is_empty() {
        message("The search string is empty.");
        return;
    }
    app::with(|a| a.last_find = Some(spec.clone()));
    let mut count = 0;
    let mut all = false;
    let (sel_from, sel_to) = t.get_selection();
    let mut pos = if spec.backward { sel_from } else { sel_to };
    let find_len = spec.find.chars().count() as i64;
    let repl_len = spec.replace_with.chars().count() as i64;
    while let Some(i) = find_next(&spec, &t.get_value(), pos, spec.backward) {
        select_range(i, i + find_len);
        let answer = if all {
            5
        } else {
            let c = open_dialog(1024, None);
            for b in [5, 6, 7] {
                c.closes_with(b);
            }
            let r = c.show();
            c.destroy();
            r
        };
        match answer {
            5 | 7 => {
                if answer == 7 {
                    all = true;
                }
                t.replace(i, i + find_len, &spec.replace_with);
                count += 1;
                pos = if spec.backward { i } else { i + repl_len };
            }
            6 => pos = if spec.backward { i } else { i + find_len },
            _ => break,
        }
    }
    message(&format!("{count} replacements were made."));
}

// ------------------------------------------------------------------ View

fn set_view(change: impl FnOnce(&mut app::View)) {
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            change(&mut d.view);
        }
        a.apply_view();
    });
}

fn set_font(font: ViewFont) {
    set_view(|v| v.font = font);
}

fn six_key(on: bool) {
    let braille = app::with(|a| a.doc().map(|d| d.kind == DocKind::Braille));
    match braille {
        Some(false) if on => message("Six-key entry is for braille documents."),
        Some(_) => set_view(|v| v.six_key = on),
        None => {}
    }
}

// ------------------------------------------------------------------ Layout

fn style_names() -> Vec<String> {
    app::with(|a| a.doc().map(|d| d.styles.iter().map(|s| s.name.clone()).collect()).unwrap_or_default())
}

/// Apply Style and Delete Style: a list of the document's styles. Returns the chosen one.
fn choose_style(dialog: i32) -> Option<Style> {
    let names = style_names();
    if names.is_empty() {
        message("This document has no styles yet. Use Document, Add Style to create one.");
        return None;
    }
    let d = open_dialog(dialog, None);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    d.set_items(7, &refs, Some(0));
    d.set_items(9, STYLE_SHOW, Some(0));
    let ok = d.show() == 1;
    let chosen = d.selected_text(7);
    d.destroy();
    if !ok {
        return None;
    }
    let chosen = chosen?;
    app::with(|a| a.doc().and_then(|doc| doc.styles.iter().find(|s| s.name == chosen).cloned()))
}

fn apply_named_style(style: Style) {
    wrap_selection(&Shown::marked(&format!("[es~{}]", style.name)), &Shown::marked(&format!("[ee~{}]", style.name)));
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            d.last_style = Some(style);
        }
    });
}

fn apply_style() {
    if let Some(s) = choose_style(1046) {
        apply_named_style(s);
    }
}

fn apply_last_style() {
    match app::with(|a| a.doc().and_then(|d| d.last_style.clone())) {
        Some(s) => apply_named_style(s),
        None => message("No style has been applied yet in this document."),
    }
}

fn join_lines() {
    let Some(t) = active_text() else { return };
    let value = t.get_value();
    let pos = app::char_to_byte(&value, t.get_insertion_point());
    if let Some(nl) = value[pos..].find('\n') {
        let at = app::byte_to_char(&value, pos + nl);
        t.replace(at, at + 1, " ");
    }
}

fn set_page_number() {
    let d = open_dialog(1042, None);
    d.set_text(7, "1");
    d.set_radio(&[9, 10], 0);
    let ok = d.show() == 1;
    let page = d.text(7);
    let roman = d.radio(&[9, 10]) == Some(1);
    d.destroy();
    if ok {
        insert_shown(&Shown::marked(&format!("[{}][pv{}]", if roman { "pntr" } else { "pnta" }, page.trim())));
    }
}

/// Layout, Header and Footer: the text between the start and end codes, or the code that discontinues it.
fn header_footer(dialog: i32, start: &str, end: &str, none: &str) {
    if active_text().is_none() {
        return;
    }
    let d = open_dialog(dialog, None);
    d.set_radio(&[9, 10, 11, 12], 0);
    let ok = d.show() == 1;
    let text = d.text(7);
    let off = d.radio(&[9, 10, 11, 12]) == Some(3);
    d.destroy();
    if ok {
        // The codes are codes; the header or footer text typed is text.
        let mut s = Shown::marked(if off { none } else { start });
        if !off {
            s.push(&Shown::plain(&text));
            s.push(&Shown::marked(end));
        }
        insert_shown(&s);
    }
}

fn code_list() {
    if active_text().is_none() {
        return;
    }
    let d = open_dialog(1041, None);
    d.set_checked(8, true);
    let items: Vec<String> = codes::CODE_LIST.iter().map(|(c, desc)| format!("{c} ({desc})")).collect();
    let refs: Vec<&str> = items.iter().map(String::as_str).collect();
    d.set_items(18, &refs, Some(0));
    d.set_text(20, &format!("[{}]", codes::CODE_LIST[0].0));
    d.set_text(22, "");
    if let Some(list) = d.win(18).and_then(|w| w.as_widget::<ListBox>()) {
        let chosen = d.win(20).and_then(|w| w.as_widget::<StaticText>());
        list.on_selection_changed(move |e| {
            if let (Some(i), Some(c)) = (e.get_selection(), chosen) {
                if let Some((code, _)) = codes::CODE_LIST.get(i as usize) {
                    c.set_label(&format!("[{code}]"));
                }
            }
        });
    }
    d.closes_with(6);
    let r = d.show();
    let chosen = d.selection(18);
    let params = d.text(22);
    d.destroy();
    if r == 6 {
        message(PLANNED_HELP);
        return;
    }
    if r != 1 {
        return;
    }
    let params = params.trim().to_string();
    if params.chars().any(char::is_whitespace) {
        message("The parameters to a code must not contain spaces.");
        return;
    }
    if let Some((code, _)) = chosen.and_then(|i| codes::CODE_LIST.get(i as usize)) {
        insert_shown(&Shown::marked(&format!("[{code}{params}]")));
    }
}

fn fill_table_properties(d: &Dlg) {
    d.set_items(
        7,
        &["Table: [Automatic]", "Table: Columnar", "Table: Stairstep", "Table: Listed", "Table: Linear", "Table: Two-Page"],
        Some(0),
    );
    d.set_items(9, &["Do Not Swap", "Force Swap", "Swap if More Compact"], Some(0));
    d.set_text(11, "2");
    d.set_text(13, "1");
    d.set_text(15, "2");
    d.set_text(17, "0");
}

// ------------------------------------------------------------------ Document

fn embosser_setup(global: bool) {
    let doc = app::with(|a| a.doc().map(|d| (d.kind, d.cells, d.lines)));
    let title = global.then_some("Global: Embosser Setup");
    let key = if global { "global" } else { "document" };
    let Some(d) = settings_dialog(1032, key, title, |d| {
        d.set_items(7, &["(No Target Embosser)"], Some(0));
        d.set_items(13, FORMS, Some(0));
        d.set_text(15, "40");
        d.set_text(17, "25");
        d.set_text(19, "0");
        d.set_text(21, "0");
        for b in [8, 9, 10] {
            d.on_button(b, || message(PLANNED_EMBOSS));
        }
    }) else {
        return;
    };
    // The document's own cells and lines show in a braille document's setup.
    if let (false, Some((DocKind::Braille, _, _))) = (global, doc) {
        let (cells, lines) = (d.number(15), d.number(17));
        app::with(|a| {
            if let Some(doc) = a.doc_mut() {
                if let Some(c) = cells.filter(|c| (10..=100).contains(c)) {
                    doc.cells = c as u32;
                }
                if let Some(l) = lines.filter(|l| (5..=100).contains(l)) {
                    doc.lines = l as u32;
                }
            }
            a.apply_view();
        });
    }
    d.destroy();
}

fn printer_setup(global: bool) {
    let title = if global { "Global: Printer Setup..." } else { "Document: Printer Setup..." };
    let key = if global { "global" } else { "document" };
    let doc = app::with(|a| a.doc().map(|d| (d.kind, d.cells, d.lines)));
    let Some(d) = settings_dialog(1071, key, Some(title), |d| {
        d.set_items(1137, &["Letter (8.5 x 11 inches)", "Legal (8.5 x 14 inches)", "A4 (210 x 297 mm)"], Some(0));
        d.set_items(1138, &["Automatically Select"], Some(0));
        d.set_radio(&[1056, 1057], 0);
        for c in [1155, 1157, 1156, 1158] {
            d.set_text(c, "1\"");
        }
        let (lines, chars) = match doc {
            Some((DocKind::Print, c, l)) if !global => (l, c),
            _ => (50, 80),
        };
        d.set_text(2003, &lines.to_string());
        d.set_text(2005, &chars.to_string());
        d.on_button(1026, || message(PLANNED_PRINT));
        d.on_button(1038, || message(PLANNED_HELP));
    }) else {
        return;
    };
    if let (false, Some((DocKind::Print, _, _))) = (global, doc) {
        let (lines, chars) = (d.number(2003), d.number(2005));
        app::with(|a| {
            if let Some(doc) = a.doc_mut() {
                if let Some(l) = lines.filter(|l| (5..=160).contains(l)) {
                    doc.lines = l as u32;
                }
                if let Some(c) = chars.filter(|c| (10..=100).contains(c)) {
                    doc.cells = c as u32;
                }
            }
            a.apply_view();
        });
    }
    d.destroy();
}

/// The grade codes Document, Learning Tables takes out of the initial codes before it puts its own in.
const GRADE_CODES: [&str; 10] = ["[g1]", "[g1l]", "[g1u]", "[g1.5]", "[g1.5l]", "[g1.5u]", "[g2]", "[g2l]", "[g2u]", "[vrn~inf]"];

/// The initial codes with every grade code taken out and `code` put at their end.
fn regrade(initial: &str, code: &str) -> String {
    let mut out = initial.to_string();
    for g in GRADE_CODES {
        out = out.replace(g, "");
    }
    out.push_str(code);
    out
}

/// Gives the active document `tables` and initial codes changed by `initial`, then refreshes the menus and the
/// translated line.
fn set_document_tables(tables: Option<[String; 4]>, initial: impl FnOnce(&str) -> String) {
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            if let Some(t) = tables {
                d.tables = t;
            }
            d.initial_codes = initial(&d.initial_codes);
            d.last_line_key.clear();
            d.dirty = true;
        }
        a.refresh_menus();
        a.update_status();
    });
}

/// The language entry of the active document, its step in use, and the entry.
fn active_entry() -> Option<(usize, Option<usize>, bm_core::table_list::TableSetEntry)> {
    let tables = app::with(|a| a.doc().map(|d| d.tables.clone()))?;
    translation::with_list(|l| l.entry_for(&tables).map(|(e, s)| (e, s, l.entries[e].clone())))
}

/// Document, Learning Tables, Uncontracted or Fully Contracted: the document's language gives its four tables back
/// (a learning step in use ends), and the grade code in the initial codes changes to `code` ([g1l] uncontracted,
/// [g2u] fully contracted). The tables themselves do not change; tables without grade codes ignore them.
fn set_grade(code: &str) {
    let tables = active_entry().map(|(_, _, e)| e.tables);
    set_document_tables(tables, |i| regrade(i, code));
}

/// Document, Translation Tables, a region: the languages listed are that region's (and those the open documents use).
fn set_region(region: u32) {
    translation::set_region(region);
    settings_store::save_region(region);
    app::with(|a| a.refresh_menus());
}

/// Document, Translation Tables, a language: the document takes that entry's four tables, and every grade code
/// leaves its initial codes.
fn choose_language(entry: usize) {
    let Some(tables) = translation::with_list(|l| l.entries.get(entry).map(|e| e.tables.clone())) else { return };
    set_document_tables(Some(tables), |i| regrade(i, ""));
}

/// Document, Learning Tables, a learning course: a list of its steps; the step chosen takes the place of the
/// print-to-braille rules table, the language's other three tables staying.
fn choose_learning_step(course: usize) {
    let Some((entry, step, e)) = active_entry() else { return };
    let courses = translation::with_list(|l| l.courses(entry));
    let Some(c) = courses.get(course) else { return };
    let names: Vec<String> = c.steps.iter().map(|&s| e.steps[s].name.clone()).collect();
    let choices: Vec<&str> = names.iter().map(String::as_str).collect();
    let frame = app::frame();
    let dialog = SingleChoiceDialog::builder(&frame, "Learning step:", &format!("{} - Learning Tables", c.name), &choices).build();
    dialog.set_selection(step.and_then(|s| c.steps.iter().position(|&x| x == s)).unwrap_or(0) as i32);
    if dialog.show_modal() != wxdragon::id::ID_OK {
        return;
    }
    let Some(&s) = c.steps.get(dialog.get_selection().max(0) as usize) else { return };
    let mut tables = e.tables.clone();
    tables[0] = e.steps[s].table.clone();
    set_document_tables(Some(tables), |i| i.to_string());
}

fn set_hyphen(id: i32) {
    app::with(|a| {
        if let Some(d) = a.doc_mut() {
            d.hyphen = match id {
                2901 => Hyphen::None,
                2902 => Hyphen::Dashes,
                2903 => Hyphen::DashesHyphens,
                2904 => Hyphen::French,
                _ => Hyphen::French113,
            };
        }
    });
}

fn add_style() {
    if active_text().is_none() {
        return;
    }
    let names = style_names();
    let d = open_dialog(1025, None);
    d.set_text(7, "New Style Name");
    let mut basis = vec!["(No Style)".to_string()];
    basis.extend(names.iter().cloned());
    let refs: Vec<&str> = basis.iter().map(String::as_str).collect();
    d.set_items(10, &refs, Some(0));
    d.set_items(12, STYLE_SHOW, Some(0));
    let ok = d.show() == 1;
    let (name, nested, base, begin, end) = (d.text(7).trim().to_string(), d.checked(8), d.selected_text(10), d.text(14), d.text(16));
    d.destroy();
    if !ok {
        return;
    }
    if name.is_empty() {
        message("The style name is empty.");
        return;
    }
    if name == "New Style Name" {
        message("You must give the style a name other than \"New Style Name\".");
        return;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        message("The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore.");
        return;
    }
    if names.contains(&name) {
        message(&format!("A style named {name} already exists."));
        return;
    }
    app::with(|a| {
        if let Some(doc) = a.doc_mut() {
            let based = base.and_then(|b| doc.styles.iter().find(|s| s.name == b).cloned());
            doc.styles.push(Style {
                name,
                nested,
                begin: if begin.is_empty() {
                    based.as_ref().map(|s| s.begin.clone()).unwrap_or_default()
                } else {
                    crate::translation::codes_from_dialog(&begin)
                },
                end: if end.is_empty() {
                    based.map(|s| s.end).unwrap_or_default()
                } else {
                    crate::translation::codes_from_dialog(&end)
                },
            });
            doc.last_line_key.clear();
        }
    });
}

fn modify_style() {
    let names = style_names();
    if names.is_empty() {
        message("This document has no styles yet. Use Document, Add Style to create one.");
        return;
    }
    let d = open_dialog(1026, None);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    d.set_items(8, &refs, Some(0));
    d.set_items(10, STYLE_SHOW, Some(0));
    let load = move |d: &Dlg| {
        if let Some(name) = d.selected_text(8) {
            if let Some(s) = app::with(|a| a.doc().and_then(|doc| doc.styles.iter().find(|s| s.name == name).cloned())) {
                d.set_text(12, &crate::translation::codes_for_dialog(&s.begin));
                d.set_text(14, &crate::translation::codes_for_dialog(&s.end));
                // The code fields come disabled in the dialog resource; with a style chosen they can be edited.
                for cid in 11..=14 {
                    d.enable(cid, true);
                }
            }
        }
    };
    load(&d);
    let dialog_id = d.id;
    let dialog = d.dialog;
    if let Some(list) = d.win(8).and_then(|w| w.as_widget::<ListBox>()) {
        list.on_selection_changed(move |_| load(&Dlg { id: dialog_id, dialog }));
    }
    d.on_button(6, move || {
        let outer = Dlg { id: dialog_id, dialog };
        let Some(old) = outer.selected_text(8) else { return };
        let r = Dlg::open(&outer.dialog, 1027, None);
        r.set_text(7, &old);
        r.set_checked(8, true);
        let ok = r.show() == 1;
        let (new, everywhere) = (r.text(7).trim().to_string(), r.checked(8));
        r.destroy();
        if !ok || new.is_empty() {
            return;
        }
        app::with(|a| {
            if let Some(doc) = a.doc_mut() {
                if let Some(s) = doc.styles.iter_mut().find(|s| s.name == old) {
                    s.name = new.clone();
                }
                doc.last_line_key.clear();
                if everywhere {
                    // The style's codes are renamed, in the running text where codes are marked; text that only
                    // reads like the code is left as it is.
                    let (o, c) = (bm_core::doctext::OPEN, bm_core::doctext::CLOSE);
                    let running = doc.shown().to_running()
                        .replace(&format!("{o}es~{old}{c}"), &format!("{o}es~{new}{c}"))
                        .replace(&format!("{o}ee~{old}{c}"), &format!("{o}ee~{new}{c}"));
                    doc.set_shown(&Shown::from_running(&running));
                }
            }
        });
        let names = style_names();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let pos = names.iter().position(|n| *n == new).map(|p| p as u32);
        outer.set_items(8, &refs, pos);
    });
    let ok = d.show() == 1;
    let (name, begin, end) = (d.selected_text(8), d.text(12), d.text(14));
    d.destroy();
    if !ok {
        return;
    }
    if let Some(name) = name {
        app::with(|a| {
            if let Some(doc) = a.doc_mut() {
                if let Some(s) = doc.styles.iter_mut().find(|s| s.name == name) {
                    // A field is taken back only when it was changed, so text in a style that reads like a bracket
                    // (a braille cell in a contents title) is not made into a code by showing it.
                    use crate::translation::{codes_for_dialog, codes_from_dialog};
                    if begin != codes_for_dialog(&s.begin) {
                        s.begin = codes_from_dialog(&begin);
                    }
                    if end != codes_for_dialog(&s.end) {
                        s.end = codes_from_dialog(&end);
                    }
                }
                doc.last_line_key.clear();
            }
        });
    }
}

fn delete_style() {
    if let Some(s) = choose_style(1047) {
        app::with(|a| {
            if let Some(d) = a.doc_mut() {
                d.styles.retain(|x| x.name != s.name);
                d.last_line_key.clear();
            }
        });
    }
}

fn create_template() {
    let d = open_dialog(1028, None);
    d.set_items(10, &["English (UEB) - Standard", "English (UEB) - Textbook Format"], None);
    d.set_radio(&[19, 20], 0);
    d.on_button(18, || message(PLANNED_TEMPLATES));
    let ok = d.show() == 1;
    d.destroy();
    if ok {
        message(PLANNED_TEMPLATES);
    }
}

// ------------------------------------------------------------------ Global

const FONT_FACES: &[ViewFont] = &[ViewFont::Print, ViewFont::Braille, ViewFont::SimBraille];

fn default_views() {
    let current = app::with(|a| a.settings.default_views.clone());
    let d = open_dialog(1033, None);
    d.set_radio(&[7, 8, 9], FONT_FACES.iter().position(|f| *f == current.translated_font).unwrap_or(1));
    d.set_checked(10, current.print_full_page);
    d.set_checked(11, current.print_codes);
    d.set_checked(12, current.print_translated);
    d.set_radio(&[14, 15, 16], FONT_FACES.iter().position(|f| *f == current.braille_font).unwrap_or(0));
    d.set_checked(17, current.braille_full_page);
    d.set_checked(18, current.braille_codes);
    d.set_checked(19, current.braille_translated);
    d.set_checked(20, current.six_key);
    if d.show() == 1 {
        let v = app::DefaultViews {
            translated_font: FONT_FACES[d.radio(&[7, 8, 9]).unwrap_or(1)],
            print_full_page: d.checked(10),
            print_codes: d.checked(11),
            print_translated: d.checked(12),
            braille_font: FONT_FACES[d.radio(&[14, 15, 16]).unwrap_or(0)],
            braille_full_page: d.checked(17),
            braille_codes: d.checked(18),
            braille_translated: d.checked(19),
            six_key: d.checked(20),
        };
        settings_store::save_default_views(&v);
        app::with(|a| {
            a.settings.default_views = v;
            a.apply_view();
        });
    }
    d.destroy();
}

fn internationalization() {
    let (current, uia) = app::with(|a| (a.settings.encoding.clone(), a.settings.reader.uia));
    let list = encodings::all();
    let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
    let pos = list.iter().position(|e| e.id == current).map(|p| p as u32);
    let Some(d) = settings_dialog(1051, "", None, |d| {
        d.set_items(7, &["English"], Some(0));
        d.set_items(10, &names, pos);
        d.set_radio(&[11, 12], 0);
        d.set_checked(14, uia);
        d.set_radio(&[15, 16], 0);
    }) else {
        return;
    };
    let chosen = d.selection(10).and_then(|i| list.get(i as usize)).map(|e| e.id.clone());
    let uia = d.checked(14);
    d.destroy();
    app::with(|a| a.settings.reader.uia = uia);
    settings_store::save_internationalization(chosen.as_deref().unwrap_or(&current), uia);
    let Some(new) = chosen else { return };
    if new == current {
        return;
    }
    // Braille documents keep their cells: their text is rewritten in the new encoding's characters; codes keep theirs.
    app::with(|a| {
        let before = encodings::find(&current);
        let after = encodings::find(&new);
        for doc in a.docs.iter_mut().filter(|d| d.kind == DocKind::Braille) {
            let shown = translation::convert_braille(&doc.shown(), before, after);
            doc.set_shown(&shown);
        }
        a.settings.encoding = new;
        for doc in a.docs.iter_mut() {
            doc.last_line_key.clear();
        }
        a.apply_view();
    });
}

fn global_fonts() {
    let current = app::with(|a| a.settings.fonts.clone());
    let faces = ["Courier New", "Consolas", "Arial", "Times New Roman", "Verdana"];
    let status = ["Segoe UI", "Arial", "Courier New", "Verdana"];
    let d = open_dialog(1049, None);
    d.set_items(7, &faces, faces.iter().position(|f| *f == current.print_face).map(|p| p as u32).or(Some(0)));
    d.set_text(9, &current.print_size.to_string());
    d.set_text(11, &current.braille_size.to_string());
    d.set_checked(12, true);
    d.set_text(14, "12");
    d.set_text(16, "18");
    d.set_items(18, &status, status.iter().position(|f| *f == current.status_face).map(|p| p as u32).or(Some(0)));
    d.set_text(20, &current.status_size.to_string());
    if d.show() == 1 {
        let print_face = d.selected_text(7);
        let (ps, bs, ss) = (d.number(9), d.number(11), d.number(20));
        let status_face = d.selected_text(18);
        app::with(|a| {
            let f = &mut a.settings.fonts;
            if let Some(p) = print_face {
                f.print_face = p;
            }
            if let Some(n) = ps.filter(|n| (6..=72).contains(n)) {
                f.print_size = n as i32;
            }
            if let Some(n) = bs.filter(|n| (6..=72).contains(n)) {
                f.braille_size = n as i32;
            }
            if let Some(n) = ss.filter(|n| (6..=36).contains(n)) {
                f.status_size = n as i32;
            }
            if let Some(s) = status_face {
                f.status_face = s;
            }
            settings_store::save_fonts(f);
            crate::set_status_font(a);
            a.apply_view();
        });
    }
    d.destroy();
}

fn view_preferences() {
    let (set, verbose) = app::with(|a| (a.settings.six_key_set.clone(), a.settings.verbose_labels));
    let Some(d) = settings_dialog(1034, "", None, |d| {
        d.set_text(7, "0");
        d.set_checked(9, verbose);
        d.set_items(17, &["Hide All", "Hide Idle", "Show All"], Some(0));
        d.set_text(19, &set);
    }) else {
        return;
    };
    let keys = d.text(19);
    let verbose = d.checked(9);
    d.destroy();
    // Check box 9 (verbose labels) sets how the status line shows the character at the caret.
    app::with(|a| {
        a.settings.verbose_labels = verbose;
        a.update_status();
    });
    if keys.chars().count() != 6 {
        message("You must type exactly six keys in as the \"Six-Key Key Set.\"  The keys you type represent, in order, what you will type for Dots 1, 2, 3, 4, 5, and 6 when entering braille by six-key chording.");
        return;
    }
    let keys = keys.to_lowercase();
    settings_store::save_six_key_set(&keys);
    app::with(|a| a.settings.six_key_set = keys);
}

fn shortcut_preferences() {
    let s = app::with(|a| a.settings.shortcuts.clone());
    let d = open_dialog(1022, None);
    d.set_text(7, &s.grade0);
    d.set_text(9, &s.flush_right_leader);
    d.set_text(11, &s.cbc);
    d.set_text(13, &s.new_line);
    d.set_text(15, &s.new_page);
    d.set_text(17, &s.new_ref_page);
    d.set_radio(&[19, 20, 21], s.page_nav.min(2));
    d.set_radio(&[23, 24, 25], s.control_page_nav.min(2));
    if d.show() == 1 {
        let new = app::Shortcuts {
            grade0: d.text(7),
            flush_right_leader: d.text(9),
            cbc: d.text(11),
            new_line: d.text(13),
            new_page: d.text(15),
            new_ref_page: d.text(17),
            page_nav: d.radio(&[19, 20, 21]).unwrap_or(s.page_nav),
            control_page_nav: d.radio(&[23, 24, 25]).unwrap_or(s.control_page_nav),
        };
        settings_store::save_shortcuts(&new);
        app::with(|a| a.settings.shortcuts = new);
    }
    d.destroy();
}

/// At start-up on Windows: offers the JAWS script set in `source` for installing (jaws_install.rs), with the
/// question of dialog 1058 (Yes 4, No 5, "Do not ask this again" 8). A kept answer is kept as the answer plus one in
/// the settings section AddIns (1 no, 2 yes). After installing, Yes to the restart question starts the program again.
#[cfg(windows)]
pub fn jaws_start_up(source: &std::path::Path) {
    use crate::jaws_install::{self, OfferAnswer};
    let section = jaws_install::SETTINGS_SECTION;
    let restart = jaws_install::at_start_up(
        source,
        |key| settings_store::kept_answer(section, key).map(|a| a == 1),
        |key, install| settings_store::keep_answer(section, key, u32::from(install)),
        || {
            let d = open_dialog(1058, None);
            d.closes_with(4);
            d.closes_with(5);
            let closed = d.show();
            let answer = OfferAnswer { install: closed == 4, do_not_ask_again: d.checked(8) };
            d.destroy();
            answer
        },
        || yes_no(jaws_install::RESTART_TEXT),
    );
    if restart {
        if let Ok(exe) = std::env::current_exe() {
            if std::process::Command::new(exe).spawn().is_ok() {
                std::process::exit(0);
            }
        }
    }
}

fn about() {
    let d = open_dialog(1016, None);
    d.set_text(8, &format!("Version {}", env!("CARGO_PKG_VERSION")));
    d.set_text(10, &format!("Braille translation: the BrailleMaker engine, {}", crate::translation::table_count()));
    d.show();
    d.destroy();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::open_styles;

    #[test]
    fn formatted_braille_in_caps_national_and_unicode_encodings() {
        let doc = Shown::plain(",! cat\u{a0}sat");
        let pages = bm_core::format::format(&doc, &bm_core::format::Layout::braille(12, 2), None);
        let na = encodings::find(encodings::DEFAULT);
        assert_eq!(brf_bytes(&pages, na, 0), b",! CAT SAT\r\n\r\n\x0c".to_vec());
        // North American English, not in capitals.
        let pos = 1 + encodings::all().iter().position(|e| e.id == encodings::DEFAULT).unwrap();
        assert_eq!(brf_bytes(&pages, na, pos), b",! cat sat\r\n\r\n\x0c".to_vec());
        // Unicode braille: every cell, the blank ones too, in the braille block.
        let unicode = 1 + encodings::all().len();
        assert_eq!(String::from_utf8(brf_bytes(&pages, na, unicode)).unwrap(), "⠠⠮⠀⠉⠁⠞⠀⠎⠁⠞\r\n\r\n\u{c}");
        // A character no cell stands for.
        let pages = bm_core::format::format(&Shown::plain("a\u{e9}"), &bm_core::format::Layout::braille(12, 1), None);
        assert_eq!(brf_bytes(&pages, na, 0), b"A*\r\n\x0c".to_vec());
    }

    #[test]
    fn formatted_text_is_western_european() {
        let pages = bm_core::format::format(&Shown::plain("Caf\u{e9} \u{3b1}"), &bm_core::format::Layout::print(20, 1), None);
        assert_eq!(prf_bytes(&pages), b"Caf\xe9 #\r\n\x0c".to_vec());
    }

    #[test]
    fn finds_whole_words_both_ways() {
        let spec = FindSpec { find: "the".into(), replace_with: String::new(), match_case: false, backward: false, whole_words: true };
        let text = "other The the";
        assert_eq!(find_next(&spec, text, 0, false), Some(6));
        assert_eq!(find_next(&spec, text, 13, true), Some(10));
    }

    #[test]
    fn open_styles_follow_the_codes() {
        assert_eq!(open_styles(&Shown::marked("[es~a]x[es~b]y[ee~b]z"), 21), vec!["a".to_string()]);
        // Typed brackets are text, not style codes.
        assert!(open_styles(&Shown::plain("[es~a]x"), 7).is_empty());
    }
}
