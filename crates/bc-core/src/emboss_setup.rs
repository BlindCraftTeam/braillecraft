//! Embosser setups, document embosser layouts and the embossing job's options (re/spec/25-emboss.md sections 2b to
//! 2d, 8 and 9, spec 06), shared by the desktop program and the web build.
//!
//! An embosser setup names a model from the embosser models (embossers/embossers.txt, with myembossers.txt beside it
//! replacing models by name) and holds the user's choices for it: the setup name, the default form, where the output
//! goes (a printer, a port or a file), the serial port settings, the device settings and the advanced settings. A
//! document keeps the name of its setup and its own page layout (form, cells, lines, margins, interpoint, banner page,
//! interline print).

use serde::{Deserialize, Serialize};

use crate::emboss::{enabling_start, index_start, parse_codes, EnablingStart, IndexStart, LineEnd, Options};
use crate::embossers::{Catalog, Form, Model};

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
pub const BAUDS: &[&str] = &["110", "300", "600", "1200", "2400", "4800", "9600", "14400", "19200", "38400", "57600", "115200"];
pub const PARITIES: &[&str] = &["No parity", "Even parity", "Odd parity", "Mark parity"];
pub const HANDSHAKING: &[&str] = &["Hardware", "Software (XON/XOFF)", "None"];
pub const VERSIONS: &[&str] = &["(unspecified)", "Version 1", "Version 2", "Version 3 (ROM 10.24 or older)", "Version 3 (ROM 10.25 or newer)", "Version 4", "Version 5"];
pub const LINE_ENDS: &[&str] = &["Carriage return, Line feed (PC)", "Carriage return only (Mac)", "Line feed only (UNIX)"];
pub const PAGE_ENDS: &[&str] = &["End of Line and Form Feed", "Form Feed only", "Pad page with End of Line", "Control-K (LED-120)", "Nothing", "Puma-style (0x83)"];
pub const CASES: &[&str] = &["Lowercase", "Uppercase"];

/// The form descriptions the setup shows, by form id. INFERENCE: the reference program's form description strings
/// (its string table from 10088) follow the same order as these ids sorted; the table matching them was not traced.
pub const FORM_NAMES: &[(&str, &str)] = &[
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
pub fn default_form(model: &Model, european: bool) -> String {
    let order: &[&str] = if european { &["A4", "12_w", "11_w", "11_n", "10_n"] } else { &["11_w", "11_n", "10_n"] };
    order
        .iter()
        .find(|id| model.forms.iter().any(|f| f.id == **id))
        .map(|s| s.to_string())
        .or_else(|| model.forms.first().map(|f| f.id.clone()))
        .unwrap_or_default()
}

impl Setup {
    /// A new setup of `label`'s model, with the model's own values (the Generic model's where it gives none);
    /// `european` chooses the European order of default forms.
    pub fn for_model(cat: &Catalog, label: &str, european: bool) -> Setup {
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
            form: model.map(|m| default_form(m, european)).unwrap_or_default(),
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

/// A document's embosser page layout (the formatting block, spec 25 2c).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub form: String,
    pub top_margin: u32,
    pub binding: u32,
    pub interpoint: bool,
    pub banner: bool,
    /// 0 off, 1 ink and braille, 2 braille-only pass.
    pub interline: u32,
}

/// Whether a layout of `cells` by `lines` fits `form`. INFERENCE: the reference's fit test
/// (dxemb_form_validate_settings) was not decoded; this is the rule the Embosser Setup dialog keeps: cells and binding
/// margin share the form's line, lines and top margin its page, on the interline page when interline print is on
/// and the form gives one.
pub fn fits(form: &Form, layout: &Layout, cells: u32, lines: u32) -> bool {
    let (max_cells, max_lines) = if layout.interline != 0 { form.interline.map(|i| i.0).unwrap_or(form.maximum) } else { form.maximum };
    cells + layout.binding <= max_cells && lines + layout.top_margin <= max_lines
}

/// What the checks before an embossing job found (spec 25 9.1), in the reference's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Ready,
    /// The document's paper size is not defined on its embosser (message 2364; the job stops).
    FormNotDefined,
    /// The document's layout does not fit its paper (message 2362; a question).
    DoesNotFit,
}

/// The checks after a setup has been found (message 2892 is the caller's: no setup chosen).
pub fn check(model: &Model, layout: &Layout, cells: u32, lines: u32) -> Check {
    match model.forms.iter().find(|f| f.id == layout.form) {
        None => Check::FormNotDefined,
        Some(form) if !fits(form, layout, cells, lines) => Check::DoesNotFit,
        Some(_) => Check::Ready,
    }
}

/// Message 2364, for a setup and a form as they are shown.
pub fn form_not_defined_message(setup: &str, form: &str, cells: u32, lines: u32) -> String {
    format!(
        "This document is set to use a paper size (\"{form}\") that is not defined on its assigned embosser (\"{setup}\").  You must fix this in the \"Document: Embosser Settings\" dialogue before embossing the document; you should choose an embosser and paper size capable of fitting {lines} lines of {cells} cells to avoid reformatting the document."
    )
}

/// Message 2362, a Yes/No question.
pub fn does_not_fit_message(setup: &str, form: &str, cells: u32, lines: u32) -> String {
    format!(
        "This document is set for a braille layout that won't fit properly on its assigned embosser (\"{setup}\") and paper size (\"{form}\").  You should fix this in the \"Document: Embosser Settings\" dialogue before embossing; you should choose an embosser and paper size capable of fitting {lines} lines of {cells} cells to avoid reformatting the document.  Do you want to emboss this anyway?"
    )
}

/// Message 2892.
pub const NO_SETUP_MESSAGE: &str = "Before embossing this document, you must use Embosser Setup in the Document menu to choose an embosser.";

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

/// The byte pipeline's options for a job of `copies` copies of a document laid out `cells` by `lines` with
/// `layout`, on `setup` and its `model`: the driver's job start (spec 25 section 8), padding, line and page ends.
/// `encoding` is the byte sent for each cell in the setup's output encoding, None for North American English.
/// INFERENCES, as the flags behind them were not traced (spec 25 8.4): the Index paper is sent in millimetres for
/// models marked IndexPagemm; Index V3 and later and Enabling models with escape codes set their margins by command
/// (no padding) and an Index V3 or later makes the copies itself (MC); the Enabling side argument is @; Enabling's
/// lines per page count the top margin.
pub fn job_options(s: &Setup, model: &Model, layout: &Layout, cells: u32, lines: u32, copies: u32, encoding: Option<[u8; 64]>) -> Options {
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
        encoding,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../embossers/embossers.txt");
        Catalog::from_texts(&std::fs::read_to_string(path).unwrap(), None).unwrap()
    }

    #[test]
    fn a_new_setup_takes_its_models_values_and_survives_json() {
        let cat = catalog();
        let s = Setup::for_model(&cat, "Index Everest-D V5", false);
        assert_eq!(s.form, "11_w");
        assert!(s.interpoint_enabled);
        let back: Setup = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn the_checks_before_a_job_come_in_the_reference_order() {
        let cat = catalog();
        let model = find_model(&cat, "Index Everest-D V5").unwrap();
        let mut layout = Layout { form: "11_w".into(), ..Layout::default() };
        assert_eq!(check(model, &layout, 40, 25), Check::Ready);
        assert_eq!(check(model, &layout, 60, 25), Check::DoesNotFit);
        layout.form = "zz".into();
        assert_eq!(check(model, &layout, 40, 25), Check::FormNotDefined);
    }
}
