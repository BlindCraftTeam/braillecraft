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
    /// TranSend SE page (spec 25 section 14): which printers the ink printer list shows (0 supported, 1 supported or
    /// compatible, 2 any printer), the ink printer, the interline print font, its size (0 small, 1 large interline,
    /// 2 large overlapped), and the horizontal and vertical ink adjustments in hundredths of an inch. A setup kept
    /// before these existed takes the defaults the reference gives a new setup (spec 25 14.1).
    #[serde(default)]
    pub transend_filter: usize,
    #[serde(default)]
    pub ink_printer: String,
    #[serde(default = "default_interline_font")]
    pub interline_font: String,
    #[serde(default)]
    pub transend_font_size: usize,
    #[serde(default)]
    pub ink_adjust_h: i32,
    #[serde(default)]
    pub ink_adjust_v: i32,
    /// Device Settings: Manage ET Speaks (7) and Leave ET Speaks On (8).
    #[serde(default)]
    pub et_speaks_manage: bool,
    #[serde(default)]
    pub et_speaks_on: bool,
}

impl Setup {
    /// A number field of the Device Settings page as the setup holds it (0 when empty or not a number): 27 the
    /// leftmost cell placement, 30 the rightmost cell placement, 33 the top margin setting.
    pub fn device_number(&self, cid: i32) -> u32 {
        self.device_other.iter().find(|(c, _)| *c == cid).and_then(|(_, v)| v.trim().parse().ok()).unwrap_or(0)
    }
}

/// The interline print font a new setup takes (spec 25 14.1).
fn default_interline_font() -> String {
    "Arial".into()
}

/// The TranSend SE page's printer choices, as the reference lists them (strings 10268 to 10270), in the order of
/// TranSendSEFilter's values.
pub const TRANSEND_FILTERS: &[&str] = &["Choose a Supported Printer:", "Choose a Supported or Compatible Printer:", "Choose any Printer:"];

/// The printers the TranSend SE page offers (embossers/transend-printers.txt), each kept as compared: spaces,
/// "ESC/P2" and "ESC/P" left out (spec 25 14.8 item 1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TranSendPrinters {
    pub supported: Vec<String>,
    pub compatible: Vec<String>,
}

/// A printer or driver name as the list compares it: every space, then every "ESC/P2", then every "ESC/P" left out,
/// in that order and with case kept (spec 25 14.8 item 1).
pub fn transend_normalize(name: &str) -> String {
    name.replace(' ', "").replace("ESC/P2", "").replace("ESC/P", "")
}

/// A name split as the compatible test splits it: each run of digits stands as one "#" in the text part, and every
/// digit of the name joins into one number (spec 25 14.8 item 2).
fn split_number(name: &str) -> (String, u64) {
    let (mut text, mut number, mut in_digits) = (String::new(), 0u64, false);
    for c in name.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                text.push('#');
            }
            number = number.wrapping_mul(10).wrapping_add(u64::from(c as u8 - b'0'));
            in_digits = true;
        } else {
            text.push(c);
            in_digits = false;
        }
    }
    (text, number)
}

/// One line of the list file as the reference reads it (spec 25 14.8 item 4): a bar and a space, or a bar at the
/// line end, ends the line there; two bars are one bar; a bar and three digits are the character of that code; a bar
/// and anything else is a bar and the character after it is dropped. Trailing spaces and a carriage return are
/// left off. None when the line holds only a note (it is passed over).
fn transend_line(raw: &str) -> Option<String> {
    let raw = raw.strip_suffix('\r').unwrap_or(raw);
    let chars: Vec<char> = raw.chars().collect();
    let (mut out, mut i, mut noted) = (String::new(), 0, false);
    while i < chars.len() {
        if chars[i] != '|' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        match chars.get(i + 1) {
            None | Some(' ') => {
                noted = true;
                break;
            }
            Some('|') => {
                out.push('|');
                i += 2;
            }
            Some(_) if chars.len() >= i + 4 && chars[i + 1..i + 4].iter().all(|c| c.is_ascii_digit()) => {
                let code: u32 = chars[i + 1..i + 4].iter().collect::<String>().parse().unwrap_or(256);
                if let Some(c) = char::from_u32(code).filter(|_| code <= 255) {
                    out.push(c);
                }
                i += 4;
            }
            Some(_) => {
                out.push('|');
                i += 2;
            }
        }
    }
    let out = out.trim_end_matches(' ').to_string();
    if noted && out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Reads the list file: its first line that is not a note must be "-TranSendSE 1"; then "Supported:" and
/// "Compatible:" (compared as names are, ignoring case) start their lists, and every other line is an entry of the
/// list last started; a blank line ends the reading (spec 25 14.7 item 1 and 14.8 items 1 and 4). None when the file
/// is not such a list.
pub fn transend_printers(text: &str) -> Option<TranSendPrinters> {
    let mut lines = text.split('\n').filter_map(transend_line);
    if lines.next()?.trim() != "-TranSendSE 1" {
        return None;
    }
    let mut list = TranSendPrinters::default();
    let mut current: Option<&mut Vec<String>> = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let name = transend_normalize(&line);
        if name.eq_ignore_ascii_case("Supported:") {
            current = Some(&mut list.supported);
        } else if name.eq_ignore_ascii_case("Compatible:") {
            current = Some(&mut list.compatible);
        } else if let Some(l) = current.as_mut() {
            l.push(name);
        }
    }
    Some(list)
}

impl TranSendPrinters {
    /// Whether the printer choice `filter` lets the ink printer list show a printer with driver `driver` (spec 25
    /// 14.8 item 6): 0 when its name is a supported one ignoring case; 1 when it has the text part of a compatible
    /// one ignoring case with a number at least that one's (the supported list is not looked at, as in the
    /// reference); 2 always.
    pub fn allows(&self, filter: usize, driver: &str) -> bool {
        let name = transend_normalize(driver);
        match filter {
            0 => self.supported.iter().any(|s| s.eq_ignore_ascii_case(&name)),
            1 => {
                let (text, number) = split_number(&name);
                self.compatible.iter().any(|c| {
                    let (ct, cn) = split_number(c);
                    ct.eq_ignore_ascii_case(&text) && cn <= number
                })
            }
            _ => true,
        }
    }
}

/// The ports Write to Port lists for `model`, as the reference builds the list (spec 25 16.1): lpt1 to lpt3 unless
/// the model has NoParallel, com1 to com9 unless it has NoSerial. YesUSB adds the reference's USB list, which is
/// always empty, so it adds nothing. YesFTDI adds the FTDI devices by their descriptions (spec 25 16.2), which the
/// caller reads from the FTDI library (ftd2xx) where it is installed; without it there are none. The port names are
/// lower case as the reference stores them; the list is in the order the reference's sorted list shows it, without
/// regard to case. No model, no ports.
pub fn ports_with(model: Option<&Model>, ftdi: &[String]) -> Vec<String> {
    let Some(m) = model else { return Vec::new() };
    let mut out = Vec::new();
    if !m.flag("NoSerial") {
        out.extend((1..=9).map(|n| format!("com{n}")));
    }
    if !m.flag("NoParallel") {
        out.extend((1..=3).map(|n| format!("lpt{n}")));
    }
    if m.flag("YesFTDI") {
        out.extend(ftdi.iter().filter(|d| !d.is_empty()).cloned());
    }
    out.sort_by_key(|p| p.to_lowercase());
    out
}

/// The ports for `model` with no FTDI devices (the web build, and tests).
pub fn ports(model: Option<&Model>) -> Vec<String> {
    ports_with(model, &[])
}

/// Whether the port name `name` is one of the FTDI devices `ftdi`: equal to a description, without regard to case
/// (spec 25 21.1).
pub fn is_ftdi(name: &str, ftdi: &[String]) -> bool {
    ftdi.iter().any(|d| d.eq_ignore_ascii_case(name.trim()))
}
pub const BAUDS: &[&str] = &["110", "300", "600", "1200", "2400", "4800", "9600", "14400", "19200", "38400", "57600", "115200"];
pub const PARITIES: &[&str] = &["No parity", "Even parity", "Odd parity", "Mark parity"];
pub const HANDSHAKING: &[&str] = &["Hardware", "Software (XON/XOFF)", "None"];
pub const VERSIONS: &[&str] = &["(unspecified)", "Version 1", "Version 2", "Version 3 (ROM 10.24 or older)", "Version 3 (ROM 10.25 or newer)", "Version 4", "Version 5"];
pub const LINE_ENDS: &[&str] = &["Carriage return, Line feed (PC)", "Carriage return only (Mac)", "Line feed only (UNIX)"];
pub const PAGE_ENDS: &[&str] = &["End of Line and Form Feed", "Form Feed only", "Pad page with End of Line", "Control-K (LED-120)", "Nothing", "Puma-style (0x83)"];
pub const CASES: &[&str] = &["Lowercase", "Uppercase"];

/// The form descriptions the setup shows, by form id: the reference's table of form descriptions in its English
/// texts (spec 25 section 12.3). Ids are matched without regard to case; a form not in the table shows its id (as A5
/// and B4_landscape do in the reference).
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
    ("A5_landscape", "A5 (landscape)"),
    ("bbc", "Canute eReader page size"),
    ("NBP", "38 by 28 Braille Page used by NBP"),
    ("PUMA", "Puma Zinc Plate 340mm x 285mm"),
    ("RusBook", "Russian bookpage"),
    ("RusNote", "Russian notebook"),
    ("RusNote_folio", "Russian notebook (folded booklet)"),
];

/// The descriptions a model whose FormNamePrefix is "once" shows in place of the plain ones (spec 25 12.3); the prefix
/// picks these texts, it is not written before them.
pub const ONCE_FORM_NAMES: &[(&str, &str)] = &[
    ("10_n", "10 inch length, 34 cells"),
    ("11_n", "11 inch length, 34 cells"),
    ("11_w", "11 inch length, 42 cells"),
    ("12_n", "12 inch length, 34 cells"),
    ("12_w", "12 inch length, 42 cells"),
    ("13_n", "13 inch length, 34 cells"),
    ("13_w", "13 inch length, 42 cells"),
];

/// A form as the setup lists it.
pub fn form_name(id: &str) -> String {
    form_name_with_prefix(id, "")
}

/// A form as the setup lists it for a model with FormNamePrefix `prefix` (empty for none): the variant for that
/// prefix where there is one, else the plain description, else the id.
pub fn form_name_with_prefix(id: &str, prefix: &str) -> String {
    let find = |table: &[(&str, &str)]| table.iter().find(|(i, _)| i.eq_ignore_ascii_case(id)).map(|(_, n)| n.to_string());
    let variant = if prefix.eq_ignore_ascii_case("once") { find(ONCE_FORM_NAMES) } else { None };
    variant.or_else(|| find(FORM_NAMES)).unwrap_or_else(|| id.to_string())
}

/// Whether a region (its ISO 3166 two-letter code) takes the European order of default forms. The reference tests
/// whether the user locale's country code, which is the region's international calling code, begins with 3 or 4
/// (spec 25 12.3); these are the regions of calling-code zones 3 and 4 (ITU-T E.164), the list being this program's
/// way of making that test where the system gives a region rather than a calling code.
pub fn a4_region(region: &str) -> bool {
    const ZONES_3_4: &[&str] = &[
        "GR", "NL", "BE", "FR", "ES", "GI", "PT", "LU", "IE", "IS", "AL", "MT", "CY", "FI", "AX", "BG", "HU", "LT", "LV",
        "EE", "MD", "AM", "BY", "AD", "MC", "SM", "VA", "UA", "RS", "ME", "XK", "HR", "SI", "BA", "MK", "IT", "RO", "CH",
        "CZ", "SK", "LI", "AT", "GB", "GG", "JE", "IM", "DK", "SE", "NO", "SJ", "PL", "DE",
    ];
    ZONES_3_4.iter().any(|r| r.eq_ignore_ascii_case(region))
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

/// The form a new setup of `model` starts with: the first the model has of 11_w, 11_n, 10_n (A4, 12_w, 12_n, 11_w,
/// 11_n, 10_n in Europe, spec 25 12.2), else its first form.
/// The places in VERSIONS of a model label's Index firmware versions ("V5", or a range such as "V3_early-V3"); None
/// for a version the list does not hold.
fn version_range(versions: &str) -> Option<(usize, usize)> {
    let place = |v: &str| match v.trim() {
        "V1" => Some(1),
        "V2" => Some(2),
        "V3_early" => Some(3),
        "V3" => Some(4),
        "V4" => Some(5),
        "V5" => Some(6),
        _ => None,
    };
    match versions.split_once('-') {
        Some((a, b)) => Some((place(a)?, place(b)?)),
        None => place(versions).map(|v| (v, v)),
    }
}

pub fn default_form(model: &Model, european: bool) -> String {
    let order: &[&str] = if european { &["A4", "12_w", "12_n", "11_w", "11_n", "10_n"] } else { &["11_w", "11_n", "10_n"] };
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
            version: single_version(label),
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
            transend_filter: 0,
            ink_printer: String::new(),
            interline_font: default_interline_font(),
            transend_font_size: 0,
            ink_adjust_h: 0,
            ink_adjust_v: 0,
            et_speaks_manage: false,
            et_speaks_on: false,
        }
    }

    /// This setup after its model is changed to `label` on the setup sheet, as the reference makes it (spec 25 18.3):
    /// the new model's serial settings (apart from parity), Advanced settings, escape codes, interpoint, encoding,
    /// letter case and punctuation, signature sheet counts, rightmost cell and PRINT55 location; the form kept when
    /// the new model has it, else the model's default; Embosser Version the model's single version, else the old one
    /// when the model's range holds it, else unspecified. Everything else stays as it was: the name, the output,
    /// Initialize Serial Port, parity, the left and top margins, line space, reverse page order, Z-fold, seek top of
    /// form, the custom codes and the TranSend SE values.
    pub fn with_model(&self, cat: &Catalog, label: &str, european: bool) -> Setup {
        let new = Setup::for_model(cat, label, european);
        let model = find_model(cat, label);
        let form = if model.is_some_and(|m| m.forms.iter().any(|f| f.id == self.form)) { self.form.clone() } else { new.form.clone() };
        let version = match cat.versions(label).and_then(version_range) {
            Some((first, last)) if first == last => first,
            Some((first, last)) if (first..=last).contains(&self.version) => self.version,
            _ => 0,
        };
        // Device Settings fields kept from the setup: line space (16), left margin (27), top margin (33); the
        // signature sheet counts (21, 23), the rightmost cell (30) and the PRINT55 location (43) take the model's.
        let device_other = self.device_other.iter().filter(|(cid, _)| [16, 27, 33].contains(cid)).cloned().collect();
        Setup {
            model: label.to_string(),
            form,
            version,
            device_other,
            baud: new.baud,
            data_bits: new.data_bits,
            stop_bits: new.stop_bits,
            handshaking: new.handshaking,
            escape_codes: new.escape_codes,
            interpoint_enabled: new.interpoint_enabled,
            encoding: new.encoding,
            high_punctuation: new.high_punctuation,
            upper_case: new.upper_case,
            end_of_line: new.end_of_line,
            end_of_page: new.end_of_page,
            eol_on_full_line: new.eol_on_full_line,
            eop_on_full_page: new.eop_on_full_page,
            pad_lines: new.pad_lines,
            prompt_new_sheet: new.prompt_new_sheet,
            one_at_a_time: new.one_at_a_time,
            text_delay: new.text_delay,
            control_delay: new.control_delay,
            ..self.clone()
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

/// The version number of a model listed with one firmware version ("Index Everest-D V5" is V5, 6), as the Device
/// Settings version list then holds only that version and chooses it (audit print-emboss E11); 0 for a model listed
/// with a range of versions or none. Numbers: V1 1, V2 2, V3_early 3, V3 4, V4 5, V5 6.
pub fn single_version(label: &str) -> usize {
    match label.rsplit(' ').next().unwrap_or("") {
        "V1" => 1,
        "V2" => 2,
        "V3_early" => 3,
        "V3" => 4,
        "V4" => 5,
        "V5" => 6,
        _ => 0,
    }
}

/// The Index driver level of a setup: its version number as the Device Settings list gives it (V1 1, V2 2, V3_early
/// 3, V3 4, V4 5, V5 6; 0 unspecified), as the reference's driver reads it.
fn index_level(s: &Setup) -> u8 {
    s.version.min(6) as u8
}

/// The byte pipeline's options for a job of `copies` copies of a document laid out `cells` by `lines` with
/// `layout`, on `setup` and its `model`: the driver's job start (spec 25 section 8), padding, line and page ends, as
/// the reference's drivers make them (audit print-emboss E2 to E5). An Index V3 or later makes the copies itself (MC).
/// INFERENCE: the keywords RotatePaperSizings, IndexPagemm and LabellerROM are the names of the model flags the
/// reference reads by number (0x55, 0x3d, 0x56).
pub fn job_options(s: &Setup, model: &Model, layout: &Layout, cells: u32, lines: u32, copies: u32, encoding: Option<[u8; 64]>) -> Options {
    let form = model.forms.iter().find(|f| f.id.eq_ignore_ascii_case(&layout.form));
    let driver = model.driver();
    let (index, enabling) = (driver.contains("Index"), driver.contains("Enabling"));
    let interpoint = layout.interpoint && s.interpoint_enabled;
    let binding = layout.binding + form.map_or(0, |f| f.cell_offset);
    let level = if index { index_level(s) } else { 0 };
    let booklet = form.is_some_and(|f| f.double_page);
    // 8-dot line spacing: a TranSend SE ink printer named, interline print on, the large interline font.
    let eight_dot = !s.ink_printer.trim().is_empty() && layout.interline != 0 && s.transend_font_size == 1;
    let mut start = Vec::new();
    if index {
        start = index_start(&IndexStart {
            level,
            form_width: form.map_or(0, |f| f.width),
            form_length: form.map_or(0, |f| f.length),
            form_cells: form.map_or(cells + binding, |f| f.maximum.0),
            form_id: layout.form.clone(),
            booklet,
            rotated: model.flag("RotatePaperSizings"),
            to_mm: model.flag("IndexPagemm"),
            sideways_codes: model.flag("IndexZfold"),
            z_fold: s.z_fold,
            eight_dot,
            cells,
            binding,
            lines,
            top_margin: layout.top_margin,
            interpoint,
            copies,
            sequence: model.text("InitSequence").unwrap_or("").to_string(),
        });
    } else if enabling && s.escape_codes {
        // Lines per page: the form's largest number of lines (with a TranSend SE ink printer, its length at 400 or
        // 490 to the inch), less the model's LessLinesInterpoint when interpoint.
        let form_lines = match form {
            Some(f) if !s.ink_printer.trim().is_empty() => f.length * 100 / if s.transend_font_size == 1 { 490 } else { 400 },
            Some(f) => f.maximum.1,
            None => lines + layout.top_margin,
        };
        let less = if interpoint { model.number("LessLinesInterpoint").unwrap_or(0).max(0) as u32 } else { 0 };
        start = enabling_start(&EnablingStart {
            full: true,
            side: if interpoint { b'@' } else { b'A' },
            seek_top_of_form: s.seek_top_of_form,
            voice: s.et_speaks_manage,
            voice_on: s.et_speaks_on,
            dot_table: eight_dot,
            eight_dot,
            perforation_skip: model.flag("CanDoInterpoint"),
            interpoint,
            label_mode: model.flag("LabellerROM"),
            form_length: form.map_or(110, |f| f.length),
            lines: form_lines.saturating_sub(less),
            cells,
            binding,
            init: String::new(),
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
        booklet,
        automated: s.escape_codes || model.flag("UsingEscapeCodes") || (index && level >= 4),
        no_pad: (index && level >= 5) || (booklet && index && level >= 4) || driver.contains("ThielAdvanced"),
        device_left: s.device_number(27),
        device_right: s.device_number(30),
        device_top: s.device_number(33),
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
        typewriter: driver == "Emb_Typewriter_EmbosserDriver",
        form_lines: form.map_or(lines + layout.top_margin, |f| f.maximum.1),
        page_wrap: model.flag("PageWrap"),
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
    fn form_names_are_the_references_texts() {
        assert_eq!(form_name("11_w"), "11.5 by 11 inches");
        // Ids match without regard to case; a form not in the table shows its id.
        assert_eq!(form_name("nbp"), "38 by 28 Braille Page used by NBP");
        assert_eq!(form_name("A5"), "A5");
        assert_eq!(form_name("B4_landscape"), "B4_landscape");
        assert_eq!(form_name("bbc"), "Canute eReader page size");
        // The "once" prefix picks its own texts and leaves the others plain.
        assert_eq!(form_name_with_prefix("11_w", "ONCE"), "11 inch length, 42 cells");
        assert_eq!(form_name_with_prefix("A4", "ONCE"), "A4");
        assert_eq!(form_name_with_prefix("11_w", ""), "11.5 by 11 inches");
    }

    #[test]
    fn regions_of_calling_zones_three_and_four_take_the_european_forms() {
        for r in ["GB", "DE", "FR", "PT", "IE", "CZ", "gb"] {
            assert!(a4_region(r), "{r}");
        }
        for r in ["US", "CA", "RU", "AU", "JP", ""] {
            assert!(!a4_region(r), "{r}");
        }
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

    #[test]
    fn a_change_of_model_takes_and_keeps_what_the_reference_does() {
        let cat = catalog();
        let mut s = Setup::for_model(&cat, "Enabling Juliet Pro 60", false);
        s.name = "Mine".into();
        s.output = Output::Port("com3".into());
        s.parity = 2;
        s.baud = 1200;
        s.custom_codes = true;
        s.ink_adjust_h = 5;
        s.device_other = vec![(27, "3".into()), (30, "38".into())];
        let t = s.with_model(&cat, "Enabling Marathon", false);
        let fresh = Setup::for_model(&cat, "Enabling Marathon", false);
        assert_eq!(t.model, "Enabling Marathon");
        // Kept.
        assert_eq!((t.name.as_str(), &t.output, t.parity, t.custom_codes, t.ink_adjust_h), ("Mine", &Output::Port("com3".into()), 2, true, 5));
        assert_eq!(t.device_other, vec![(27, "3".to_string())]);
        // The model's.
        assert_eq!((t.baud, t.data_bits, t.stop_bits, t.handshaking), (fresh.baud, fresh.data_bits, fresh.stop_bits, fresh.handshaking));
        assert_eq!((t.end_of_line, t.end_of_page, t.encoding.as_str()), (fresh.end_of_line, fresh.end_of_page, fresh.encoding.as_str()));
        // Embosser Version: a single version is taken, a range keeps an old version inside it, else unspecified.
        let mut i = Setup::for_model(&cat, "Index Everest-D V2", false);
        i.version = 4;
        assert_eq!(i.with_model(&cat, "Index Basic-D V3_early-V3", false).version, 4);
        assert_eq!(i.with_model(&cat, "Index Everest-D V5", false).version, 6);
        i.version = 2;
        assert_eq!(i.with_model(&cat, "Index Basic-D V3_early-V3", false).version, 0);
        assert_eq!(i.with_model(&cat, "Enabling Marathon", false).version, 0);
    }

    #[test]
    fn the_port_list_follows_the_models_port_keywords() {
        let cat = catalog();
        let all: Vec<String> = (1..=9).map(|n| format!("com{n}")).chain((1..=3).map(|n| format!("lpt{n}"))).collect();
        let plain = cat
            .names()
            .into_iter()
            .find(|n| find_model(&cat, n).is_some_and(|m| !m.flag("NoSerial") && !m.flag("NoParallel")))
            .unwrap();
        assert_eq!(ports(find_model(&cat, &plain)), all, "{plain}");
        // NoParallel only: the com ports.
        assert_eq!(ports(find_model(&cat, "JTR Corporation ESA-2000")), all[..9].to_vec());
        // NoSerial and NoParallel: none, as with no model.
        assert!(ports(find_model(&cat, "Enabling Trident")).is_empty());
        assert!(ports(None).is_empty());
        // YesFTDI with NoSerial and NoParallel: the FTDI descriptions only, sorted without regard to case; a model
        // without YesFTDI never shows them.
        let ftdi = vec!["eBrailler Cosmo".to_string(), "Another FT232R".to_string()];
        assert_eq!(ports_with(find_model(&cat, "EBrailler Cosmo"), &ftdi), vec!["Another FT232R", "eBrailler Cosmo"]);
        assert_eq!(ports_with(find_model(&cat, "JTR Corporation ESA-2000"), &ftdi), all[..9].to_vec());
        assert!(is_ftdi("EBRAILLER COSMO", &ftdi) && !is_ftdi("com1", &ftdi));
    }

    #[test]
    fn a_setup_kept_before_the_transend_values_takes_their_defaults() {
        let cat = catalog();
        let s = Setup::for_model(&cat, "Index Everest-D V5", false);
        let mut json: serde_json::Value = serde_json::to_value(&s).unwrap();
        for key in ["transend_filter", "ink_printer", "interline_font", "transend_font_size", "ink_adjust_h", "ink_adjust_v"] {
            json.as_object_mut().unwrap().remove(key);
        }
        let old: Setup = serde_json::from_value(json).unwrap();
        assert_eq!((old.transend_filter, old.ink_printer.as_str(), old.interline_font.as_str()), (0, "", "Arial"));
        assert_eq!((old.transend_font_size, old.ink_adjust_h, old.ink_adjust_v), (0, 0, 0));
    }

    #[test]
    fn the_shipped_transend_list_matches_as_the_reference_does() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../embossers/transend-printers.txt");
        let list = transend_printers(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!((list.supported.len(), list.compatible.len()), (27, 3));
        // The worked results of spec 25 14.8: (driver, filter 0, filter 1).
        for (driver, supported, compatible) in [
            ("Epson EX-820", false, true),
            ("EPSON LX-300+II ESC/P", true, false),
            ("Epson LQ-2180 ESC/P 2", true, true),
            ("EPSON FX-890 ESC/P", true, true),
            ("Epson FX-2190", true, true),
            ("HP LaserJet 4", false, false),
        ] {
            assert_eq!(list.allows(0, driver), supported, "{driver} supported");
            assert_eq!(list.allows(1, driver), compatible, "{driver} compatible");
            assert!(list.allows(2, driver), "{driver} any");
        }
        // A lower model number than the compatible entry is not compatible.
        assert!(!list.allows(1, "Epson EX-700"));
    }

    #[test]
    fn the_transend_list_reader_keeps_the_references_line_rules() {
        let text = "| note\r\n-TranSendSE 1\r\nSupported:\r\nEpson A-1 | a note after the name\r\nBar||Name\r\n\r\nEpson B-2\r\n";
        let list = transend_printers(text).unwrap();
        // A note after a name is left off, two bars are one bar, and the blank line ends the reading.
        assert_eq!(list.supported, vec!["EpsonA-1".to_string(), "Bar|Name".to_string()]);
        assert_eq!(transend_printers("Supported:\nEpson A-1\n"), None);
    }
}
