//! Hyphenation (spec 36): the hyphenation control tables (tables/hyphenation/*.hct, in the original's text form
//! with the copyright lines left out) and the break-point search of the page formatter: a break point marked in
//! the text (the character U+001D after a cell), a break after or before a dash, after an internal hyphen, or
//! (French tables) a break found by a right-to-left automaton over the word's characters, with a hyphen inserted.

/// A hyphenation control table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyphenTable {
    /// 0 no breaks, 1 at dashes, 2 also after internal hyphens, 3 also the automaton.
    pub mode: u8,
    pub dash: (char, char),
    pub hyphen: char,
    /// The least length of a word the table applies to.
    pub min_word: usize,
    /// The stored lengths of the original (section 2.2): cells before a dash break plus one, before an internal
    /// hyphen less one, before an automaton break less one.
    pub min_dash: i64,
    pub min_hyphen: i64,
    pub min_algo: i64,
    /// A character which, first in a word, excludes it from the automaton.
    pub exclude: char,
    /// The transition strings: beginning of word, the automaton states (file state 0 first), the class exclusion
    /// ("lower signs").
    pub begin: Vec<u8>,
    pub states: Vec<Vec<u8>>,
    pub lower: Vec<u8>,
}

/// The table's file name for the document's Hyphen Table choice (0 None, 1 Dashes, 2 Dashes and Hyphens, 3
/// French, 4 French 113): French in a print document takes the dashes-and-hyphens table (spec 36 1.2).
pub fn file_name(choice: u8, print: bool) -> &'static str {
    match (choice, print) {
        (0, _) => "none",
        (2, _) | (3, true) => "dashes-hyphens",
        (3, false) => "french",
        (4, _) => "french-113",
        _ => "dashes",
    }
}

/// The table's data lines: a "|" followed by a space or the line end starts a comment, "||" is a literal "|",
/// "|ddd" the character with that code; a line holding only a comment is skipped; trailing spaces are trimmed.
fn data_lines(text: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let bytes = raw.as_bytes();
        let mut line: Vec<u8> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'|' {
                match bytes.get(i + 1) {
                    None | Some(b' ') => break,
                    Some(b'|') => {
                        line.push(b'|');
                        i += 2;
                    }
                    Some(d) if d.is_ascii_digit() => {
                        let digits = &raw[i + 1..(i + 4).min(raw.len())];
                        let n: u32 = digits.parse().map_err(|_| format!("bad character code in {raw}"))?;
                        if digits.len() != 3 || n > 255 {
                            return Err(format!("bad character code in {raw}"));
                        }
                        line.push(n as u8);
                        i += 4;
                    }
                    Some(_) => break,
                }
            } else {
                line.push(b);
                i += 1;
            }
        }
        while line.last() == Some(&b' ') {
            line.pop();
        }
        // A line that is only a comment (its first character the marker) is skipped; a blank line too. A line
        // with text before its comment is a record, even an empty one (the spare records).
        if raw.starts_with('|') || raw.trim().is_empty() {
            continue;
        }
        out.push(line);
    }
    Ok(out)
}

impl HyphenTable {
    /// The None table as shipped (breaks nowhere but at marked breaks), for a document whose table is not at hand.
    pub fn none() -> HyphenTable {
        HyphenTable {
            mode: 0,
            dash: ('-', '-'),
            hyphen: '-',
            min_word: 4,
            min_dash: 3,
            min_hyphen: 1,
            min_algo: 1,
            exclude: '#',
            begin: b"1000000000001010000000011000000000000000000000000000000000000001".to_vec(),
            states: vec![b"10000001000001".to_vec(), b"10000001000001001111111101".to_vec()],
            lower: b"1000000100001110111111111101".to_vec(),
        }
    }

    /// Reads a table in the original's text form (spec 36 section 2).
    pub fn parse(text: &str) -> Result<HyphenTable, String> {
        let lines = data_lines(text)?;
        let mut it = lines.into_iter();
        let mut next = |what: &str| it.next().ok_or_else(|| format!("hyphenation table ends before {what}"));
        let number = |line: &[u8], lo: i64, hi: i64, what: &str| -> Result<i64, String> {
            let s = String::from_utf8_lossy(line);
            let n: i64 = s.trim().split(|c: char| !(c.is_ascii_digit() || c == '-')).next().unwrap_or("").parse().map_err(|_| format!("{what}: {s}"))?;
            if n < lo || n > hi {
                return Err(format!("{what} out of range: {n}"));
            }
            Ok(n)
        };
        // A string record keeps the bytes from its second character on.
        let string = |line: Vec<u8>| -> Vec<u8> { line.into_iter().skip(1).collect() };
        if String::from_utf8_lossy(&next("the signature")?).trim() != "-hinit 1" {
            return Err("not a hyphenation table (no -hinit 1)".into());
        }
        let mode = number(&next("the mode")?, 0, 3, "mode")? as u8;
        let dash = string(next("the dash")?);
        let dash = (dash.first().map_or('-', |&b| b as char), dash.get(1).map_or('-', |&b| b as char));
        let hyphen = string(next("the hyphen")?).first().map_or('-', |&b| b as char);
        let min_word = number(&next("the word length")?, 2, i64::MAX, "minimum word length")? as usize;
        let min_dash = number(&next("the dash length")?, 1, i64::MAX, "minimum before a dash")? + 1;
        let min_hyphen = number(&next("the hyphen length")?, 1, i64::MAX, "minimum before a hyphen")? - 1;
        let min_algo = number(&next("the algorithm length")?, 1, i64::MAX, "minimum before an algorithm break")? - 1;
        next("spare")?;
        next("spare")?;
        let exclude = string(next("the exclusion character")?).first().map_or('#', |&b| b as char);
        let begin = string(next("the beginning string")?);
        let count = number(&next("the state count")?, 2, 16, "states")? as usize;
        let mut states = Vec::with_capacity(count);
        for k in 0..count {
            states.push(string(next(&format!("state {k}"))?));
        }
        let lower = string(next("the class exclusion string")?);
        if String::from_utf8_lossy(&next("the terminator")?).trim() != "***" {
            return Err("hyphenation table has no terminator".into());
        }
        Ok(HyphenTable { mode, dash, hyphen, min_word, min_dash, min_hyphen, min_algo, exclude, begin, states, lower })
    }

    /// A character's value in a transition string: column c - 0x20, the digit value of what stands there (letters
    /// A to O give 17 to 31, '#' gives -13, a space -16), 0 past the end.
    fn value(c: char, s: &[u8]) -> i64 {
        let k = (c as i64 - 0x20).max(0) as usize;
        if k + 1 > s.len() {
            0
        } else {
            s[k] as i64 - b'0' as i64
        }
    }
}

/// The word being broken: its cells with their document places, and whether a marked break may follow each cell.
pub struct Word<'a> {
    pub cells: &'a mut Vec<(char, Option<usize>)>,
    pub flags: &'a mut Vec<bool>,
}

/// The break-point search of the original (spec 36 section 5): how many cells of the word (from cell `start`) go on
/// the current line, 0 for no break. A hyphen may be inserted into the word (its cell counted). `room` is the
/// cells free on the line; `wb_min` the [wbN] override of the cells before an automaton break (-1 for the table's);
/// `hyphen_override` the [svcbh] hyphen; `x` the fallback gate of the original (its callers leave it unset).
pub fn break_point(table: &HyphenTable, word: Word, start: usize, room: i64, x: i64, wb_min: i64, hyphen_override: Option<char>) -> usize {
    let n = word.cells.len() as i64;
    let cell = |i: i64| -> char { word.cells[(i - 1) as usize].0 };
    let room_minus = room - 1;
    let mut best: i64 = 0;
    let mut insert = false;
    let mut source: Option<usize> = None;
    let start = start as i64;
    // 5.1 A marked break: the highest in the window.
    let mut i = (n - 1).min(start + room_minus);
    while start < i {
        if word.flags.get((i - 1) as usize).copied().unwrap_or(false) {
            best = i;
            insert = true;
            source = word.cells[(i - 1) as usize].1;
            break;
        }
        i -= 1;
    }
    if n >= table.min_word as i64 && table.mode != 0 {
        // 5.2 Dash: after the dash when its end fits, else before it.
        let floor = best.max(table.min_dash);
        let mut i = n;
        'dash: loop {
            loop {
                i -= 1;
                if i <= floor {
                    break 'dash;
                }
                if cell(i) == table.dash.1 && cell(i - 1) == table.dash.0 {
                    break;
                }
            }
            if i <= room {
                best = i;
                insert = false;
                break 'dash;
            }
            if i - 2 <= room && i - 2 > best {
                best = i - 2;
                insert = false;
                break 'dash;
            }
        }
        if table.mode != 1 {
            // 5.3 The word trimmed with the transition strings.
            let end_state = table.states.get(1).map(Vec::as_slice).unwrap_or(&[]);
            let mut t = n;
            while t > 1 && HyphenTable::value(cell(t), end_state) == 1 {
                t -= 1;
            }
            let trim_end = t;
            while t > 1 && HyphenTable::value(cell(t), &table.lower) == 1 {
                t -= 1;
            }
            let last_non_lower = t;
            let mut f = 1;
            while f < trim_end && HyphenTable::value(cell(f), &table.begin) == 1 {
                f += 1;
            }
            let first_real = f;
            while f < trim_end && HyphenTable::value(cell(f), &table.lower) == 1 {
                f += 1;
            }
            let first_non_lower = f;
            // 5.4 An internal hyphen.
            let mut i = (trim_end - 1).min(last_non_lower);
            let floor = (first_real + table.min_hyphen).max(first_non_lower).max(best + 1);
            while floor < i {
                if cell(i) == table.hyphen && i <= room {
                    best = i;
                    insert = false;
                    break;
                }
                i -= 1;
            }
            // 5.5 The automaton (mode 3), from the end of the word towards its start.
            if table.mode != 2 && cell(first_real) != table.exclude && n < 0xa0 {
                let min_before = if wb_min >= 0 { wb_min } else { table.min_algo };
                let floor = (first_real + min_before).max(first_non_lower).max(best);
                let accept = room_minus.min(last_non_lower);
                let mut state: i64 = 1;
                let mut i = trim_end;
                while floor <= i {
                    let c = cell(i);
                    let s = table.states.get(state as usize).map(Vec::as_slice).unwrap_or(&[]);
                    state = HyphenTable::value(c, s);
                    if state < 0 {
                        if i < n && c == cell(i + 1) && i <= accept {
                            best = i;
                            insert = true;
                            break;
                        }
                        state += 0x10;
                    } else if state > 0xf {
                        if state < 0x20 {
                            if i <= accept {
                                best = i;
                                insert = true;
                                break;
                            }
                            state -= 0x10;
                        } else {
                            state = 0;
                        }
                    }
                    i -= 1;
                }
            }
        }
    }
    // 5.6 The fallback: a hyphen after any cell that is not a braille prefix indicator.
    if best < 1 && x < n && x > 1 {
        let mut i = (n - 1).min(start + room_minus);
        while start < i {
            if !matches!(cell(i), ',' | '"' | ';' | '@' | '.' | '^' | '_' | '#') {
                best = i;
                insert = true;
                break;
            }
            i -= 1;
        }
    }
    // 5.7 The hyphen inserted after the first part.
    if best > 0 && insert {
        let h = hyphen_override.unwrap_or(table.hyphen);
        word.cells.insert(best as usize, (h, source));
        word.flags.insert(best as usize, false);
        return best as usize + 1;
    }
    best.max(0) as usize
}

/// The split point of a computer-braille word ([wb-cb], spec 36 section 6): over the first min(n - 3, room)
/// cells, the last position past the third whose character is below 0x80 and not alphanumeric breaks after it
/// (before it when it is "_", which also skips the cell after it); none gives the negative of the last index
/// scanned.
pub fn cb_split(cells: &[(char, Option<usize>)], room: usize) -> i64 {
    let n = cells.len();
    let limit = room.min(n.saturating_sub(3));
    let mut found: i64 = 0;
    let mut last: i64 = 0;
    let mut i = 0usize;
    while i < limit {
        let c = cells[i].0;
        if i > 2 {
            last = i as i64;
            if (c as u32) < 0x80 && !c.is_ascii_alphanumeric() {
                found = i as i64 + i64::from(c != '_');
            }
        }
        if c == '_' {
            i += 1;
        }
        i += 1;
    }
    if found < 1 {
        -last
    } else {
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DASHYP: &str = "| t\n-hinit 1\n2\n--\n-\n4\n2\n2\n2\n | spare\n | spare\n# | x\n| !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_\n 1000000000001010000000011000000000000000000000000000000000000001\n2\n 10000001000001             | 0\n 10000001000001001111111101 | 1\n 1000000100001110111111111101\n*** | end\n";

    fn word(s: &str) -> (Vec<(char, Option<usize>)>, Vec<bool>) {
        (s.chars().enumerate().map(|(k, c)| (c, Some(k))).collect(), vec![false; s.chars().count()])
    }

    #[test]
    fn a_table_reads_and_breaks_at_dashes_and_hyphens() {
        let t = HyphenTable::parse(DASHYP).unwrap();
        assert_eq!((t.mode, t.dash, t.hyphen, t.min_word, t.min_dash, t.min_hyphen, t.min_algo, t.exclude), (2, ('-', '-'), '-', 4, 3, 1, 1, '#'));
        assert_eq!(t.states.len(), 2);
        // "well--known" with room for 6: the dash ends the line (6 cells); with room for 5 the dash starts the next
        // line (4 cells).
        let (mut c, mut f) = word("well--known");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 6, 0, -1, None), 6);
        let (mut c, mut f) = word("well--known");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 5, 0, -1, None), 4);
        // An internal hyphen: "self-made" breaks after the hyphen; nothing is inserted.
        let (mut c, mut f) = word("self-made");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 7, 0, -1, None), 5);
        assert_eq!(c.len(), 9);
        // No break point in "elephant" at room 5: 0 (the formatter cuts or moves the word).
        let (mut c, mut f) = word("elephant");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 5, 0, -1, None), 0);
        // A marked break after "ele": a hyphen is inserted and counted (4 cells on the line).
        let (mut c, mut f) = word("elephant");
        f[2] = true;
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 5, 0, -1, None), 4);
        assert_eq!(c.iter().map(|c| c.0).collect::<String>(), "ele-phant");
    }

    #[test]
    fn the_shipped_tables_read() {
        for name in ["none", "dashes", "dashes-hyphens", "french", "french-113"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/hyphenation").join(format!("{name}.hct"));
            let text = std::fs::read_to_string(&path).unwrap();
            let t = HyphenTable::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(t.states.len(), if name.starts_with("french") { 5 } else { 2 }, "{name}");
        }
        assert_eq!((file_name(3, true), file_name(3, false), file_name(4, true), file_name(1, false)), ("dashes-hyphens", "french", "french-113", "dashes"));
    }

    #[test]
    fn the_french_automaton_breaks_between_syllables() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/hyphenation/french.hct");
        let t = HyphenTable::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        // "camarade" at room 5: ca-ma | rade: the last syllable break that fits is after "cama" (4 cells plus the
        // hyphen).
        let (mut c, mut f) = word("camarade");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 5, 0, -1, None), 5);
        assert_eq!(c.iter().map(|c| c.0).collect::<String>(), "cama-rade");
        // "pomme" breaks only between the doubled letters.
        let (mut c, mut f) = word("pomme");
        assert_eq!(break_point(&t, Word { cells: &mut c, flags: &mut f }, 0, 4, 0, -1, None), 4);
        assert_eq!(c.iter().map(|c| c.0).collect::<String>(), "pom-me");
    }

    #[test]
    fn a_computer_braille_word_splits_after_punctuation() {
        let (c, _) = word("www.example.com/path");
        assert_eq!(cb_split(&c, 12), 12);
        let (c, _) = word("abcdefghijkl");
        assert_eq!(cb_split(&c, 8), -7);
    }
}
