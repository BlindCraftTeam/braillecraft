//! Six-key entry (View, Six-Key Entry, command 1307; spec 30 6f, spec 04f): six keys of the keyboard pressed
//! together make a braille cell. The rules are shared by the desktop program and the web build.
//!
//! A key is named by its place on the keyboard: the character that place gives on the US layout ("f" for the key
//! that types f there), so a chord keeps its keys on another layout. The six keys of a set, as typed in Global,
//! View Preferences, are taken as US places (INFERENCE: the layout the set was typed in is not known to BrailleCraft;
//! the US layout is the set's default layout).
//!
//! While chording is on, a press of one of the six keys adds its dot and types nothing; the cell is typed when the
//! first chord key is let go (the keys let go after it find no chord). Space pressed during a chord types the cell at
//! once and a space when the chord key comes up. A held chord key types the cell again on each repeat. Every other
//! letter key does nothing while chording is on.

/// The six keys when none are set, dots 1 to 6.
pub const DEFAULT_SET: &str = "fdsjkl";

/// Message 2768: a six-key set refused in Global, View Preferences.
pub const SET_PROBLEM: &str = concat!(
    "You must type exactly six keys in as the \"Six-Key Key Set.\"  The keys you type represent, in order, what you ",
    "will type for Dots 1, 2, 3, 4, 5, and 6 when entering braille by six-key chording."
);

/// The keys a set may hold besides the letters, by their US places.
const PUNCTUATION: &str = ";,./[\\]'";

/// What Global, View Preferences says about a six-key set, or None when it is taken: exactly six keys, no two the
/// same (case ignored, the set is kept in small letters), each a letter or one of ; , . / [ \ ] '.
pub fn six_key_set_problem(keys: &str) -> Option<&'static str> {
    let k: Vec<char> = keys.chars().map(|c| c.to_ascii_lowercase()).collect();
    let allowed = k.iter().all(|c| c.is_ascii_lowercase() || PUNCTUATION.contains(*c));
    let distinct = (0..k.len()).all(|i| !k[..i].contains(&k[i]));
    (k.len() != 6 || !allowed || !distinct).then_some(SET_PROBLEM)
}

/// How keys are taken in a document: never chorded (a print document with six-key entry off), chorded with Shift
/// held (a braille document with six-key entry off), or chorded with Shift up (six-key entry on, either kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    ShiftChords,
    Chords,
}

impl Mode {
    /// The mode of a braille or print document with six-key entry on or off.
    pub fn of(braille: bool, six_key: bool) -> Mode {
        if six_key {
            Mode::Chords
        } else if braille {
            Mode::ShiftChords
        } else {
            Mode::Off
        }
    }
}

/// What a chord types: a cell (its dots, bit 0 dot 1) or a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typed {
    Cell(u8),
    Space,
}

/// What becomes of a key: it goes on as usual, it does nothing, or it types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyResult {
    Pass,
    Swallow,
    Type(Typed),
}

/// The chord being held: its dots, and whether Space typed the cell already (a space is then owed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    pub dots: u8,
    pub space_owed: bool,
}

impl Chord {
    /// A key pressed with Control and Alt up (keys with either always go on): `dot` its dot (0 to 5) when it is one
    /// of the six keys, `letter` when it is a letter key (A to Z by its US place), `repeat` for a key held down.
    pub fn key_down(&mut self, dot: Option<usize>, letter: bool, shift: bool, repeat: bool, mode: Mode) -> KeyResult {
        let chording = match mode {
            Mode::Off => false,
            Mode::ShiftChords => shift,
            Mode::Chords => !shift,
        };
        if !(dot.is_some() || letter) || !chording {
            return KeyResult::Pass;
        }
        if let Some(d) = dot {
            self.dots |= 1 << d;
        }
        if !repeat {
            return KeyResult::Swallow;
        }
        // A repeat types the cell again (or the space owed); the chord stays.
        let cell = (self.dots != 0).then_some(Typed::Cell(self.dots));
        let typed = if self.space_owed { Some(Typed::Space) } else { cell };
        self.space_owed = false;
        typed.map_or(KeyResult::Swallow, KeyResult::Type)
    }

    /// Space pressed: with a chord held it types the cell and owes a space; the chord stays. Else it goes on.
    pub fn space_down(&mut self) -> KeyResult {
        if self.dots == 0 {
            return KeyResult::Pass;
        }
        self.space_owed = true;
        KeyResult::Type(Typed::Cell(self.dots))
    }

    /// A key let go: one of the six keys with a chord held types the cell (or the space owed) and ends the chord;
    /// any other letter key, or a chord key with no chord, does nothing; other keys go on.
    pub fn key_up(&mut self, dot: Option<usize>, letter: bool) -> KeyResult {
        if !(dot.is_some() || letter) {
            return KeyResult::Pass;
        }
        if dot.is_none() || self.dots == 0 {
            return KeyResult::Swallow;
        }
        let typed = if self.space_owed { Typed::Space } else { Typed::Cell(self.dots) };
        *self = Chord::default();
        KeyResult::Type(typed)
    }
}

/// The dot of key `key` (its US place, in small letters) in the six-key set `set`.
pub fn dot_of(set: &str, key: char) -> Option<usize> {
    let key = key.to_ascii_lowercase();
    set.chars().map(|c| c.to_ascii_lowercase()).position(|c| c == key)
}

/// The US place of the key with PC scan code `scan` (set 1: Windows key messages; Linux evdev key codes, the X
/// key code less 8): the letters and the keys a set may hold.
pub fn key_of_scan_code(scan: u32) -> Option<char> {
    const ROWS: [(u32, &str); 3] = [(0x10, "qwertyuiop[]"), (0x1e, "asdfghjkl;'"), (0x2c, "zxcvbnm,./")];
    if scan == 0x2b {
        return Some('\\');
    }
    ROWS.iter().find_map(|&(first, keys)| scan.checked_sub(first).and_then(|k| keys.chars().nth(k as usize)))
}

/// The US place of the key with macOS virtual key code `code` (kVK_ANSI_...).
pub fn key_of_mac_code(code: u32) -> Option<char> {
    // 0x12 to 0x1d are the digit row.
    const KEYS: &str = "asdfhgzxcv\0bqweryt\0\0\0\0\0\0\0\0\0\0\0\0]ou[ip\0lj'k;\\,/nm.";
    KEYS.chars().nth(code as usize).filter(|&c| c != '\0')
}

/// The US place of the key a browser names `code` (KeyboardEvent.code).
pub fn key_of_browser_code(code: &str) -> Option<char> {
    if let Some(l) = code.strip_prefix("Key").filter(|l| l.len() == 1) {
        return l.chars().next().filter(char::is_ascii_uppercase).map(|c| c.to_ascii_lowercase());
    }
    Some(match code {
        "Semicolon" => ';',
        "Comma" => ',',
        "Period" => '.',
        "Slash" => '/',
        "BracketLeft" => '[',
        "Backslash" => '\\',
        "BracketRight" => ']',
        "Quote" => '\'',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: Option<usize> = Some(0);
    const D: Option<usize> = Some(1);

    #[test]
    fn chord_types_at_first_release() {
        let mut c = Chord::default();
        assert_eq!(c.key_down(F, true, false, false, Mode::Chords), KeyResult::Swallow);
        assert_eq!(c.key_down(D, true, false, false, Mode::Chords), KeyResult::Swallow);
        assert_eq!(c.key_up(D, true), KeyResult::Type(Typed::Cell(0b11)));
        assert_eq!(c.key_up(F, true), KeyResult::Swallow);
    }

    #[test]
    fn shift_passes_in_six_key_mode() {
        let mut c = Chord::default();
        assert_eq!(c.key_down(F, true, true, false, Mode::Chords), KeyResult::Pass);
        assert_eq!(c.dots, 0);
    }

    #[test]
    fn shift_chords_in_braille_without_six_key() {
        let mode = Mode::of(true, false);
        assert_eq!(mode, Mode::ShiftChords);
        let mut c = Chord::default();
        assert_eq!(c.key_down(F, true, true, false, mode), KeyResult::Swallow);
        assert_eq!(c.key_up(F, true), KeyResult::Type(Typed::Cell(1)));
        assert_eq!(c.key_down(F, true, false, false, mode), KeyResult::Pass);
    }

    #[test]
    fn print_document_without_six_key_never_chords() {
        let mode = Mode::of(false, false);
        let mut c = Chord::default();
        assert_eq!(c.key_down(F, true, false, false, mode), KeyResult::Pass);
        assert_eq!(c.key_down(F, true, true, false, mode), KeyResult::Pass);
        assert_eq!(Mode::of(false, true), Mode::Chords);
    }

    #[test]
    fn space_during_chord_types_cell_then_space() {
        let mut c = Chord::default();
        c.key_down(F, true, false, false, Mode::Chords);
        assert_eq!(c.space_down(), KeyResult::Type(Typed::Cell(1)));
        assert_eq!(c.key_up(F, true), KeyResult::Type(Typed::Space));
        assert_eq!(c, Chord::default());
    }

    #[test]
    fn lone_space_passes() {
        assert_eq!(Chord::default().space_down(), KeyResult::Pass);
    }

    #[test]
    fn repeat_types_again() {
        let mut c = Chord::default();
        c.key_down(F, true, false, false, Mode::Chords);
        assert_eq!(c.key_down(F, true, false, true, Mode::Chords), KeyResult::Type(Typed::Cell(1)));
        assert_eq!(c.dots, 1);
        assert_eq!(c.key_up(F, true), KeyResult::Type(Typed::Cell(1)));
    }

    #[test]
    fn other_letters_are_swallowed() {
        let mut c = Chord::default();
        assert_eq!(c.key_down(None, true, false, false, Mode::Chords), KeyResult::Swallow);
        assert_eq!(c.key_up(None, true), KeyResult::Swallow);
        assert_eq!(c.key_down(None, false, false, false, Mode::Chords), KeyResult::Pass);
    }

    #[test]
    fn six_key_set_problem_rules() {
        assert_eq!(six_key_set_problem("fdsjkl"), None);
        assert_eq!(six_key_set_problem("FDSJKL"), None);
        assert_eq!(six_key_set_problem("fdsjk"), Some(SET_PROBLEM));
        assert_eq!(six_key_set_problem("fdsjkk"), Some(SET_PROBLEM));
        assert_eq!(six_key_set_problem("fdsjkK"), Some(SET_PROBLEM));
        assert_eq!(six_key_set_problem("fdsjk1"), Some(SET_PROBLEM));
        assert_eq!(six_key_set_problem("a;s,d."), None);
    }

    #[test]
    fn key_places_by_scan_mac_and_browser_codes() {
        assert_eq!(key_of_scan_code(0x21), Some('f'));
        assert_eq!(key_of_scan_code(0x27), Some(';'));
        assert_eq!(key_of_scan_code(0x2b), Some('\\'));
        assert_eq!(key_of_scan_code(0x35), Some('/'));
        assert_eq!(key_of_scan_code(0x02), None);
        assert_eq!(key_of_mac_code(0x03), Some('f'));
        assert_eq!(key_of_mac_code(0x26), Some('j'));
        assert_eq!(key_of_mac_code(0x2f), Some('.'));
        assert_eq!(key_of_mac_code(0x0a), None);
        assert_eq!(key_of_browser_code("KeyF"), Some('f'));
        assert_eq!(key_of_browser_code("Quote"), Some('\''));
        assert_eq!(key_of_browser_code("Digit1"), None);
        assert_eq!(dot_of("fdsjkl", 'K'), Some(4));
    }
}
