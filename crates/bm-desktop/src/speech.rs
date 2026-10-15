//! Screen reader support for the document windows.
//!
//! The editing area is the system's own multi-line text control, so every screen reader reads it through the
//! system text interfaces. On top of that the document window has reading keys of its own, which work the same with
//! every screen reader because the program speaks through the system announcement service:
//!
//! - Alt+7 speaks the translated line; Alt+8 the current style; Alt+9 page, line and column; Alt+comma the
//!   position on the line (twice quickly: characters to the end); Alt+period the characters on the line (twice
//!   quickly: the left spaces); Alt+End the last word on the line. Alt+0 stays the menu shortcut for Grade 0.
//! - Alt+U cycles what is spoken when moving up or down in a braille document (the translated line, the line as
//!   braille cells, nothing more); Alt+I cycles the announcement of style changes and skipped lines; Alt+P cycles
//!   the echo for braille entry; Alt+W turns the echo of completed braille words on or off.
//! - Alt+semicolon moves the focus to the translated line, and back.
//!
//! Announcements go to UI Automation notifications on Windows, the AT-SPI notification signal on Linux and the
//! accessibility announcement notification on macOS. With BM_SPEECH_LOG set in the environment each announcement
//! is also written to standard error, for checking the program without a screen reader.

use std::ffi::c_void;

/// What is spoken, beyond the line itself, when moving up or down in a braille document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrailleLines {
    Translated,
    Cells,
    Silent,
}

#[derive(Clone, Debug)]
pub struct ReaderSettings {
    /// Global, Internationalization: Support UI Automation.
    pub uia: bool,
    pub braille_lines: BrailleLines,
    pub style_changes: bool,
    pub skipped_lines: bool,
    /// Echo for braille entry: 0 same as typing echo, 1 silent, 2 characters, 3 words, 4 characters and words.
    pub entry_echo: u8,
    pub completed_words: bool,
    /// Speak braille characters by their braille sign names ("a n d sign") rather than as plain characters.
    pub local_signs: bool,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        ReaderSettings {
            uia: true,
            braille_lines: BrailleLines::Translated,
            style_changes: false,
            skipped_lines: false,
            entry_echo: 0,
            completed_words: false,
            local_signs: true,
        }
    }
}

// ------------------------------------------------------------------ wording

pub const BLANK: &str = "blank";
pub const NO_LOCATION: &str = "location not available in coded view. press Alt+F3 to turn codes off";
pub const CODED_VIEW: &str = "coded view";
pub const TRANSLATED_LINE_ON: &str = "translated line on";
pub const TURNING_ON_TRANSLATED_LINE: &str = "turning on translated line";
pub const NONE: &str = "None";

impl ReaderSettings {
    /// Alt+U: translated line, then braille characters, then silent, then the translated line again.
    pub fn cycle_braille_lines(&mut self) -> &'static str {
        self.braille_lines = match self.braille_lines {
            BrailleLines::Translated => BrailleLines::Cells,
            BrailleLines::Cells => BrailleLines::Silent,
            BrailleLines::Silent => BrailleLines::Translated,
        };
        match self.braille_lines {
            BrailleLines::Translated => "use translated line for up and down in braille",
            BrailleLines::Cells => "read line as braille characters for up and down in braille",
            BrailleLines::Silent => "silent for up and down in braille",
        }
    }

    /// Alt+I: style changes on, then skipped lines on as well, then style changes off, then both off.
    pub fn cycle_style_announcements(&mut self) -> String {
        match (self.style_changes, self.skipped_lines) {
            (false, false) => self.style_changes = true,
            (true, false) => self.skipped_lines = true,
            (true, true) => self.style_changes = false,
            (false, true) => self.skipped_lines = false,
        }
        format!(
            "Announcements: change of style {}. Skipped lines {}.",
            if self.style_changes { "on" } else { "off" },
            if self.skipped_lines { "on" } else { "off" }
        )
    }

    /// Alt+P: the echo for braille entry, in turn.
    pub fn cycle_entry_echo(&mut self) -> String {
        self.entry_echo = (self.entry_echo + 1) % 5;
        let names = ["same as typing echo.", "silent.", "characters.", "words.", "characters and words."];
        format!("Echo for Braille entry is {}", names[self.entry_echo as usize])
    }

    /// Alt+W: the echo of the completed braille word when space is pressed.
    pub fn toggle_completed_words(&mut self) -> &'static str {
        self.completed_words = !self.completed_words;
        if self.completed_words { "echo completed braille words on" } else { "echo completed braille words off" }
    }

    pub fn echo_characters(&self) -> bool {
        matches!(self.entry_echo, 2 | 4)
    }

    pub fn echo_words(&self) -> bool {
        matches!(self.entry_echo, 3 | 4) || self.completed_words
    }
}

// ------------------------------------------------------------------ braille cells by name

/// Names of the braille cells: ASCII braille in the North American encoding (space to underscore, letters, and the
/// lower-case forms of the signs that share a cell), Russian braille letters, and Unicode braille patterns.
const ASCII_32: &str = "space;t h e sign;dot 5;number sign;e d sign;s h sign;a n d sign;dot 3;o f sign;w i t h sign;c h sign;i n g sign;dot 6;dots 3 6;dots 4 6;s t sign";
const ASCII_48: &str = "low j;low a;low b;low c;low d;low e;low f;low g;low h;low i;w h sign;dots 5 6;g h sign;f o r sign;a r sign;t h sign;dot 4";
const ASCII_91: &str = "o w sign;o u sign;e r sign;dots 4 5;dots 4 5 6;dot 4;a;b;c;d;e;f;g;h;i;j";
const ASCII_107: &str = "k;l;m;n;o;p;q;r;s;t;u;v;w;x;y;z";
const ASCII_123: &str = "dots 4 6;dots 4 5 6;dot 6;dots 1 2 4 5 6;hard space";
const RUSSIAN_1072: &str = "dot 1;dots 1 2;dots 2 4 5 6;dots 1 2 4 5;dots 1 4 5;dots 1 5;dots 2 4 5;dots 1 3 5 6;dots 2 4;dots 1 2 3 4 6;dots 1 3;dots 1 2 3;dots 1 3 4;dots 1 3 4 5;dots 1 3 5;dots 1 2 3 4";
const RUSSIAN_1088: &str = "dots 1 2 3 5;dots 2 3 4;dots 2 3 4 5;dots 1 3 6;dots 1 2 4;dots 1 2 5;dots 1 4;dots 1 2 3 4 5;dots 1 5 6;dots 1 3 4 6;dots 1 2 3 5 6;dots 2 3 4 6;dots 2 3 4 5 6;dots 2 4 6;dots 1 2 5 6;dots 1 2 4 6;dots 1 6";
const UNICODE_10240: &str = "space;a;low a;b;dot 3;k;low b;l;dot 4;c;i;f;s t sign;m;s;p";
const UNICODE_10256: &str = "dot 5;e;low c;h;low i;o;low f;r;dots 4 5;d;j;g;a r sign;n;t;q";
const UNICODE_10272: &str = "dot 6;c h sign;low e;g h sign;hyphen;u;low h;v;dots 4 6;s h sign;o w sign;e d sign;i n g sign;x;t h e sign;a n d sign";
const UNICODE_10288: &str = "dots 5 6;w h sign;low d;o u sign;low j;z;low g;o f sign;dots 4 5 6;t h sign;w;e r sign;number sign;y;w i t h sign;f o r sign";

fn nth(table: &str, i: u32) -> Option<String> {
    table.split(';').nth(i as usize).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The spoken name of one braille cell, or None for a character outside the braille encodings.
pub fn cell_name(c: char) -> Option<String> {
    let v = c.to_ascii_lowercase() as u32;
    match v {
        0x1E | 0xA0 => nth(ASCII_123, 4),
        32..=47 => nth(ASCII_32, v - 32),
        48..=64 => nth(ASCII_48, v - 48),
        91..=106 => nth(ASCII_91, v - 91),
        107..=122 => nth(ASCII_107, v - 107),
        123..=127 => nth(ASCII_123, v - 123),
        1072..=1087 => nth(RUSSIAN_1072, v - 1072),
        1088..=1105 => nth(RUSSIAN_1088, v - 1088),
        10240..=10255 => nth(UNICODE_10240, v - 10240),
        10256..=10271 => nth(UNICODE_10256, v - 10256),
        10272..=10287 => nth(UNICODE_10272, v - 10272),
        10288..=10303 => nth(UNICODE_10288, v - 10288),
        _ => None,
    }
}

/// A run of braille spoken cell by cell; two or more spaces in a row are spoken as a count ("3 spaces").
pub fn speak_cells(text: &str, local_signs: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ' ' && chars.get(i + 1) == Some(&' ') {
            let run = chars[i..].iter().take_while(|&&c| c == ' ').count();
            parts.push(format!("{run} spaces"));
            i += run;
            continue;
        }
        let c = chars[i];
        parts.push(if local_signs { cell_name(c).unwrap_or_else(|| c.to_string()) } else { c.to_string() });
        i += 1;
    }
    parts.join(", ")
}

/// A line spoken as braille cells; a blank line is "blank".
pub fn speak_line_as_cells(line: &str, local_signs: bool) -> String {
    let trimmed = line.trim_end();
    if trimmed.trim().is_empty() {
        BLANK.to_string()
    } else {
        speak_cells(trimmed, local_signs)
    }
}

// ------------------------------------------------------------------ positions on the line

/// Alt+comma: the position of the caret on the line (1-based); pressed twice quickly, the characters after it.
pub fn line_position(line: &str, column: usize, twice: bool) -> String {
    let full = line.chars().count() as i64;
    let to_end = full - column as i64 + 1;
    if twice {
        format!("{} to last nonspace character", (to_end - 1).max(0))
    } else if to_end == 0 {
        format!("position {column} end of line")
    } else {
        format!("position {column}")
    }
}

/// Alt+period: the characters on the line; pressed twice quickly, the spaces at its start.
pub fn line_length(line: &str, twice: bool) -> String {
    let full = line.chars().count();
    if twice {
        let text = line.trim_start().chars().count();
        format!("{} left spaces", full - text)
    } else {
        format!("{full} total characters.")
    }
}

/// Alt+End: the last word of the line (the part after the last space or hyphen).
pub fn last_word(line: &str) -> &str {
    line.rsplit([' ', '-']).next().unwrap_or("")
}

/// Alt+9: "P1 L3 C12".
pub fn location(page: usize, line: usize, column: usize) -> String {
    format!("P{page} L{line} C{column}")
}

// ------------------------------------------------------------------ announcing

/// Speaks `text` through the system announcement service. `widget` is the native handle of the focused control
/// (window handle on Windows, GTK widget on Linux, view on macOS). `interrupt` is for what the user asked to hear;
/// announcements that follow a caret movement do not interrupt.
pub fn announce(widget: *mut c_void, text: &str, interrupt: bool) {
    if text.is_empty() {
        return;
    }
    if std::env::var_os("BM_SPEECH_LOG").is_some() {
        eprintln!("[speech{}] {text}", if interrupt { "!" } else { "" });
    }
    imp::announce(widget, text, interrupt);
}

/// Gives a control its accessible name on Linux, where wxWidgets does not pass one to the accessibility interface
/// (on Windows and macOS the control's accessibility label does that).
pub fn set_name(widget: *mut c_void, name: &str) {
    imp::set_name(widget, name);
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[link(name = "uiautomationcore")]
    extern "system" {
        fn UiaHostProviderFromHwnd(hwnd: *mut c_void, provider: *mut *mut c_void) -> i32;
        fn UiaRaiseNotificationEvent(
            provider: *mut c_void,
            kind: i32,
            processing: i32,
            display: *const u16,
            activity: *const u16,
        ) -> i32;
    }

    #[link(name = "oleaut32")]
    extern "system" {
        fn SysAllocString(text: *const u16) -> *mut u16;
        fn SysFreeString(text: *mut u16);
    }

    /// NotificationKind_Other; NotificationProcessing_ImportantMostRecent or NotificationProcessing_All.
    const KIND_OTHER: i32 = 4;
    const IMPORTANT_MOST_RECENT: i32 = 1;
    const ALL: i32 = 2;

    pub fn announce(hwnd: *mut c_void, text: &str, interrupt: bool) {
        if hwnd.is_null() {
            return;
        }
        unsafe {
            let mut provider: *mut c_void = std::ptr::null_mut();
            if UiaHostProviderFromHwnd(hwnd, &mut provider) < 0 || provider.is_null() {
                return;
            }
            let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            let id: Vec<u16> = "BrailleMaker".encode_utf16().chain(Some(0)).collect();
            let display = SysAllocString(wide.as_ptr());
            let activity = SysAllocString(id.as_ptr());
            UiaRaiseNotificationEvent(
                provider,
                KIND_OTHER,
                if interrupt { IMPORTANT_MOST_RECENT } else { ALL },
                display,
                activity,
            );
            SysFreeString(display);
            SysFreeString(activity);
            // IUnknown::Release, the third entry of the interface table.
            let table = *(provider as *const *const usize);
            let release: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(*table.add(2));
            release(provider);
        }
    }

    pub fn set_name(_hwnd: *mut c_void, _name: &str) {}
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSAccessibilityAnnouncementRequestedNotification: *mut c_void;
        static NSAccessibilityAnnouncementKey: *mut c_void;
        static NSAccessibilityPriorityKey: *mut c_void;
        fn NSAccessibilityPostNotificationWithUserInfo(element: *mut c_void, notification: *mut c_void, info: *mut c_void);
    }

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    /// NSAccessibilityPriorityHigh and NSAccessibilityPriorityMedium.
    const PRIORITY_HIGH: isize = 90;
    const PRIORITY_MEDIUM: isize = 50;

    unsafe fn class(name: &str) -> *mut c_void {
        let n = CString::new(name).unwrap_or_default();
        objc_getClass(n.as_ptr())
    }

    unsafe fn sel(name: &str) -> *mut c_void {
        let n = CString::new(name).unwrap_or_default();
        sel_registerName(n.as_ptr())
    }

    pub fn announce(_view: *mut c_void, text: &str, interrupt: bool) {
        let Ok(c_text) = CString::new(text) else { return };
        unsafe {
            let send: unsafe extern "C" fn() = objc_msgSend;
            let send_none: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(send);
            let send_str: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void =
                std::mem::transmute(send);
            let send_int: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void = std::mem::transmute(send);
            let send_dict: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *const *mut c_void,
                *const *mut c_void,
                usize,
            ) -> *mut c_void = std::mem::transmute(send);

            let app = send_none(class("NSApplication"), sel("sharedApplication"));
            let string = send_str(class("NSString"), sel("stringWithUTF8String:"), c_text.as_ptr());
            let priority = send_int(
                class("NSNumber"),
                sel("numberWithInteger:"),
                if interrupt { PRIORITY_HIGH } else { PRIORITY_MEDIUM },
            );
            if app.is_null() || string.is_null() || priority.is_null() {
                return;
            }
            let values = [string, priority];
            let keys = [NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey];
            let info = send_dict(
                class("NSDictionary"),
                sel("dictionaryWithObjects:forKeys:count:"),
                values.as_ptr(),
                keys.as_ptr(),
                2,
            );
            NSAccessibilityPostNotificationWithUserInfo(app, NSAccessibilityAnnouncementRequestedNotification, info);
        }
    }

    pub fn set_name(_view: *mut c_void, _name: &str) {}
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "gtk-3")]
    extern "C" {
        fn gtk_widget_get_toplevel(widget: *mut c_void) -> *mut c_void;
        fn gtk_window_get_focus(window: *mut c_void) -> *mut c_void;
        fn gtk_widget_get_accessible(widget: *mut c_void) -> *mut c_void;
        fn gtk_widget_is_toplevel(widget: *mut c_void) -> i32;
        fn gtk_bin_get_type() -> usize;
        fn gtk_bin_get_child(bin: *mut c_void) -> *mut c_void;
    }

    #[link(name = "atk-1.0")]
    extern "C" {
        fn atk_object_get_type() -> usize;
        fn atk_object_set_name(accessible: *mut c_void, name: *const c_char);
    }

    #[link(name = "gobject-2.0")]
    extern "C" {
        fn g_signal_lookup(name: *const c_char, itype: usize) -> u32;
        fn g_signal_emit_by_name(instance: *mut c_void, name: *const c_char, ...);
        fn g_type_check_instance_is_a(instance: *mut c_void, itype: usize) -> i32;
    }

    /// Names the widget and, for a container such as the scrolled window around a multi-line text view, the widget
    /// inside it, which is the one that takes the focus.
    pub fn set_name(widget: *mut c_void, name: &str) {
        if widget.is_null() {
            return;
        }
        let Ok(c_name) = CString::new(name) else { return };
        unsafe {
            let mut targets = vec![widget];
            if g_type_check_instance_is_a(widget, gtk_bin_get_type()) != 0 {
                let child = gtk_bin_get_child(widget);
                if !child.is_null() {
                    targets.push(child);
                }
            }
            for w in targets {
                let accessible = gtk_widget_get_accessible(w);
                if !accessible.is_null() {
                    atk_object_set_name(accessible, c_name.as_ptr());
                }
            }
        }
    }

    /// AtkLive: ATK_LIVE_POLITE and ATK_LIVE_ASSERTIVE.
    const POLITE: i32 = 1;
    const ASSERTIVE: i32 = 2;

    pub fn announce(widget: *mut c_void, text: &str, interrupt: bool) {
        if widget.is_null() {
            return;
        }
        let (Ok(c_text), Ok(signal)) = (CString::new(text), CString::new("notification")) else { return };
        unsafe {
            // The "notification" signal of accessible objects (ATK 2.50 and later), which the AT-SPI bridge passes
            // to screen readers as an announcement.
            if g_signal_lookup(signal.as_ptr(), atk_object_get_type()) == 0 {
                return;
            }
            let mut target = widget;
            let top = gtk_widget_get_toplevel(widget);
            if !top.is_null() && gtk_widget_is_toplevel(top) != 0 {
                let focus = gtk_window_get_focus(top);
                if !focus.is_null() {
                    target = focus;
                }
            }
            let accessible = gtk_widget_get_accessible(target);
            if accessible.is_null() {
                return;
            }
            g_signal_emit_by_name(accessible, signal.as_ptr(), c_text.as_ptr(), if interrupt { ASSERTIVE } else { POLITE });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_by_name() {
        assert_eq!(cell_name('a').as_deref(), Some("a"));
        assert_eq!(cell_name('A').as_deref(), Some("a"));
        assert_eq!(cell_name('&').as_deref(), Some("a n d sign"));
        assert_eq!(cell_name('4').as_deref(), Some("low d"));
        assert_eq!(cell_name(',').as_deref(), Some("dot 6"));
        assert_eq!(cell_name('\u{2801}').as_deref(), Some("a"));
        assert_eq!(cell_name('\u{283F}').as_deref(), Some("f o r sign"));
        assert_eq!(speak_cells(",!   c", true), "dot 6, t h e sign, 3 spaces, c");
        assert_eq!(speak_line_as_cells("   ", true), "blank");
    }

    #[test]
    fn line_positions() {
        assert_eq!(line_position("abc", 2, false), "position 2");
        assert_eq!(line_position("abc", 4, false), "position 4 end of line");
        assert_eq!(line_position("abc", 1, true), "2 to last nonspace character");
        assert_eq!(line_position("abc", 4, true), "0 to last nonspace character");
        assert_eq!(line_length("  abc", false), "5 total characters.");
        assert_eq!(line_length("  abc", true), "2 left spaces");
        assert_eq!(last_word("one two-three"), "three");
        assert_eq!(location(1, 3, 12), "P1 L3 C12");
    }

    #[test]
    fn setting_cycles() {
        let mut s = ReaderSettings::default();
        assert_eq!(s.cycle_braille_lines(), "read line as braille characters for up and down in braille");
        assert_eq!(s.cycle_braille_lines(), "silent for up and down in braille");
        assert_eq!(s.cycle_braille_lines(), "use translated line for up and down in braille");
        assert_eq!(s.cycle_style_announcements(), "Announcements: change of style on. Skipped lines off.");
        assert_eq!(s.cycle_style_announcements(), "Announcements: change of style on. Skipped lines on.");
        assert_eq!(s.cycle_style_announcements(), "Announcements: change of style off. Skipped lines on.");
        assert_eq!(s.cycle_style_announcements(), "Announcements: change of style off. Skipped lines off.");
        assert_eq!(s.cycle_entry_echo(), "Echo for Braille entry is silent.");
        assert!(!s.echo_characters());
        s.cycle_entry_echo();
        assert!(s.echo_characters());
        assert_eq!(s.toggle_completed_words(), "echo completed braille words on");
        assert!(s.echo_words());
    }
}
