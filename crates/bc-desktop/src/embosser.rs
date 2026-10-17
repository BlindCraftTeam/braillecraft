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
    static TRANSEND: RefCell<TranSendPrinters> = RefCell::new(TranSendPrinters::default());
}

/// Reads the embosser models from `folder` (embossers.txt, and myembossers.txt beside it if present), and the
/// printers the TranSend SE page offers (transend-printers.txt; without it, or when it is not such a list, the
/// supported and compatible choices offer no printer, as the reference's empty lists do).
pub fn load(folder: &Path) -> Result<usize, String> {
    let r = Catalog::read(&folder.join("embossers.txt"), Some(&folder.join("myembossers.txt")));
    let n = r.as_ref().map(|c| c.models.len()).map_err(Clone::clone);
    CATALOG.with(|c| *c.borrow_mut() = Some(r));
    let list = std::fs::read_to_string(folder.join("transend-printers.txt"))
        .ok()
        .and_then(|t| bc_core::emboss_setup::transend_printers(&t))
        .unwrap_or_default();
    TRANSEND.with(|t| *t.borrow_mut() = list);
    n
}

/// The system's printers that TranSend SE printer choice `filter` lets the ink printer list show, by name in the
/// system's order (spec 25 14.8 item 6).
fn ink_printers(filter: usize) -> Vec<String> {
    let printers = bc_core::emboss_out::printers();
    TRANSEND.with(|t| {
        let list = t.borrow();
        printers.into_iter().filter(|p| list.allows(filter, &p.driver)).map(|p| p.name).collect()
    })
}

thread_local! {
    /// The setups whose unsupported ink printer has been warned about while the program runs (spec 25 17.1: once a
    /// setup).
    static INK_WARNED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Before an embossing job with an ink pass on a TranSend SE embosser (spec 25 17.1): when the setup's ink printer is
/// not a supported printer (its driver only compatible, or not on the list at all), message 10272 with OK and
/// Cancel, once a setup; false when Cancel is chosen.
pub fn ink_printer_accepted(setup: &Setup, ok_cancel: impl FnOnce(&str) -> bool) -> bool {
    if setup.ink_printer.is_empty() {
        return true;
    }
    let driver = bc_core::emboss_out::printers().into_iter().find(|p| p.name == setup.ink_printer).map(|p| p.driver).unwrap_or_default();
    if TRANSEND.with(|t| t.borrow().allows(0, &driver)) {
        return true;
    }
    let name = setup.shown_name();
    if INK_WARNED.with(|w| w.borrow().contains(&name)) {
        return true;
    }
    INK_WARNED.with(|w| w.borrow_mut().push(name));
    ok_cancel("The printer selected for ink output is not supported. Enabling Technologies cannot provide any Technical Support on unsupported printers.")
}

/// Runs `f` with the embosser models, or the reason they could not be read (embossing is then disabled).
pub fn with_catalog<R>(f: impl FnOnce(Result<&Catalog, &str>) -> R) -> R {
    CATALOG.with(|c| match &*c.borrow() {
        Some(Ok(cat)) => f(Ok(cat)),
        Some(Err(e)) => f(Err(e)),
        None => f(Err("The embosser models were not found.")),
    })
}

pub use bc_core::emboss_setup::{find_model, form_name, form_name_with_prefix, Layout, Output, Setup, TranSendPrinters, BAUDS, CASES, HANDSHAKING, LINE_ENDS, PAGE_ENDS, PARITIES, TRANSEND_FILTERS, VERSIONS};

/// Whether new setups take the European order of default forms (spec 25 12.3): the reference tests the user locale's
/// country code; here the region of the system's language (its canonical name, such as en_GB: the user locale on
/// Windows, the locale settings on macOS, LANG on Linux) is tested as bc_core::emboss_setup::a4_region does.
fn european() -> bool {
    let name = wxdragon::translations::Locale::get_language_canonical_name(wxdragon::translations::Locale::get_system_language())
        .unwrap_or_default();
    let region = name.split(['_', '-']).nth(1).unwrap_or("").split(['.', '@']).next().unwrap_or("");
    bc_core::emboss_setup::a4_region(region)
}

/// The FormNamePrefix of the model of kept setup `name` (empty when it has none).
pub fn setup_form_prefix(name: &str) -> String {
    let Some(setup) = load_setup(name) else { return String::new() };
    with_catalog(|cat| {
        cat.ok()
            .and_then(|cat| find_model(cat, &setup.model))
            .and_then(|m| m.text("FormNamePrefix").map(str::to_string))
            .unwrap_or_default()
    })
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

/// Shows `s` on the sheet's pages. `models` is set when the sheet opens: the model list, the setup name and the output
/// (printer list, port list, file name) are filled only then, because a change of model in the reference leaves
/// those as they are (spec 25 15.3); the forms, the serial settings and the other pages are filled each time.
fn fill_sheet(general: &Dlg, device: &Dlg, advanced: &Dlg, s: &Setup, cat: &Catalog, models: bool) {
    let model = find_model(cat, &s.model);
    if models {
        let mut names = vec!["(Please select a model)".to_string()];
        names.extend(cat.names());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        general.set_items(5, &refs, Some(names.iter().position(|n| *n == s.model).unwrap_or(0) as u32));
        general.set_text(7, &s.name);
        // The system's printers, which the list sorts, with no entry of its own (spec 25 14.6): the stored printer
        // is chosen, else the first; with no printer the list stays empty.
        let printers: Vec<String> = bc_core::emboss_out::printers().into_iter().map(|p| p.name).collect();
        let printer_refs: Vec<&str> = printers.iter().map(String::as_str).collect();
        let stored = match &s.output {
            Output::Printer(p) => printer_refs.iter().position(|x| x == p).map(|i| i as u32),
            _ => None,
        };
        general.set_items(16, &printer_refs, stored);
        if stored.is_none() && !printer_refs.is_empty() {
            general.select(16, 0);
        }
        match &s.output {
            Output::Printer(_) => general.set_radio(&[13, 14, 15], 0),
            Output::Port(_) => general.set_radio(&[13, 14, 15], 1),
            Output::File(f) => {
                general.set_radio(&[13, 14, 15], 2);
                general.set_text(18, f);
            }
        }
        fill_ports(general, model, match &s.output {
            Output::Port(p) => Some(p.as_str()),
            _ => None,
        });
    }
    // The serial settings: a change of model gives the new model's (apart from parity and Initialize Serial Port,
    // which `s` keeps; spec 25 18.3).
    general.set_checked(21, s.serial_init);
    general.set_items(23, BAUDS, BAUDS.iter().position(|b| *b == s.baud.to_string()).map(|i| i as u32));
    general.set_items(25, PARITIES, Some(s.parity.min(3) as u32));
    general.set_items(27, &["7", "8"], Some(u32::from(s.data_bits == 8)));
    general.set_items(29, &["1", "2"], Some(u32::from(s.stop_bits == 2)));
    general.set_items(31, HANDSHAKING, Some(s.handshaking.min(2) as u32));
    let forms: Vec<String> = model.map(|m| m.forms.iter().map(|f| form_name(&f.id)).collect()).unwrap_or_default();
    let form_refs: Vec<&str> = forms.iter().map(String::as_str).collect();
    let chosen = model.and_then(|m| m.forms.iter().position(|f| f.id == s.form)).map(|i| i as u32);
    general.set_items(11, &form_refs, chosen);
    general.enable(11, !form_refs.is_empty());
    // Device settings.
    device.set_items(5, VERSIONS, Some(s.version.min(VERSIONS.len() - 1) as u32));
    device.set_checked(6, s.escape_codes);
    device.set_checked(7, s.et_speaks_manage);
    device.set_checked(8, s.et_speaks_on);
    device.set_checked(9, s.seek_top_of_form);
    device.set_checked(34, s.interpoint_enabled);
    let encodings: Vec<&str> = model.map(|m| m.encodings()).unwrap_or_default();
    let encodings = if encodings.is_empty() { vec!["North American English"] } else { encodings };
    device.set_items(37, &encodings, encodings.iter().position(|e| *e == s.encoding).map(|i| i as u32).or(Some(0)));
    device.set_items(39, CASES, Some(u32::from(s.high_punctuation)));
    device.set_items(41, CASES, Some(u32::from(s.upper_case)));
    device.set_checked(44, s.reverse_order);
    device.set_checked(52, s.z_fold);
    // The page's other fields as the setup has them; one it does not hold is empty (after a change of model those
    // the model gives, spec 25 18.3).
    for cid in [16, 21, 23, 27, 30, 33, 43] {
        let value = s.device_other.iter().find(|(c, _)| *c == cid).map_or("", |(_, v)| v.as_str());
        device.set_text(cid, value);
    }
    show_device(device, model, s.version, s.escape_codes);
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

/// The Write to Port list (17) for `model` when the sheet opens (spec 25 16.1 and 16.3): with Write to Port chosen the
/// setup's `port` is chosen, else LPT1; either is found without regard to case, and one the list does not hold
/// gives its first port.
fn fill_ports(general: &Dlg, model: Option<&Model>, port: Option<&str>) {
    let ports = bc_core::emboss_setup::ports_with(model, &crate::ftdi::devices());
    let refs: Vec<&str> = ports.iter().map(String::as_str).collect();
    let want = port.unwrap_or("LPT1");
    let chosen = refs.iter().position(|p| p.eq_ignore_ascii_case(want)).or((!refs.is_empty()).then_some(0));
    general.set_items(17, &refs, chosen.map(|i| i as u32));
}

/// Page 10001 with or without a model, as the reference sets it on a change of model (spec 25 15.3): with none,
/// every control below the model choice is greyed; with one, the setup name (6, 7), the form (10, 12) and Send to
/// Printer (13) are usable again. INFERENCE: Write to Port and Write to File (14, 15) are made usable with them; the
/// reference's code for them on a change of model was not found.
fn show_general(general: &Dlg, has_model: bool) {
    if !has_model {
        for cid in 6..=31 {
            general.enable(cid, false);
        }
        return;
    }
    for cid in [6, 7, 10, 12, 13, 14, 15] {
        general.enable(cid, true);
    }
}

/// The output controls for the output now chosen, as the reference sets them (spec 25 16.3): only the chosen
/// output's own field is usable (the printer list, the port list, or the file name and its note); the Serial group
/// and Initialize Serial Port only for a port whose name begins with "com", and the serial settings below it only
/// while it is checked; Advanced's Send one character at a time (20) only for a port or a file. With no model none
/// of them is usable.
fn show_output(general: &Dlg, advanced: &Dlg, has_model: bool) {
    let kind = general.radio(&[13, 14, 15]).filter(|_| has_model);
    general.enable(16, kind == Some(0));
    general.enable(17, kind == Some(1));
    general.enable(18, kind == Some(2));
    general.enable(19, kind == Some(2));
    let serial = kind == Some(1) && general.selected_text(17).is_some_and(|p| p.starts_with("com"));
    general.enable(20, serial);
    general.enable(21, serial);
    for cid in 22..=31 {
        general.enable(cid, serial && general.checked(21));
    }
    advanced.enable(20, kind.is_some_and(|k| k != 0));
}

/// The TranSend SE page (sheet page 10003) showing `s` (spec 25 section 14): the printer choice (5), the printers it
/// lets the list show (6) with the setup's ink printer chosen, every font (8) with the interline font chosen, the
/// font size (10 to 12) and the two ink adjustments (15, 17). A stored printer or font the system no longer has is
/// left unchosen, as in the reference.
fn fill_transend(page: &Dlg, s: &Setup) {
    page.set_items(5, TRANSEND_FILTERS, Some(s.transend_filter.min(2) as u32));
    fill_ink_printers(page, s.transend_filter, &s.ink_printer);
    let fonts = crate::print::font_names();
    let font_refs: Vec<&str> = fonts.iter().map(String::as_str).collect();
    page.set_items(8, &font_refs, font_refs.iter().position(|f| *f == s.interline_font).map(|i| i as u32));
    page.set_radio(&[10, 11, 12], s.transend_font_size.min(2));
    page.set_text(15, &s.ink_adjust_h.to_string());
    page.set_text(17, &s.ink_adjust_v.to_string());
}

/// List 6 of the TranSend SE page: the system's printers that printer choice `filter` lets it show, in the system's
/// order, with `chosen` chosen when it is among them.
fn fill_ink_printers(page: &Dlg, filter: usize, chosen: &str) {
    let names = ink_printers(filter);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    page.set_items(6, &refs, refs.iter().position(|n| *n == chosen).map(|i| i as u32));
}

/// The TranSend SE page's values read into `s`. The reference stores each value as it changes, so a list with
/// nothing chosen leaves the stored value as it was; an adjustment that is not a whole number is stored as 0, as
/// the reference's reading of a number field gives.
fn read_transend(page: &Dlg, s: &mut Setup) {
    s.transend_filter = page.chosen_in(5, TRANSEND_FILTERS).unwrap_or(s.transend_filter);
    if let Some(p) = page.selected_text(6) {
        s.ink_printer = p;
    }
    if let Some(f) = page.selected_text(8) {
        s.interline_font = f;
    }
    s.transend_font_size = page.radio(&[10, 11, 12]).unwrap_or(s.transend_font_size);
    s.ink_adjust_h = page.number(15).and_then(|n| i32::try_from(n).ok()).unwrap_or(0);
    s.ink_adjust_v = page.number(17).and_then(|n| i32::try_from(n).ok()).unwrap_or(0);
}

/// The Device Settings page as the reference sets it (re/spec/25-emboss.md, Device Settings page (resource
/// 10002)): the page holds one group of controls per embosser driver at the same places, and only the group of the
/// model's driver is shown; the margin rows, the interpoint check box and the encoding rows show by the driver and
/// by whether escape codes are in use. `version` is the place chosen in VERSIONS, `escapes` the Allow escape codes
/// check box.
fn show_device(device: &Dlg, model: Option<&Model>, version: usize, escapes: bool) {
    let driver = model.map_or("", |m| m.driver());
    let (index, enabling, braillo, tiger, elekul) = (
        driver == "Emb_IndexEmbosserDriver",
        driver == "Emb_EnablingEmbosserDriver",
        driver == "Emb_BrailloEmbosserDriver",
        driver == "Emb_TigerEmbosserDriver",
        driver == "Emb_ElekulEmbosserDriver",
    );
    let flag = |k: &str| model.is_some_and(|m| m.flag(k));
    // Escape codes in use: the model uses them, or the setup allows them.
    let in_use = flag("UsingEscapeCodes") || escapes;
    let (margins, interpoint, encoding) = if tiger || elekul {
        (false, false, false)
    } else if index {
        // Below Embosser Version 4: "(unspecified)" to the two Version 3 entries.
        (version < 5, true, true)
    } else if enabling {
        (!escapes, !escapes, true)
    } else if braillo {
        (false, false, true)
    } else {
        (!in_use, !in_use, true)
    };
    let rows = margins && model.is_some();
    let groups: [(i32, i32, bool); 14] = [
        (4, 5, index),
        (6, 6, enabling),
        (7, 11, enabling && in_use),
        (12, 14, braillo),
        // INFERENCE: the reference shows 15-18 and 19-23 for Braillo models by submodel bits whose meaning was not
        // traced (spec 25, unknowns); both rows lie at the same place, so neither is shown.
        (15, 23, false),
        (24, 24, tiger),
        (25, 25, margins),
        (26, 28, rows && !flag("NoLeftMargin")),
        (29, 31, rows && !flag("NoRightMargin")),
        (32, 33, rows && !flag("NoTopMargin")),
        (34, 34, interpoint),
        (35, 41, encoding),
        (42, 51, elekul),
        (52, 52, index),
    ];
    for (first, last, on) in groups {
        for cid in first..=last {
            device.set_shown(cid, on);
        }
    }
    // Enabled: with no model chosen, 6 to 31 are not; Manage ET Speaks and Seek Top of Form need escape codes in use
    // and the model's support; interpoint and Z-fold need the model's support.
    for cid in 6..=31 {
        device.enable(cid, model.is_some());
    }
    device.enable(7, in_use && flag("ETSpeaks"));
    device.enable(8, in_use && flag("ETSpeaks"));
    device.enable(9, in_use && flag("AllowSeekTOF"));
    device.enable(34, flag("CanDoInterpoint"));
    device.enable(52, flag("IndexZfold"));
}

/// The Device Settings groups shown again for the model, version and escape codes now chosen on the sheet.
fn refresh_device(general: &Dlg, device: &Dlg) {
    let model = general.selected_text(5).unwrap_or_default();
    let version = device.chosen_in(5, VERSIONS).unwrap_or(0);
    let escapes = device.checked(6);
    with_catalog(|cat| {
        if let Ok(cat) = cat {
            show_device(device, find_model(cat, &model), version, escapes);
        }
    });
}

/// The TranSend SE page filled from a new setup of the first model with TranSend, for the layout check's picture.
pub fn layout_transend_page(page: &Dlg) {
    with_catalog(|cat| {
        if let Ok(cat) = cat {
            if let Some(name) = cat.names().into_iter().find(|n| find_model(cat, n).is_some_and(|m| m.flag("TranSend"))) {
                fill_transend(page, &new_setup(cat, &name));
            }
        }
    });
}

/// The setup sheet for `setup`, filled. As in the reference (spec 25 15.1) all four pages are made: General, Device
/// Settings, TranSend SE and Advanced, in that order; the caller takes the TranSend SE tab away for a model without
/// TranSend.
fn build_sheet(setup: &Setup, cat: &Catalog) -> (Dlg, Vec<Dlg>) {
    let pages = [(10001, "General"), (10002, "Device Settings"), (10003, "TranSend SE"), (10004, "Advanced")];
    let title = format!("Embosser Setup - {}", if setup.shown_name().is_empty() { "Untitled Configuration".to_string() } else { setup.shown_name() });
    let (sheet, p) = Dlg::sheet(&crate::app::frame(), 10000, &title, &pages);
    // The reference places Configuration location (8, 9) and Default Form (10, 11) at the same spot and shows one.
    // INFERENCE (which one it shows when was not traced): Default Form; setups are kept in the user's own settings,
    // so a location for all users has no meaning here.
    p[0].hide(8);
    p[0].hide(9);
    fill_sheet(&p[0], &p[1], &p[3], setup, cat, true);
    fill_transend(&p[2], setup);
    (sheet, p)
}

/// One model name for each embosser driver, for the layout check (dlg::layout_check).
pub fn layout_models() -> Vec<String> {
    with_catalog(|cat| {
        let Ok(cat) = cat else { return Vec::new() };
        let mut drivers = Vec::new();
        let mut names = Vec::new();
        for name in cat.names() {
            if let Some(m) = find_model(cat, &name) {
                if !drivers.contains(&m.driver().to_string()) {
                    drivers.push(m.driver().to_string());
                    names.push(name);
                }
            }
        }
        names
    })
}

/// The Device Settings page shown for the first Index model, for the layout check's picture.
pub fn layout_device_page(device: &Dlg) {
    with_catalog(|cat| {
        if let Ok(cat) = cat {
            let name = cat.names().into_iter().find(|n| find_model(cat, n).is_some_and(|m| m.driver() == "Emb_IndexEmbosserDriver"));
            if let Some(name) = name {
                let s = new_setup(cat, &name);
                show_device(device, find_model(cat, &name), s.version, s.escape_codes);
            }
        }
    });
}

/// The setup sheet with every page for model `name`, as the layout check opens it.
pub fn layout_sheet(name: &str) -> Option<(Dlg, Vec<Dlg>)> {
    with_catalog(|cat| {
        let cat = cat.ok()?;
        Some(build_sheet(&new_setup(cat, name), cat))
    })
}

/// Whether the TranSend SE tab shows for `model` (spec 25 15.3): only for a model with the TranSend keyword, never
/// with no model.
fn has_transend(model: Option<&Model>) -> bool {
    model.is_some_and(|m| m.flag("TranSend"))
}

/// The setup sheet (dialogs 10000 to 10004): `setup` shown, changed on OK. Returns false when cancelled. A change of
/// model works as in the reference (spec 25 15.2 to 15.4 and 16): the forms and the Device Settings and Advanced
/// pages take the new model's values, the port list follows the model's ports, the TranSend SE tab is added or taken
/// away, and the output, the serial settings and the TranSend SE values stay as they were.
pub fn edit(setup: &mut Setup, cat: &Catalog) -> bool {
    // The FTDI devices are listed once as the sheet opens, as the reference builds its port lists (spec 25 16.1).
    crate::ftdi::refresh();
    let (sheet, p) = build_sheet(setup, cat);
    let general = &p[0];
    let device = &p[1];
    let transend_page = &p[2];
    let advanced = &p[3];
    let opened = find_model(cat, &setup.model);
    Dlg::set_sheet_page(&p, 2, "TranSend SE", has_transend(opened));
    show_general(general, opened.is_some());
    show_output(general, advanced, opened.is_some());
    // A new printer choice shows the printers it allows, keeping the printer chosen when it is still among them.
    if let Some(filter) = transend_page.win(5).and_then(|w| w.as_widget::<Choice>()) {
        let tp = handle(transend_page);
        filter.on_selection_changed(move |_| {
            let chosen = tp.selected_text(6).unwrap_or_default();
            fill_ink_printers(&tp, tp.chosen_in(5, TRANSEND_FILTERS).unwrap_or(0), &chosen);
        });
    }
    // The output controls follow the output and the port chosen and Initialize Serial Port.
    let output_changed = || {
        let (g, adv) = (handle(general), handle(advanced));
        move || {
            let has_model = !g.selected_text(5).unwrap_or_default().starts_with('(');
            show_output(&g, &adv, has_model);
        }
    };
    for cid in [13, 14, 15] {
        general.on_radio(cid, output_changed());
    }
    if let Some(ports) = general.win(17).and_then(|w| w.as_widget::<Choice>()) {
        let changed = output_changed();
        ports.on_selection_changed(move |_| changed());
    }
    if let Some(init) = general.win(21).and_then(|w| w.as_widget::<CheckBox>()) {
        let changed = output_changed();
        init.on_toggled(move |_| changed());
    }
    // The ports the list was last made from: the reference makes it again only when the model's kinds of port
    // change, and then leaves nothing in it chosen (spec 25 16.1).
    let last_ports = std::rc::Rc::new(std::cell::RefCell::new(bc_core::emboss_setup::ports_with(opened, &crate::ftdi::devices())));
    let pages: Vec<Dlg> = p.iter().map(handle).collect();
    // The model the pages show (the form list holds its forms), and the setup the sheet opened with.
    let shown_model = std::rc::Rc::new(std::cell::RefCell::new(setup.model.clone()));
    let base = setup.clone();
    if let Some(models) = general.win(5).and_then(|w| w.as_widget::<Choice>()) {
        models.on_selection_changed(move |_| {
            let name = pages[0].selected_text(5).unwrap_or_default();
            with_catalog(|cat| {
                let Ok(cat) = cat else { return };
                let model = if name.starts_with('(') { None } else { find_model(cat, &name) };
                match model {
                    // The setup as the pages show it, changed to the new model as the reference changes it (spec 25
                    // 18.3), shown again.
                    Some(_) => {
                        let shown = read_sheet(&pages, &base, cat, &shown_model.borrow());
                        fill_sheet(&pages[0], &pages[1], &pages[3], &shown.with_model(cat, &name, european()), cat, false);
                        *shown_model.borrow_mut() = name.clone();
                    }
                    None => show_device(&pages[1], None, 0, false),
                }
                let ports = bc_core::emboss_setup::ports_with(model, &crate::ftdi::devices());
                if *last_ports.borrow() != ports {
                    let refs: Vec<&str> = ports.iter().map(String::as_str).collect();
                    pages[0].set_items(17, &refs, None);
                    *last_ports.borrow_mut() = ports;
                }
                show_general(&pages[0], model.is_some());
                show_output(&pages[0], &pages[3], model.is_some());
                Dlg::set_sheet_page(&pages, 2, "TranSend SE", has_transend(model));
            });
        });
    }
    // As in the reference, a change of Embosser Version or of Allow escape codes shows the page's groups again.
    if let Some(versions) = device.win(5).and_then(|w| w.as_widget::<Choice>()) {
        let (g, dv) = (handle(general), handle(device));
        versions.on_selection_changed(move |_| refresh_device(&g, &dv));
    }
    if let Some(escapes) = device.win(6).and_then(|w| w.as_widget::<CheckBox>()) {
        let (g, dv) = (handle(general), handle(device));
        escapes.on_toggled(move |_| refresh_device(&g, &dv));
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
        let mut s = read_sheet(&p, setup, cat, &model);
        let taken = setup_names().into_iter().any(|n| n == s.shown_name() && n != setup.shown_name());
        if taken {
            crate::commands::message(&format!("You already have a configuration named \"{}\".  Please give a different name.", s.shown_name()));
            continue;
        }
        s.all_users = false;
        *setup = s;
        break true;
    };
    sheet.destroy();
    result
}

/// The setup the sheet's pages show for model `model` (whose forms the form list holds), over `base` for what no
/// page shows.
fn read_sheet(p: &[Dlg], base: &Setup, cat: &Catalog, model: &str) -> Setup {
    let (general, device, transend_page, advanced) = (&p[0], &p[1], &p[2], &p[3]);
    let mut s = base.clone();
    s.model = model.to_string();
    s.name = general.text(7).trim().to_string();
    if let (Some(m), Some(shown)) = (find_model(cat, model), general.selected_text(11)) {
        if let Some(f) = m.forms.iter().find(|f| form_name(&f.id) == shown) {
            s.form = f.id.clone();
        }
    }
    s.output = match general.radio(&[13, 14, 15]) {
        Some(0) => Output::Printer(general.selected_text(16).unwrap_or_default()),
        Some(2) => Output::File(general.text(18).trim().to_string()),
        // With no port chosen (the list made again for a new model), the port kept is the one stored before, as the
        // reference stores the target name only when a port is chosen (spec 25 16.3).
        _ => Output::Port(general.selected_text(17).unwrap_or_else(|| match &base.output {
            Output::Port(p) => p.clone(),
            _ => bc_core::emboss_setup::ports_with(find_model(cat, model), &crate::ftdi::devices()).into_iter().next().unwrap_or_default(),
        })),
    };
    s.serial_init = general.checked(21);
    s.baud = general.selected_text(23).and_then(|b| b.parse().ok()).unwrap_or(s.baud);
    s.parity = general.chosen_in(25, PARITIES).unwrap_or(s.parity);
    s.data_bits = general.selected_text(27).and_then(|b| b.parse().ok()).unwrap_or(s.data_bits);
    s.stop_bits = general.selected_text(29).and_then(|b| b.parse().ok()).unwrap_or(s.stop_bits);
    s.handshaking = general.chosen_in(31, HANDSHAKING).unwrap_or(s.handshaking);
    s.version = device.chosen_in(5, VERSIONS).unwrap_or(0);
    s.escape_codes = device.checked(6);
    s.et_speaks_manage = device.checked(7);
    s.et_speaks_on = device.checked(8);
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
    // The TranSend SE values stay as they were across a change of model (spec 25 15.4), so they are read from the
    // page whether its tab shows or not.
    read_transend(transend_page, &mut s);
    s
}
