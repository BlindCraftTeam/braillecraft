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
use crate::translation;
use bm_core::table_list::TableList;

// Messages for parts of the program that later build steps bring. They say what is missing and where it comes.
const PLANNED_SAVE: &str = "Saving needs the BrailleMaker document format, which is built in step 7 of the plan. Until then, copy the text out with Select All and Copy.";
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
    if let Some(t) = active_text() {
        let (from, to) = t.get_selection();
        t.replace(from, to, text);
        t.set_insertion_point(from + text.chars().count() as i64);
        t.set_focus();
    }
}

fn wrap_selection(before: &str, after: &str) {
    if let Some(t) = active_text() {
        let (from, to) = t.get_selection();
        let value = t.get_value();
        let inner = value[app::char_to_byte(&value, from)..app::char_to_byte(&value, to)].to_string();
        t.replace(from, to, &format!("{before}{inner}{after}"));
        let caret = from + (before.chars().count() + inner.chars().count()) as i64;
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
    match code {
        Insert::Text(s) => insert_text(s),
        Insert::Pair(a, b) => wrap_selection(a, b),
        Insert::Setting(which) => {
            let s = app::with(|a| {
                let sc = &a.settings.shortcuts;
                match which {
                    Dynamic::Grade0 => sc.grade0.clone(),
                    Dynamic::Cbc => sc.cbc.clone(),
                    Dynamic::FlushRightLeader => sc.flush_right_leader.clone(),
                    Dynamic::NewPage => sc.new_page.clone(),
                    Dynamic::NewReferencePage => sc.new_ref_page.clone(),
                    Dynamic::TerminationSign => {
                        if a.doc().is_some_and(|d| d.kind == DocKind::Braille) { ",'".into() } else { "&@".into() }
                    }
                    Dynamic::HardSpace => "\u{a0}".into(),
                }
            });
            insert_text(&s);
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
        1105 | 1106 | 1107 => message(PLANNED_SAVE),
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
                wrap_selection("[htbs]", "[htbe]");
            }
        }
        1503 => {
            settings_dialog(1062, "", None, fill_table_properties);
        }

        2601 => embosser_setup(false),
        2602 => printer_setup(false),
        2604 => {
            settings_dialog(1031, "", None, |d| {
                for g in [[7, 8, 9, 10, 11], [13, 14, 15, 16, 17]] {
                    d.set_radio(&g, 2);
                }
                for g in [[19, 20, 21, 22, 23], [25, 26, 27, 28, 29]] {
                    d.set_radio(&g, 0);
                }
                d.set_text(31, "1");
            });
        }
        2701..=2705 => set_region(TableList::region_bit((id - 2701) as usize)),
        2706 => set_region(bm_core::table_list::ALL_REGIONS),
        9000..=9399 => choose_language((id - crate::menu::LANGUAGE_BASE) as usize),
        9500..=9509 => choose_learning_step((id - crate::menu::COURSE_BASE) as usize),
        2801 => set_grade("[g1l]"),
        2802 => set_grade("[g2u]"),
        2901..=2905 => set_hyphen(id),
        2606 => message(PLANNED_SAVE),
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

fn file_new() {
    let d = open_dialog(1017, None);
    d.set_radio(&[7, 8], 0);
    d.set_items(10, &["English (UEB) - Standard", "English (UEB) - Textbook Format"], Some(0));
    for b in [11, 12, 13, 14] {
        d.on_button(b, || message(PLANNED_TEMPLATES));
    }
    let ok = d.show() == 1;
    let braille = d.radio(&[7, 8]) == Some(1);
    d.destroy();
    if ok {
        documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, None, "");
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
    // Braille is stored in North American ASCII braille; it is shown in the encoding chosen in Internationalization.
    let text = if braille {
        let to = app::with(|a| a.encoding());
        translation::convert_braille(&doc.text, encodings::find(encodings::DEFAULT), to)
    } else {
        doc.text
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, name, &text);
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            let d = &mut a.docs[i];
            d.dirty = false;
            d.path = Some(path.to_path_buf());
            d.view.translated = doc.translated_line;
            d.view.six_key = braille && doc.six_key;
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
            let key = documents::new_document(if braille { DocKind::Braille } else { DocKind::Print }, name, &text);
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
        Some(true) => {
            message(PLANNED_SAVE);
            false
        }
        Some(false) => true,
    }
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

/// Words longer than the line are broken; otherwise a line breaks at the last blank cell that fits.
fn wrap_braille(line: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest: Vec<char> = line.chars().collect();
    while rest.len() > width {
        let cut = rest[..=width].iter().rposition(|c| *c == ' ').filter(|&i| i > 0).unwrap_or(width);
        out.push(rest[..cut].iter().collect::<String>().trim_end().to_string());
        rest = rest[cut..].iter().collect::<String>().trim_start().chars().collect();
    }
    out.push(rest.into_iter().collect());
    out
}

/// File, Translate: a NEW document made from the open one, which is left untouched. A print document becomes a
/// braille document (forward translation, lines wrapped to the cells per line); a braille document becomes a print
/// document (back-translation). Page layout by codes and styles comes with the formatter.
fn translate_document() {
    let Some((text, kind, tables, initial, name, untitled, cells)) = app::with(|a| {
        a.doc().map(|d| {
            (d.text.get_value(), d.kind, d.tables.clone(), d.initial_codes.clone(), d.name.clone(), d.untitled, d.cells)
        })
    }) else {
        return;
    };
    if text.trim().is_empty() {
        message("The file that you are trying to translate is empty.");
        return;
    }
    let enc = app::with(|a| a.encoding());
    let to_braille = kind == DocKind::Print;
    let width = if to_braille { 40 } else { cells as usize };
    let mut out: Vec<String> = Vec::new();
    for line in text.split('\n') {
        // The document's initial codes are in force on every line; the new document carries them too, so their
        // echo at the start of each translated line is left off.
        let with_codes = format!("{initial}{line}");
        let r = if to_braille {
            translation::forward(&tables[0], &with_codes, enc).map(|s| s.trim_end().to_string())
        } else {
            translation::back(&tables[2], &with_codes, enc)
        }
        .map(|s| s.strip_prefix(initial.as_str()).map(str::to_string).unwrap_or(s));
        match r {
            Ok(s) if to_braille => out.extend(wrap_braille(&s, width)),
            Ok(s) => out.push(s),
            Err(e) => {
                message(&format!("Translation failed: {e}"));
                return;
            }
        }
    }
    let new_name = (!untitled).then(|| {
        let stem = ["dxp", "dxb", "txt", "brf", "brl", "bru", "prf"]
            .iter()
            .find_map(|ext| name.strip_suffix(&format!(".{ext}")).or_else(|| name.strip_suffix(&format!(".{}", ext.to_uppercase()))))
            .unwrap_or(&name)
            .to_string();
        format!("{stem}{}", if to_braille { ".brf" } else { ".txt" })
    });
    let key = documents::new_document(if to_braille { DocKind::Braille } else { DocKind::Print }, new_name, &out.join("\n"));
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            a.docs[i].tables = tables.clone();
            a.docs[i].initial_codes = initial.clone();
            a.docs[i].dirty = true;
            if to_braille {
                a.docs[i].cells = 40;
            }
        }
    });
}

// ------------------------------------------------------------------ Edit

fn edit_standard(id: i32) {
    // The native text control carries out undo, redo and the clipboard commands itself.
    if let Some(t) = active_text() {
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
    });
}

fn change_case() {
    let Some(t) = active_text() else { return };
    let (from, to) = t.get_selection();
    if from == to {
        return;
    }
    let text = t.get_string_selection();
    let changed = if text == text.to_uppercase() { text.to_lowercase() } else { text.to_uppercase() };
    t.replace(from, to, &changed);
    t.set_selection(from, to);
}

fn goto_position() {
    let Some((value, pos, lines)) = app::with(|a| a.doc().map(|d| (d.text.get_value(), d.text.get_insertion_point(), d.lines.max(1) as usize))) else {
        return;
    };
    let (line, column) = app::line_and_column(&value, pos);
    let d = open_dialog(1019, None);
    d.set_text(7, &(line / lines + 1).to_string());
    d.set_text(10, &(line % lines + 1).to_string());
    d.set_text(12, &(column + 1).to_string());
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
    let all: Vec<&str> = value.split('\n').collect();
    let target = (page as usize - 1) * lines + (l as usize - 1);
    if target >= all.len() {
        message("You cannot go to that location in the document.");
        return;
    }
    let offset: usize = all[..target].iter().map(|s| s.chars().count() + 1).sum::<usize>()
        + (c as usize - 1).min(all[target].chars().count());
    select_range(offset as i64, offset as i64);
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
    wrap_selection(&format!("[es~{}]", style.name), &format!("[ee~{}]", style.name));
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
        insert_text(&format!("[{}][pv{}]", if roman { "pntr" } else { "pnta" }, page.trim()));
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
        insert_text(&if off { none.to_string() } else { format!("{start}{text}{end}") });
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
        insert_text(&format!("[{code}{params}]"));
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
                begin: if begin.is_empty() { based.as_ref().map(|s| s.begin.clone()).unwrap_or_default() } else { begin },
                end: if end.is_empty() { based.map(|s| s.end).unwrap_or_default() } else { end },
            });
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
                d.set_text(12, &s.begin);
                d.set_text(14, &s.end);
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
                if everywhere {
                    let v = doc.text.get_value()
                        .replace(&format!("[es~{old}]"), &format!("[es~{new}]"))
                        .replace(&format!("[ee~{old}]"), &format!("[ee~{new}]"));
                    doc.text.set_value(&v);
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
            if let Some(s) = a.doc_mut().and_then(|doc| doc.styles.iter_mut().find(|s| s.name == name)) {
                s.begin = begin;
                s.end = end;
            }
        });
    }
}

fn delete_style() {
    if let Some(s) = choose_style(1047) {
        app::with(|a| {
            if let Some(d) = a.doc_mut() {
                d.styles.retain(|x| x.name != s.name);
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
    let Some(new) = chosen else { return };
    if new == current {
        return;
    }
    // Braille documents keep their cells: their text is rewritten in the new encoding's characters.
    app::with(|a| {
        let before = encodings::find(&current);
        let after = encodings::find(&new);
        for doc in a.docs.iter_mut().filter(|d| d.kind == DocKind::Braille) {
            let text = after.cells_to_text(&before.text_to_cells(&doc.text.get_value()));
            doc.text.set_value(&text);
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
            crate::set_status_font(a);
            a.apply_view();
        });
    }
    d.destroy();
}

fn view_preferences() {
    let set = app::with(|a| a.settings.six_key_set.clone());
    let Some(d) = settings_dialog(1034, "", None, |d| {
        d.set_text(7, "0");
        d.set_items(17, &["Hide All", "Hide Idle", "Show All"], Some(0));
        d.set_text(19, &set);
    }) else {
        return;
    };
    let keys = d.text(19);
    d.destroy();
    if keys.chars().count() != 6 {
        message("You must type exactly six keys in as the \"Six-Key Key Set.\"  The keys you type represent, in order, what you will type for Dots 1, 2, 3, 4, 5, and 6 when entering braille by six-key chording.");
        return;
    }
    app::with(|a| a.settings.six_key_set = keys.to_lowercase());
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
    let radios = app::with(|a| a.memory.get(1022, "").cloned());
    match radios {
        Some(v) => d.restore(&v.into_iter().filter(|(c, _)| (19..=25).contains(c)).collect::<Vec<_>>()),
        None => {
            d.set_radio(&[19, 20, 21], 0);
            d.set_radio(&[23, 24, 25], 1);
        }
    }
    if d.show() == 1 {
        let new = app::Shortcuts {
            grade0: d.text(7),
            flush_right_leader: d.text(9),
            cbc: d.text(11),
            new_line: d.text(13),
            new_page: d.text(15),
            new_ref_page: d.text(17),
        };
        let values = d.values();
        app::with(|a| {
            a.settings.shortcuts = new;
            a.memory.put(1022, "", values);
        });
    }
    d.destroy();
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
    fn wraps_at_the_last_blank_that_fits() {
        assert_eq!(wrap_braille("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(wrap_braille("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
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
        assert_eq!(open_styles("[es~a]x[es~b]y[ee~b]z", 21), vec!["a".to_string()]);
    }
}
