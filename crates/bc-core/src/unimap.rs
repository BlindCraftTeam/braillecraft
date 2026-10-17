//! The character map step of text imports: each character of the UTF-8 text is looked up in the character map tables
//! and replaced by what they give for it, as the original program's character map step does it.
//!
//! The tables are plain text files (tables/unimap). A line "U+00B2: [ps]2[pe]" maps one character (four or five hex
//! digits) to a text, in which "[" and "]" stand for the code marks 0x1C and 0x1F and "{U+XXXX}" for a character; an
//! empty text deletes the character. "U+0041,U+0301: ..." maps a sequence of characters, the longest one matching
//! winning. "FONT: name" lines start a table used only inside text marked with that font ("font~name" code), and
//! "HAN: language" lines a table for Chinese characters in that reading. Every other table line goes to the default
//! table. "| " starts a comment, and a line starting with "*" is a page heading and is skipped.
//!
//! While mapping: a character the tables do not give stays as it is when it is below U+10000, and becomes the
//! Unknown Characters text ("*", nothing, or "{U+XXXXXXXX}") above that, and every character from U+20000 up does;
//! control characters pass as they are; a code (0x1C ... 0x1F) passes with only its plain ASCII characters, and a
//! font code is taken out and switches the font table.

use std::collections::HashMap;

/// What a table gives for one character: its single mapping and the sequences that start with it.
#[derive(Debug, Default, Clone)]
struct Entry {
    single: Option<Vec<u8>>,
    /// Sequences: the characters after the first, and the text.
    sequences: Vec<(Vec<u32>, Vec<u8>)>,
}

type Table = HashMap<u32, Entry>;

/// The character map tables.
#[derive(Debug, Default, Clone)]
pub struct UniMap {
    default: Option<Table>,
    fonts: Vec<Table>,
    /// Font names (in lower case) and their tables.
    font_names: HashMap<String, usize>,
    /// The Chinese character tables by reading: Mandarin, Cantonese, Japanese, Korean (the fifth has no name).
    han: [Option<Table>; 5],
    /// Lines that could not be read: file and line.
    pub errors: Vec<String>,
}

/// Which table the lines of a file go to.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Default,
    Font(usize),
    Han(usize),
}

impl UniMap {
    /// Reads the table files, each given by its name and bytes.
    pub fn read(files: &[(&str, &[u8])]) -> UniMap {
        let mut map = UniMap::default();
        for (name, bytes) in files {
            map.read_file(name, bytes);
        }
        map
    }

    fn table(&mut self, target: Target) -> &mut Table {
        match target {
            Target::Default => self.default.get_or_insert_with(Table::new),
            Target::Font(i) => &mut self.fonts[i],
            Target::Han(i) => self.han[i].get_or_insert_with(Table::new),
        }
    }

    fn read_file(&mut self, file: &str, bytes: &[u8]) {
        let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(bytes);
        let mut target = Target::Default;
        let (mut singles, mut has_font, mut has_han) = (0usize, false, false);
        for (n, line) in lines(bytes).into_iter().enumerate() {
            let at = |what: &str| format!("{file} line {}: {what}", n + 1);
            // The text before the first "| ", trimmed.
            let line = match line.windows(2).position(|w| w == b"| ") {
                Some(i) => &line[..i],
                None => line,
            };
            let line = trim(line);
            if line.is_empty() || line[0] == b'*' {
                continue;
            }
            if starts_with_ci(line, b"FONT:") {
                if singles > 0 || has_han {
                    self.errors.push(at("Font specification cannot follow a mapping"));
                    continue;
                }
                if !has_font {
                    self.fonts.push(Table::new());
                    target = Target::Font(self.fonts.len() - 1);
                    has_font = true;
                }
                let name = String::from_utf8_lossy(trim(&line[5..])).to_lowercase();
                if let Target::Font(i) = target {
                    self.font_names.insert(name, i);
                }
                continue;
            }
            if starts_with_ci(line, b"HAN:") {
                let name = String::from_utf8_lossy(trim(&line[4..])).to_lowercase();
                let n = match name.as_str() {
                    "mandarin" => 0,
                    "cantonese" | "yue" => 1,
                    "japanese" => 2,
                    "korean" => 3,
                    _ => {
                        self.errors.push(at(&format!("Invalid Han specification: {name}")));
                        continue;
                    }
                };
                if singles > 0 || has_font || has_han {
                    self.errors.push(at("A Han specification must come first"));
                    continue;
                }
                target = Target::Han(n);
                has_han = true;
                continue;
            }
            match parse_mapping(line) {
                Some((first, rest, value)) => {
                    let entry = self.table(target).entry(first).or_default();
                    if rest.is_empty() {
                        entry.single = Some(value);
                        singles += 1;
                    } else {
                        // A later sequence for the same characters replaces the earlier one.
                        entry.sequences.retain(|(s, _)| *s != rest);
                        entry.sequences.push((rest, value));
                    }
                }
                None => self.errors.push(at(&format!("Syntax error: {}", String::from_utf8_lossy(line)))),
            }
        }
    }

    /// Looks `c` up: in the font table in use, then for a Chinese character in the table of the reading `han`, then
    /// in the default table. Returns the text and how many of the following characters the match took.
    fn lookup<'a>(&'a self, c: u32, after: &[u32], font: Option<usize>, han: i32) -> Option<(&'a [u8], usize)> {
        if c >= 0x20000 {
            return None;
        }
        let in_table = |t: &'a Table| find(t, c, after);
        if let Some(found) = font.and_then(|f| self.fonts.get(f)).and_then(in_table) {
            return Some(found);
        }
        if is_han(c, han) {
            if let Some(found) = usize::try_from(han).ok().and_then(|h| self.han.get(h)).and_then(|t| t.as_ref()).and_then(in_table) {
                return Some(found);
            }
        }
        self.default.as_ref().and_then(in_table)
    }

    /// Maps UTF-8 text (read without validation, as the importer reads it). `han` is the reading of Chinese
    /// characters (0 Mandarin to 3 Korean, -1 none); `unknown` the Unknown Characters setting (0 "*", 1 nothing, 2
    /// the hex value) for characters above U+FFFF the tables do not give.
    pub fn map(&self, input: &[u8], han: i32, unknown: u8) -> Vec<u8> {
        let chars = utf8_values(input);
        let mut out = Vec::with_capacity(input.len());
        let mut font: Option<usize> = None;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            i += 1;
            match c {
                0 | 0x1f => {}
                0x1c => {
                    // A code: its plain ASCII characters up to 0x1F.
                    let mut text = Vec::new();
                    while i < chars.len() && chars[i] != 0x1f {
                        if (0x20..0x7f).contains(&chars[i]) {
                            text.push(chars[i] as u8);
                        }
                        i += 1;
                    }
                    i += 1;
                    if let Some(name) = text.strip_prefix(b"font~") {
                        font = self.font_names.get(&String::from_utf8_lossy(name).to_lowercase()).copied();
                    } else {
                        out.push(0x1c);
                        out.extend_from_slice(&text);
                        out.push(0x1f);
                    }
                }
                c if c < 0x20 => out.push(c as u8),
                c => match self.lookup(c, &chars[i..], font, han) {
                    Some((text, used)) => {
                        out.extend_from_slice(text);
                        i += used;
                    }
                    None if c < 0x10000 => put_utf8(&mut out, c),
                    None => match unknown {
                        0 => out.push(b'*'),
                        1 => {}
                        _ => out.extend_from_slice(format!("{{U+{c:08X}}}").as_bytes()),
                    },
                },
            }
        }
        out
    }
}

/// `c` in table `t`: the longest sequence starting with it that `after` matches, else its single mapping; the text
/// and how many characters of `after` the match took.
fn find<'t>(t: &'t Table, c: u32, after: &[u32]) -> Option<(&'t [u8], usize)> {
    let e = t.get(&c)?;
    let best = e.sequences.iter().filter(|(s, _)| after.len() >= s.len() && after[..s.len()] == s[..]).max_by_key(|(s, _)| s.len());
    match best {
        Some((s, v)) => Some((v.as_slice(), s.len())),
        None => e.single.as_deref().map(|v| (v, 0)),
    }
}

/// A Chinese character for the reading `han`: U+3400 to U+9FFF and U+F900 to U+FAFF, with U+3000 to U+30FF for
/// Japanese and the private use characters U+E000 to U+F7FF for Cantonese.
fn is_han(c: u32, han: i32) -> bool {
    let page = c >> 8;
    (0x34..=0x9f).contains(&page)
        || (0xf9..=0xfa).contains(&page)
        || (han == 2 && page == 0x30)
        || (han == 1 && (0xe0..=0xf7).contains(&page))
}

/// The lines of a table file: they end at CR, LF, CR LF or LF CR.
fn lines(bytes: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\r' || b == b'\n' {
            out.push(&bytes[start..i]);
            i += 1;
            if i < bytes.len() && (bytes[i] == b'\r' || bytes[i] == b'\n') && bytes[i] != b {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        out.push(&bytes[start..]);
    }
    out
}

fn trim(mut s: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = s {
        if !first.is_ascii_whitespace() {
            break;
        }
        s = rest;
    }
    while let [rest @ .., last] = s {
        if !last.is_ascii_whitespace() {
            break;
        }
        s = rest;
    }
    s
}

fn starts_with_ci(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// "U+hhhh" (or five hex digits when the line is long enough) at the start of `s`: the value and the bytes taken.
fn code_at(s: &[u8]) -> Option<(u32, usize)> {
    if s.len() < 2 || !(s[0] == b'U' || s[0] == b'u') || s[1] != b'+' {
        return None;
    }
    let hex = |d: &[u8]| -> Option<u32> {
        let t = std::str::from_utf8(d).ok()?;
        t.bytes().all(|b| b.is_ascii_hexdigit()).then(|| u32::from_str_radix(t, 16).ok()).flatten()
    };
    if s.len() >= 2 + 6 {
        if let Some(v) = hex(&s[2..7]) {
            return Some((v, 7));
        }
    }
    if s.len() >= 6 {
        if let Some(v) = hex(&s[2..6]) {
            return Some((v, 6));
        }
    }
    None
}

/// A mapping line: the first character, the characters after it in a sequence, and the text.
fn parse_mapping(line: &[u8]) -> Option<(u32, Vec<u32>, Vec<u8>)> {
    let (first, mut at) = code_at(line)?;
    let mut rest = Vec::new();
    while line.get(at) == Some(&b',') {
        let (c, n) = code_at(&line[at + 1..])?;
        rest.push(c & 0xffff);
        at += 1 + n;
    }
    if line.get(at) != Some(&b':') || (rest.is_empty() && first >= 0x20000) {
        return None;
    }
    Some((first, rest, value(trim(&line[at + 1..]))))
}

/// A mapping's text: "[" outside a code is 0x1C, "]" inside one 0x1F, "{U+XXXX}" (four hex digits) that character
/// in UTF-8; everything else as it is.
fn value(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut in_code = false;
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        match b {
            b'[' if !in_code => {
                out.push(0x1c);
                in_code = true;
            }
            b']' if in_code => {
                out.push(0x1f);
                in_code = false;
            }
            b'{' if s.len() >= i + 8
                && (s[i + 1] == b'u' || s[i + 1] == b'U')
                && s[i + 2] == b'+'
                && s[i + 3..i + 7].iter().all(u8::is_ascii_hexdigit)
                && s[i + 7] == b'}' =>
            {
                let v = u32::from_str_radix(std::str::from_utf8(&s[i + 3..i + 7]).unwrap_or("0"), 16).unwrap_or(0);
                put_utf8(&mut out, v);
                i += 8;
                continue;
            }
            _ => out.push(b),
        }
        i += 1;
    }
    out
}

/// The characters of UTF-8 text read without validation, as the formatted text importer reads it: a lone trailing
/// byte is itself, a first byte takes the bytes after it whatever they are, and a sequence the end cuts short ends
/// the text.
fn utf8_values(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let first = bytes[i];
        let extra = crate::pimbri::TRAILING[first as usize] as usize;
        if i + extra >= bytes.len() && extra > 0 {
            break;
        }
        let mut acc: u32 = 0;
        for k in 0..extra {
            acc = acc.wrapping_add(u32::from(bytes[i + k])).wrapping_mul(64);
        }
        acc = acc.wrapping_add(u32::from(bytes[i + extra]));
        let v = acc.wrapping_sub(crate::pimbri::UTF8_OFFSETS[extra]);
        out.push(if v > 0x7fff_ffff { 0xfffd } else { v });
        i += extra + 1;
    }
    out
}

/// A character value as UTF-8 (values that are not Unicode characters written by the same rule, up to four bytes;
/// anything larger as U+FFFD).
fn put_utf8(out: &mut Vec<u8>, c: u32) {
    match c {
        0..=0x7f => out.push(c as u8),
        0x80..=0x7ff => out.extend_from_slice(&[0xc0 | (c >> 6) as u8, 0x80 | (c & 0x3f) as u8]),
        0x800..=0xffff => {
            out.extend_from_slice(&[0xe0 | (c >> 12) as u8, 0x80 | ((c >> 6) & 0x3f) as u8, 0x80 | (c & 0x3f) as u8])
        }
        0x10000..=0x10ffff => out.extend_from_slice(&[
            0xf0 | (c >> 18) as u8,
            0x80 | ((c >> 12) & 0x3f) as u8,
            0x80 | ((c >> 6) & 0x3f) as u8,
            0x80 | (c & 0x3f) as u8,
        ]),
        _ => out.extend_from_slice("\u{fffd}".as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> UniMap {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/unimap");
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
            .collect();
        files.sort();
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        UniMap::read(&refs)
    }

    #[test]
    fn table_lines() {
        let m = UniMap::read(&[(
            "t",
            b"| comment\r\n*00: page\r\nU+00B2: [ps]2[pe] | superscript\r\nU+0082: {U+2019}\r\nU+0640:   | deleted\r\nU+10000: a\r\nU+0041,U+0301: {U+00C1}\r\nnonsense\r\n".as_slice(),
        )]);
        assert_eq!(m.errors.len(), 1);
        assert_eq!(m.map("\u{b2}".as_bytes(), -1, 0), b"\x1cps\x1f2\x1cpe\x1f".to_vec());
        assert_eq!(m.map("\u{82}x\u{640}y".as_bytes(), -1, 0), "\u{2019}xy".as_bytes().to_vec());
        assert_eq!(m.map("\u{10000}".as_bytes(), -1, 0), b"a".to_vec());
        // The longest sequence wins; the plain letter passes.
        assert_eq!(m.map("A\u{301}A".as_bytes(), -1, 0), "\u{c1}A".as_bytes().to_vec());
    }

    #[test]
    fn unmapped_characters_and_controls() {
        let m = UniMap::read(&[("t", b"U+00E9: e\r\n".as_slice())]);
        // Below U+10000 a character the tables lack passes; above, the Unknown Characters text.
        assert_eq!(m.map("\u{263a}\u{1f600}".as_bytes(), -1, 0), "\u{263a}*".as_bytes().to_vec());
        assert_eq!(m.map("\u{1f600}".as_bytes(), -1, 1), Vec::<u8>::new());
        assert_eq!(m.map("\u{1f600}".as_bytes(), -1, 2), b"{U+0001F600}".to_vec());
        // Controls pass, 0 and a lone 0x1F go; a code keeps its plain ASCII characters.
        assert_eq!(m.map(b"a\tb\n\x00\x1fc", -1, 0), b"a\tb\nc".to_vec());
        assert_eq!(m.map("\x1cp\u{e9}s\x1f".as_bytes(), -1, 0), b"\x1cps\x1f".to_vec());
    }

    #[test]
    fn font_codes_switch_tables() {
        let m = UniMap::read(&[("t", b"U+0061: A\r\n".as_slice()), ("f", b"FONT: Test Font\r\nU+0061: b\r\n".as_slice())]);
        assert_eq!(m.map(b"a\x1cfont~test font\x1fa\x1cfont~other\x1fa", -1, 0), b"AbA".to_vec());
    }

    #[test]
    fn shipped_tables() {
        let m = shipped();
        assert!(m.errors.is_empty(), "{:?}", &m.errors[..m.errors.len().min(5)]);
        // ASCII and the dollar sign pass; U+00B2 becomes a superscript code; a half-width katakana becomes full width.
        assert_eq!(m.map(b"$p Text 42.", -1, 0), b"$p Text 42.".to_vec());
        assert_eq!(m.map("\u{b2}".as_bytes(), -1, 0), b"\x1cps\x1f2\x1cpe\x1f".to_vec());
        assert_eq!(m.map("\u{ff80}".as_bytes(), -1, 0), "\u{30bf}".as_bytes().to_vec());
    }
}
