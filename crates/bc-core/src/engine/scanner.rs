//! The scanner: reads the text the way the scan table says and hands the rule engine its input.
//!
//! Three reading layers, then the word layer:
//! - `Reader::raw` turns runs of blanks, line ends, tabs and form feeds into one run marker (0xFF02 plus the number
//!   of spaces); 0xFF00 means nothing is waiting, 0xFF01 is the end of the input.
//! - `Reader::line` handles line ends (joined, a space, or a new-line code), puts in the paragraph code for a new
//!   paragraph, and joins a word hyphenated at a line end.
//! - `Reader::next` turns a run of spaces into one space (or two after a full stop, or a tab code).
//! - `Scanner::fill` splits the characters into word records, at most 25 waiting at a time.
//! - `Scanner::output` writes the words: each word in lower case with the capital sign in front of a capital
//!   letter, two capital signs in front of an all-capital word, three in front of a capital passage (more than the
//!   table's number of all-capital words in a row) and the capital termination after it; format words pass
//!   through. Between a font's start and end codes it writes the font's code in front of the words (see `Font`),
//!   and around a doubled word ("anak-anak") the table's doubled-word codes.
//!
//! The words wait in a ring of 25 that shares one character buffer; the ring is filled to 25 words unless the
//! buffer has come round again to the next word to write.
//!
//! A word read as a web or mail address (when the table has icb on) is written between the computer-braille codes
//! (spec 07 section E2): the start string in front of it, and the end string after it, or before its last
//! character when that is a full stop or a comma.

use super::callbacks::StyleTable;

/// The computer-braille start and end strings written around an address (the scanner's two constant strings).
const CB_START: [i64; 3] = [0x0e, 0x63, 0x0f];
const CB_END: [i64; 4] = [0x0e, 0x2f, 0x63, 0x0f];
use super::scan_table::{Font, ScanTable};
use super::EngineError;

pub(crate) const EOF_OUT: i64 = 0x100;

fn is_lower(c: i64) -> bool {
    (0x61..=0x7a).contains(&c)
}

fn is_upper(c: i64) -> bool {
    (0x41..=0x5a).contains(&c)
}

fn is_digit(c: i64) -> bool {
    (0x30..=0x39).contains(&c)
}

fn is_letter(c: i64) -> bool {
    is_lower(c) || is_upper(c)
}

/// A character that counts as neither capital nor small; no character in the character table is marked so.
fn case_neutral(_c: i64) -> bool {
    false
}

fn to_lower(c: i64) -> i64 {
    if is_upper(c) {
        c + 0x20
    } else {
        c
    }
}

/// Format codes that end a capital passage: matched whole, or (for the last four) by their start.
const CODES_EXACT_1: [&str; 14] = ["#", ":", ";", "<", ">", "aln", "ca", "cb", "cp", "cu", "cz", "d", "fc", "fr"];
const CODES_EXACT_2: [&str; 53] = [
    "gd", "h", "if", "im", "ind", "iq", "kb", "kd", "kp", "ld", "lg", "line", "lnb", "lpr", "me", "mfc", "musi", "pg",
    "pi", "pl", "pm", "psa", "psc", "pst", "pty", "q", "rd", "remw", "rfe", "rfs", "rn", "rp", "sd", "si", "sl", "ta",
    "te", "tlc", "tle", "tlp", "tls", "tn", "top", "ts", "tt", "tx", "uce", "ucs", "vc", "vs", "wl", "xcs", "\u{f8}",
];
const CODES_PREFIX: [&str; 4] = ["p", "sc", "sk", "st"];

fn code_ends_passage(name: &str, cpe_fts: i64, cpe_l: i64) -> bool {
    if CODES_EXACT_1.contains(&name) || (cpe_fts != 0 && name == "fte") {
        return true;
    }
    if cpe_fts != 0 && name == "fts" {
        return true;
    }
    if cpe_l != 0 && name.starts_with('l') {
        return true;
    }
    CODES_EXACT_2.contains(&name) || CODES_PREFIX.iter().any(|p| name.starts_with(p))
}

fn text_of(codes: &[i64]) -> String {
    codes.iter().map(|&x| char::from_u32((x & 0xff) as u32).unwrap_or('\0')).collect()
}

/// The three reading layers. The table is passed in on each call because format words met while scanning can
/// change its line, space and paragraph modes.
struct Reader<'a> {
    /// The input codes, read once each in order: a large file is never held as a vector of codes.
    inp: Box<dyn Iterator<Item = i64> + 'a>,
    pend: i64,
    ff_i: usize,
    lines: i64,
    tab_errors: i64,
    ff_count: i64,
    /// Characters pushed back by the line layer, taken from the end.
    stk: Vec<i64>,
    at_line_start: bool,
    la: i64,
    /// Characters pushed back by the space layer, taken from the end.
    stk2: Vec<i64>,
    rep: i64,
    rep_kind: i64,
    was_stop: bool,
    cc: i64,
    la2: i64,
    /// What a read past the input gives: 0xFF01 for a document's codes, 0xFF7F for the bytes of a file.
    eof: i64,
    /// The code read after a line end in byte mode 1 that was not part of it.
    held: Option<i64>,
}

impl<'a> Reader<'a> {
    fn new(t: &ScanTable, codes: Box<dyn Iterator<Item = i64> + 'a>, eof: i64) -> Reader<'a> {
        let mut r = Reader {
            inp: codes,
            eof,
            pend: 0xff00,
            ff_i: 0,
            lines: 0,
            tab_errors: 0,
            ff_count: 0,
            stk: Vec::new(),
            at_line_start: true,
            la: 0,
            stk2: Vec::new(),
            rep: 0,
            rep_kind: 0,
            was_stop: false,
            cc: 0,
            la2: 0,
            held: None,
        };
        // The text layers read ahead; the byte modes read nothing until asked.
        if t.binmod == 0 {
            r.la = r.raw(t);
            r.la2 = r.line(t);
        }
        r
    }

    /// The next code as the table's input mode reads it: 0 through the text layers; 1 one byte at a time with a
    /// line end (CR, LF, CR LF or LF CR) as one LF; 2 one byte at a time as it is. In modes 1 and 2 the end of the
    /// input is 0xFF00.
    fn code(&mut self, t: &ScanTable) -> i64 {
        match t.binmod {
            0 => self.next(t),
            1 => {
                let Some(c) = self.held.take().or_else(|| self.inp.next()) else { return 0xff00 };
                if c == 0x0a || c == 0x0d {
                    if let Some(d) = self.inp.next() {
                        if (d != 0x0a && d != 0x0d) || d == c {
                            self.held = Some(d);
                        }
                    }
                    return 0x0a;
                }
                c
            }
            _ => self.inp.next().unwrap_or(0xff00),
        }
    }

    /// One input code; at the end, the end code (every time it is asked).
    fn byte(&mut self) -> i64 {
        self.inp.next().unwrap_or(self.eof)
    }

    fn raw(&mut self, t: &ScanTable) -> i64 {
        if self.pend != 0xff00 {
            let mut v = self.pend;
            if v == 0x0c {
                v = t.ff_text.get(self.ff_i).copied().unwrap_or(0);
                self.ff_i += 1;
                if self.ff_i < t.ff_text.len() {
                    return v;
                }
                self.pend = 0xff00;
                return v;
            }
            if v == 0xff00 || v == 0xff01 {
                return v;
            }
            self.pend = 0xff00;
            return v;
        }
        let c = self.byte();
        if c == 0xff01 {
            return 0xff01;
        }
        if c > 0x20 && c != 0xff7f {
            return c;
        }
        self.pend = c;
        let mut run: i64 = 0xff02;
        loop {
            let p = self.pend;
            if p == 0x20 {
                run += 1;
            } else if p == 0x0a {
                self.lines += 1;
                if t.trailing_blanks != 0 {
                    run = 0xff02;
                }
                break;
            } else if p == 0x09 {
                if t.tabs < 0 {
                    self.tab_errors += 1;
                } else {
                    run += t.tabs;
                }
            } else if p == 0x0c {
                if t.ff_text.first().is_some_and(|&f| f > 0x1f) {
                    self.ff_i = 0;
                    run += 1;
                    break;
                }
                if t.ff_text.first() == Some(&0x15) {
                    self.ff_count += 1;
                }
            } else {
                if p == 0xff7f {
                    self.pend = 0xff00;
                }
                break;
            }
            if run > 0xfffb {
                run -= 0xfa;
            }
            self.pend = self.byte();
            if !(self.pend < 0x21 || self.pend == 0xff7f) {
                break;
            }
        }
        if run < 0xff03 {
            run = self.pend;
            if run != 0xff00 && run != 0xff01 {
                self.pend = 0xff00;
            }
        }
        run
    }

    fn line(&mut self, t: &ScanTable) -> i64 {
        if let Some(c) = self.stk.pop() {
            return c;
        }
        loop {
            loop {
                while self.at_line_start {
                    if (self.la == 0x0a && (t.no_par == 1 || t.no_par == 3)) || (self.la > 0xff03 && t.no_par > 1) {
                        loop {
                            self.la = self.raw(t);
                            if self.la == 0x0a || self.la > 0xff01 {
                                continue;
                            }
                            break;
                        }
                        self.at_line_start = false;
                        // Comes out as: 0xFF03, then the flag, the paragraph code, 0xFF03.
                        let par = &t.par_code;
                        let mut s = vec![0xff03];
                        for k in 1..=par.len() {
                            s.push(par[par.len() - k]);
                        }
                        s.push(t.flag);
                        self.stk = s;
                        return 0xff03;
                    }
                    self.at_line_start = false;
                }
                let c = self.la;
                self.la = self.raw(t);
                if c > 0xff01 {
                    return c;
                }
                if c == 0x0a {
                    break;
                }
                if t.rejoin < 1 || c != 0x2d {
                    return c;
                }
                if self.la != 0x0a {
                    if self.la != 0x2d || t.rejoin != 2 {
                        return 0x2d;
                    }
                    self.la = self.raw(t);
                    self.stk = vec![0x2d];
                    return 0x2d;
                }
                // A hyphen at the end of the line: the word is joined again.
                self.la = self.raw(t);
            }
            self.at_line_start = true;
            if t.newline == 1 {
                return 0xff03;
            }
            if t.newline == 2 {
                break;
            }
        }
        // New-line marking: 0xFF03, then the flag, "<", 0xFF03.
        self.stk = vec![0xff03, 0x3c, t.flag];
        0xff03
    }

    fn next(&mut self, t: &ScanTable) -> i64 {
        if self.rep >= 1 {
            if self.rep_kind == 4 {
                self.rep -= 1;
                return 0x20;
            }
            if t.flag == 0x1c {
                self.rep -= 1;
                return 0x1e;
            }
            if self.rep_kind == t.flag {
                self.rep_kind = 0x27;
                return t.flag;
            }
            self.rep_kind = t.flag;
            self.rep -= 1;
            return 0x27;
        }
        if let Some(c) = self.stk2.pop() {
            return c;
        }
        let mut once = false;
        if self.cc > 0 {
            once = self.cc == 1;
            self.cc -= 1;
        }
        loop {
            let c = self.la2;
            self.la2 = self.line(t);
            if c < 0xff02 {
                self.was_stop = c == t.fullstop;
                return c;
            }
            let mut n = c - 0xff02;
            while self.la2 > 0xff01 {
                n += self.la2 - 0xff02;
                if n > 0xf9 {
                    n -= 0xfa;
                }
                self.la2 = self.line(t);
            }
            if (t.spaces == 0 || t.spaces == 1) && self.la2 == 0x17 {
                self.cc = 3;
            }
            if once {
                continue;
            }
            if n == 1 && t.spaces != 3 {
                return 0x20;
            }
            if n == 0 {
                continue;
            }
            match t.spaces {
                0 => return 0x20,
                1 => {
                    if !self.was_stop {
                        return 0x20;
                    }
                    self.was_stop = false;
                    if t.flag == 0x1c {
                        self.rep = n - 1;
                        self.rep_kind = t.flag;
                        return 0x20;
                    }
                    self.stk2 = vec![0x20, 0x27];
                    return t.flag;
                }
                2 | 4 => {
                    self.rep = n - 1;
                    self.rep_kind = t.flag;
                    return 0x20;
                }
                3 => {
                    if t.flag == 0x1c {
                        self.stk2 = vec![0x1f, 0x3e];
                        return 0x1c;
                    }
                    self.stk2 = vec![0x20, 0x3e, t.flag];
                    return 0x20;
                }
                _ => continue,
            }
        }
    }
}

/// A word record.
#[derive(Clone)]
struct Word {
    /// Where the word's characters start in the shared character buffer.
    start: usize,
    url: i64,
    /// One more than the place of the hyphen of a doubled word (0: none).
    hyph_pos: usize,
    hyph_n: i64,
    /// The length of the doubled part.
    dup: usize,
    /// `hyph_pos` kept once the code in front of the first copy is written (0: none).
    f10: usize,
    chars: Vec<i64>,
    term: i64,
    /// -1 no letter yet, 0 has small letters, 1 capitals only, 2 format word, 3 two-character format word.
    kind: i64,
    after_space: i64,
    ends_passage: i64,
}

impl Word {
    fn new(start: usize) -> Word {
        Word {
            start,
            url: 0,
            hyph_pos: 0,
            hyph_n: 0,
            dup: 0,
            f10: 0,
            chars: Vec::new(),
            term: 0,
            kind: -1,
            after_space: 0,
            ends_passage: 0,
        }
    }
}

/// A font while scanning: 0 off, 1 its code in front of each word, 2 a passage about to start, 3 a passage running.
struct FontState {
    font: Font,
    state: u8,
}

struct Scanner<'a> {
    t: ScanTable,
    styles: Option<&'a StyleTable>,
    fonts: Vec<FontState>,
    /// Where the second font check goes on from.
    font_k: usize,
    /// The next free place in the shared character buffer, and the ring slot of the next word to write.
    pos: usize,
    head: usize,
    rd: Reader<'a>,
    /// The output characters: [0] flag, [1] capital sign, [2] two-character format word, [3] format word,
    /// [4] capital termination, [8] the nxsls setting.
    c: [i64; 9],
    ring: std::collections::VecDeque<Word>,
    end: bool,
    last: i64,
    split_next: bool,
    flag_seen: bool,
    nwncb: bool,
    /// 0 none, 1 capital word, 2 capital passage.
    caps: i64,
    caps_enabled: bool,
    cessation_on: i64,
    queue: std::collections::VecDeque<i64>,
    /// Where each output code goes: the caller's vector of codes, or a file's bytes.
    out: Box<dyn FnMut(i64) + 'a>,
}

impl<'a> Scanner<'a> {
    fn new(t: ScanTable, codes: Box<dyn Iterator<Item = i64> + 'a>, eof: i64, styles: Option<&'a StyleTable>, out: Box<dyn FnMut(i64) + 'a>) -> Scanner<'a> {
        let rd = Reader::new(&t, codes, eof);
        // Capital signs are written only when words are folded and the table has a capital sign.
        let caps_enabled = t.fold_words != 2 && t.out_cap != 0;
        let fonts = t.fonts.iter().map(|f| FontState { font: f.clone(), state: 0 }).collect();
        let mut c = [0i64; 9];
        c[0] = t.flag;
        c[1] = t.out_cap;
        c[2] = t.out_format2;
        c[3] = t.out_format;
        c[4] = t.out_capterm;
        c[8] = t.nxsls;
        let cessation_on = t.cessation;
        Scanner {
            t,
            styles,
            fonts,
            font_k: 0,
            pos: 0,
            head: 0,
            rd,
            c,
            ring: Default::default(),
            end: false,
            // Starts as if after a space.
            last: 0x20,
            split_next: false,
            flag_seen: false,
            nwncb: false,
            caps: 0,
            caps_enabled,
            cessation_on,
            queue: Default::default(),
            out,
        }
    }

    /// The format word, after its flag, is `name` (small and capital letters alike).
    fn name_is(w: &Word, name: &str) -> bool {
        let got = &w.chars[1.min(w.chars.len())..];
        got.len() == name.chars().count()
            && got.iter().zip(name.chars()).all(|(&c, n)| c < 0x100 && to_lower(c) == n.to_ascii_lowercase() as i64)
    }

    /// For a style's begin word [es~name] or end word [ee~name], when the document's styles are known: the style
    /// name (from the word's fifth character) and which codes (0 begin, 1 end).
    fn style_word(&self, w: &Word) -> Option<(Vec<u8>, i64)> {
        self.styles?;
        if w.chars.len() < 4
            || to_lower(w.chars[1]) != 0x65
            || !(to_lower(w.chars[2]) == 0x73 || to_lower(w.chars[2]) == 0x65)
            || w.chars[3] != 0x7e
        {
            return None;
        }
        let name: Vec<u8> = w.chars[4..]
            .iter()
            .take_while(|&&c| c != 0 && c != 0x1f && c != 0x20 && c != 0x60)
            .take(39)
            .map(|&c| (c & 0xff) as u8)
            .collect();
        Some((name, if to_lower(w.chars[2]) == 0x65 { 1 } else { 0 }))
    }

    fn style_codes(&self, name: &[u8], which: i64) -> Option<Vec<u8>> {
        self.styles?.lookup(name, which)
    }

    /// A code in a style's codes: mode 0 a code that starts with `name`, mode 2 the code `name` itself (followed
    /// by 0x1F, a space or a backquote).
    fn find_code(&self, codes: Option<&[u8]>, name: &str, mode: u8) -> bool {
        let Some(codes) = codes else { return false };
        let s: Vec<i64> = codes.iter().map(|&b| i64::from(b)).chain([0]).collect();
        let n: Vec<i64> = name.chars().map(|c| c.to_ascii_lowercase() as i64).chain([0]).collect();
        let at = |v: &Vec<i64>, k: usize| v.get(k).copied().unwrap_or(0);
        let mut i = 0;
        loop {
            if at(&s, i) == 0 {
                return false;
            }
            while at(&s, i) != 0 && at(&s, i) != self.t.flag {
                i += 1;
            }
            if at(&s, i) == 0 {
                return false;
            }
            i += 1;
            let mut j = 0;
            loop {
                let a = at(&s, i);
                let a = if a < 0x80 { to_lower(a) } else { a };
                if a != at(&n, j) || a == 0 {
                    break;
                }
                i += 1;
                j += 1;
                if at(&s, i) == 0x7e && mode == 1 {
                    return true;
                }
            }
            if at(&n, j) == 0 && (mode == 0 || matches!(at(&s, i), 0x1f | 0x20 | 0x60)) {
                return true;
            }
        }
    }

    /// The word is the font's end code, or the end word of a style whose end codes hold it.
    fn is_font_end(&self, w: &Word, end: &str) -> bool {
        if Self::name_is(w, end) {
            return true;
        }
        match self.style_word(w) {
            Some((name, 1)) => self.find_code(self.style_codes(&name, 1).as_deref(), end, 2),
            _ => false,
        }
    }

    /// True when the font's end code comes, or the waiting words run out, before `n` words that follow a space
    /// (format words and words joined to the one before are not counted).
    fn end_before(&self, end: &str, n: i64) -> bool {
        let mut count = 0;
        for w in &self.ring {
            if w.kind > 1 || w.after_space == 0 {
                if w.kind == 2 && self.is_font_end(w, end) {
                    return true;
                }
                continue;
            }
            count += 1;
            if count == n {
                return false;
            }
        }
        true
    }

    /// The next waiting word that is not a format word starts with "_" (looked for up to the ring's last slot,
    /// without going round).
    fn next_underscore(&self) -> bool {
        for (k, w) in self.ring.iter().enumerate() {
            if self.head + k > 24 {
                return false;
            }
            if w.kind < 2 {
                return w.chars.first() == Some(&0x5f);
            }
        }
        false
    }

    /// The doubled part of a word: the longest run of letters before the hyphen equal to the run after it
    /// (small and capital letters alike; literal pairs skipped). Keeps the word's last value when none is longer.
    fn dup_len(w: &mut Word) -> usize {
        let m = w.hyph_pos as i64 - 1;
        let len = w.chars.len() as i64;
        let limit = m.min(len - m);
        let ch = |k: i64| if k >= 0 && k < len { w.chars[k as usize] } else { 0 };
        let mut dup = w.dup;
        for k in 1..=limit {
            let mut bad = false;
            let (mut x, mut j) = (0i64, 0i64);
            'compare: while x < k {
                while ch(m - k + x) == 0x17 {
                    x += 2;
                    if k <= x {
                        break 'compare;
                    }
                }
                while ch(w.hyph_pos as i64 + j) == 0x17 {
                    j += 2;
                    if len < w.hyph_pos as i64 + j {
                        break 'compare;
                    }
                }
                let a = ch(m - k + x);
                if to_lower(a) != to_lower(ch(w.hyph_pos as i64 + j)) || a == 0x2d || !is_letter(a) {
                    bad = true;
                }
                j += 1;
                x += 1;
            }
            if !bad {
                dup = k as usize;
            }
        }
        w.dup = dup;
        dup
    }

    /// Moves to the hyphen after the current one.
    fn next_hyphen(w: &mut Word) {
        if let Some(k) = (w.hyph_pos..w.chars.len()).find(|&k| w.chars[k] == 0x2d) {
            w.hyph_pos = k + 1;
        }
    }

    /// The character is written literally: it is 0x17 or follows one.
    fn literal(w: &Word, c: i64) -> bool {
        if w.chars.last() == Some(&0x17) {
            return true;
        }
        c == 0x17
    }

    /// Splits the input into words until 25 are waiting or the input ends; sooner only when the character buffer
    /// has come round to the next word to write (closer than 0x529 places to it, or with a code still open).
    /// False when the input had already ended.
    fn fill(&mut self) -> bool {
        if self.end {
            return false;
        }
        let mut reuse: Option<Word> = None;
        loop {
            let mut w = match reuse.take() {
                None => {
                    if self.end || self.ring.len() == 25 {
                        return true;
                    }
                    if let Some(next) = self.ring.front() {
                        if self.pos <= next.start && (next.start - self.pos < 0x529 || self.flag_seen) {
                            return true;
                        }
                    }
                    Word::new(self.pos)
                }
                Some(old) => Word::new(old.start),
            };
            let mut url_state: i64 = 0;
            let nwncb_before = self.nwncb;
            self.nwncb = false;
            if self.last == 0x20 || self.last == 0x1f || self.split_next {
                self.split_next = false;
                w.after_space = 1;
            } else {
                w.chars = vec![self.last];
                w.after_space = 0;
                if is_upper(self.last) {
                    w.kind = 1;
                } else if is_lower(self.last) {
                    w.kind = 0;
                }
            }
            let c0 = self.c[0];
            loop {
                let c = self.rd.code(&self.t);
                self.last = c;
                if c == 0x1c {
                    self.flag_seen = true;
                }
                if c == 0x1f {
                    self.flag_seen = false;
                    break;
                }
                if c == 0x20 || c == 0xff00 || c == 0xff01 {
                    break;
                }
                let n = w.chars.len();
                if w.kind < 2 {
                    if n == 1 && w.chars[0] == c0 {
                        if c0 == 0x1c || (((c < 0x80 && c != 0x2c && c != 0x2e) || c == 0xf8) && !is_digit(c)) {
                            if !is_letter(c) && c != 0x23 && c != 0xf8 && c0 != 0x1c {
                                w.kind = 3;
                                w.chars[0] = self.c[2];
                            } else {
                                w.kind = 2;
                                w.chars[0] = self.c[3];
                            }
                        }
                    } else if n > 0 {
                        if c == c0 || (c0 == 0x1c && (c == 0x1d || c == 0x1e)) {
                            break;
                        }
                        if self.cessation_on == 1 && !Self::literal(&w, c) {
                            if w.kind == -1 {
                                if is_lower(c) || is_upper(c) {
                                    break;
                                }
                            } else if !is_lower(c) && !is_upper(c) && c == 0x27 {
                                break;
                            }
                        }
                    }
                } else if w.kind == 2 {
                    if c == 0x60 {
                        self.split_next = true;
                    }
                    if c == 0x1f || c == 0x60 {
                        break;
                    }
                } else if w.kind == 3 && n == 2 {
                    break;
                }
                if w.start + w.chars.len() > 0xc7e {
                    // The word reaches the end of the character buffer.
                    break;
                }
                if w.kind == -1 {
                    if !Self::literal(&w, c) {
                        if is_upper(c) {
                            w.kind = 1;
                        } else if is_lower(c) {
                            w.kind = 0;
                        }
                    }
                } else if w.kind == 1 && !Self::literal(&w, c) && is_lower(c) {
                    w.kind = 0;
                }
                w.chars.push(c);
                let len = w.chars.len();
                if c != 0x17 && (len == 1 || w.chars[len - 2] != 0x17) {
                    if c == 0x2d && self.t.dbl_adj != 0 {
                        w.hyph_n += 1;
                        if w.hyph_pos == 0 {
                            w.hyph_pos = len;
                        }
                    }
                    // Web and mail address detection, used only with computer-braille switching.
                    let k = (if self.t.icb != 0 { 1 } else { 0 }) + url_state - 1;
                    if k == 0 || k == 3 {
                        if 0x2f < c && c < 0x3a {
                            url_state = 3;
                        }
                        if c == 0x40 || (c == 0x3a && url_state == 0) {
                            url_state = 1;
                        }
                    } else if k == 1 {
                        if c == 0x2e {
                            url_state = 2;
                        }
                    } else if k == 2 {
                        url_state = -1;
                    }
                }
            }
            if self.last == 0xff00 || self.last == 0xff01 {
                self.end = true;
            }
            w.term = self.last;
            if w.kind > 1 {
                if let Some(slot) = self.t.modes.iter().position(|m| Self::name_is(&w, m)) {
                    // The mode words: line ends, spaces and paragraphs change while reading.
                    match slot {
                        0..=2 => self.t.newline = slot as i64,
                        3 => self.t.spaces = 2,
                        4 => self.t.spaces = 0,
                        5 => self.t.spaces = 1,
                        6 => self.t.spaces = 3,
                        7 => self.t.no_par = 3,
                        8 => self.t.no_par = 0,
                        9 => self.t.no_par = 1,
                        _ => self.t.no_par = 2,
                    }
                    reuse = Some(w);
                    continue;
                }
                if Self::name_is(&w, "nwncb") {
                    self.nwncb = true;
                    reuse = Some(w);
                    continue;
                }
                if let Some(word) = ["cpe-l", "cpe-l-off", "cpe-fts", "cpe-fts-off"].into_iter().find(|x| Self::name_is(&w, x)) {
                    match word {
                        "cpe-l" => self.t.cpe_l = 1,
                        "cpe-fts" => self.t.cpe_fts = 1,
                        // Both "off" words clear the same setting.
                        _ => self.t.cpe_l = 0,
                    }
                    reuse = Some(w);
                    continue;
                }
                if self.t.triple_caps == 0 {
                    w.ends_passage = 1;
                } else if let Some(codes) = self.style_word(&w).map(|(name, which)| self.style_codes(&name, which)) {
                    // A style's word ends a passage when one of the style's codes does (its first four letters).
                    let codes: Vec<i64> = codes.unwrap_or_default().into_iter().map(i64::from).chain([0]).collect();
                    let mut ends = false;
                    let mut i = 0;
                    while codes[i] != 0 && !ends {
                        if codes[i] == 0x1c {
                            let mut short = Vec::new();
                            loop {
                                i += 1;
                                if !(short.len() < 4 && codes[i] != 0 && codes[i] != 0x7e && codes[i] != 0x1f) {
                                    break;
                                }
                                short.push(to_lower(codes[i]));
                            }
                            ends = code_ends_passage(&text_of(&short), self.t.cpe_fts, self.t.cpe_l);
                            if ends || codes[i] == 0 {
                                break;
                            }
                        }
                        i += 1;
                    }
                    w.ends_passage = ends as i64;
                } else {
                    let low: Vec<i64> = w.chars.iter().map(|&x| to_lower(x)).collect();
                    let mut short = Vec::new();
                    for &x in low.iter().skip(1).take(4) {
                        if x >= 0x100 || x == 0x7e || x == 0x1f || x == 0x20 {
                            break;
                        }
                        short.push(x);
                    }
                    w.ends_passage = code_ends_passage(&text_of(&short), self.t.cpe_fts, self.t.cpe_l) as i64;
                }
            }
            if self.t.icb != 0 && !nwncb_before && url_state == -1 {
                w.url = 1;
            }
            self.pos += w.chars.len() + 1;
            if self.pos > 0x755 {
                self.pos = 0;
            }
            // A two-character format word whose second character is "$" becomes the flag alone, an ordinary word.
            if w.kind == 3 && w.chars.get(1) == Some(&0x24) {
                w.chars = vec![self.c[0]];
                w.kind = 0;
            }
            self.ring.push_back(w);
        }
    }

    /// True unless `n` capitals-only words come before a word with small letters or a code that ends a passage.
    fn caps_break(&self, n: i64) -> bool {
        let mut count = 0;
        for w in &self.ring {
            if w.kind >= 2 {
                if w.ends_passage != 0 {
                    return true;
                }
                continue;
            }
            if w.kind == 0 {
                return true;
            }
            if w.kind == 1 {
                count += 1;
                if count == n {
                    return false;
                }
            }
        }
        true
    }

    fn next_not_caps(w: &Word, i: usize) -> bool {
        if i >= w.chars.len() {
            return false;
        }
        if w.chars[i] == 0x17 && i + 2 >= w.chars.len() {
            return true;
        }
        !is_upper(w.chars[i]) && !case_neutral(w.chars[i])
    }

    /// Case folding: 0 to capitals (within the table's range), 1 to small letters, 2 as written.
    fn fold(&self, c: i64, mode: i64) -> i64 {
        match mode {
            0 if self.t.fold_lo <= c && c <= self.t.fold_hi => c - 0x20,
            1 => to_lower(c),
            _ => c,
        }
    }

    /// Writes a code (a two-byte code first byte first), then whatever was queued.
    fn put(&mut self, c: i64) {
        if c > 0xff {
            (self.out)(c >> 8);
            (self.out)(c & 0xff);
        } else {
            (self.out)(c);
        }
        while let Some(q) = self.queue.pop_front() {
            (self.out)(q);
        }
    }

    /// Queues a string after whatever is queued and writes its first character, then the rest.
    fn put_str(&mut self, s: &[i64]) {
        self.queue.extend(s.iter().copied());
        if let Some(first) = self.queue.pop_front() {
            self.put(first);
        }
    }

    fn end_of_input(&mut self) {
        if self.t.spare > 1 {
            (self.out)(0xff);
            (self.out)(0x7f);
        }
        (self.out)(EOF_OUT);
    }

    fn run(&mut self) -> Result<(), EngineError> {
        if self.t.ninmod == 1 {
            // The codes pass through as read, with no words made, then the end code.
            loop {
                let c = self.rd.code(&self.t);
                if c == 0xff00 || c == 0xff01 {
                    (self.out)(EOF_OUT);
                    return Ok(());
                }
                (self.out)(c);
            }
        }
        if self.t.spare == 1 || self.t.spare == 3 {
            // The spare marking in front of the text.
            (self.out)(0xff);
            (self.out)(0x80);
        }
        loop {
            let more = self.fill();
            if !more && self.ring.is_empty() {
                self.end_of_input();
                return Ok(());
            }
            let Some(mut w) = self.ring.pop_front() else {
                self.end_of_input();
                return Ok(());
            };
            self.head = (self.head + 1) % 25;
            let mut i = 0usize;
            let ordinary;
            if w.kind == 1 && self.t.triple_caps != 0 && self.caps != 2 && !self.caps_break(self.t.triple_caps) {
                // A capital passage starts: three capital signs.
                self.caps = 2;
                self.queue.push_back(self.c[1]);
                self.queue.push_back(self.c[1]);
                self.put(self.c[1]);
                ordinary = true;
            } else {
                ordinary = w.kind < 2;
                if ordinary && self.caps == 1 {
                    self.caps = 0;
                }
            }
            if ordinary {
                if w.url != 0 {
                    // An address: the computer-braille start in front of the word and its font codes.
                    self.put_str(&CB_START);
                }
                // The font codes in front of the word.
                loop {
                    if let Some(k) = self.fonts.iter().position(|f| f.state == 2) {
                        // A passage starts: the code twice.
                        let code = self.fonts[k].font.code;
                        self.put(code);
                        self.fonts[k].state = 3;
                        self.put(code);
                        continue;
                    }
                    let mut hit = None;
                    for k in self.font_k..self.fonts.len() {
                        let (state, end) = (self.fonts[k].state, self.fonts[k].font.end.clone());
                        if w.after_space != 0 && state == 1 {
                            // Word by word: the code in front of each word, until the last before the end code.
                            self.font_k = k + 1;
                            if self.end_before(&end, 1) {
                                self.fonts[k].state = 0;
                            }
                            hit = Some(k);
                            break;
                        }
                        if state == 3 && self.end_before(&end, 1) {
                            // The last word of a passage.
                            self.fonts[k].state = 0;
                            self.font_k = k + 1;
                            hit = Some(k);
                            break;
                        }
                    }
                    match hit {
                        Some(k) => {
                            let code = self.fonts[k].font.code;
                            self.put(code);
                        }
                        None => {
                            self.font_k = 0;
                            break;
                        }
                    }
                }
                loop {
                    if w.f10 != 0 && i == w.chars.len() {
                        // The doubled word ends with the word: the code after the second copy.
                        w.f10 = 0;
                        w.dup = 0;
                        self.put(self.t.dbl_adj_after);
                        continue;
                    }
                    if i == w.chars.len() {
                        break;
                    }
                    let c = w.chars[i];
                    i += 1;
                    if c == 0x17 && i < w.chars.len() {
                        self.queue.push_back(w.chars[i]);
                        i += 1;
                        self.put(c);
                        continue;
                    }
                    if i == w.chars.len() && w.url != 0 && (c == 0x2e || c == 0x2c) {
                        // A full stop or comma ending an address is left outside it.
                        w.url = 0;
                        i -= 1;
                        self.put_str(&CB_END);
                        continue;
                    }
                    // A doubled word ("anak-anak"): the table's code in front of the first copy and the next code after
                    // the second.
                    if w.hyph_pos != 0 && w.chars.get(w.hyph_pos).is_some_and(|&h| is_letter(h)) {
                        Self::dup_len(&mut w);
                    }
                    if w.dup < 2 && w.hyph_n > 1 {
                        while w.hyph_n > 0 && w.dup < 2 {
                            Self::next_hyphen(&mut w);
                            Self::dup_len(&mut w);
                            w.hyph_n -= 1;
                        }
                    }
                    if w.dup < 2 {
                        w.dup = 0;
                        w.hyph_pos = 0;
                    } else if w.hyph_pos != 0 && i == w.hyph_pos - w.dup {
                        w.f10 = w.hyph_pos;
                        w.hyph_pos = 0;
                        i -= 1;
                        self.put(self.t.dbl_adj);
                        continue;
                    } else if w.f10 != 0 && i == w.f10 + 1 + w.dup {
                        w.f10 = 0;
                        w.dup = 0;
                        i -= 1;
                        self.put(self.t.dbl_adj_after);
                        continue;
                    }
                    let mut out_first = None;
                    let fold_words = self.t.fold_words;
                    if self.caps == 0 {
                        if (fold_words == 0 || fold_words == 1) && is_upper(c) && self.caps_enabled {
                            if i == w.chars.len() || Self::next_not_caps(&w, i) {
                                // A capital letter.
                                out_first = Some(self.c[1]);
                            } else {
                                // A capital word: the sign twice.
                                out_first = Some(self.c[1]);
                                self.caps = 1;
                                self.queue.push_back(self.c[1]);
                            }
                        }
                    } else if !(fold_words == 0 || fold_words == 1) || is_lower(c) {
                        if self.caps == 1 {
                            self.caps = 0;
                        }
                        if self.c[4] != 0 {
                            out_first = Some(self.c[4]);
                        }
                    } else if (fold_words == 0 || fold_words == 1)
                        && self.cessation_on == 1
                        && !is_upper(c)
                        && !case_neutral(c)
                        && self.caps == 1
                    {
                        self.caps = 0;
                    }
                    if let Some(f) = out_first {
                        self.put(f);
                    }
                    let folded = self.fold(c, fold_words);
                    self.put(folded);
                }
            } else {
                // Format words pass through, folded as the table says.
                let lg = w.kind == 2
                    && w.chars.len() > 3
                    && to_lower(w.chars[1]) == 0x6c
                    && to_lower(w.chars[2]) == 0x67
                    && w.chars[3] == 0x7e;
                if w.kind == 2 {
                    let codes = self.style_word(&w).and_then(|(name, which)| self.style_codes(&name, which));
                    // A font starts at its start code (or a style whose codes hold it), unless the next word
                    // starts with "_": word by word when its end comes within its number of words, else a passage.
                    for k in 0..self.fonts.len() {
                        let (start, end, words) =
                            (self.fonts[k].font.start.clone(), self.fonts[k].font.end.clone(), self.fonts[k].font.words);
                        if (Self::name_is(&w, &start) || self.find_code(codes.as_deref(), &start, 2))
                            && !self.next_underscore()
                        {
                            self.fonts[k].state = if words == 0 || self.end_before(&end, words) { 1 } else { 2 };
                        }
                    }
                    if self.t.nxsls != 0 {
                        let first = |k: usize| w.chars.get(k).map_or(0, |&c| to_lower(c));
                        if !self.find_code(codes.as_deref(), "cb", 0) && (first(1) != 0x63 || first(2) != 0x62) {
                            if self.find_code(codes.as_deref(), "tx", 0) || (first(1) == 0x74 && first(2) == 0x78) {
                                self.cessation_on = 1;
                            }
                        } else {
                            self.cessation_on = 0;
                        }
                    }
                }
                for (k, &c) in w.chars.iter().enumerate() {
                    let c = if !lg || k + 1 < 5 { self.fold(c, self.t.fold_controls) } else { c };
                    self.put(c);
                }
            }
            if self.caps == 2 && self.caps_break(1) {
                // The capital passage ends.
                self.caps = 0;
                self.put(self.c[4]);
            }
            if w.url != 0 {
                // The address ends with the word.
                w.url = 0;
                self.put_str(&CB_END);
            }
            if w.term == 0xff00 || w.term == 0xff01 {
                self.end_of_input();
                return Ok(());
            }
            if w.term == 0x20 || w.term == 0x1f || w.term == 0x60 {
                self.put(w.term);
            }
        }
    }
}

/// The scanner's output for internal text given as its bytes (two-byte codes as their two bytes, first byte
/// first), the end code left out: the codes the rule engine reads. The bytes are read one at a time, so the text is
/// never held as eight-byte codes before the scan.
pub(crate) fn scan_bytes(t: &ScanTable, bytes: Vec<u8>, styles: Option<&StyleTable>) -> Result<Vec<i64>, EngineError> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 8);
    {
        let sink = |c: i64| {
            if c != super::chars::EOF {
                out.push(c);
            }
        };
        let mut s = Scanner::new(t.clone(), Box::new(bytes.into_iter().map(i64::from)), 0xff01, styles, Box::new(sink));
        s.run()?;
    }
    Ok(out)
}

/// The codes the scanner reads from the bytes of a file, one at a time: a line end (CR, LF, CR LF or LF CR) is one
/// LF; a byte marked as the first of a two-byte code makes one code with the byte after it (a later first byte
/// starts it again; after 0x9C, a byte with a one-byte capital gives that capital, any other byte stays 0x9C and
/// the byte); a first byte at the end of the file is dropped.
struct FileCodes<'a> {
    bytes: &'a [u8],
    i: usize,
    lead: Option<i64>,
}

impl FileCodes<'_> {
    /// The next byte as a code, line ends joined; None at the end.
    fn line_code(&mut self) -> Option<i64> {
        let c = *self.bytes.get(self.i)?;
        self.i += 1;
        if c == b'\n' || c == b'\r' {
            // The other line-end byte right after it belongs to the same line end.
            if let Some(&d) = self.bytes.get(self.i) {
                if (d == b'\n' || d == b'\r') && d != c {
                    self.i += 1;
                }
            }
            Some(0x0a)
        } else {
            Some(i64::from(c))
        }
    }
}

impl Iterator for FileCodes<'_> {
    type Item = i64;

    fn next(&mut self) -> Option<i64> {
        loop {
            let c = self.line_code()?;
            if super::chars::flags(c) & 0x20 != 0 {
                self.lead = Some(c << 8);
                continue;
            }
            return Some(match self.lead.take() {
                None => c,
                Some(0x9c00) => {
                    let upper = super::chars::kind(c).2;
                    if upper == 0x9c {
                        0x9c00 | c
                    } else {
                        upper
                    }
                }
                Some(l) => l | c,
            });
        }
    }
}

#[cfg(test)]
fn file_codes(bytes: &[u8]) -> Vec<i64> {
    FileCodes { bytes, i: 0, lead: None }.collect()
}

/// The import scan of a file: the bytes read through the scanner with `t` (no styles), each code it gives written
/// as its low byte until the end. The reference uses it on braille and print text files before the import rules.
/// The bytes are read and the codes written one at a time, so the file is never held as codes.
pub(crate) fn scan_file(t: &ScanTable, bytes: &[u8]) -> Result<Vec<u8>, EngineError> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 8);
    let mut ended = false;
    {
        let sink = |c: i64| {
            if c == EOF_OUT {
                ended = true;
            } else if !ended {
                out.push((c & 0xff) as u8);
            }
        };
        // The byte modes read the file's bytes as they are; text mode reads them as codes.
        let codes: Box<dyn Iterator<Item = i64>> = if t.binmod == 0 {
            Box::new(FileCodes { bytes, i: 0, lead: None })
        } else {
            Box::new(bytes.iter().map(|&b| i64::from(b)))
        };
        let mut s = Scanner::new(t.clone(), codes, 0xff7f, None, Box::new(sink));
        s.run()?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str) -> ScanTable {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/scan").join(name);
        ScanTable::read(&std::fs::read(path).unwrap()).unwrap()
    }

    fn run(name: &str, input: &[u8]) -> Vec<u8> {
        scan_file(&table(name), input).unwrap()
    }

    #[test]
    fn file_codes_join_line_ends_and_two_byte_codes() {
        assert_eq!(file_codes(b"a\r\nb\n\rc\r\rd"), vec![0x61, 0x0a, 0x62, 0x0a, 0x63, 0x0a, 0x0a, 0x64]);
        // 0x9C before a letter with a one-byte capital gives that capital; a first byte at the end is dropped.
        assert_eq!(file_codes(&[0x9c, 0x82, 0xb0, 0x41, 0xb0]), vec![0x90, 0xb041]);
        assert_eq!(file_codes(&[0x9c, 0x83]), vec![0x9c83]);
    }

    #[test]
    fn braille_table_reads_format_words_and_bars() {
        let t = table("import-braille.sct");
        // The flag of the braille table is the bar, read from a line that starts with two bars.
        assert_eq!(t.flag, 0x7c);
        assert_eq!(run("import-braille.sct", b"|P This is |IND5 a test."), b"\x1dp this is \x1dind5 a test.".to_vec());
        assert_eq!(run("import-braille.sct", b"A|$B ||C |:D"), b"a|b \x1e|c \x1e:d".to_vec());
        assert_eq!(run("import-braille.sct", b"ABC\r\rDEF  GHI\tJ"), b"abc \x1dp def ghi j".to_vec());
    }

    /// A word with "@" or ":" then "." and more is an address: with icb on it goes between the computer-braille
    /// start and end strings, a full stop or comma ending it left outside; with icb off nothing changes.
    #[test]
    fn addresses_go_between_the_computer_braille_codes() {
        let mut t = table("import-print.sct");
        let scan = |t: &ScanTable, text: &str| -> Vec<u8> {
            scan_bytes(t, text.bytes().collect(), None).unwrap().iter().map(|&c| c as u8).collect()
        };
        assert_eq!(scan(&t, "mail me@example.org, then"), b"mail me@example.org, then".to_vec());
        t.icb = 1;
        let cases: [(&str, &[u8]); 5] = [
            ("see http://example.com now", b"see \x0ec\x0fhttp://example.com\x0e/c\x0f now"),
            ("see http://example.com.", b"see \x0ec\x0fhttp://example.com\x0e/c\x0f."),
            ("mail me@example.org, then", b"mail \x0ec\x0fme@example.org\x0e/c\x0f, then"),
            ("see www.example.com.", b"see www.example.com."),
            ("a:b.c d", b"\x0ec\x0fa:b.c\x0e/c\x0f d"),
        ];
        for (text, want) in cases {
            assert_eq!(scan(&t, text), want.to_vec(), "{text}");
        }
    }

    /// Input mode 1 reads bytes with line ends joined and mode 2 bytes as they are, both without the text layers
    /// (runs of spaces kept); ninmod passes the codes through with no words made; spare 1 and 3 write FF 80 in
    /// front of the text, spare 2 and 3 FF 7F at its end.
    #[test]
    fn byte_modes_pass_through_and_spare_marks() {
        let base = table("import-print.sct");
        let cases: [((i64, i64, i64), &[u8], &[u8]); 7] = [
            ((1, 0, 0), b"Ab  c\nd\ne", b"Ab  c\nd"),
            ((2, 0, 0), b"Ab  c\r\nd\re", b"Ab  c\r\nd"),
            ((2, 1, 0), b"Ab  c\r\nd\re", b"Ab  c\r\nd"),
            ((0, 1, 0), b"Ab c\r d\re", b"Ab c d"),
            ((0, 0, 1), b"\xff\x80Ab c\r d\re", b"\xff\x80Ab c d"),
            ((0, 0, 3), b"\xff\x80Ab c\r d\re\xff\x7f", b"\xff\x80Ab c d\xff\x7f"),
            ((0, 0, 2), b"Ab c\r d\re\xff\x7f", b"Ab c d\xff\x7f"),
        ];
        for ((mode, nin, spare), codes, file) in cases {
            let mut t = base.clone();
            (t.binmod, t.ninmod, t.spare) = (mode, nin, spare);
            let out: Vec<u8> = scan_bytes(&t, b"Ab  c\r\nd\re".to_vec(), None).unwrap().iter().map(|&c| c as u8).collect();
            assert_eq!(out, codes.to_vec(), "codes, mode {mode} ninmod {nin} spare {spare}");
            assert_eq!(scan_file(&t, b"Ab  c\r\nd").unwrap(), file.to_vec(), "file, mode {mode} ninmod {nin} spare {spare}");
        }
    }

    #[test]
    fn print_table_joins_hyphens_and_reads_dollar_words() {
        let input = b"$p This is a $ind5 test with $$5 and caf\x82.\r\n\r\n  New para, tab\there.\r\nhyphen-\r\nated |bar|";
        assert_eq!(
            run("import-print.sct", input),
            b"\x1dp This is a \x1dind5 test with $5 and caf\x82. \x1dp New para, tab here. hyphenated |bar|".to_vec()
        );
        assert_eq!(run("import-print.sct", b"a--\nb"), b"a-b".to_vec());
        assert_eq!(run("import-print.sct", b"a- \nb"), b"ab".to_vec());
        assert_eq!(run("import-print.sct", b"$WB-NO x"), b"\x1dwb-no x".to_vec());
        assert_eq!(run("import-print.sct", &[0x9c, 0x82]), vec![0x90]);
    }
}
