//! Autosave's files (Global, File Locations): the index of the documents the last autosave pass wrote, and the
//! names of the files they are written in. The desktop program writes them in the autosave folder and reads the
//! index again at start-up, for the Autosave Recovery Wizard.
//!
//! The index (autosave.txt, written as autosave.new and renamed over it at the end of a pass) has one line per
//! document written, in window order:
//! - "n;saved;path" for a document with a document file (path is its full file name);
//! - "n;unsaved;path" for a document with a file name but no document file (an imported file);
//! - "n;untitled;b5" or "n;untitled;p5" for an untitled braille (b) or print (p) document and its untitled number.
//!
//! n is the document's autosave number: its file is bc00000.asv for 0, a document file in the current format.

/// The index the last pass wrote, and the one a pass writes before it is renamed over it.
pub const INDEX: &str = "autosave.txt";
pub const INDEX_NEW: &str = "autosave.new";

/// The file autosave number `number` is written in.
pub fn file_name(number: u32) -> String {
    format!("bc{number:05}.asv")
}

/// What an autosaved document was when it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Its document file's full name.
    Saved(String),
    /// The full name of the file it was imported from.
    Unsaved(String),
    /// An untitled document: braille or print, and its untitled number.
    Untitled { braille: bool, number: u32 },
}

/// One line of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub number: u32,
    pub origin: Origin,
}

impl Entry {
    /// The entry's line in the index, with its line end.
    pub fn line(&self) -> String {
        let n = self.number;
        match &self.origin {
            Origin::Saved(path) => format!("{n};saved;{path}\n"),
            Origin::Unsaved(path) => format!("{n};unsaved;{path}\n"),
            Origin::Untitled { braille, number } => format!("{n};untitled;{}{number}\n", if *braille { 'b' } else { 'p' }),
        }
    }
}

/// A whole number at the start of `s` after any blanks, as C's "%d" reads one, and the text after it.
pub fn leading_number(s: &str) -> Option<(i64, &str)> {
    let t = s.trim_start();
    let digits_at = usize::from(t.starts_with(['-', '+']));
    let end = t[digits_at..].find(|c: char| !c.is_ascii_digit()).map_or(t.len(), |e| e + digits_at);
    if end == digits_at {
        return None;
    }
    Some((t[..end].parse().ok()?, &t[end..]))
}

/// The index's entries in its order. A line that does not start with a number and ";", or whose word after it is not
/// saved, unsaved or untitled (in any case), is skipped.
pub fn parse_index(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        let Some((n, rest)) = leading_number(line) else { continue };
        let Some(rest) = rest.strip_prefix(';') else { continue };
        let Ok(number) = u32::try_from(n) else { continue };
        let Some((word, data)) = rest.split_once(';') else { continue };
        let origin = match word.to_ascii_lowercase().as_str() {
            "saved" => Origin::Saved(data.to_string()),
            "unsaved" => Origin::Unsaved(data.to_string()),
            "untitled" => {
                let braille = !data.starts_with(['p', 'P']);
                // INFERENCE: an untitled number that cannot be read is taken as 0; BrailleCraft never writes one.
                let number = data.get(1..).and_then(leading_number).map_or(0, |(n, _)| n.max(0) as u32);
                Origin::Untitled { braille, number }
            }
            _ => continue,
        };
        out.push(Entry { number, origin });
    }
    out
}

/// The lowest autosave number not in `used` (the current and previous numbers of every open document).
pub fn lowest_free(used: &[i32]) -> u32 {
    (0..).find(|n| !used.contains(&(*n as i32))).unwrap_or(0)
}

/// Whether a "saved" entry is offered for recovery: not when its document file is at least as new as its autosave
/// file (or the autosave file cannot be read); always when the document file cannot be read. The times are
/// `dos_stamp` values of the two files' modification times.
pub fn keep_saved(document: Option<u32>, autosave: Option<u32>) -> bool {
    match (document, autosave) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(d), Some(a)) => d < a,
    }
}

/// A time (seconds since 1970, UTC) as a file's date and time are compared: the date in the high 16 bits (years
/// from 1980, month, day) and the time in the low 16 (hours, minutes, seconds in twos), so two times in the same two
/// seconds are equal. None before 1980 or after 2107, which the form cannot hold.
pub fn dos_stamp(seconds: i64) -> Option<u32> {
    let days = seconds.div_euclid(86_400);
    let secs = seconds.rem_euclid(86_400);
    // Days since 1970 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    if !(1980..=2107).contains(&year) {
        return None;
    }
    let date = ((year - 1980) << 9) | (month << 5) | day;
    let time = ((secs / 3600) << 11) | ((secs / 60 % 60) << 5) | (secs % 60 / 2);
    Some(((date as u32) << 16) | time as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_names_and_the_lowest_free_number() {
        assert_eq!(file_name(0), "bc00000.asv");
        assert_eq!(file_name(42), "bc00042.asv");
        assert_eq!(lowest_free(&[]), 0);
        assert_eq!(lowest_free(&[-1, -1]), 0);
        assert_eq!(lowest_free(&[0, 1, 3, -1]), 2);
        assert_eq!(lowest_free(&[1, 0]), 2);
    }

    #[test]
    fn autosave_index_lines_have_three_forms() {
        let saved = Entry { number: 0, origin: Origin::Saved("C:\\Docs\\book.dxp".into()) };
        let unsaved = Entry { number: 1, origin: Origin::Unsaved("/home/a/letter.docx".into()) };
        let untitled = Entry { number: 2, origin: Origin::Untitled { braille: true, number: 3 } };
        let print = Entry { number: 3, origin: Origin::Untitled { braille: false, number: 1 } };
        assert_eq!(saved.line(), "0;saved;C:\\Docs\\book.dxp\n");
        assert_eq!(unsaved.line(), "1;unsaved;/home/a/letter.docx\n");
        assert_eq!(untitled.line(), "2;untitled;b3\n");
        assert_eq!(print.line(), "3;untitled;p1\n");
        let text: String = [&saved, &unsaved, &untitled, &print].iter().map(|e| e.line()).collect();
        assert_eq!(parse_index(&text), vec![saved, unsaved, untitled, print]);
    }

    #[test]
    fn autosave_index_reader_skips_bad_lines() {
        let text = "0;SAVED;C:\\a;b.dxb\r\nnot a line\n;saved;x\n-1;saved;x\n4;other;x\n5saved;x\n6;untitled;P7\n7;Unsaved;y\n";
        let e = parse_index(text);
        assert_eq!(
            e,
            vec![
                Entry { number: 0, origin: Origin::Saved("C:\\a;b.dxb".into()) },
                Entry { number: 6, origin: Origin::Untitled { braille: false, number: 7 } },
                Entry { number: 7, origin: Origin::Unsaved("y".into()) },
            ]
        );
    }

    #[test]
    fn autosave_drops_a_saved_entry_whose_file_is_newer() {
        let t = dos_stamp(1_760_000_000).unwrap();
        let later = dos_stamp(1_760_000_010).unwrap();
        assert!(keep_saved(Some(t), Some(later)));
        assert!(!keep_saved(Some(later), Some(t)));
        assert!(!keep_saved(Some(t), Some(t)));
        // The same two seconds are the same time.
        assert_eq!(dos_stamp(1_760_000_000), dos_stamp(1_760_000_001));
        assert!(!keep_saved(Some(t), None));
        assert!(keep_saved(None, None));
        assert!(keep_saved(None, Some(t)));
    }

    #[test]
    fn dos_stamps_of_known_times() {
        // 2026-10-18 13:45:30 UTC.
        let s = 1_792_331_130;
        let stamp = dos_stamp(s).unwrap();
        assert_eq!(stamp >> 16, ((2026 - 1980) << 9) | (10 << 5) | 18);
        assert_eq!(stamp & 0xffff, (13 << 11) | (45 << 5) | 15);
        assert_eq!(dos_stamp(0), None);
        assert_eq!(dos_stamp(315_532_800), Some((1 << 5 | 1) << 16));
    }
}
