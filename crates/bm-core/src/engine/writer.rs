//! The writer: turns the rule engine's output into the final text.
//!
//! - Command pairs 0x10 plus a letter: "a" starts collecting a format text, "c" writes the collected texts
//!   (innermost first) and goes back to normal, "b" back to normal, "d" and "e" write the collected texts after
//!   the next character.
//! - Format words held between 0x11 and 0x12 (a two-part format word is placed around its space).
//! - For tables that write print (writer mode 2): capital commands 0x02 (next letter), 0x0E (capitals on) and
//!   0x0F (capitals off).
//! - Words go into a line buffer; a line is broken past column 65, and a line end is written as a space unless
//!   the caller asks for real line ends. Output mode 3 writes every byte as it comes.
//!
//! Also here: the character table (chitab.txt), which gives the Unicode form of each internal code and the capital
//! form of two-byte codes.

use std::collections::HashMap;
use std::path::Path;

use super::chars::{kind, flags, latin1, parse_hex, split_lines};

// ------------------------------------------------------------------ the character table

/// The character table: lines of fields separated by "~": code; small-letter code; text form; its byte; second
/// text form; its byte; Unicode; repeat count (all hex). A repeat count n fills n codes in a row, each number one
/// higher. The first line is a header.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chitab {
    unicode: HashMap<i64, i64>,
    upper2: HashMap<i64, i64>,
}

/// BrailleMaker's own character table, built into the program.
const CHARACTERS: &str = include_str!("../../../../tables/characters.txt");

impl Chitab {
    /// BrailleMaker's character table (tables/characters.txt): tab-separated lines of code, small-letter code,
    /// Unicode, count (hex, "-" for none), then text forms the engine does not use; "#" starts a comment line.
    pub fn parse_characters(text: &str) -> Chitab {
        let mut t = Chitab::default();
        let num = |f: Option<&&str>| match f {
            Some(&"-") | None => None,
            Some(s) => parse_hex(s),
        };
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            let Some(code) = num(f.first()) else { continue };
            let (lower, uni) = (num(f.get(1)), num(f.get(2)));
            let n = num(f.get(3)).unwrap_or(1);
            t.add(code, lower, uni, n);
        }
        t
    }

    /// The character table built into the program.
    pub fn builtin() -> Chitab {
        Chitab::parse_characters(CHARACTERS)
    }

    fn add(&mut self, code: i64, lower: Option<i64>, uni: Option<i64>, n: i64) {
        for i in 0..n {
            if let Some(u) = uni {
                self.unicode.insert(code + i, u + i);
            }
            if let Some(l) = lower {
                if l + i >= 0x100 {
                    self.upper2.insert(l + i, code + i);
                }
            }
        }
    }

    pub fn parse(text: &str) -> Chitab {
        let mut t = Chitab::default();
        for line in split_lines(text).into_iter().skip(1) {
            let parts: Vec<&str> = line.split('~').collect();
            if parts.len() < 7 || super::chars::strip(parts[0]).is_empty() {
                continue;
            }
            let Some(code) = parse_hex(parts[0]) else { continue };
            let lower = parse_hex(parts[1]);
            let uni = parse_hex(parts[6]);
            let rep = parts.get(7).and_then(|p| parse_hex(p));
            let n = match rep {
                Some(r) if r != 0 => r,
                _ => 1,
            };
            t.add(code, lower, uni, n);
        }
        t
    }

    /// Reads chitab.txt from a folder; an empty table when the folder has none.
    pub fn read_folder(folder: &Path) -> Chitab {
        let path = super::find_file(folder, "chitab.txt");
        std::fs::read(path).map(|b| Chitab::parse(&latin1(&b))).unwrap_or_default()
    }

    /// The Unicode form of an internal code; "*" (0x2A) when it has none.
    pub fn unicode_of(&self, code: i64) -> i64 {
        match self.unicode.get(&code) {
            Some(&u) if u != 0 => u,
            _ => 0x2a,
        }
    }

    /// The capital form of a code: from the code page for one-byte codes (0x9C plus the code when it has no
    /// one-byte capital), from the table for two-byte codes.
    fn upper_pair(&self, code: i64) -> i64 {
        if code < 0x100 {
            let (_, lower, upper) = kind(code);
            if upper == 0x9c {
                return lower | 0x9c00;
            }
            return upper;
        }
        match self.upper2.get(&code) {
            Some(&u) if u != 0 => u,
            _ => code,
        }
    }
}

// ------------------------------------------------------------------ the writer

pub(crate) struct Writer<'a> {
    chitab: &'a Chitab,
    /// 2: the table writes print with capitals; 3: raw.
    mode: i64,
    out_mode: i64,
    flag_char: i64,
    /// Bit 0: write real line ends.
    flags: i64,
    file: Vec<u8>,
    map_codes: bool,
    open_as: i64,
    close_as: i64,
    word_max: usize,
    split: bool,
    col: i64,
    last: i64,
    spaces: i64,
    word: Vec<u8>,
    state: i64,
    pending: i64,
    held: Vec<u8>,
    caps: i64,
    depth: usize,
    coll: Vec<u8>,
    coll_len: [usize; 32],
    after: i64,
    in_code: bool,
    lead: i64,
    count: i64,
    space_at: i64,
    mark_at: i64,
    stray: i64,
}

impl<'a> Writer<'a> {
    pub fn new(chitab: &'a Chitab, writer_mode: i64, out_mode: i64, flag_char: i64, flags: i64, override_mode: i64) -> Self {
        let mut w = Writer {
            chitab,
            mode: writer_mode,
            out_mode,
            flag_char,
            flags,
            file: Vec::new(),
            map_codes: false,
            open_as: 0x1c,
            close_as: 0x1f,
            word_max: 0x83,
            split: false,
            col: 0,
            last: 0,
            spaces: 0,
            word: Vec::new(),
            state: 0,
            pending: 0,
            held: Vec::new(),
            caps: 0,
            depth: 0,
            coll: vec![0; 30 * 40 + 41],
            coll_len: [0; 32],
            after: 0,
            in_code: false,
            lead: 0,
            count: 1,
            space_at: -1,
            mark_at: -1,
            stray: 0,
        };
        w.setup(override_mode);
        w.reset();
        w
    }

    /// The output mode decides the word limit, whether long words are split, and how code marks are written.
    fn setup(&mut self, override_mode: i64) {
        if override_mode >= 0 {
            self.out_mode = override_mode;
        }
        self.map_codes = false;
        self.open_as = 0x1c;
        self.close_as = 0x1f;
        match self.out_mode {
            0 | 2 => {
                self.word_max = 0x82;
                self.split = true;
            }
            4 => {
                if self.flag_char == 0x1c {
                    self.word_max = 0x83;
                    self.split = false;
                } else {
                    self.word_max = 0x82;
                    self.split = true;
                    self.map_codes = true;
                    self.open_as = if self.flag_char == 0x24 { 0x7c } else { 0x24 };
                    self.close_as = 0x20;
                }
            }
            6 => {
                self.word_max = 0x82;
                self.split = true;
                self.map_codes = true;
                self.open_as = 0x7c;
                self.close_as = 0x20;
            }
            7 => {
                self.word_max = 0x82;
                self.split = true;
                self.map_codes = true;
                self.open_as = 0x24;
                self.close_as = 0x20;
            }
            _ => {
                self.word_max = 0x83;
                self.split = false;
            }
        }
        self.col = 0;
        self.last = 0;
        self.spaces = 0;
        self.word.clear();
    }

    fn reset(&mut self) {
        self.state = if self.mode == 3 { 0 } else { 1 };
        self.pending = 0;
        self.held.clear();
        self.caps = 0;
        self.depth = 0;
        self.coll = vec![0; 30 * 40 + 41];
        self.coll_len = [0; 32];
        self.after = 0;
        self.in_code = false;
        self.lead = 0;
        self.count = 1;
        self.space_at = -1;
        self.mark_at = -1;
        self.stray = 0;
    }

    fn putc(&mut self, c: i64) {
        self.file.push((c & 0xff) as u8);
    }

    /// One byte into the line.
    fn byte(&mut self, c: i64) {
        if self.out_mode == 3 {
            self.putc(c);
            return;
        }
        let mut c = c;
        if self.map_codes {
            if c == 0x1c {
                c = self.open_as;
            } else if c == 0x1f {
                c = self.close_as;
            }
        }
        if (c < 0x1c || c == 0x20) && c != 0x17 {
            if c == 0x20 {
                if !self.word.is_empty() || self.spaces > 0x3e {
                    self.word_out();
                }
                self.spaces += 1;
            } else if c == 10 {
                self.word_out();
                self.newline();
            }
        } else {
            if self.word_max <= self.word.len() {
                self.word_out();
                if self.split {
                    self.putc(0x2d);
                    self.putc(10);
                }
                self.last = 0;
                self.col = 0;
            }
            self.word.push((c & 0xff) as u8);
        }
    }

    /// Writes the waiting spaces and the buffered word, breaking the line past column 65.
    fn word_out(&mut self) {
        if self.spaces > 0 {
            if self.word.len() as i64 + self.spaces + self.col > 0x41 {
                self.newline();
                self.spaces -= 1;
            }
            if self.spaces > 0 {
                for _ in 0..self.spaces {
                    self.putc(0x20);
                }
                self.last = 0x20;
                self.col += self.spaces;
                self.spaces = 0;
            }
        }
        if !self.word.is_empty() {
            let word = std::mem::take(&mut self.word);
            self.file.extend_from_slice(&word);
            self.last = *word.last().unwrap() as i64;
            self.col += word.len() as i64;
        }
    }

    fn newline(&mut self) {
        if self.flags & 1 == 0 {
            self.putc(0x20);
        } else if !self.split || (self.last != 0x20 && self.last != 0x2d) {
            self.putc(10);
        } else {
            self.putc(0x20);
            self.putc(0x2d);
            self.putc(10);
        }
        self.last = 0;
        self.col = 0;
    }

    fn end(&mut self) {
        if self.out_mode != 3 {
            self.word_out();
            if self.last != 0 {
                self.newline();
            }
        }
    }

    fn flush_held(&mut self, c: i64) {
        let held = std::mem::take(&mut self.held);
        for b in held {
            self.byte(b as i64);
        }
        if c > 0 {
            self.byte(c);
        }
        if self.mode != 3 {
            self.state = 1;
            self.pending = 0;
        }
    }

    fn place_held(&mut self) {
        if self.count == 2 && self.space_at >= 0 && self.mark_at >= 0 {
            let h = std::mem::take(&mut self.held);
            let (mark, space) = (self.mark_at as usize, self.space_at as usize);
            for &b in h.iter().take(mark) {
                self.byte(b as i64);
            }
            for &b in h.iter().skip(space + 1) {
                self.byte(b as i64);
            }
            if self.mode == 2 && h.get(mark).copied().unwrap_or(0) != 0x25 {
                self.byte(0x20);
            }
            for &b in h.iter().take(space).skip(mark) {
                self.byte(b as i64);
            }
            if self.mode != 3 {
                self.state = 1;
                self.pending = 0;
            }
        } else {
            self.flush_held(-1);
        }
    }

    fn flush_coll(&mut self) {
        let mut i = self.depth;
        while i > 0 {
            i -= 1;
            for j in 0..self.coll_len[i] {
                let b = self.coll[i * 40 + j];
                self.byte(b as i64);
            }
        }
        self.depth = 0;
    }

    fn stray(&mut self) {
        self.stray = (self.stray + 1).min(0x7fff_ffff);
    }

    /// One code from the engine; 0x100 ends the output.
    pub fn put(&mut self, c: i64) {
        if c == 0x100 {
            self.flush_held(-1);
            self.end();
            return;
        }
        if self.pending == 2 {
            self.pending = 0;
            let u = self.chitab.upper_pair(self.lead << 8 | c);
            let saved = self.caps;
            self.caps = 0;
            if u >> 8 != 0 {
                self.put(u >> 8);
            }
            self.put(u & 0xff);
            self.caps = saved;
            if self.caps == 1 {
                self.caps = 0;
            }
            return;
        }
        if self.pending == 1 {
            self.byte(c);
            self.pending = 0;
            return;
        }
        if self.pending == 3 {
            self.pending = 0;
            match c {
                0x61 => {
                    // Collect a format text.
                    self.state = 3;
                    self.coll_len[self.depth] = 0;
                    if self.depth > 0x1d {
                        return;
                    }
                    self.depth += 1;
                }
                0x62 => self.state = 1,
                0x63 => {
                    self.flush_coll();
                    self.state = 1;
                }
                0x64 => self.after = 2,
                0x65 => self.after = 1,
                _ => {}
            }
            return;
        }

        let mut v = c;
        if self.mode == 2 {
            if c < 0x20 {
                match c {
                    2 => {
                        self.caps = 1;
                        return;
                    }
                    0x0e => {
                        self.caps = 2;
                        return;
                    }
                    0x0f => {
                        self.caps = 0;
                        return;
                    }
                    0x1c => self.in_code = true,
                    0x1f => self.in_code = false,
                    -2 => v = 0x9c,
                    _ => {}
                }
            } else if flags(c) & 2 == 0 {
                if flags(c) & 0x20 != 0 && self.caps > 0 {
                    self.pending = 2;
                    self.lead = c;
                    return;
                }
            } else if self.caps > 0 {
                if self.caps == 1 {
                    self.caps = 0;
                }
                v = kind(c).2;
                if v == 0x9c {
                    self.put(-2);
                    v = c;
                }
            }
        }

        if self.state == 0 {
            self.byte(v);
        } else if v == 0x10 {
            self.pending = 3;
        } else {
            if v == 0x1c {
                self.in_code = true;
            } else if v == 0x1f {
                self.in_code = false;
            }
            match self.state {
                1 => {
                    if v < 0x1c {
                        if v == 0x11 {
                            self.state = 2;
                            self.count = 1;
                            self.space_at = -1;
                            self.mark_at = -1;
                        } else if v == 0x12 || v == 0x13 {
                        } else if v == 0x17 {
                            self.byte(v);
                            self.pending = 1;
                        } else {
                            self.stray();
                        }
                    } else {
                        if v > 0x1f && !self.in_code && self.after == 1 {
                            self.flush_coll();
                            self.after = 0;
                        }
                        self.byte(v);
                        if flags(v) & 0x20 == 0 && v > 0x1f && !self.in_code && self.after == 2 {
                            self.flush_coll();
                            self.after = 0;
                        }
                    }
                }
                2 => {
                    if v == 0x17 {
                        self.byte(0x17);
                        self.pending = 1;
                    }
                    if v < 0x1c || v == 0x20 {
                        if v == 0x11 {
                            self.flush_held(-1);
                            self.state = 2;
                            self.count = 1;
                            self.space_at = -1;
                            self.mark_at = -1;
                        } else if v == 0x12 {
                            self.place_held();
                        } else if v == 0x13 {
                        } else if v == 0x20 {
                            if self.count < 2 && self.held.len() < 0x28 {
                                self.held.push(0x20);
                                self.space_at = self.held.len() as i64 - 1;
                                self.count += 1;
                            } else {
                                self.flush_held(0x20);
                            }
                        } else {
                            self.stray();
                        }
                    } else if self.held.len() < 0x28 {
                        self.held.push((v & 0xff) as u8);
                        if self.mode == 2 {
                            if self.mark_at == -1 {
                                self.mark_at = self.held.len() as i64 - 1;
                            }
                        } else if v == 0x23 && self.count == 1 {
                            self.mark_at = self.held.len() as i64 - 1;
                        }
                    } else {
                        self.flush_held(v);
                    }
                }
                3 => {
                    if v < 0x1c {
                        if v == 0x17 {
                            self.byte(v);
                            self.pending = 1;
                        } else if !matches!(v, 0x11..=0x13) {
                            self.stray();
                        }
                    } else if self.depth > 0 {
                        let i = self.depth - 1;
                        let at = i * 40 + self.coll_len[i];
                        if at < self.coll.len() {
                            self.coll[at] = (v & 0xff) as u8;
                        }
                        if self.coll_len[i] < 40 {
                            self.coll_len[i] += 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn text(&self) -> String {
        latin1(&self.file)
    }
}
