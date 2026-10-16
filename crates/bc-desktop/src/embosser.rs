//! Embosser setups and the embosser setup sheet (re/spec/25-emboss.md sections 2b to 2d, spec 06).
//!
//! An embosser setup names a model from the embosser models (embossers/embossers.txt, with myembossers.txt beside it
//! replacing models by name) and holds the user's choices for it: the setup name, the default form, where the output
//! goes (a printer, a port or a file), the serial port settings, the device settings and the advanced settings. The
//! setups are kept between runs (settings section "Embosser", one value per setup); one of them is the default
//! (Global, Embosser Setup). A document keeps the name of its setup and its own page layout (form, cells, lines,
//! margins, interpoint, banner page, interline print).

use std::cell::RefCell;
use std::path::Path;

use bc_core::embossers::{Catalog, Model};

use wxdragon::prelude::*;

use crate::dlg::Dlg;
use crate::settings_store;

thread_local! {
    static CATALOG: RefCell<Option<Result<Catalog, String>>> = const { RefCell::new(None) };
}

/// Reads the embosser models from `folder` (embossers.txt, and myembossers.txt beside it if present).
pub fn load(folder: &Path) -> Result<usize, String> {
    let r = Catalog::read(&folder.join("embossers.txt"), Some(&folder.join("myembossers.txt")));
    let n = r.as_ref().map(|c| c.models.len()).map_err(Clone::clone);
    CATALOG.with(|c| *c.borrow_mut() = Some(r));
    n
}

/// Runs `f` with the embosser models, or the reason they could not be read (embossing is then disabled).
pub fn with_catalog<R>(f: impl FnOnce(Result<&Catalog, &str>) -> R) -> R {
    CATALOG.with(|c| match &*c.borrow() {
        Some(Ok(cat)) => f(Ok(cat)),
        Some(Err(e)) => f(Err(e)),
        None => f(Err("The embosser models were not found.")),
    })
}

pub use bc_core::emboss_setup::{find_model, form_name, Layout, Output, Setup, BAUDS, CASES, HANDSHAKING, LINE_ENDS, PAGE_ENDS, PARITIES, PORTS, VERSIONS};

/// Whether new setups take the European order of default forms (spec 06): a language other than US English.
fn european() -> bool {
    std::env::var("LANG").is_ok_and(|l| !l.is_empty() && !l.starts_with("en_US") && !l.starts_with("C"))
}

/// A new setup of `label`'s model, with the model's own values.
pub fn new_setup(cat: &Catalog, label: &str) -> Setup {
    Setup::for_model(cat, label, european())
}

// ------------------------------------------------------------------ kept setups

const SECTION: &str = "Embosser";

/// The names of the kept setups.
pub fn setup_names() -> Vec<String> {
    settings_store::get(SECTION, "Setups").and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
}

pub fn load_setup(name: &str) -> Option<Setup> {
    settings_store::get(SECTION, &format!("Setup {name}")).and_then(|v| serde_json::from_str(&v).ok())
}

/// Keeps `setup` under its shown name, replacing a setup kept under `old` (a renamed setup).
pub fn save_setup(setup: &Setup, old: Option<&str>) {
    let name = setup.shown_name();
    let mut names = setup_names();
    if let Some(old) = old.filter(|o| *o != name) {
        names.retain(|n| n != old);
        settings_store::set(SECTION, &format!("Setup {old}"), "");
        if default_setup().as_deref() == Some(old) {
            set_default_setup(&name);
        }
    }
    if !names.contains(&name) {
        names.push(name.clone());
    }
    settings_store::set(SECTION, "Setups", &serde_json::to_string(&names).unwrap_or_default());
    settings_store::set(SECTION, &format!("Setup {name}"), &serde_json::to_string(setup).unwrap_or_default());
}

pub fn remove_setup(name: &str) {
    let mut names = setup_names();
    names.retain(|n| n != name);
    settings_store::set(SECTION, "Setups", &serde_json::to_string(&names).unwrap_or_default());
    settings_store::set(SECTION, &format!("Setup {name}"), "");
    if default_setup().as_deref() == Some(name) {
        set_default_setup("");
    }
}

/// The default setup's name (Global, Embosser Setup), if any.
pub fn default_setup() -> Option<String> {
    settings_store::get(SECTION, "Default").filter(|v| !v.is_empty())
}

pub fn set_default_setup(name: &str) {
    settings_store::set(SECTION, "Default", name);
}

/// The layout new documents take (Global, Embosser Setup), with its cells and lines.
pub fn default_layout() -> (Layout, u32, u32) {
    settings_store::get(SECTION, "Layout")
        .and_then(|v| serde_json::from_str::<(Layout, u32, u32)>(&v).ok())
        .unwrap_or((Layout::default(), 40, 25))
}

pub fn save_default_layout(layout: &Layout, cells: u32, lines: u32) {
    settings_store::set(SECTION, "Layout", &serde_json::to_string(&(layout, cells, lines)).unwrap_or_default());
}

// ------------------------------------------------------------------ the embossing job

/// The byte sent for each cell in a setup's output encoding; None for North American English, which the byte
/// pipeline sends itself. The encoding tables are the program's own (Global, Internationalization); a character
/// above one byte is sent as a space.
fn encoding_codes(name: &str) -> Option<[u8; 64]> {
    if name.eq_ignore_ascii_case("North American English") {
        return None;
    }
    let enc = crate::encodings::all().iter().find(|e| e.name.eq_ignore_ascii_case(name))?;
    let mut codes = [b' '; 64];
    for (pattern, code) in codes.iter_mut().enumerate() {
        let c = enc.cell_char(pattern) as u32;
        if c <= 0xff {
            *code = c as u8;
        }
    }
    Some(codes)
}

/// The byte pipeline's options for a job (bc_core::emboss_setup::job_options), with the setup's output encoding
/// from the program's own encoding tables.
pub fn job_options(s: &Setup, model: &Model, layout: &Layout, cells: u32, lines: u32, copies: u32) -> bc_core::emboss::Options {
    bc_core::emboss_setup::job_options(s, model, layout, cells, lines, copies, encoding_codes(&s.encoding))
}

/// Where a setup's output goes; None for Write to File with no name (the file is asked for each time).
pub fn target(s: &Setup) -> Option<bc_core::emboss_out::Target> {
    use bc_core::emboss_out::{Serial, Target};
    Some(match &s.output {
        Output::File(f) if f.trim().is_empty() => return None,
        Output::File(f) => Target::File(f.trim().into()),
        Output::Printer(p) => Target::Printer(p.clone()),
        Output::Port(p) => Target::Port {
            name: p.clone(),
            serial: s.serial_init.then(|| Serial {
                baud: s.baud,
                parity: s.parity,
                data_bits: s.data_bits,
                stop_bits: s.stop_bits,
                handshaking: s.handshaking,
            }),
        },
    })
}

// ------------------------------------------------------------------ the setup sheet

/// A second handle on a sheet page, for event handlers.
fn handle(d: &Dlg) -> Dlg {
    Dlg { id: d.id, dialog: d.dialog, page: d.page }
}

/// Shows `s` on the sheet's pages. The model list itself is filled only when `models` is set (a change of model
/// fills the other fields from the new model and leaves the list as the user chose it).
fn fill_sheet(general: &Dlg, device: &Dlg, advanced: &Dlg, s: &Setup, cat: &Catalog, models: bool) {
    if models {
        let mut names = vec!["(Please select a model)".to_string()];
        names.extend(cat.names());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        general.set_items(5, &refs, Some(names.iter().position(|n| *n == s.model).unwrap_or(0) as u32));
        general.set_text(7, &s.name);
    }
    let model = find_model(cat, &s.model);
    let forms: Vec<String> = model.map(|m| m.forms.iter().map(|f| form_name(&f.id)).collect()).unwrap_or_default();
    let form_refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let chosen = model.and_then(|m| m.forms.iter().position(|f| f.id == s.form)).map(|i| i as u32);
    general.set_items(11, &form_refs, chosen);
    general.enable(11, !form_refs.is_empty());
    general.set_items(16, &["(default printer)"], Some(0));
    general.set_items(17, PORTS, Some(0));
    match &s.output {
        Output::Printer(p) => {
            general.set_radio(&[13, 14, 15], 0);
            if !p.is_empty() {
                general.set_items(16, &[p.as_str()], Some(0));
            }
        }
        Output::Port(p) => {
            general.set_radio(&[13, 14, 15], 1);
            if let Some(i) = PORTS.iter().position(|x| x == p) {
                general.set_items(17, PORTS, Some(i as u32));
            }
        }
        Output::File(f) => {
            general.set_radio(&[13, 14, 15], 2);
            general.set_text(18, f);
        }
    }
    general.set_checked(21, s.serial_init);
    general.set_items(23, BAUDS, BAUDS.iter().position(|b| *b == s.baud.to_string()).map(|i| i as u32));
    general.set_items(25, PARITIES, Some(s.parity.min(3) as u32));
    general.set_items(27, &["7", "8"], Some(u32::from(s.data_bits == 8)));
    general.set_items(29, &["1", "2"], Some(u32::from(s.stop_bits == 2)));
    general.set_items(31, HANDSHAKING, Some(s.handshaking.min(2) as u32));
    // Device settings.
    let has = |k: &str| model.is_some_and(|m| m.flag(k));
    let index = model.is_some_and(|m| m.driver().contains("Index"));
    device.set_items(5, VERSIONS, Some(s.version.min(VERSIONS.len() - 1) as u32));
    device.set_checked(6, s.escape_codes);
    device.set_checked(9, s.seek_top_of_form);
    device.set_checked(34, s.interpoint_enabled);
    let encodings: Vec<&str> = model.map(|m| m.encodings()).unwrap_or_default();
    let encodings = if encodings.is_empty() { vec!["North American English"] } else { encodings };
    device.set_items(37, &encodings, encodings.iter().position(|e| *e == s.encoding).map(|i| i as u32).or(Some(0)));
    device.set_items(39, CASES, Some(u32::from(s.high_punctuation)));
    device.set_items(41, CASES, Some(u32::from(s.upper_case)));
    device.set_checked(44, s.reverse_order);
    device.set_checked(52, s.z_fold);
    for (cid, v) in &s.device_other {
        device.set_text(*cid, v);
    }
    // INFERENCE: the device fields shown for a model follow its properties (which the reference shows per model
    // was not traced): the version for Index models, escape codes for models that use them, interpoint for
    // models that can do it, Z-fold for Index Z-fold models, seek top of form where allowed.
    for (cid, on) in [
        (4, index),
        (5, index),
        (6, has("ETCAllowEscapeCodes") || has("UsingEscapeCodes")),
        (7, has("ETSpeaks")),
        (8, has("ETSpeaks")),
        (9, has("AllowSeekTOF")),
        (34, has("CanDoInterpoint")),
        (52, has("IndexZfold")),
    ] {
        device.enable(cid, on);
    }
    // Advanced.
    advanced.set_checked(5, s.custom_codes);
    advanced.set_text(7, &s.start_codes);
    advanced.set_text(9, &s.end_codes);
    advanced.set_items(12, LINE_ENDS, Some(s.end_of_line.min(2) as u32));
    advanced.set_items(14, PAGE_ENDS, Some(s.end_of_page.min(5) as u32));
    advanced.set_checked(15, s.eol_on_full_line);
    advanced.set_checked(16, s.eop_on_full_page);
    advanced.set_checked(17, s.pad_lines);
    advanced.set_checked(18, s.prompt_new_sheet);
    advanced.set_checked(20, s.one_at_a_time);
    advanced.set_text(22, &s.text_delay.to_string());
    advanced.set_text(24, &s.control_delay.to_string());
}

/// The setup sheet (dialogs 10000 to 10004): `setup` shown, changed on OK. Returns false when cancelled. Choosing a
/// model fills the pages with that model's own values. The TranSend SE page shows only for models with TranSend
/// (INFERENCE: which pages the reference shows per model was not traced).
pub fn edit(setup: &mut Setup, cat: &Catalog) -> bool {
    let transend = find_model(cat, &setup.model).is_some_and(|m| m.flag("TranSend"));
    let mut pages = vec![(10001, "General"), (10002, "Device Settings")];
    if transend {
        pages.push((10003, "TranSend SE"));
    }
    pages.push((10004, "Advanced"));
    let title = format!("Embosser Setup - {}", if setup.shown_name().is_empty() { "Untitled Configuration".to_string() } else { setup.shown_name() });
    let (sheet, p) = Dlg::sheet(&crate::app::frame(), 10000, &title, &pages);
    let general = &p[0];
    let device = &p[1];
    let advanced = p.last().expect("advanced page");
    // The reference places Configuration location (8, 9) and Default Form (10, 11) at the same spot and shows one.
    // INFERENCE (which one it shows when was not traced): Default Form; setups are kept in the user's own settings,
    // so a location for all users has no meaning here.
    general.hide(8);
    general.hide(9);
    fill_sheet(general, device, advanced, setup, cat, true);
    let (g, dv, adv) = (handle(general), handle(device), handle(advanced));
    if let Some(models) = general.win(5).and_then(|w| w.as_widget::<ComboBox>()) {
        models.on_selection_changed(move |_| {
            let model = g.selected_text(5).unwrap_or_default();
            if model.starts_with('(') {
                return;
            }
            with_catalog(|cat| {
                if let Ok(cat) = cat {
                    fill_sheet(&g, &dv, &adv, &new_setup(cat, &model), cat, false);
                }
            });
        });
    }
    let result = loop {
        if sheet.show() != 1 {
            break false;
        }
        let model = general.selected_text(5).unwrap_or_default();
        if model.starts_with('(') {
            crate::commands::message("Please select an embosser model.");
            continue;
        }
        let mut s = if model == setup.model { setup.clone() } else { new_setup(cat, &model) };
        s.name = general.text(7).trim().to_string();
        let taken = setup_names().into_iter().any(|n| n == s.shown_name() && n != setup.shown_name());
        if taken {
            crate::commands::message(&format!("You already have a configuration named \"{}\".  Please give a different name.", s.shown_name()));
            continue;
        }
        s.all_users = false;
        if let (Some(m), Some(shown)) = (find_model(cat, &model), general.selected_text(11)) {
            if let Some(f) = m.forms.iter().find(|f| form_name(&f.id) == shown) {
                s.form = f.id.clone();
            }
        }
        s.output = match general.radio(&[13, 14, 15]) {
            Some(0) => Output::Printer(general.selected_text(16).filter(|p| !p.starts_with('(')).unwrap_or_default()),
            Some(2) => Output::File(general.text(18).trim().to_string()),
            _ => Output::Port(general.selected_text(17).unwrap_or_else(|| "lpt1".into())),
        };
        s.serial_init = general.checked(21);
        s.baud = general.selected_text(23).and_then(|b| b.parse().ok()).unwrap_or(s.baud);
        s.parity = general.chosen_in(25, PARITIES).unwrap_or(s.parity);
        s.data_bits = general.selected_text(27).and_then(|b| b.parse().ok()).unwrap_or(s.data_bits);
        s.stop_bits = general.selected_text(29).and_then(|b| b.parse().ok()).unwrap_or(s.stop_bits);
        s.handshaking = general.chosen_in(31, HANDSHAKING).unwrap_or(s.handshaking);
        s.version = device.chosen_in(5, VERSIONS).unwrap_or(0);
        s.escape_codes = device.checked(6);
        s.seek_top_of_form = device.checked(9);
        s.interpoint_enabled = device.checked(34);
        s.encoding = device.selected_text(37).unwrap_or(s.encoding);
        s.high_punctuation = device.chosen_in(39, CASES) == Some(1);
        s.upper_case = device.chosen_in(41, CASES) == Some(1);
        s.reverse_order = device.checked(44);
        s.z_fold = device.checked(52);
        s.device_other = [16, 21, 23, 27, 30, 33, 43].iter().map(|c| (*c, device.text(*c))).collect();
        s.custom_codes = advanced.checked(5);
        s.start_codes = advanced.text(7);
        s.end_codes = advanced.text(9);
        s.end_of_line = advanced.chosen_in(12, LINE_ENDS).unwrap_or(0);
        s.end_of_page = advanced.chosen_in(14, PAGE_ENDS).unwrap_or(0);
        s.eol_on_full_line = advanced.checked(15);
        s.eop_on_full_page = advanced.checked(16);
        s.pad_lines = advanced.checked(17);
        s.prompt_new_sheet = advanced.checked(18);
        s.one_at_a_time = advanced.checked(20);
        s.text_delay = advanced.number(22).unwrap_or(0).max(0) as u32;
        s.control_delay = advanced.number(24).unwrap_or(0).max(0) as u32;
        *setup = s;
        break true;
    };
    sheet.destroy();
    result
}
