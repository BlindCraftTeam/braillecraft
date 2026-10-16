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

use serde::{Deserialize, Serialize};

use bm_core::embossers::{Catalog, Model};

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

/// Where a setup's output goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Output {
    Printer(String),
    Port(String),
    /// A file name; empty to ask for one each time.
    File(String),
}

/// One embosser setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setup {
    pub name: String,
    /// The model as the list shows it: its name and firmware range, e.g. "Index Everest-D V5".
    pub model: String,
    pub all_users: bool,
    /// The default form's id.
    pub form: String,
    pub output: Output,
    pub serial_init: bool,
    pub baud: u32,
    /// 0 none, 1 even, 2 odd, 3 mark.
    pub parity: usize,
    pub data_bits: u32,
    pub stop_bits: u32,
    /// 0 hardware, 1 software (XON/XOFF), 2 none.
    pub handshaking: usize,
    /// Device settings: Index version (0 unspecified, 1 to 6 as the list gives them), escape codes allowed,
    /// interpoint enabled, the output encoding's section name, high punctuation, upper case letters, reverse page
    /// order, Z-fold, seek top of form, and the page's other fields as set (kept as they are).
    pub version: usize,
    pub escape_codes: bool,
    pub interpoint_enabled: bool,
    pub encoding: String,
    pub high_punctuation: bool,
    pub upper_case: bool,
    pub reverse_order: bool,
    pub z_fold: bool,
    pub seek_top_of_form: bool,
    pub device_other: Vec<(i32, String)>,
    /// Advanced: custom job codes, end of line (0 CR LF, 1 CR, 2 LF) and end of page (0 to 6) indicators, the full
    /// line and full page choices, line padding, the new sheet prompt, and one character at a time with delays.
    pub custom_codes: bool,
    pub start_codes: String,
    pub end_codes: String,
    pub end_of_line: usize,
    pub end_of_page: usize,
    pub eol_on_full_line: bool,
    pub eop_on_full_page: bool,
    pub pad_lines: bool,
    pub prompt_new_sheet: bool,
    pub one_at_a_time: bool,
    pub text_delay: u32,
    pub control_delay: u32,
}

/// The ports Write to Port lists.
pub const PORTS: &[&str] = &["com1", "com2", "com3", "com4", "lpt1", "lpt2", "lpt3", "lpt4", "usb"];
const BAUDS: &[&str] = &["110", "300", "600", "1200", "2400", "4800", "9600", "14400", "19200", "38400", "57600", "115200"];
const PARITIES: &[&str] = &["No parity", "Even parity", "Odd parity", "Mark parity"];
const HANDSHAKING: &[&str] = &["Hardware", "Software (XON/XOFF)", "None"];
const VERSIONS: &[&str] = &["(unspecified)", "Version 1", "Version 2", "Version 3 (ROM 10.24 or older)", "Version 3 (ROM 10.25 or newer)", "Version 4", "Version 5"];
const LINE_ENDS: &[&str] = &["Carriage return, Line feed (PC)", "Carriage return only (Mac)", "Line feed only (UNIX)"];
const PAGE_ENDS: &[&str] = &["End of Line and Form Feed", "Form Feed only", "Pad page with End of Line", "Control-K (LED-120)", "Nothing", "Puma-style (0x83)"];
const CASES: &[&str] = &["Lowercase", "Uppercase"];

/// The form descriptions the setup shows, by form id. INFERENCE: the reference program's form description strings
/// (its string table from 10088) follow the same order as these ids sorted; the table matching them was not traced.
const FORM_NAMES: &[(&str, &str)] = &[
    ("10_d", "23 by 10 inches (folded booklet)"),
    ("10_mn", "8 by 10 inches"),
    ("10_n", "8.5 by 10 inches"),
    ("10_nd", "17 by 10 inches (folded booklet)"),
    ("10_n_side", "8.5 by 10 inches (landscape)"),
    ("10_w", "11.5 by 10 inches"),
    ("11_d", "23 by 11 inches (folded booklet)"),
    ("11_gem", "10 by 11 inches"),
    ("11_n", "Letter (8.5 by 11 inches)"),
    ("11_nd", "Tabloid (17 by 11 inches, folded booklet)"),
    ("11_n_side", "Letter (8.5 by 11 inches, landscape)"),
    ("11_vw", "15 by 11 inches"),
    ("11_w", "11.5 by 11 inches"),
    ("11_w_side", "11.5 by 11 inches (landscape)"),
    ("12_d", "23 by 12 inches (folded booklet)"),
    ("12_n", "8.5 by 12 inches"),
    ("12_nd", "17 by 12 inches (folded booklet)"),
    ("12_vw", "15 by 12 inches"),
    ("12_w", "11.5 by 12 inches"),
    ("13.5_d", "23 by 13.5 inches (folded booklet)"),
    ("13_d", "23 by 13 inches (folded booklet)"),
    ("13_n", "8.5 by 13 inches"),
    ("13_w", "11.5 by 13 inches"),
    ("14_n", "Legal (8.5 by 14 inches)"),
    ("A3", "A3"),
    ("A3_ex_folio", "A3 Extended (folded booklet)"),
    ("A3_folio", "A3 (folded booklet)"),
    ("A3_landscape", "A3 (landscape)"),
    ("A4", "A4"),
    ("A4_ex", "A4 Extended (250 by 297 mm)"),
    ("A4_folio", "A4 (folded booklet)"),
    ("A4_landscape", "A4 (landscape)"),
    ("A5", "A5"),
    ("A5_landscape", "A5 (landscape)"),
    ("B4_landscape", "B4 (landscape)"),
    ("nbp", "38 by 28 Braille Page used by NBP"),
    ("RusBook", "Russian bookpage"),
    ("RusNote", "Russian notebook"),
    ("RusNote_folio", "Russian notebook (folded booklet)"),
];

/// A form as the setup lists it.
pub fn form_name(id: &str) -> String {
    FORM_NAMES.iter().find(|(i, _)| *i == id).map(|(_, n)| n.to_string()).unwrap_or_else(|| id.to_string())
}

/// The model of a list label ("Index Everest-D V5": a name, then a firmware range when the model gives one).
pub fn find_model<'a>(cat: &'a Catalog, label: &str) -> Option<&'a Model> {
    cat.models.iter().find(|m| {
        m.names.iter().any(|n| match &n.versions {
            Some(v) => format!("{} {}", n.name, v) == label,
            None => n.name == label,
        })
    })
}

/// The form a new setup of `model` starts with: the first the model has of 11_w, 11_n, 10_n (A4, 12_w, 11_w, 11_n,
/// 10_n in Europe, spec 06), else its first form.
fn default_form(model: &Model) -> String {
    let european = std::env::var("LANG").is_ok_and(|l| !l.is_empty() && !l.starts_with("en_US") && !l.starts_with("C"));
    let order: &[&str] = if european { &["A4", "12_w", "11_w", "11_n", "10_n"] } else { &["11_w", "11_n", "10_n"] };
    order
        .iter()
        .find(|id| model.forms.iter().any(|f| f.id == **id))
        .map(|s| s.to_string())
        .or_else(|| model.forms.first().map(|f| f.id.clone()))
        .unwrap_or_default()
}

impl Setup {
    /// A new setup of `label`'s model, with the model's own values (the Generic model's where it gives none).
    pub fn for_model(cat: &Catalog, label: &str) -> Setup {
        let model = find_model(cat, label);
        let number = |key: &str, default: i64| model.and_then(|m| cat.number(m, key)).unwrap_or(default);
        let flag = |key: &str| model.is_some_and(|m| m.flag(key));
        let handshaking = match model.and_then(|m| m.text("Handshaking")).map(|s| s.to_ascii_lowercase()) {
            Some(h) if h == "software" => 1,
            Some(h) if h == "none" => 2,
            _ => 0,
        };
        Setup {
            name: String::new(),
            model: label.to_string(),
            all_users: false,
            form: model.map(default_form).unwrap_or_default(),
            output: Output::Port("lpt1".into()),
            serial_init: true,
            baud: number("Baud", 9600) as u32,
            parity: 0,
            data_bits: number("DataBits", 8) as u32,
            stop_bits: number("StopBits", 1) as u32,
            handshaking,
            version: 0,
            escape_codes: flag("ETCAllowEscapeCodes") || flag("UsingEscapeCodes"),
            interpoint_enabled: flag("CanDoInterpoint"),
            encoding: model.and_then(|m| m.encodings().first().map(|s| s.to_string())).unwrap_or_else(|| "North American English".into()),
            high_punctuation: false,
            upper_case: flag("LettersUpperCase"),
            reverse_order: false,
            z_fold: false,
            seek_top_of_form: false,
            device_other: Vec::new(),
            custom_codes: false,
            start_codes: String::new(),
            end_codes: String::new(),
            end_of_line: if flag("CrMac") { 1 } else if flag("LfUnix") { 2 } else { 0 },
            end_of_page: number("EndofPage", 0).clamp(0, 6) as usize,
            eol_on_full_line: !flag("LineWrap"),
            eop_on_full_page: !flag("PageWrap"),
            pad_lines: flag("LinesPadded"),
            prompt_new_sheet: flag("PromptNewPage"),
            one_at_a_time: flag("OneByOne"),
            text_delay: number("AdditionalDelay", 0) as u32,
            control_delay: number("ControlDelay", 0) as u32,
        }
    }

    /// The name the setup is listed under: its own, else its model's.
    pub fn shown_name(&self) -> String {
        if self.name.trim().is_empty() {
            self.model.clone()
        } else {
            self.name.clone()
        }
    }
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

/// A document's embosser page layout (the formatting block, spec 25 2c).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub form: String,
    pub top_margin: u32,
    pub binding: u32,
    pub interpoint: bool,
    pub banner: bool,
    /// 0 off, 1 ink and braille, 2 braille-only pass.
    pub interline: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { form: String::new(), top_margin: 0, binding: 0, interpoint: false, banner: false, interline: 0 }
    }
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

/// The Index driver level of a setup: its version choice, else the firmware range its model is listed with.
/// INFERENCE (spec 25 8.3): versions 3, 4 and 5 are levels 4, 5 and 6; versions 1 and 2 build no parameter string.
fn index_level(s: &Setup) -> u8 {
    match s.version {
        1 => return 1,
        2 => return 2,
        3 | 4 => return 4,
        5 => return 5,
        6 => return 6,
        _ => {}
    }
    let range = s.model.rsplit(' ').next().unwrap_or("");
    match range.rsplit('-').next().unwrap_or(range) {
        "V5" => 6,
        "V4" => 5,
        "V3" | "V3_early" => 4,
        "V2" => 2,
        "V1" => 1,
        _ => 0,
    }
}

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

/// The byte pipeline's options for a job of `copies` copies of a document laid out `cells` by `lines` with
/// `layout`, on `setup` and its `model`: the driver's job start (spec 25 section 8), padding, line and page ends.
/// INFERENCES, as the flags behind them were not traced (spec 25 8.4): the Index paper is sent in millimetres for
/// models marked IndexPagemm; Index V3 and later and Enabling models with escape codes set their margins by command
/// (no padding) and an Index V3 or later makes the copies itself (MC); the Enabling side argument is @; Enabling's
/// lines per page count the top margin.
pub fn job_options(s: &Setup, model: &Model, layout: &Layout, cells: u32, lines: u32, copies: u32) -> bm_core::emboss::Options {
    use bm_core::emboss::{enabling_start, index_start, parse_codes, EnablingStart, IndexStart, LineEnd, Options};
    let form = model.forms.iter().find(|f| f.id == layout.form);
    let driver = model.driver();
    let (index, enabling) = (driver.contains("Index"), driver.contains("Enabling"));
    let interpoint = layout.interpoint && s.interpoint_enabled;
    let binding = layout.binding + form.map_or(0, |f| f.cell_offset);
    let escape = s.escape_codes && (model.flag("ETCAllowEscapeCodes") || model.flag("UsingEscapeCodes"));
    let level = if index { index_level(s) } else { 0 };
    let mut start = Vec::new();
    if index {
        start = index_start(&IndexStart {
            level,
            form_width: form.map_or(0, |f| f.width),
            form_length: form.map_or(0, |f| f.length),
            form_cells: form.map_or(cells + binding, |f| f.maximum.0),
            form_id: layout.form.clone(),
            booklet: form.is_some_and(|f| f.double_page),
            to_mm: model.flag("IndexPagemm"),
            z_fold: s.z_fold && model.flag("IndexZfold"),
            cells,
            binding,
            lines,
            interpoint,
            copies,
            sequence: model.text("InitSequence").unwrap_or("").to_string(),
            ..IndexStart::default()
        });
    } else if enabling && escape {
        start = enabling_start(&EnablingStart {
            full: true,
            side: b'@',
            seek_top_of_form: s.seek_top_of_form && model.flag("AllowSeekTOF"),
            voice: model.flag("ETSpeaks"),
            interpoint,
            form_length: form.map_or(110, |f| f.length),
            lines: lines + layout.top_margin,
            cells,
            binding,
            ..EnablingStart::default()
        });
    }
    if s.custom_codes {
        start.extend(parse_codes(&s.start_codes));
    }
    let puma = s.encoding.to_ascii_lowercase().contains("puma");
    Options {
        cells,
        lines,
        top_margin: layout.top_margin,
        binding,
        interpoint,
        margins_by_command: escape || (index && level >= 4),
        letters_upper: s.upper_case,
        high_punctuation: s.high_punctuation,
        encoding: encoding_codes(&s.encoding),
        line_end: if puma { LineEnd::Puma } else { [LineEnd::CrLf, LineEnd::Cr, LineEnd::Lf][s.end_of_line.min(2)] },
        end_of_page: s.end_of_page.min(6) as u8,
        eol_on_full_line: s.eol_on_full_line,
        eop_on_full_page: s.eop_on_full_page,
        pad_lines: s.pad_lines,
        make_up_sides: !model.flag("NoMakeupSides"),
        start,
        end: if s.custom_codes { parse_codes(&s.end_codes) } else { Vec::new() },
        control_d: model.flag("ControlD"),
        control_z: model.flag("ControlZ"),
        close_sync: Vec::new(),
        copies_by_embosser: index && level >= 4 && copies > 1,
    }
}

/// Where a setup's output goes; None for Write to File with no name (the file is asked for each time).
pub fn target(s: &Setup) -> Option<bm_core::emboss_out::Target> {
    use bm_core::emboss_out::{Serial, Target};
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
                    fill_sheet(&g, &dv, &adv, &Setup::for_model(cat, &model), cat, false);
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
        let mut s = if model == setup.model { setup.clone() } else { Setup::for_model(cat, &model) };
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
