//! eBraille and EPUB 3 (spec 23): Save As eBraille (.ebrl) for braille documents and EPUB 3 (.epub) for print
//! documents, File, Save eBraille, and Document, eBraille Metadata with its three sub-dialogs. The dialogs are the
//! original's own XRC dialogs (kept with the decoded resource data outside the repository) loaded by resource id 2606 (the menu command)
//! and 26061 to 26063 (the Attributes, Braille Systems and Languages dialogs); their controls are found by the
//! original's names. The package itself is built by bc_core::ebraille.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use bc_core::ebraille::{self, BrailleSystem, Metadata, Property, StyleKind};
use wxdragon::datetime::DateTime;
use wxdragon::prelude::*;
use wxdragon::window::Window;

use crate::app::{self, DocKind};
use crate::commands::{message, once_notice};
use crate::dlg::Dlg;

fn named(d: &Dlg, name: &str) -> Option<Window> {
    d.dialog.find_window_by_name(name)
}

fn text(d: &Dlg, name: &str) -> String {
    named(d, name).and_then(|w| w.as_widget::<TextCtrl>()).map(|t| t.get_value()).unwrap_or_default()
}

fn set_text(d: &Dlg, name: &str, value: &str) {
    if let Some(t) = named(d, name).and_then(|w| w.as_widget::<TextCtrl>()) {
        t.set_value(value);
    }
}

/// The lines of a "1 per line" field, trimmed, empty ones left out.
fn lines(value: &str) -> Vec<String> {
    value.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn checked(d: &Dlg, name: &str) -> bool {
    named(d, name).and_then(|w| w.as_widget::<CheckBox>()).map(|c| c.get_value()).unwrap_or(false)
}

fn set_checked(d: &Dlg, name: &str, on: bool) {
    if let Some(c) = named(d, name).and_then(|w| w.as_widget::<CheckBox>()) {
        c.set_value(on);
    }
}

fn radio(d: &Dlg, name: &str) -> u8 {
    named(d, name).and_then(|w| w.as_widget::<RadioBox>()).map(|r| r.get_selection().max(0) as u8).unwrap_or(0)
}

fn set_radio(d: &Dlg, name: &str, n: u8) {
    if let Some(r) = named(d, name).and_then(|w| w.as_widget::<RadioBox>()) {
        r.set_selection(i32::from(n));
    }
}

fn date(d: &Dlg, name: &str) -> String {
    named(d, name)
        .and_then(|w| w.as_widget::<DatePickerCtrl>())
        .map(|p| {
            let v = p.get_value();
            format!("{:04}-{:02}-{:02}", v.year(), v.month(), v.day())
        })
        .unwrap_or_default()
}

fn set_date(d: &Dlg, name: &str, value: &str) {
    let Some(p) = named(d, name).and_then(|w| w.as_widget::<DatePickerCtrl>()) else { return };
    let parts: Vec<i32> = value.split('-').filter_map(|p| p.parse().ok()).collect();
    if let [y, m, day] = parts[..] {
        p.set_value(&DateTime::new(y, m as u16, day as i16, 0, 0, 0));
    }
}

fn list_items(d: &Dlg, name: &str) -> Vec<String> {
    let Some(l) = named(d, name).and_then(|w| w.as_widget::<ListBox>()) else { return Vec::new() };
    (0..l.get_count()).filter_map(|i| l.get_string(i)).collect()
}

fn set_list(d: &Dlg, name: &str, items: &[String]) {
    if let Some(l) = named(d, name).and_then(|w| w.as_widget::<ListBox>()) {
        l.clear();
        for i in items {
            l.append(i);
        }
    }
}

fn list_selection(d: &Dlg, name: &str) -> Option<u32> {
    named(d, name).and_then(|w| w.as_widget::<ListBox>()).and_then(|l| l.get_selection())
}

fn combo(d: &Dlg, name: &str) -> String {
    named(d, name).and_then(|w| w.as_widget::<ComboBox>()).map(|c| c.get_value()).unwrap_or_default()
}

fn fill_combo(d: &Dlg, name: &str, items: &[&str]) {
    if let Some(c) = named(d, name).and_then(|w| w.as_widget::<ComboBox>()) {
        for i in items {
            c.append(i);
        }
    }
}

fn on_button(d: &Dlg, name: &str, handler: impl FnMut() + 'static) {
    if let Some(b) = named(d, name).and_then(|w| w.as_widget::<Button>()) {
        let mut handler = handler;
        b.on_click(move |_| handler());
    }
}

/// The package bytes of document `key`: its running text, its styles' kinds, its braille encoding's cells and its
/// metadata through bc_core::ebraille.
fn package(key: u64) -> Option<Vec<u8>> {
    app::with(|a| {
        let i = a.index_of(key)?;
        let d = &a.docs[i];
        let running = d.shown().to_running();
        let styles: Vec<(String, String)> = d.styles.iter().map(|s| (s.name.clone(), s.begin.clone())).collect();
        let kinds = |name: &str| -> StyleKind {
            // A style the document does not define is written as a paragraph style.
            styles.iter().find(|(n, _)| n == name).map_or(StyleKind::Block, |(n, begin)| ebraille::kind_of(n, begin))
        };
        let enc = a.encoding();
        let cell = |c: char| enc.text_to_cells(&c.to_string()).chars().next().map_or(0, |u| (u as u32).saturating_sub(0x2800) as u8);
        Some(ebraille::package(&ebraille::Input { running: &running, braille: d.kind == DocKind::Braille, kinds: &kinds, cell: &cell, meta: &d.ebraille }))
    })
}

/// Save As eBraille or EPUB 3 to `path`: the quality notice first (every save until its box is checked; Cancel
/// stops the save), then the package written. The document keeps its own file and name.
pub fn save_as(key: u64, path: &Path) -> bool {
    if !once_notice(ebraille::QUALITY_WARNING_KEY, ebraille::QUALITY_WARNING) {
        return false;
    }
    write_package(key, path)
}

fn write_package(key: u64, path: &Path) -> bool {
    let Some(bytes) = package(key) else { return false };
    if let Err(e) = std::fs::write(path, bytes) {
        message(&format!("{}: {e}", path.display()));
        return false;
    }
    true
}

/// File, Save eBraille (1107): the quality notice first, as the original's dispatch shows it before its save (spec 23
/// 8.1), then the file asked for (eBraille for a braille document, EPUB 3 for a print one; the original's caller of
/// its save was not traced, so the name is asked for as Save As asks it), then the package written.
pub fn save_ebraille() {
    let Some((key, braille, name)) = app::with(|a| a.doc().map(|d| (d.key, d.kind == DocKind::Braille, d.name.clone()))) else { return };
    if !once_notice(ebraille::QUALITY_WARNING_KEY, ebraille::QUALITY_WARNING) {
        return;
    }
    let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_string();
    let (wildcard, ext) = if braille { ("eBraille (*.ebrl)|*.ebrl", "ebrl") } else { ("EPUB 3 (*.epub)|*.epub", "epub") };
    let f = app::frame();
    let dialog = FileDialog::builder(&f)
        .with_message("Save eBraille")
        .with_wildcard(wildcard)
        .with_default_file(&format!("{stem}.{ext}"))
        .with_style(FileDialogStyle::Save | FileDialogStyle::OverwritePrompt)
        .build();
    if dialog.show_modal() != ID_OK {
        return;
    }
    let Some(path) = dialog.get_path() else { return };
    let chosen = std::path::PathBuf::from(path);
    let chosen = if chosen.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) { chosen } else { chosen.with_extension(ext) };
    write_package(key, &chosen);
}

/// The language names the Languages and Braille Systems dialogs offer: the translation table list's languages
/// (INFERENCE: the original's list contents were not read).
fn language_names() -> Vec<String> {
    crate::translation::with_list(|l| l.entries.iter().map(|e| e.name.clone()).collect())
}

/// The Attributes dialog (26061) for one field: a name chosen from the attribute names, a value, the chosen
/// attributes as "name=value" rows with Add, Update and Remove. Returns the attributes on OK.
fn attributes_dialog(current: &[(String, String)]) -> Option<Vec<(String, String)>> {
    let d = Rc::new(Dlg::open_xrc(&app::frame(), 26061));
    fill_combo(&d, "cbAvailableAttributes", &ebraille::ATTRIBUTE_NAMES);
    let rows: Vec<String> = current.iter().map(|(n, v)| format!("{n}={v}")).collect();
    set_list(&d, "lbDefinedAttributes", &rows);
    let dd = d.clone();
    on_button(&d, "btnAddAttribute", move || {
        let name = combo(&dd, "cbAvailableAttributes");
        if name.trim().is_empty() {
            return;
        }
        if let Some(l) = named(&dd, "lbDefinedAttributes").and_then(|w| w.as_widget::<ListBox>()) {
            l.append(&format!("{}={}", name.trim(), text(&dd, "tcAttributeValue")));
        }
    });
    let dd = d.clone();
    on_button(&d, "btnUpdateAttribute", move || {
        let Some(i) = list_selection(&dd, "lbDefinedAttributes") else { return };
        let name = combo(&dd, "cbAvailableAttributes");
        if let Some(l) = named(&dd, "lbDefinedAttributes").and_then(|w| w.as_widget::<ListBox>()) {
            l.set_string(i, &format!("{}={}", name.trim(), text(&dd, "tcAttributeValue")));
        }
    });
    let dd = d.clone();
    on_button(&d, "btnRemoveAttribute", move || {
        if let Some(i) = list_selection(&dd, "lbDefinedAttributes") {
            if let Some(l) = named(&dd, "lbDefinedAttributes").and_then(|w| w.as_widget::<ListBox>()) {
                l.delete(i);
            }
        }
    });
    let ok = d.show() == 1;
    let out = list_items(&d, "lbDefinedAttributes")
        .iter()
        .map(|row| match row.split_once('=') {
            Some((n, v)) => (n.to_string(), v.to_string()),
            None => (row.clone(), String::new()),
        })
        .collect();
    Rc::try_unwrap(d).ok().map(|d| d.destroy());
    ok.then_some(out)
}

/// The Languages dialog (26063): the chosen languages, in order, with Add, Remove, Up and Down.
fn languages_dialog(current: &[String]) -> Option<Vec<String>> {
    let d = Rc::new(Dlg::open_xrc(&app::frame(), 26063));
    let names = language_names();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_combo(&d, "cbAvailableLanguages", &refs);
    set_list(&d, "lbDefinedLanguages", current);
    let dd = d.clone();
    on_button(&d, "btnAdd", move || {
        let v = combo(&dd, "cbAvailableLanguages");
        if !v.trim().is_empty() {
            if let Some(l) = named(&dd, "lbDefinedLanguages").and_then(|w| w.as_widget::<ListBox>()) {
                l.append(v.trim());
            }
        }
    });
    let dd = d.clone();
    on_button(&d, "btnRemove", move || remove_selected(&dd, "lbDefinedLanguages"));
    let dd = d.clone();
    on_button(&d, "btnUp", move || move_selected(&dd, "lbDefinedLanguages", -1));
    let dd = d.clone();
    on_button(&d, "btnDown", move || move_selected(&dd, "lbDefinedLanguages", 1));
    let dd = d.clone();
    on_button(&d, "btnShowLanguageAttributes", move || {
        attributes_dialog(&[]);
        let _ = &dd;
    });
    let ok = d.show() == 1;
    let out = list_items(&d, "lbDefinedLanguages");
    Rc::try_unwrap(d).ok().map(|d| d.destroy());
    ok.then_some(out)
}

fn remove_selected(d: &Dlg, name: &str) {
    if let (Some(i), Some(l)) = (list_selection(d, name), named(d, name).and_then(|w| w.as_widget::<ListBox>())) {
        l.delete(i);
    }
}

fn move_selected(d: &Dlg, name: &str, by: i32) {
    let Some(l) = named(d, name).and_then(|w| w.as_widget::<ListBox>()) else { return };
    let Some(i) = l.get_selection() else { return };
    let to = i as i32 + by;
    if to < 0 || to >= l.get_count() as i32 {
        return;
    }
    let (Some(a), Some(b)) = (l.get_string(i), l.get_string(to as u32)) else { return };
    l.set_string(i, &b);
    l.set_string(to as u32, &a);
    l.set_selection(to as u32, true);
}

/// The Braille Systems dialog (26062): language, grade and specialization make one chosen system, listed as
/// "language grade specialization".
fn braille_systems_dialog(current: &[BrailleSystem]) -> Option<Vec<BrailleSystem>> {
    let d = Rc::new(Dlg::open_xrc(&app::frame(), 26062));
    let names = language_names();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    fill_combo(&d, "cbAvailableBrailleSystemLanguages", &refs);
    let rows: Vec<String> = current.iter().map(|s| format!("{} {} {}", s.language, s.grade, s.specialization)).collect();
    set_list(&d, "lbDefinedBrailleSystems", &rows);
    let dd = d.clone();
    on_button(&d, "btnAdd", move || {
        let language = combo(&dd, "cbAvailableBrailleSystemLanguages");
        if language.trim().is_empty() {
            return;
        }
        let grade = ebraille::GRADES[usize::from(radio(&dd, "rbBrailleSystemGrades")).min(3)];
        let spec = ebraille::SPECIALIZATIONS[usize::from(radio(&dd, "rbBrailleSystemSpecializations")).min(4)];
        if let Some(l) = named(&dd, "lbDefinedBrailleSystems").and_then(|w| w.as_widget::<ListBox>()) {
            l.append(&format!("{} {grade} {spec}", language.trim()));
        }
    });
    let dd = d.clone();
    on_button(&d, "btnRemove", move || remove_selected(&dd, "lbDefinedBrailleSystems"));
    let dd = d.clone();
    on_button(&d, "btnUp", move || move_selected(&dd, "lbDefinedBrailleSystems", -1));
    let dd = d.clone();
    on_button(&d, "btnDown", move || move_selected(&dd, "lbDefinedBrailleSystems", 1));
    on_button(&d, "btnShowBrailleSystemAttributes", || {
        attributes_dialog(&[]);
    });
    let ok = d.show() == 1;
    let out = list_items(&d, "lbDefinedBrailleSystems")
        .iter()
        .map(|row| {
            let mut parts = row.rsplitn(3, ' ');
            let specialization = parts.next().unwrap_or("None").to_string();
            let grade = parts.next().unwrap_or("grade2").to_string();
            let language = parts.next().unwrap_or("").to_string();
            BrailleSystem { language, grade, specialization }
        })
        .collect();
    Rc::try_unwrap(d).ok().map(|d| d.destroy());
    ok.then_some(out)
}

/// The metadata as the dialog's controls hold it.
fn read_back(d: &Dlg, state: &Metadata) -> Metadata {
    let mut m = state.clone();
    m.titles = lines(&text(d, "tcTitle"));
    m.creators = lines(&text(d, "tcCreator"));
    m.producers = lines(&text(d, "tcProducer"));
    m.date = date(d, "dpDate");
    m.id_type = radio(d, "rbIdentifierTypes");
    m.identifier = text(d, "tcIdentifier");
    m.date_copyrighted = date(d, "dpDateCopyrighted");
    m.tactile_graphics = checked(d, "chkTactileGraphics");
    m.complete_transcription = checked(d, "chkCompleteTranscription");
    m.publishers = lines(&text(d, "tcPublisher"));
    m.education_levels = lines(&text(d, "tcEducationLevels"));
    m.description = text(d, "tcDescription");
    m.rights = lines(&text(d, "tcRights"));
    m.source_type = radio(d, "rbSourceTypes");
    m.source = text(d, "tcSource");
    m.subjects = lines(&text(d, "tcSubject"));
    m.properties = list_items(d, "lbDefinedProperties")
        .iter()
        .map(|row| {
            let (name, value) = row.split_once('=').unwrap_or((row, ""));
            let attributes = state.properties.iter().find(|p| p.name == name && p.value == value).map(|p| p.attributes.clone()).unwrap_or_default();
            Property { name: name.to_string(), value: value.to_string(), attributes }
        })
        .collect();
    m
}

/// The identifier the preview shows: as the package writes it, with the template's placeholder where the save
/// makes a new UUID.
fn identifier_of(m: &Metadata) -> String {
    let value = m.identifier.trim();
    match (m.id_type, value.is_empty()) {
        (_, true) => "urn:uuid:{GUID}".to_string(),
        (0, _) => format!("urn:isbn:{value}"),
        (1, _) => format!("urn:doi:{value}"),
        _ => format!("urn:uuid:{value}"),
    }
}

fn refresh_preview(d: &Dlg, state: &Metadata) {
    let m = read_back(d, state);
    let mut shown = m.clone();
    shown.edited_xml = None;
    set_text(d, "tcXMLPreview", &ebraille::metadata_xml(&shown, &identifier_of(&shown)));
}

/// Document, eBraille Metadata (2606): the dialog filled from the document's metadata; the sub-dialogs from its
/// buttons; on OK the metadata is read back (the preview as edited when Allow editing is on and it was changed)
/// and the document is marked modified when it changed, as the original does.
pub fn metadata_dialog() {
    let Some((key, meta)) = app::with(|a| a.doc().map(|d| (d.key, d.ebraille.clone()))) else { return };
    let d = Rc::new(Dlg::open_xrc(&app::frame(), 2606));
    let state = Rc::new(RefCell::new(meta.clone()));
    set_text(&d, "tcTitle", &meta.titles.join("\n"));
    set_text(&d, "tcCreator", &meta.creators.join("\n"));
    set_text(&d, "tcProducer", &meta.producers.join("\n"));
    let today = ebraille::today();
    set_date(&d, "dpDate", if meta.date.is_empty() { &today } else { &meta.date });
    set_radio(&d, "rbIdentifierTypes", meta.id_type);
    set_text(&d, "tcIdentifier", &meta.identifier);
    set_date(&d, "dpDateCopyrighted", if meta.date_copyrighted.is_empty() { &today } else { &meta.date_copyrighted });
    set_checked(&d, "chkTactileGraphics", meta.tactile_graphics);
    set_checked(&d, "chkCompleteTranscription", meta.complete_transcription);
    set_text(&d, "tcPublisher", &meta.publishers.join("\n"));
    set_text(&d, "tcEducationLevels", &meta.education_levels.join("\n"));
    set_text(&d, "tcDescription", &meta.description);
    set_text(&d, "tcRights", &meta.rights.join("\n"));
    set_radio(&d, "rbSourceTypes", meta.source_type);
    set_text(&d, "tcSource", &meta.source);
    set_text(&d, "tcSubject", &meta.subjects.join("\n"));
    fill_combo(&d, "cbAvailableProperties", &ebraille::PROPERTY_NAMES);
    let rows: Vec<String> = meta.properties.iter().map(|p| format!("{}={}", p.name, p.value)).collect();
    set_list(&d, "lbDefinedProperties", &rows);
    set_checked(&d, "chkAllowEditing", meta.edited_xml.is_some());
    if let Some(t) = named(&d, "tcXMLPreview").and_then(|w| w.as_widget::<TextCtrl>()) {
        t.set_editable(meta.edited_xml.is_some());
    }
    match &meta.edited_xml {
        Some(xml) => set_text(&d, "tcXMLPreview", xml),
        None => refresh_preview(&d, &meta),
    }

    // The Options... buttons of the fields: the attributes of that field.
    for (button, field) in [
        ("btnShowTitleAttributes", "title"),
        ("btnShowCreatorAttributes", "creator"),
        ("btnShowDateAttributes", "date"),
        ("btnShowIdentifierAttributes", "identifier"),
        ("btnShowPublisherAttributes", "publisher"),
        ("btnShowDescriptionAttributes", "description"),
        ("btnShowRightsAttributes", "rights"),
        ("btnShowSourceAttributes", "source"),
        ("btnShowSubjectAttributes", "subject"),
    ] {
        let st = state.clone();
        on_button(&d, button, move || {
            let current = st.borrow().field_attributes.iter().find(|(f, _)| f == field).map(|(_, a)| a.clone()).unwrap_or_default();
            if let Some(attrs) = attributes_dialog(&current) {
                let mut s = st.borrow_mut();
                s.field_attributes.retain(|(f, _)| f != field);
                s.field_attributes.push((field.to_string(), attrs));
            }
        });
    }
    let st = state.clone();
    on_button(&d, "btnShowLanguages", move || {
        let current = st.borrow().languages.clone();
        if let Some(l) = languages_dialog(&current) {
            st.borrow_mut().languages = l;
        }
    });
    let st = state.clone();
    on_button(&d, "btnShowBrailleSystems", move || {
        let current = st.borrow().braille_systems.clone();
        if let Some(s) = braille_systems_dialog(&current) {
            st.borrow_mut().braille_systems = s;
        }
    });
    // The Additional page: properties as "name=value" rows.
    let dd = d.clone();
    on_button(&d, "btnAddProperty", move || {
        let name = combo(&dd, "cbAvailableProperties");
        if name.trim().is_empty() {
            return;
        }
        if let Some(l) = named(&dd, "lbDefinedProperties").and_then(|w| w.as_widget::<ListBox>()) {
            l.append(&format!("{}={}", name.trim(), text(&dd, "tcPropertyValue")));
        }
    });
    let dd = d.clone();
    on_button(&d, "btnUpdateProperty", move || {
        let Some(i) = list_selection(&dd, "lbDefinedProperties") else { return };
        let name = combo(&dd, "cbAvailableProperties");
        if let Some(l) = named(&dd, "lbDefinedProperties").and_then(|w| w.as_widget::<ListBox>()) {
            l.set_string(i, &format!("{}={}", name.trim(), text(&dd, "tcPropertyValue")));
        }
    });
    let dd = d.clone();
    on_button(&d, "btnRemoveProperty", move || remove_selected(&dd, "lbDefinedProperties"));
    let (dd, st) = (d.clone(), state.clone());
    on_button(&d, "btnShowPropertyAttributes", move || {
        let Some(i) = list_selection(&dd, "lbDefinedProperties") else { return };
        let Some(row) = named(&dd, "lbDefinedProperties").and_then(|w| w.as_widget::<ListBox>()).and_then(|l| l.get_string(i)) else { return };
        let (name, value) = row.split_once('=').unwrap_or((row.as_str(), ""));
        let current = st.borrow().properties.iter().find(|p| p.name == name && p.value == value).map(|p| p.attributes.clone()).unwrap_or_default();
        if let Some(attrs) = attributes_dialog(&current) {
            let mut s = st.borrow_mut();
            match s.properties.iter_mut().find(|p| p.name == name && p.value == value) {
                Some(p) => p.attributes = attrs,
                None => s.properties.push(Property { name: name.to_string(), value: value.to_string(), attributes: attrs }),
            }
        }
    });
    // Allow editing: the preview becomes editable; off again, it is made afresh from the fields.
    if let Some(c) = named(&d, "chkAllowEditing").and_then(|w| w.as_widget::<CheckBox>()) {
        let (dd, st) = (d.clone(), state.clone());
        c.on_toggled(move |_| {
            let on = checked(&dd, "chkAllowEditing");
            if let Some(t) = named(&dd, "tcXMLPreview").and_then(|w| w.as_widget::<TextCtrl>()) {
                t.set_editable(on);
            }
            if !on {
                refresh_preview(&dd, &st.borrow());
            }
        });
    }
    let answer = d.show();
    if answer == 1 {
        let mut new = read_back(&d, &state.borrow());
        new.edited_xml = if checked(&d, "chkAllowEditing") {
            let shown = text(&d, "tcXMLPreview");
            let mut plain = new.clone();
            plain.edited_xml = None;
            (shown != ebraille::metadata_xml(&plain, &identifier_of(&plain))).then_some(shown)
        } else {
            None
        };
        app::with(|a| {
            if let Some(i) = a.index_of(key) {
                if a.docs[i].ebraille != new {
                    a.docs[i].ebraille = new;
                    a.docs[i].dirty = true;
                }
            }
        });
    }
    Rc::try_unwrap(d).ok().map(|d| d.destroy());
}
