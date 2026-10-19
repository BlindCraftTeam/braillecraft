//! The HTML and SGML import's own steps (the HTML recipe in import_chain): the kind test, the character set a file
//! names (tables/import/html-charsets.txt), the character-set step, and the tag scanner that follows the control
//! character step. The scanner reads the text as bytes through a window of 599 bytes and checks nothing about the
//! markup: a tag is copied as it is written, so a page whose tags are not closed, or not closed in order, imports as
//! well as one that is (spec B22).

use crate::word::{Options, PrintPages};

/// How much of a file the kind test and the character set read: its first 2048 bytes.
pub const SAMPLE: usize = 0x800;
/// The scanner's window, and how far into it the scanner gets before the window moves on to where it is.
const WINDOW: usize = 599;
const MOVE_AFTER: usize = 100;

const CHARSETS: &str = include_str!("../../../tables/import/html-charsets.txt");

/// The sample as text: up to its first 0 byte.
fn as_text(sample: &[u8]) -> &[u8] {
    &sample[..sample.iter().position(|&b| b == 0).unwrap_or(sample.len())]
}

/// How many "</" and "<!--" a sample holds (counted at every byte of it, read as text).
pub fn tag_marks(sample: &[u8]) -> usize {
    let s = as_text(sample);
    (0..s.len()).filter(|&i| s[i..].starts_with(b"</") || s[i..].starts_with(b"<!--")).count()
}

/// The same count over the file's first 1023 UTF-16 units (little-endian), up to the first 0 unit.
fn tag_marks_utf16(bytes: &[u8]) -> usize {
    let units: Vec<u16> =
        bytes.chunks_exact(2).take(1023).map(|p| u16::from_le_bytes([p[0], p[1]])).take_while(|&u| u != 0).collect();
    let [lt, slash, bang, dash] = [b'<', b'/', b'!', b'-'].map(u16::from);
    (0..units.len()).filter(|&i| units[i..].starts_with(&[lt, slash]) || units[i..].starts_with(&[lt, bang, dash, dash])).count()
}

/// Whether a file reads as HTML or SGML in UTF-16: more than 3 "</" or "<!--" among its first UTF-16 units.
pub fn is_utf16_markup(bytes: &[u8]) -> bool {
    tag_marks_utf16(bytes) > 3
}

/// Whether a file is HTML or SGML: its first 2048 bytes read as text and hold more than 3 "</" or "<!--".
pub fn is_markup(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(SAMPLE)];
    crate::filekind::text_stats(sample).is_some() && tag_marks(sample) > 3
}

/// The value the kind test gives an HTML or SGML file for the character-set step: 3 when the file reads as UTF-16,
/// else the code page its first 2048 bytes name with charset= (`charset`).
pub fn submethod(bytes: &[u8]) -> u32 {
    if is_utf16_markup(bytes) {
        3
    } else {
        charset(&bytes[..bytes.len().min(SAMPLE)])
    }
}

/// The code page the sample names: the first "charset" (in any case) followed by spaces and "=", then spaces, an
/// optional double quote and spaces; the name is at most 39 bytes, up to a double quote or ">", looked up in the
/// character set table without regard to case. 0 when there is none or the name is not in the table.
pub fn charset(sample: &[u8]) -> u32 {
    let s = as_text(sample);
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut from = 0;
    let equals = loop {
        let Some(k) = s[from..].windows(7).position(|w| w.eq_ignore_ascii_case(b"charset")) else { return 0 };
        let mut p = from + k + 7;
        while at(p) == b' ' {
            p += 1;
        }
        if at(p) == b'=' {
            break p;
        }
        from = p;
    };
    let mut p = equals + 1;
    while at(p) == b' ' {
        p += 1;
    }
    if at(p) == b'"' {
        p += 1;
    }
    while at(p) == b' ' {
        p += 1;
    }
    let name: Vec<u8> = s[p.min(s.len())..].iter().take(39).take_while(|&&b| b != b'"' && b != b'>').copied().collect();
    code_page(&name)
}

/// A character set name's code page from the table; 0 when it is not listed.
fn code_page(name: &[u8]) -> u32 {
    for line in CHARSETS.lines() {
        if line.is_empty() || line.starts_with('|') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.as_bytes().eq_ignore_ascii_case(name) {
            return value.trim().bytes().take_while(u8::is_ascii_digit).fold(0u32, |n, d| n.wrapping_mul(10).wrapping_add(u32::from(d - b'0')));
        }
    }
    0
}

/// The character-set step, run when the kind test's value is not 0: the bytes read as CP437 (1), Mac Roman (2),
/// UTF-16 little-endian (3) or the code page the table gives the page's charset= name, and written as UTF-8; UTF-8
/// (65001) and a code page with no converter are read as UTF-8. Reading the named code page is BrailleCraft's own
/// (OWNER 2026-10-18): the original reads every named page as UTF-8, which garbles a windows-1251 page. A byte-order
/// mark is kept (the character step drops it). A malformed UTF-8 sequence becomes U+FFFD.
pub fn charset_step(bytes: &[u8], submethod: u32) -> Vec<u8> {
    let page = match submethod {
        1 => "WINDOWS-437",
        2 => "MacRoman",
        3 => "UTF-16LE",
        // The code pages of the table that the code page converters do not have; encoding_rs reads them.
        51932 => return encoding_rs::EUC_JP.decode_without_bom_handling(bytes).0.into_owned().into_bytes(),
        51949 => return encoding_rs::EUC_KR.decode_without_bom_handling(bytes).0.into_owned().into_bytes(),
        20936 => return encoding_rs::GBK.decode_without_bom_handling(bytes).0.into_owned().into_bytes(),
        n => match crate::codepages::name_of_page(n).filter(|&name| name != "UTF-8") {
            Some(name) => name,
            None => return String::from_utf8_lossy(bytes).into_owned().into_bytes(),
        },
    };
    let mut text = String::with_capacity(bytes.len());
    if submethod == 3 && bytes.starts_with(&[0xFF, 0xFE]) {
        text.push('\u{feff}');
    }
    text.push_str(&crate::codepages::decode(page, bytes));
    text.into_bytes()
}

/// A code point as the scanner writes a numeric reference: up to six bytes in the UTF-8 pattern, a value of
/// 0x80000000 or more as the two bytes FF BD.
fn utf8_bytes(v: u32, out: &mut Vec<u8>) {
    let (n, mut c) = match v {
        0..=0x7F => (1, v),
        0x80..=0x7FF => (2, v),
        0x800..=0xFFFF => (3, v),
        0x1_0000..=0x1F_FFFF => (4, v),
        0x20_0000..=0x3FF_FFFF => (5, v),
        0x400_0000..=0x7FFF_FFFF => (6, v),
        _ => (2, 0xFFFD),
    };
    const FIRST: [u32; 7] = [0, 0, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC];
    let mut b = [0u8; 6];
    for k in (1..n).rev() {
        b[k] = (c & 0x3F) as u8 | 0x80;
        c >>= 6;
    }
    b[0] = (c | FIRST[n]) as u8;
    out.extend_from_slice(&b[..n]);
}

/// The tag scanner over the text after the control character step (spec 28 6c with no style map; B22.7): text bytes
/// are copied, spaces before the first text dropped and the others held until the next thing written, numeric
/// references written as their characters (&, < and > as &amp;, &lt; and &gt;), named references copied; every tag
/// is copied as it is written, from its "<" to the first ">" outside double quotes, end tags, comments, declarations
/// and processing instructions alike, except the code instruction <?bc ...?> and the bc: and bcml: elements the
/// scanner reads by name.
pub fn scan(input: &[u8], opt: &Options) -> Vec<u8> {
    let scanned = opt.print_pages == PrintPages::Scanned;
    let we = input.len().min(WINDOW);
    let mut s = Scanner {
        input,
        pos: 0,
        ws: 0,
        we,
        eof: we == input.len(),
        out: Vec::with_capacity(input.len()),
        text_seen: false,
        newline: false,
        spaces: 0,
        scanned,
        reset: scanned,
        name: Vec::new(),
        rows: 1,
        cols: 1,
    };
    s.run();
    s.out
}

struct Scanner<'a> {
    input: &'a [u8],
    pos: usize,
    /// The window: input[ws..we]; past its end the scanner reads 0, as at the end of the text.
    ws: usize,
    we: usize,
    eof: bool,
    out: Vec<u8>,
    text_seen: bool,
    newline: bool,
    spaces: usize,
    /// The scanned print pages flag (bc:scannedpagebreak sets it) and the value a page break's end gives it back.
    scanned: bool,
    reset: bool,
    /// The last tag name read; a tag whose name cannot be read leaves it as it was.
    name: Vec<u8>,
    rows: i32,
    cols: i32,
}

impl Scanner<'_> {
    fn at(&self, i: usize) -> u8 {
        if i < self.we { self.input[i] } else { 0 }
    }

    /// What is held is written: a line end, or else the spaces.
    fn flush(&mut self) {
        if self.newline {
            self.out.push(b'\n');
        } else {
            self.out.resize(self.out.len() + self.spaces, b' ');
        }
        self.newline = false;
        self.spaces = 0;
    }

    fn code(&mut self, name: &[u8]) {
        self.out.push(0x1C);
        self.out.extend_from_slice(name);
        self.out.push(0x1F);
    }

    fn run(&mut self) {
        loop {
            if self.pos >= self.we && self.eof {
                return;
            }
            if self.pos - self.ws > MOVE_AFTER && !self.eof {
                self.ws = self.pos;
                self.we = (self.pos + WINDOW).min(self.input.len());
                self.eof = self.we == self.input.len();
                continue;
            }
            match self.input[self.pos] {
                b'\r' | b'\n' => {
                    if self.text_seen {
                        self.newline = true;
                    }
                    self.pos += 1;
                }
                b' ' => {
                    if self.text_seen {
                        self.spaces += 1;
                    }
                    self.pos += 1;
                }
                b'<' => self.tag(),
                b'&' if self.at(self.pos + 1) == b'#' => self.reference(),
                c => {
                    self.flush();
                    self.text_seen = true;
                    self.out.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// A numeric reference: the number read as the C library reads one (spaces and a sign allowed, "0x" after
    /// "&#x"), its ";" taken when it follows.
    fn reference(&mut self) {
        let (base, start) = if matches!(self.at(self.pos + 2), b'x' | b'X') { (16, self.pos + 3) } else { (10, self.pos + 2) };
        let mut p = start;
        while matches!(self.at(p), b' ' | 9..=13) {
            p += 1;
        }
        let negative = self.at(p) == b'-';
        if matches!(self.at(p), b'-' | b'+') {
            p += 1;
        }
        if base == 16 && self.at(p) == b'0' && matches!(self.at(p + 1), b'x' | b'X') {
            p += 2;
        }
        let digits = p;
        let mut v: i64 = 0;
        while let Some(d) = char::from(self.at(p)).to_digit(base) {
            v = (v * i64::from(base) + i64::from(d)).min(1 << 32);
            p += 1;
        }
        let (value, mut end) = if p == digits {
            (0i32, start)
        } else {
            let v = if negative { -v } else { v };
            (v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32, p)
        };
        if self.at(end) == b';' {
            end += 1;
        }
        self.flush();
        self.text_seen = true;
        match value {
            0x26 => self.out.extend_from_slice(b"&amp;"),
            0x3C => self.out.extend_from_slice(b"&lt;"),
            0x3E => self.out.extend_from_slice(b"&gt;"),
            // The character is written as text that ends at a 0 byte: the value 0 writes nothing.
            0 => {}
            v => utf8_bytes(v as u32, &mut self.out),
        }
        self.pos = end;
    }

    /// A tag at "<".
    fn tag(&mut self) {
        let start = self.pos;
        let mut p = start + 1;
        let mark = self.at(p);
        let (end_tag, pi) = (mark == b'/', mark == b'?');
        if end_tag || pi {
            p += 1;
        }
        // The name: up to a space, ">", the end of the window, or "/" ("?" in a processing instruction), which must
        // be followed by ">"; at most 38 bytes.
        let stop = if pi { b'?' } else { b'/' };
        let mut e = p;
        while !matches!(self.at(e), b' ' | b'>' | 0) && self.at(e) != stop {
            e += 1;
        }
        let named = !(self.at(e) == stop && self.at(e + 1) != b'>') && e - p < 39;
        let mut after = p;
        if named {
            self.name = (p..e).map(|i| self.at(i)).collect();
            while !matches!(self.at(after), b' ' | b'/' | b'>' | 0) {
                after += 1;
            }
        }
        let mut handled = false;
        if pi && named && self.name == b"bc" && e < self.we {
            let window = &self.input[e..self.we];
            if let Some(k) = window.windows(2).position(|w| w == b"?>").filter(|&k| !window[..k].contains(&0)) {
                // The code instruction: $ and ` are the code marks, &quot; a double quote.
                let mut text = Vec::with_capacity(k);
                for &b in &window[..k] {
                    text.push(match b {
                        b'$' => 0x1C,
                        b'`' => 0x1F,
                        b => b,
                    });
                }
                let text = replace(&text, b"&quot;", b"\"");
                self.flush();
                self.out.extend_from_slice(&text);
                handled = true;
            }
        }
        if !handled {
            handled = self.by_name(start, after);
        }
        // The tag runs to the first ">" outside double quotes (a handler may have moved on to a later "<").
        let mut q = self.pos;
        let mut quoted = false;
        while self.at(q) != 0 && (quoted || self.at(q) != b'>') {
            if self.at(q) == b'"' {
                quoted = !quoted;
            }
            q += 1;
        }
        if self.at(q) == b'>' {
            q += 1;
        }
        if !handled {
            self.flush();
            self.out.extend_from_slice(&self.input[self.pos..q]);
        }
        self.pos = q;
    }

    /// The elements read by name (whether the tag starts, ends or closes itself); false for any other name.
    fn by_name(&mut self, start: usize, after: usize) -> bool {
        let name = self.name.clone();
        match name.as_slice() {
            b"bc:checkbox" | b"bc:note" => {}
            // Its type attribute only steers a page parity counter that nothing reads.
            b"bc:hardpage" => self.code(b"pg"),
            b"bc:scannedpagebreak" => self.scanned = true,
            b"bc:mathtype" | b"bc:equation" => {
                // INFERENCE: the equation object is read from the Word package, which an HTML file does not have, so
                // nothing is written.
                self.flush();
                self.text_seen = true;
            }
            b"bc:pageNumStart" => self.page_number(0),
            b"bc:pageNumLRStart" => self.page_number(1),
            b"bc:pageNumURStart" => self.page_number(2),
            b"bc:pagebreak" => {
                if self.at(start + 1) == b'/' {
                    if self.scanned {
                        self.code(b"run");
                        self.scanned = self.reset;
                    }
                } else if self.self_closing(start) {
                    if self.scanned {
                        self.code(b"lea;f1");
                    }
                } else if self.scanned {
                    self.code(b"lea");
                } else {
                    // Its content is dropped, up to the next tag (which is taken with it).
                    loop {
                        self.pos += 1;
                        if self.pos >= self.we || self.input[self.pos] == b'<' {
                            break;
                        }
                    }
                }
            }
            b"bc:rps_a" => {
                let v = self.attr(after, b"name", 200).unwrap_or_default();
                self.code(&[b"rps:a~".as_slice(), &v].concat());
            }
            b"bc:rpe_a" => self.code(b"rpe:a"),
            b"bc:tab" => self.code(b">"),
            b"bc:ltab" => self.code(b"fr;p~\"."),
            b"bc:newline" => self.code(b"&lt;"),
            b"bc:DX_CHSPH" => self.out.push(0x1E),
            b"bc:xlc" => {
                if self.at(start + 1) != b'/' {
                    self.cell(after);
                }
            }
            b"bc:xlsheetData" => {
                self.rows = 1;
                self.cols = 1;
            }
            n if n.len() >= 5 && n[..5].eq_ignore_ascii_case(b"bcml:") => self.code(&n[5..]),
            _ => return false,
        }
        true
    }

    /// Whether the tag at `from` ends in "/>" (outside double quotes, before its ">").
    fn self_closing(&self, from: usize) -> bool {
        let mut quoted = false;
        let mut p = from;
        loop {
            match self.at(p) {
                0 => return false,
                b'"' => quoted = !quoted,
                b'>' if !quoted => return false,
                b'/' if !quoted && self.at(p + 1) == b'>' => return true,
                _ => {}
            }
            p += 1;
        }
    }

    /// An attribute's value from `from` (just after the tag name): attributes separated by spaces, the name matched
    /// without regard to case and followed by "=" or a space; the value runs to a space, "/" or ">" outside double
    /// quotes, with the double quotes taken out. None when it is not there, or not shorter than `size`.
    fn attr(&self, mut from: usize, name: &[u8], size: usize) -> Option<Vec<u8>> {
        loop {
            if self.at(from) != b' ' {
                return None;
            }
            let mut p = from;
            while self.at(p) == b' ' {
                p += 1;
            }
            let matched = (0..name.len()).all(|k| self.at(p + k).eq_ignore_ascii_case(&name[k]))
                && matches!(self.at(p + name.len()), b'=' | b' ');
            while !matches!(self.at(p), b' ' | b'=' | b'>' | 0) {
                p += 1;
            }
            while self.at(p) == b' ' {
                p += 1;
            }
            if self.at(p) != b'=' {
                return None;
            }
            p += 1;
            while self.at(p) == b' ' {
                p += 1;
            }
            let value = p;
            let mut quoted = false;
            while self.at(p) != 0 && (quoted || !matches!(self.at(p), b' ' | b'/' | b'>')) {
                if self.at(p) == b'"' {
                    quoted = !quoted;
                }
                p += 1;
            }
            if matched {
                return (p - value < size).then(|| self.input[value..p].iter().copied().filter(|&b| b != b'"').collect());
            }
            from = p;
        }
    }

    /// A print page number element (mode 0 arabic, 1 small roman, 2 capital roman): with scanned print pages the
    /// characters up to the next tag are read as a number (each one as a digit); without them the content is
    /// dropped. Either way the scanner moves on to the next tag, which is taken with this one.
    fn page_number(&mut self, mode: u8) {
        if self.at(self.pos + 1) == b'/' {
            if self.scanned {
                self.code(b"run");
                self.scanned = self.reset;
            }
            return;
        }
        if !self.scanned {
            loop {
                self.pos += 1;
                if self.pos >= self.we || self.input[self.pos] == b'<' {
                    return;
                }
            }
        }
        loop {
            self.pos += 1;
            if self.pos >= self.we || self.input[self.pos] == b'>' {
                break;
            }
        }
        let mut n: i32 = 0;
        loop {
            self.pos += 1;
            if self.pos >= self.we || self.input[self.pos] == b'<' {
                break;
            }
            n = n.wrapping_mul(10).wrapping_add(i32::from(self.input[self.pos] as i8) - 0x30);
        }
        let number = match mode {
            0 => n.to_string(),
            1 => crate::word::roman(i64::from(n)).to_lowercase(),
            _ => crate::word::roman(i64::from(n)),
        };
        let code: &[u8] = if mode == 0 { b"lea;f0a" } else { b"lea;f0r" };
        self.code(code);
        self.out.extend_from_slice(number.as_bytes());
    }

    /// A spreadsheet cell (bc:xlc): line codes up to its row and tab codes up to its column (from its r attribute,
    /// letters and a row number), then its value.
    fn cell(&mut self, after: usize) {
        let r = self.attr(after, b"r", 0x14).unwrap_or_default();
        let letters = r.iter().take_while(|b| b.is_ascii_alphabetic()).count();
        let col = r[..letters].iter().fold(0i32, |c, &b| c.wrapping_mul(26).wrapping_add(i32::from(b.to_ascii_uppercase()) - 0x40));
        let rest = &r[letters..];
        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        let row = rest[..digits].iter().fold(0i32, |n, &d| n.wrapping_mul(10).wrapping_add(i32::from(d - b'0')));
        if self.rows < row {
            self.cols = 1;
        }
        while self.rows < row {
            self.out.extend_from_slice(b"<bc:&lt;>");
            self.rows += 1;
        }
        while self.cols < col {
            self.out.extend_from_slice(b"<bc:&gt;>");
            self.cols += 1;
        }
        // INFERENCE: the value is written as it is; its format attribute (a spreadsheet number format) is not
        // applied.
        let num = self.attr(after, b"num", 100).unwrap_or_default();
        self.out.extend_from_slice(&num);
    }
}

/// Every `from` in `text` replaced by `to`.
fn replace(text: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_chain::{self, Files, Recipe};
    use crate::rule_script::RuleScript;
    use std::path::PathBuf;

    struct Shipped;

    fn tables() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables")
    }

    impl Files for Shipped {
        fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
            std::fs::read(tables().join("scan").join(name)).map_err(|e| e.to_string())
        }
        fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
            RuleScript::open(&tables().join("rule-scripts").join(name)).map_err(|e| e.to_string())
        }
        fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
            import_chain::UNIMAP_TABLES
                .iter()
                .map(|n| std::fs::read(tables().join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| e.to_string()))
                .collect()
        }
        fn word_map(&self, name: &str) -> Result<String, String> {
            std::fs::read_to_string(tables().join("word-styles").join(format!("{name}.map"))).map_err(|e| e.to_string())
        }
    }

    fn sample(name: &str) -> Vec<u8> {
        std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/samples").join(name)).unwrap()
    }

    /// The sample page through the HTML recipe, as text in running form.
    fn web_page() -> String {
        let got = import_chain::run(&sample("web-page.html"), &Recipe::Html(Options::default(), 0), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        crate::internal::decode(&got.text)
    }

    #[test]
    fn a_malformed_web_page_imports() {
        // Paragraphs and list items left open, a line break without "/", attributes without quotes.
        let text = web_page();
        assert!(text.contains("\x1chl1\x1fone\x1chl1\x1ftwo\x1cee~list.\x1f"), "{text:?}");
        assert!(text.contains("\x1ces~para.\x1fUnquoted attribute\x1cee~para.\x1f"), "{text:?}");
        assert!(!text.contains('<') && !text.contains('>'), "{text:?}");
    }

    #[test]
    fn a_named_entity_arrives_as_its_character() {
        assert!(web_page().contains("Caf\u{e9} and"));
    }

    #[test]
    fn nbsp_without_its_semicolon_is_a_no_break_space() {
        assert!(web_page().contains("a\u{a0} space"));
    }

    #[test]
    fn an_empty_table_cell_gives_the_cell_start_and_end_codes() {
        let text = web_page();
        assert!(text.contains("\x1ces~td\x1fa\x1cee~td\x1f\x1ces~td\x1f\x1cee~td\x1f"), "{text:?}");
        assert!(!text.contains("td/"), "{text:?}");
    }

    #[test]
    fn a_line_break_tag_gives_the_br_code() {
        // Both <br> and <br />.
        let text = web_page();
        assert_eq!(text.matches("\x1ces~br\x1f").count(), 2, "{text:?}");
        assert!(!text.contains("br/"), "{text:?}");
    }

    #[test]
    fn a_comment_goes_and_a_code_instruction_gives_its_code() {
        let text = web_page();
        assert!(!text.contains("comment"), "{text:?}");
        assert!(text.contains("\x1chl1\x1fAfter the code."), "{text:?}");
    }

    #[test]
    fn the_character_set_a_page_names_is_read() {
        let page = sample("web-page.html");
        assert_eq!(crate::filekind::detect("page.htm", &page), crate::filekind::FileKind::Sgml);
        assert_eq!(submethod(&page), 1251);
        assert_eq!(charset(b"<meta charset=\"UTF-8\">"), 65001);
        assert_eq!(charset(b"<meta charset = ISO-8859-1>"), 28591);
        assert_eq!(charset(b"<meta charset=\"x-unknown\">"), 0);
        assert_eq!(charset(b"no character set"), 0);
        // A named code page is read in that page (BrailleCraft's own): "Привет" in windows-1251, and in KOI8-R.
        assert_eq!(charset_step(b"\xcf\xf0\xe8\xe2\xe5\xf2", 1251), "Привет".as_bytes());
        assert_eq!(charset_step(b"\xf0\xd2\xc9\xd7\xc5\xd4", 20866), "Привет".as_bytes());
        assert_eq!(charset_step(b"caf\xe9", 28591), "café".as_bytes());
        assert_eq!(charset_step("café".as_bytes(), 65001), "café".as_bytes());
        assert_eq!(charset_step(b"\xa4\xa2", 51932), "あ".as_bytes());
    }

    #[test]
    fn numeric_references_are_written_after_the_control_step() {
        let got = scan(b"&#233;&#x26;&#10;&#60;x&#0;y", &Options::default());
        assert_eq!(got, "\u{e9}&amp;\n&lt;xy".as_bytes());
    }

    #[test]
    fn the_word_recipe_ends_with_its_clean_up_and_end_of_text_scripts() {
        let doc = sample("page-break-at-end.docx");
        let mut steps = Vec::new();
        let got = import_chain::run_with(&doc, &Recipe::Docx(Options::default(), "default".into(), 0), &Shipped, &mut |n, _| steps.push(n.to_string()));
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        assert!(steps.ends_with(&["end-of-text-1.rules", "end-of-text-2.rules"].map(String::from)), "{steps:?}");
        assert!(steps.contains(&"word-post.rules".to_string()), "{steps:?}");
        // The tag processor ends the text with the page break; the end-of-text scripts drop it.
        let text = crate::internal::decode(&got.text);
        assert!(text.ends_with("Last words.\x1cee~para.\x1f"), "{text:?}");
        // With the styles math and math-NLE the math scripts run too, and each of them loads.
        let math = Options { styles: vec!["math".into(), "Math-NLE".into()], ..Options::default() };
        let mut steps = Vec::new();
        let got = import_chain::run_with(&doc, &Recipe::Docx(math, "default".into(), 0), &Shipped, &mut |n, _| steps.push(n.to_string()));
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        for s in ["math-separation-1.rules", "math-separation-2.rules", "math-styles.rules", "math-nle-1.rules", "math-nle-2.rules", "math-ref-pages.rules"] {
            assert!(steps.contains(&s.to_string()), "{s}: {steps:?}");
        }
    }
}
