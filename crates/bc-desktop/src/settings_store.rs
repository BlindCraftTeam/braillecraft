//! Settings kept between runs. Each setting has a section (or none), a key and a text value: "yes" or "no",
//! a number written in decimal, or text. The program reads them at start-up and writes a setting when the
//! dialog that changes it is confirmed.
//!
//! On Windows they are registry values (text, REG_SZ) under HKEY_CURRENT_USER\Software\BrailleCraft\BrailleCraft,
//! a section being a subkey; a value missing there is looked for under HKEY_LOCAL_MACHINE at the same path, which
//! an installation or an administrator can fill with defaults for every user. Elsewhere they are a text file in the
//! user's configuration folder ($XDG_CONFIG_HOME/BrailleCraft or ~/.config/BrailleCraft on Linux,
//! ~/Library/Preferences/BrailleCraft on macOS), named settings.ini: settings without a section first, then one
//! "[section]" line before each section's settings, one "key=value" line each. A line that is neither is skipped.

use std::collections::BTreeMap;

use crate::app::{DefaultViews, Fonts, LineCodes, Settings, Shortcuts, ViewFont, ViewPrefs};

/// The section of the settings that have none.
pub const ROOT: &str = "";

/// The settings as the file holds them: section (ROOT for none), then key, then value.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FileStore {
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl FileStore {
    /// Reads the file's text. Blank lines and lines starting with "#" or ";" are comments; a line that is neither a
    /// "[section]" line nor a "key=value" line is skipped.
    pub fn parse(text: &str) -> FileStore {
        let mut store = FileStore::default();
        let mut section = ROOT.to_string();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            store.sections.entry(section.clone()).or_default().insert(key.to_string(), value.trim().to_string());
        }
        store
    }

    /// The file's text: the settings without a section first.
    pub fn text(&self) -> String {
        let mut out = String::from("# BrailleCraft settings\n");
        if let Some(root) = self.sections.get(ROOT) {
            for (k, v) in root {
                out.push_str(&format!("{k}={v}\n"));
            }
        }
        for (section, values) in self.sections.iter().filter(|(s, _)| !s.is_empty()) {
            out.push_str(&format!("\n[{section}]\n"));
            for (k, v) in values {
                out.push_str(&format!("{k}={v}\n"));
            }
        }
        out
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.sections.get(section).and_then(|s| s.get(key)).map(String::as_str)
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        self.sections.entry(section.to_string()).or_default().insert(key.to_string(), value.to_string());
    }

    pub fn clear_section(&mut self, section: &str) {
        self.sections.remove(section);
    }
}

// ------------------------------------------------------------------ where they are kept

#[cfg(not(windows))]
mod backend {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use super::FileStore;

    thread_local! {
        static STORE: RefCell<Option<FileStore>> = const { RefCell::new(None) };
    }

    /// settings.ini in the user's configuration folder.
    pub fn path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let folder = if cfg!(target_os = "macos") {
            home?.join("Library").join("Preferences").join("BrailleCraft")
        } else {
            match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                Some(c) => PathBuf::from(c).join("BrailleCraft"),
                None => home?.join(".config").join("BrailleCraft"),
            }
        };
        Some(folder.join("settings.ini"))
    }

    fn with<R>(f: impl FnOnce(&mut FileStore) -> R) -> R {
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            let store = s.get_or_insert_with(|| {
                path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| FileStore::parse(&t)).unwrap_or_default()
            });
            f(store)
        })
    }

    pub fn get(section: &str, key: &str) -> Option<String> {
        with(|s| s.get(section, key).map(str::to_string))
    }

    pub fn set(section: &str, key: &str, value: &str) {
        write(with(|s| {
            s.set(section, key, value);
            s.text()
        }));
    }

    pub fn clear_section(section: &str) {
        write(with(|s| {
            s.clear_section(section);
            s.text()
        }));
    }

    fn write(text: String) {
        let Some(path) = path() else { return };
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("BrailleCraft: the settings could not be written to {}: {e}", path.display());
        }
    }
}

#[cfg(windows)]
mod backend {
    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    type Hkey = *mut c_void;
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001u32 as usize as Hkey;
    const HKEY_LOCAL_MACHINE: Hkey = 0x8000_0002u32 as usize as Hkey;
    const KEY_READ: u32 = 0x20019;
    const KEY_WRITE: u32 = 0x20006;
    const REG_SZ: u32 = 1;
    /// The vendor key and the program's key under it.
    const PRODUCT: &str = "Software\\BrailleCraft\\BrailleCraft";

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(key: Hkey, sub: *const u16, options: u32, sam: u32, out: *mut Hkey) -> i32;
        fn RegCreateKeyExW(
            key: Hkey,
            sub: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam: u32,
            security: *const c_void,
            out: *mut Hkey,
            disposition: *mut u32,
        ) -> i32;
        fn RegQueryValueExW(key: Hkey, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
        fn RegSetValueExW(key: Hkey, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
        fn RegDeleteTreeW(key: Hkey, sub: *const u16) -> i32;
    }

    /// Takes a section's key, with every value in it, out of the user's settings.
    pub fn clear_section(section: &str) {
        if !section.is_empty() {
            unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, wide(&path(section)).as_ptr()) };
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn path(section: &str) -> String {
        if section.is_empty() {
            PRODUCT.to_string()
        } else {
            format!("{PRODUCT}\\{section}")
        }
    }

    fn read(root: Hkey, section: &str, key: &str) -> Option<String> {
        let mut h: Hkey = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(root, wide(&path(section)).as_ptr(), 0, KEY_READ, &mut h) } != 0 {
            return None;
        }
        let mut buf = vec![0u16; 2048];
        let mut len = (buf.len() * 2) as u32;
        let mut kind = 0u32;
        let r = unsafe {
            RegQueryValueExW(h, wide(key).as_ptr(), std::ptr::null_mut(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut len)
        };
        unsafe { RegCloseKey(h) };
        if r != 0 || kind != REG_SZ {
            return None;
        }
        buf.truncate((len as usize / 2).min(2048));
        Some(OsString::from_wide(&buf).to_string_lossy().trim_end_matches('\0').to_string())
    }

    pub fn get(section: &str, key: &str) -> Option<String> {
        read(HKEY_CURRENT_USER, section, key).or_else(|| read(HKEY_LOCAL_MACHINE, section, key))
    }

    pub fn set(section: &str, key: &str, value: &str) {
        let mut h: Hkey = std::ptr::null_mut();
        let mut disposition = 0u32;
        let r = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(&path(section)).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_WRITE,
                std::ptr::null(),
                &mut h,
                &mut disposition,
            )
        };
        if r != 0 {
            eprintln!("BrailleCraft: the settings key {} could not be opened ({r})", path(section));
            return;
        }
        let data = wide(value);
        unsafe {
            RegSetValueExW(h, wide(key).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32);
            RegCloseKey(h);
        }
    }
}

pub use backend::{clear_section, get, set};

/// Global, Notification Settings, OK: every kept "do not show this again" answer is forgotten, as the original
/// empties these sections (Mutable holds the message answers, AddIns the screen reader script offer, EmbossTV and
/// EmbossTVTemp the embosser prompts).
pub fn reset_notifications() {
    for section in ["Mutable", "AddIns", "EmbossTV", "EmbossTVTemp"] {
        clear_section(section);
    }
}

// ------------------------------------------------------------------ the program's settings

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn get_bool(section: &str, key: &str) -> Option<bool> {
    match get(section, key)?.to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

fn get_number(section: &str, key: &str) -> Option<i64> {
    get(section, key)?.trim().parse().ok()
}

fn get_text(section: &str, key: &str) -> Option<String> {
    get(section, key).filter(|v| !v.is_empty())
}

/// The choice of font for a view, numbered as the Default Views dialog lists them: print, braille, simulated braille.
const VIEW_FONTS: [ViewFont; 3] = [ViewFont::Print, ViewFont::Braille, ViewFont::SimBraille];

fn view_font_number(f: ViewFont) -> usize {
    VIEW_FONTS.iter().position(|v| *v == f).unwrap_or(0)
}

/// The settings kept from earlier runs, over the defaults in `s`. A value that is missing or cannot be read leaves the
/// default.
pub fn load(s: &mut Settings) {
    if let Some(v) = get_text(ROOT, "BrailleEncoding") {
        s.encoding = v;
    }
    if let Some(v) = get_text(ROOT, "ChordKeys").filter(|v| v.chars().count() == 6) {
        s.six_key_set = v;
    }
    let d = &mut s.default_views;
    if let Some(n) = get_number("Display", "dftrans").and_then(|n| VIEW_FONTS.get(n as usize)) {
        d.translated_font = *n;
    }
    if let Some(n) = get_number("Display", "dfbraille").and_then(|n| VIEW_FONTS.get(n as usize)) {
        d.braille_font = *n;
    }
    let flags: [(&str, &mut bool); 7] = [
        ("PrintShowFullPage", &mut d.print_full_page),
        ("PrintDisplayCodes", &mut d.print_codes),
        ("PrintShowTranslationLine", &mut d.print_translated),
        ("BrailleShowFullPage", &mut d.braille_full_page),
        ("BrailleDisplayCodes", &mut d.braille_codes),
        ("BrailleShowTranslationLine", &mut d.braille_translated),
        ("SixKeyBraille", &mut d.six_key),
    ];
    for (key, field) in flags {
        if let Some(b) = get_bool("Display", key) {
            *field = b;
        }
    }
    // Global, View Preferences (spec 24 2d.2).
    if let Some(b) = get_bool("Display", "SpellCodedView") {
        s.verbose_labels = b;
    }
    let p = &mut s.view_prefs;
    if let Some(n) = get_number("Display", "BellPosition") {
        p.bell_position = n;
    }
    let prefs: [(&str, &mut bool); 4] = [
        ("ShowLineNumbers", &mut p.line_numbers),
        ("CaretLocOnCreate", &mut p.locator_on_open),
        ("CaretLocOnViewSwitch", &mut p.locator_on_view_switch),
        ("CaretLocOnShift", &mut p.locator_on_shift),
    ];
    for (key, field) in prefs {
        if let Some(b) = get_bool("Display", key) {
            *field = b;
        }
    }
    if let Some(i) = get("Display", "TranslatedLineCodes")
        .and_then(|v| LineCodes::WORDS.iter().position(|w| w.eq_ignore_ascii_case(v.trim())))
    {
        p.line_codes = LineCodes::ALL[i];
    }
    let f = &mut s.fonts;
    if let Some(v) = get_text("Fonts", "PrintFont") {
        f.print_face = v;
    }
    if let Some(v) = get_text("Fonts", "StatusLineFont") {
        f.status_face = v;
    }
    let sizes: [(&str, &mut i32); 3] = [
        ("PrintDisplaySize", &mut f.print_size),
        ("BrailleDisplaySize", &mut f.braille_size),
        ("StatusLineFontSize", &mut f.status_size),
    ];
    for (key, field) in sizes {
        if let Some(n) = get_number("Fonts", key).filter(|n| (6..=72).contains(n)) {
            *field = n as i32;
        }
    }
    if let Some(b) = get_bool("Fonts", "PrintAutoSize") {
        f.print_auto = b;
    }
    let output: [(&str, &mut i32); 2] = [("PrintOutputSize", &mut f.print_output_size), ("BrailleOutputSize", &mut f.braille_output_size)];
    for (key, field) in output {
        if let Some(n) = get_number("Fonts", key).filter(|n| (1..=72).contains(n)) {
            *field = n as i32;
        }
    }
    if let Some(v) = get_text(crate::print::PrinterSetup::SECTION, "Setup") {
        if let Some(p) = crate::print::PrinterSetup::from_text(&v) {
            s.printer = p;
        }
    }
    let k = &mut s.shortcuts;
    let codes: [(&str, &mut String); 6] = [
        ("Grade0Code", &mut k.grade0),
        ("FlushRightCode", &mut k.flush_right_leader),
        ("ComputerBrailleCode", &mut k.cbc),
        ("NewLineCode", &mut k.new_line),
        ("NewPageCode", &mut k.new_page),
        ("NewRefPageCode", &mut k.new_ref_page),
    ];
    for (key, field) in codes {
        if let Some(v) = get(Shortcuts::SECTION, key) {
            *field = v;
        }
    }
    if let Some(n) = get(Shortcuts::SECTION, "PageNav").and_then(|v| Shortcuts::page_nav_number(&v)) {
        k.page_nav = n;
    }
    if let Some(n) = get(Shortcuts::SECTION, "ControlPageNav").and_then(|v| Shortcuts::page_nav_number(&v)) {
        k.control_page_nav = n;
    }
    if let Some(n) = get_number("Accessibility", "UIAutomation") {
        s.reader.uia = n != 0;
    }
    if let Some(b) = get_bool("Display", "PreferBraillePatterns") {
        s.prefer_patterns = b;
    }
    if let Some(b) = get_bool("Display", "AccessUseBraillePatterns") {
        s.access_patterns = b;
    }
}

/// The region Document, Translation Tables lists, from an earlier run.
pub fn load_region() -> Option<u32> {
    get_number(ROOT, "TranslationRegion").and_then(|n| u32::try_from(n).ok()).filter(|n| *n != 0)
}

pub fn save_region(region: u32) {
    set(ROOT, "TranslationRegion", &region.to_string());
}

/// The template File, New selects (Mark template as default); none when no default has been chosen.
pub fn load_default_template() -> Option<String> {
    get_text(ROOT, "DefaultTemplate")
}

pub fn save_default_template(name: &str) {
    set(ROOT, "DefaultTemplate", name);
}

/// File, New's Show Hidden Templates box, kept as the original keeps it (ShowAllTemplates, yes or no).
pub fn load_show_all_templates() -> bool {
    get(ROOT, "ShowAllTemplates").is_some_and(|v| v.trim().eq_ignore_ascii_case("yes"))
}

pub fn save_show_all_templates(on: bool) {
    set(ROOT, "ShowAllTemplates", yes_no(on));
}

/// The templates hidden from File, New (Hide Template), kept as one value with the names separated by "|".
pub fn load_hidden_templates() -> Vec<String> {
    get(ROOT, "HiddenTemplates")
        .map(|v| v.split('|').map(str::trim).filter(|n| !n.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

pub fn save_hidden_templates(names: &[String]) {
    set(ROOT, "HiddenTemplates", &names.join("|"));
}

/// Global, Default Views and Global, View Preferences (the six-key set).
pub fn save_default_views(d: &DefaultViews) {
    set("Display", "dftrans", &view_font_number(d.translated_font).to_string());
    set("Display", "dfbraille", &view_font_number(d.braille_font).to_string());
    set("Display", "PrintShowFullPage", yes_no(d.print_full_page));
    set("Display", "PrintDisplayCodes", yes_no(d.print_codes));
    set("Display", "PrintShowTranslationLine", yes_no(d.print_translated));
    set("Display", "BrailleShowFullPage", yes_no(d.braille_full_page));
    set("Display", "BrailleDisplayCodes", yes_no(d.braille_codes));
    set("Display", "BrailleShowTranslationLine", yes_no(d.braille_translated));
    set("Display", "SixKeyBraille", yes_no(d.six_key));
}

/// Global, View Preferences, under the reference program's names and words (spec 24 2d.2).
pub fn save_view_prefs(verbose: bool, p: &ViewPrefs) {
    set("Display", "BellPosition", &p.bell_position.to_string());
    set("Display", "SpellCodedView", yes_no(verbose));
    set("Display", "ShowLineNumbers", yes_no(p.line_numbers));
    set("Display", "CaretLocOnCreate", yes_no(p.locator_on_open));
    set("Display", "CaretLocOnViewSwitch", yes_no(p.locator_on_view_switch));
    set("Display", "CaretLocOnShift", yes_no(p.locator_on_shift));
    set("Display", "TranslatedLineCodes", LineCodes::WORDS[p.line_codes.index()]);
}

pub fn save_six_key_set(keys: &str) {
    set(ROOT, "ChordKeys", keys);
}

/// Global, Fonts.
pub fn save_fonts(f: &Fonts) {
    set("Fonts", "PrintFont", &f.print_face);
    set("Fonts", "PrintDisplaySize", &f.print_size.to_string());
    set("Fonts", "BrailleDisplaySize", &f.braille_size.to_string());
    set("Fonts", "StatusLineFont", &f.status_face);
    set("Fonts", "StatusLineFontSize", &f.status_size.to_string());
    set("Fonts", "PrintAutoSize", if f.print_auto { "yes" } else { "no" });
    set("Fonts", "PrintOutputSize", &f.print_output_size.to_string());
    set("Fonts", "BrailleOutputSize", &f.braille_output_size.to_string());
}

/// Global, Printer Setup.
pub fn save_printer(p: &crate::print::PrinterSetup) {
    set(crate::print::PrinterSetup::SECTION, "Setup", &p.to_text());
}

/// Global, Shortcut Preferences.
pub fn save_shortcuts(k: &Shortcuts) {
    let s = Shortcuts::SECTION;
    set(s, "Grade0Code", &k.grade0);
    set(s, "FlushRightCode", &k.flush_right_leader);
    set(s, "ComputerBrailleCode", &k.cbc);
    set(s, "NewLineCode", &k.new_line);
    set(s, "NewPageCode", &k.new_page);
    set(s, "NewRefPageCode", &k.new_ref_page);
    set(s, "PageNav", Shortcuts::PAGE_NAV[k.page_nav.min(2)]);
    set(s, "ControlPageNav", Shortcuts::PAGE_NAV[k.control_page_nav.min(2)]);
}

/// The code page text files are imported in, as the import dialog last chose it.
pub fn load_code_page() -> Option<String> {
    get_text(ROOT, "CodePage")
}

pub fn save_code_page(name: &str) {
    set(ROOT, "CodePage", name);
}

/// Global, Internationalization: the braille encoding, Support UI Automation, and the two Unicode braille pattern
/// choices (braille display, braille reported to screen readers).
pub fn save_internationalization(encoding: &str, uia: bool, prefer_patterns: bool, access_patterns: bool) {
    set(ROOT, "BrailleEncoding", encoding);
    set("Accessibility", "UIAutomation", if uia { "1" } else { "0" });
    set("Display", "PreferBraillePatterns", yes_no(prefer_patterns));
    set("Display", "AccessUseBraillePatterns", yes_no(access_patterns));
}

/// The answer kept for a message or question with a "Do not show (or ask) this again" box: the answer plus one (so 0
/// means none is kept), in section `section` under `key`. None when no answer is kept.
pub fn kept_answer(section: &str, key: &str) -> Option<u32> {
    get_number(section, key).filter(|n| *n > 0).map(|n| (n - 1) as u32)
}

pub fn keep_answer(section: &str, key: &str, answer: u32) {
    set(section, key, &(answer + 1).to_string());
}

/// The Word importer's Unknown Characters setting, which coded text import uses as well: its list words by position
/// ("ignore" 0 marks with "*", "asterisc" 1 writes nothing, "hex" 2 the Unicode value, as the original program reads
/// them); 0 when it is not kept.
pub fn load_unknown_characters() -> u8 {
    get("Word", "UnknownCharacters")
        .and_then(|v| ["ignore", "asterisc", "hex"].iter().position(|w| w.eq_ignore_ascii_case(v.trim())))
        .unwrap_or(0) as u8
}

const BRAIDE: &str = "FormattedBrailleImporter";
/// Where page numbers stand, as the settings name them (0 none to 4 lower right).
const CORNERS: [&str; 5] = ["NoPages", "UpperLeft", "UpperRight", "LowerLeft", "LowerRight"];
/// Which pages have a running head or foot, as the settings name them (0 none to 3 both).
const SIDES: [&str; 4] = ["None", "Odd", "Even", "Both"];

/// Global, Formatted Braille Importer, under the names the reference program keeps them by; a value that is missing
/// or not understood keeps its default (both main page numbers lower right, both reference page numbers upper right,
/// no running head or foot, 30 lines of 40 cells, formatting not preserved).
pub fn load_braide() -> bc_core::braide::Settings {
    let mut s = bc_core::braide::Settings::default();
    let word = |key: &str, list: &[&str]| {
        get(BRAIDE, key).and_then(|v| list.iter().position(|w| w.eq_ignore_ascii_case(v.trim()))).map(|i| i as i32)
    };
    for (key, field) in [
        ("OddMainPages", &mut s.odd_main),
        ("EvenMainPages", &mut s.even_main),
        ("OddReferencePages", &mut s.odd_reference),
        ("EvenReferencePages", &mut s.even_reference),
    ] {
        if let Some(v) = word(key, &CORNERS) {
            *field = v;
        }
    }
    if let Some(v) = word("Header", &SIDES) {
        s.header = v;
    }
    if let Some(v) = word("Footer", &SIDES) {
        s.footer = v;
    }
    if let Some(n) = get_number(BRAIDE, "MaxLen").filter(|n| *n > 0) {
        s.lines = n as usize;
    }
    if let Some(n) = get_number(BRAIDE, "MaxWidth").filter(|n| *n > 0) {
        s.cells = n as usize;
    }
    if let Some(b) = get_bool(BRAIDE, "PreserveFormatting") {
        s.preserve = b;
    }
    s
}

pub fn save_braide(s: &bc_core::braide::Settings) {
    let corner = |v: i32| CORNERS.get(v as usize).copied().unwrap_or(CORNERS[0]);
    let side = |v: i32| SIDES.get(v as usize).copied().unwrap_or(SIDES[0]);
    set(BRAIDE, "OddMainPages", corner(s.odd_main));
    set(BRAIDE, "EvenMainPages", corner(s.even_main));
    set(BRAIDE, "OddReferencePages", corner(s.odd_reference));
    set(BRAIDE, "EvenReferencePages", corner(s.even_reference));
    set(BRAIDE, "Header", side(s.header));
    set(BRAIDE, "Footer", side(s.footer));
    set(BRAIDE, "MaxLen", &s.lines.to_string());
    set(BRAIDE, "MaxWidth", &s.cells.to_string());
    set(BRAIDE, "PreserveFormatting", if s.preserve { "yes" } else { "no" });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_text_round_trips() {
        let mut s = FileStore::default();
        s.set(ROOT, "TranslationRegion", "2");
        s.set(ROOT, "BrailleEncoding", "german");
        s.set("Display", "BrailleShowTranslationLine", "yes");
        s.set("Shortcuts", "Grade0Code", "[cz]");
        s.set("AddIns", "jaws-68e0a1b2", "2");
        let text = s.text();
        assert!(text.find("TranslationRegion=2").unwrap() < text.find("[Display]").unwrap());
        assert_eq!(FileStore::parse(&text), s);
    }

    #[test]
    fn a_missing_file_gives_nothing_and_the_defaults_stay() {
        let s = FileStore::parse("");
        assert!(s.sections.is_empty());
        assert_eq!(s.get("Display", "SixKeyBraille"), None);
    }

    #[test]
    fn a_damaged_line_is_skipped() {
        let s = FileStore::parse("# BrailleCraft settings\nChordKeys=asdjkl\nthis line is damaged\n=no key\n[Fonts\n[Fonts]\nPrintFont=Courier New\n");
        assert_eq!(s.get(ROOT, "ChordKeys"), Some("asdjkl"));
        assert_eq!(s.get("Fonts", "PrintFont"), Some("Courier New"));
        assert_eq!(s.sections.values().map(|v| v.len()).sum::<usize>(), 2);
    }

    #[test]
    fn values_keep_equals_signs_and_codes() {
        let s = FileStore::parse("[Shortcuts]\nFlushRightCode=[fr;p~\"]\nX=a=b\n");
        assert_eq!(s.get("Shortcuts", "FlushRightCode"), Some("[fr;p~\"]"));
        assert_eq!(s.get("Shortcuts", "X"), Some("a=b"));
    }
}
