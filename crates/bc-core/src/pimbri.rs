//! Formatted plain text (a page image laid out with spaces) into running text with style codes, as the reference
//! program's formatted text importer does it.
//!
//! The input is read as UTF-8 (without any validation) page by page into a grid of at most 100 rows of 200 cells: a
//! form feed or the end of the file ends a page, and so does the 100th line. Carriage returns and backspaces overprint
//! (an underscore over a letter underlines it), tabs count as one space, and each character from U+0021 to U+00FF
//! goes through a code page 437 table that may replace it. Lines of rule characters are recognised and then count as
//! blank. When the page looks double spaced the extra blank lines are taken out.
//!
//! The page is then cut into boxes: a running head or foot with a page number, the body, text columns, and in each
//! column the groups of lines between blank lines. Each group is tried, in a fixed order, as a centred heading, a
//! left heading, a table, a letter salutation or complimentary close, a blocked (poem-like) or outline block, and is a
//! paragraph otherwise. Each box is then written as lines of text between "$es~...." and "$ee~...." style codes (or
//! "$l", "$tls", "$leaf-" and similar codes), one output line per input line, each ended by a line feed. Underlined
//! letters get "_" marks, a dollar sign is doubled when it could be read as a code, and a hyphen at a line end is
//! doubled when it is a real hyphen rather than a word broken over the line.
//!
//! The output line width is 0 and the style mode is on, which is how the original program calls this importer; the
//! width only matters for numbers inside codes that have no number in style mode, and for the layout of tables,
//! which with width 0 are always written in the stacked form. Nothing therefore depends on the braille translator
//! that the original consults for line widths.
//!
//! What the importer recognises is steered by two factor files: the input factors (thresholds for headings, tables,
//! blocks and so on) and the output factors (which codes to write). Both ship in tables/import.

/// The two factor tables that steer the importer: the input factors (how the page is analysed) and the output
/// factors (which codes are written). Each holds factors 0 to 99; a factor the file does not set is 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Factors {
    genl: [i32; 100],
    book: [i32; 100],
}

impl Factors {
    /// Reads the input factor file and the output factor file.
    ///
    /// Each file is plain text. "|" followed by a space or the line end starts a comment that runs to the end of the
    /// line, "||" is a "|" and "|" followed by three digits is the character with that code; lines that hold only a
    /// comment are skipped and trailing spaces are dropped. The first remaining line must be "-pimbri i 1" (input
    /// factors) or "-pimbri o 1" (output factors). Then come lines "N V": factor N (0 to 99) gets the value V; any
    /// other N of 0 or more is ignored. A line whose N is -1 ends the list and the next line must be "***". A line of
    /// 81 or more characters is an error.
    pub fn parse(genl_pit: &[u8], book_pot: &[u8]) -> Result<Factors, String> {
        let genl = read_factor_file(genl_pit, b"-pimbri i 1").map_err(|e| format!("input factors: {e}"))?;
        let book = read_factor_file(book_pot, b"-pimbri o 1").map_err(|e| format!("output factors: {e}"))?;
        Ok(Factors { genl, book })
    }

    /// The factor files shipped in tables/import.
    pub fn shipped() -> Result<Factors, String> {
        Factors::parse(
            include_bytes!("../../../tables/import/text-input-factors.txt"),
            include_bytes!("../../../tables/import/text-output-factors.txt"),
        )
    }

    /// Input factor `n` (0 when `n` is out of range).
    pub fn input(&self, n: usize) -> i32 {
        self.genl.get(n).copied().unwrap_or(0)
    }

    /// Output factor `n` (0 when `n` is out of range).
    pub fn output(&self, n: usize) -> i32 {
        self.book.get(n).copied().unwrap_or(0)
    }
}

/// What the factor file reader returns for the end of the file and for a comment.
const END_OF_FILE: i32 = 0x100;
const COMMENT: i32 = 0x101;

/// Reads factor files a character at a time. The original opens them as text files, so a carriage return and line
/// feed pair is read as one line feed.
struct FactorReader<'a> {
    data: &'a [u8],
    at: usize,
}

impl FactorReader<'_> {
    fn byte(&mut self) -> i32 {
        let Some(&b) = self.data.get(self.at) else { return -1 };
        self.at += 1;
        if b == b'\r' && self.data.get(self.at) == Some(&b'\n') {
            self.at += 1;
            return 10;
        }
        b as i32
    }

    /// One character with the "|" forms worked out.
    fn char(&mut self) -> i32 {
        let c = self.byte();
        if c == -1 {
            return END_OF_FILE;
        }
        if c != b'|' as i32 {
            return c;
        }
        let d = self.byte();
        if d == b' ' as i32 {
            // A comment: the rest of the line, its line feed included, is skipped.
            loop {
                let x = self.byte();
                if x == 10 || x == -1 {
                    break;
                }
            }
            return COMMENT;
        }
        if d == 10 {
            return COMMENT;
        }
        if d == b'|' as i32 {
            return b'|' as i32;
        }
        // Three digits give a character code. Anything else (the three characters are used up all the same, up to
        // the end of the file or a 0xFF byte, which the original takes for the end of the file) is a "|".
        if d == -1 || d == 0xff {
            return b'|' as i32;
        }
        let e = self.byte();
        if e == -1 || e == 0xff {
            return b'|' as i32;
        }
        let f = self.byte();
        if f == -1 || f == 0xff {
            return b'|' as i32;
        }
        let digit = |x: i32| (b'0' as i32..=b'9' as i32).contains(&x);
        if !(digit(d) && digit(e) && digit(f)) {
            return b'|' as i32;
        }
        let v = (d - 48) * 100 + (e - 48) * 10 + (f - 48);
        if v > 0xff {
            b'|' as i32
        } else {
            v
        }
    }

    /// The next line that is not only a comment, without its trailing spaces: None at the end of the file, an error
    /// when the line holds `max` characters or more.
    fn line(&mut self, max: usize) -> Result<Option<Vec<u8>>, ()> {
        loop {
            let mut buf: Vec<u8> = Vec::new();
            loop {
                if buf.len() >= max {
                    // The rest of the line is read and the line is reported as too long.
                    loop {
                        let c = self.char();
                        if c == 10 || c == END_OF_FILE || c == COMMENT {
                            break;
                        }
                    }
                    return Err(());
                }
                let c = self.char();
                if c == 10 {
                    return Ok(Some(trim_spaces(buf)));
                }
                if c == END_OF_FILE {
                    if buf.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(trim_spaces(buf)));
                }
                if c == COMMENT {
                    break;
                }
                buf.push(c as u8);
            }
            // A comment after some text ends the line; a line holding only a comment is skipped.
            if !buf.is_empty() {
                return Ok(Some(trim_spaces(buf)));
            }
        }
    }
}

fn trim_spaces(mut v: Vec<u8>) -> Vec<u8> {
    while v.last() == Some(&b' ') {
        v.pop();
    }
    v
}

/// The C library's atoi: leading white space, an optional sign, digits (saturating on overflow).
fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || (9..=13).contains(&s[i])) {
        i += 1;
    }
    let mut negative = false;
    if i < s.len() && (s[i] == b'-' || s[i] == b'+') {
        negative = s[i] == b'-';
        i += 1;
    }
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = (v * 10 + (s[i] - b'0') as i64).min(1 << 40);
        i += 1;
    }
    if negative {
        v = -v;
    }
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// The value on a factor line: the digits at the start are skipped, then everything up to the next digit, and the
/// number there is read (so a minus sign before the value is lost).
fn second_number(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    while i < s.len() && !s[i].is_ascii_digit() {
        i += 1;
    }
    if i >= s.len() {
        return 0;
    }
    atoi(&s[i..])
}

fn read_factor_file(data: &[u8], header: &[u8]) -> Result<[i32; 100], String> {
    let mut r = FactorReader { data, at: 0 };
    match r.line(300) {
        Ok(Some(l)) if l == header => {}
        _ => return Err(format!("the first line is not \"{}\"", String::from_utf8_lossy(header))),
    }
    let mut f = [0i32; 100];
    loop {
        let line = match r.line(81) {
            Ok(Some(l)) => l,
            // The original reads an empty line here again and again without end; the port stops.
            Ok(None) => return Err("the file ends before the line -1".to_string()),
            Err(()) => return Err("a line is too long".to_string()),
        };
        let n = atoi(&line);
        let v = second_number(&line);
        if n < 0 {
            if n == -1 {
                return match r.line(300) {
                    Ok(Some(l)) if l == b"***" => Ok(f),
                    _ => Err("the line after -1 is not ***".to_string()),
                };
            }
            return Err(format!("factor number {n} is below -1"));
        }
        if let Some(slot) = f.get_mut(n as usize) {
            *slot = v;
        }
    }
}

/// The character table the page reader uses for code points U+0021 to U+00FF, indexed by the code point (which is
/// read as a code page 437 byte). Columns: class flags; attributes; the replacement stored for substitution sets 0,
/// 1 and 2.
///
/// Class flags: 0x1 digit, 0x2 letter, 0x4 lower case, 0x8 upper case, 0x10 ends a sentence, 0x20 can draw a
/// horizontal rule, 0x40 can draw a vertical rule, 0x80, 0x100 and 0x200 the three kinds of "computer text"
/// characters, 0x400 closing punctuation, 0x800 opening punctuation.
///
/// Attributes: 0x20 a letter of the upper half (an accented letter), 0x10 part of a horizontal rule, 0x04 part of a
/// vertical rule (box drawing characters carry these from the start).
///
/// Replacements are bytes; one of 0x80 or more is stored sign-extended (as U+FF80 to U+FFFF). Set 0 has no accented
/// letters, set 1 keeps the accented letters of 0x80 to 0xA5, set 2 keeps every byte.
#[rustfmt::skip]
const CHARS: [(u16, u8, [u8; 3]); 256] = [
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 00 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 01 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 02 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 03 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 04 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 05 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 06 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 07 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 08 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 09 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0A control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0B control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0C control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0D control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0E control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 0F control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 10 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 11 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 12 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 13 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 14 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 15 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 16 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 17 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 18 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 19 control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1A control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1B control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1C control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1D control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1E control
    (0x0000, 0x00, [0x21, 0x21, 0x21]), // 1F control
    (0x0000, 0x00, [0x20, 0x20, 0x20]), // 20 space
    (0x0750, 0x00, [0x21, 0x21, 0x21]), // 21 !
    (0x0c10, 0x00, [0x22, 0x22, 0x22]), // 22 "
    (0x05e0, 0x00, [0x23, 0x23, 0x23]), // 23 #
    (0x0100, 0x00, [0x24, 0x24, 0x24]), // 24 $
    (0x0700, 0x00, [0x25, 0x25, 0x25]), // 25 %
    (0x0000, 0x00, [0x26, 0x26, 0x26]), // 26 &
    (0x0000, 0x00, [0x27, 0x27, 0x27]), // 27 '
    (0x0840, 0x00, [0x28, 0x28, 0x28]), // 28 (
    (0x0750, 0x00, [0x29, 0x29, 0x29]), // 29 )
    (0x0cf0, 0x00, [0x2a, 0x2a, 0x2a]), // 2A *
    (0x0ce0, 0x00, [0x2b, 0x2b, 0x2b]), // 2B +
    (0x0700, 0x00, [0x2c, 0x2c, 0x2c]), // 2C ,
    (0x0c20, 0x00, [0x2d, 0x2d, 0x2d]), // 2D -
    (0x0570, 0x00, [0x2e, 0x2e, 0x2e]), // 2E .
    (0x0200, 0x00, [0x2f, 0x2f, 0x2f]), // 2F /
    (0x0001, 0x00, [0x30, 0x30, 0x30]), // 30 0
    (0x0001, 0x00, [0x31, 0x31, 0x31]), // 31 1
    (0x0001, 0x00, [0x32, 0x32, 0x32]), // 32 2
    (0x0001, 0x00, [0x33, 0x33, 0x33]), // 33 3
    (0x0001, 0x00, [0x34, 0x34, 0x34]), // 34 4
    (0x0001, 0x00, [0x35, 0x35, 0x35]), // 35 5
    (0x0001, 0x00, [0x36, 0x36, 0x36]), // 36 6
    (0x0001, 0x00, [0x37, 0x37, 0x37]), // 37 7
    (0x0001, 0x00, [0x38, 0x38, 0x38]), // 38 8
    (0x0001, 0x00, [0x39, 0x39, 0x39]), // 39 9
    (0x0730, 0x00, [0x3a, 0x3a, 0x3a]), // 3A :
    (0x0700, 0x00, [0x3b, 0x3b, 0x3b]), // 3B ;
    (0x0080, 0x00, [0x3c, 0x3c, 0x3c]), // 3C <
    (0x00a0, 0x00, [0x3d, 0x3d, 0x3d]), // 3D =
    (0x0080, 0x00, [0x3e, 0x3e, 0x3e]), // 3E >
    (0x0710, 0x00, [0x3f, 0x3f, 0x3f]), // 3F ?
    (0x0080, 0x00, [0x40, 0x40, 0x40]), // 40 @
    (0x000a, 0x00, [0x41, 0x41, 0x41]), // 41 A
    (0x000a, 0x00, [0x42, 0x42, 0x42]), // 42 B
    (0x000a, 0x00, [0x43, 0x43, 0x43]), // 43 C
    (0x000a, 0x00, [0x44, 0x44, 0x44]), // 44 D
    (0x000a, 0x00, [0x45, 0x45, 0x45]), // 45 E
    (0x000a, 0x00, [0x46, 0x46, 0x46]), // 46 F
    (0x000a, 0x00, [0x47, 0x47, 0x47]), // 47 G
    (0x000a, 0x00, [0x48, 0x48, 0x48]), // 48 H
    (0x000a, 0x00, [0x49, 0x49, 0x49]), // 49 I
    (0x000a, 0x00, [0x4a, 0x4a, 0x4a]), // 4A J
    (0x000a, 0x00, [0x4b, 0x4b, 0x4b]), // 4B K
    (0x000a, 0x00, [0x4c, 0x4c, 0x4c]), // 4C L
    (0x000a, 0x00, [0x4d, 0x4d, 0x4d]), // 4D M
    (0x000a, 0x00, [0x4e, 0x4e, 0x4e]), // 4E N
    (0x000a, 0x00, [0x4f, 0x4f, 0x4f]), // 4F O
    (0x000a, 0x00, [0x50, 0x50, 0x50]), // 50 P
    (0x000a, 0x00, [0x51, 0x51, 0x51]), // 51 Q
    (0x000a, 0x00, [0x52, 0x52, 0x52]), // 52 R
    (0x000a, 0x00, [0x53, 0x53, 0x53]), // 53 S
    (0x000a, 0x00, [0x54, 0x54, 0x54]), // 54 T
    (0x000a, 0x00, [0x55, 0x55, 0x55]), // 55 U
    (0x000a, 0x00, [0x56, 0x56, 0x56]), // 56 V
    (0x000a, 0x00, [0x57, 0x57, 0x57]), // 57 W
    (0x000a, 0x00, [0x58, 0x58, 0x58]), // 58 X
    (0x000a, 0x00, [0x59, 0x59, 0x59]), // 59 Y
    (0x000a, 0x00, [0x5a, 0x5a, 0x5a]), // 5A Z
    (0x0880, 0x00, [0x5b, 0x5b, 0x5b]), // 5B [
    (0x00c0, 0x00, [0x5c, 0x5c, 0x5c]), // 5C \
    (0x0490, 0x00, [0x5d, 0x5d, 0x5d]), // 5D ]
    (0x0080, 0x00, [0x5e, 0x5e, 0x5e]), // 5E ^
    (0x00a0, 0x00, [0x5f, 0x5f, 0x5f]), // 5F _
    (0x0080, 0x00, [0x60, 0x60, 0x60]), // 60 `
    (0x0006, 0x00, [0x61, 0x61, 0x61]), // 61 a
    (0x0006, 0x00, [0x62, 0x62, 0x62]), // 62 b
    (0x0006, 0x00, [0x63, 0x63, 0x63]), // 63 c
    (0x0006, 0x00, [0x64, 0x64, 0x64]), // 64 d
    (0x0006, 0x00, [0x65, 0x65, 0x65]), // 65 e
    (0x0006, 0x00, [0x66, 0x66, 0x66]), // 66 f
    (0x0006, 0x00, [0x67, 0x67, 0x67]), // 67 g
    (0x0006, 0x00, [0x68, 0x68, 0x68]), // 68 h
    (0x0006, 0x00, [0x69, 0x69, 0x69]), // 69 i
    (0x0006, 0x00, [0x6a, 0x6a, 0x6a]), // 6A j
    (0x0006, 0x00, [0x6b, 0x6b, 0x6b]), // 6B k
    (0x0006, 0x00, [0x6c, 0x6c, 0x6c]), // 6C l
    (0x0006, 0x00, [0x6d, 0x6d, 0x6d]), // 6D m
    (0x0006, 0x00, [0x6e, 0x6e, 0x6e]), // 6E n
    (0x0006, 0x00, [0x6f, 0x6f, 0x6f]), // 6F o
    (0x0006, 0x00, [0x70, 0x70, 0x70]), // 70 p
    (0x0006, 0x00, [0x71, 0x71, 0x71]), // 71 q
    (0x0006, 0x00, [0x72, 0x72, 0x72]), // 72 r
    (0x0006, 0x00, [0x73, 0x73, 0x73]), // 73 s
    (0x0006, 0x00, [0x74, 0x74, 0x74]), // 74 t
    (0x0006, 0x00, [0x75, 0x75, 0x75]), // 75 u
    (0x0006, 0x00, [0x76, 0x76, 0x76]), // 76 v
    (0x0006, 0x00, [0x77, 0x77, 0x77]), // 77 w
    (0x0006, 0x00, [0x78, 0x78, 0x78]), // 78 x
    (0x0006, 0x00, [0x79, 0x79, 0x79]), // 79 y
    (0x0006, 0x00, [0x7a, 0x7a, 0x7a]), // 7A z
    (0x0880, 0x00, [0x7b, 0x7b, 0x7b]), // 7B {
    (0x00c0, 0x00, [0x7c, 0x7c, 0x7c]), // 7C |
    (0x0490, 0x00, [0x7d, 0x7d, 0x7d]), // 7D }
    (0x00a0, 0x00, [0x7e, 0x7e, 0x7e]), // 7E ~
    (0x0000, 0x20, [0x44, 0x44, 0x44]), // 7F code page 437 byte 7F
    (0x000a, 0x20, [0x43, 0x80, 0x80]), // 80 code page 437 byte 80
    (0x0006, 0x20, [0x75, 0x81, 0x81]), // 81 code page 437 byte 81
    (0x0006, 0x20, [0x65, 0x82, 0x82]), // 82 code page 437 byte 82
    (0x0006, 0x20, [0x61, 0x83, 0x83]), // 83 code page 437 byte 83
    (0x0006, 0x20, [0x61, 0x84, 0x84]), // 84 code page 437 byte 84
    (0x0006, 0x20, [0x61, 0x85, 0x85]), // 85 code page 437 byte 85
    (0x0006, 0x20, [0x61, 0x86, 0x86]), // 86 code page 437 byte 86
    (0x0006, 0x20, [0x63, 0x87, 0x87]), // 87 code page 437 byte 87
    (0x0006, 0x20, [0x65, 0x88, 0x88]), // 88 code page 437 byte 88
    (0x0006, 0x20, [0x65, 0x89, 0x89]), // 89 code page 437 byte 89
    (0x0006, 0x20, [0x65, 0x8a, 0x8a]), // 8A code page 437 byte 8A
    (0x0006, 0x20, [0x69, 0x8b, 0x8b]), // 8B code page 437 byte 8B
    (0x0006, 0x20, [0x69, 0x8c, 0x8c]), // 8C code page 437 byte 8C
    (0x0006, 0x20, [0x69, 0x8d, 0x8d]), // 8D code page 437 byte 8D
    (0x000a, 0x20, [0x41, 0x8e, 0x8e]), // 8E code page 437 byte 8E
    (0x000a, 0x20, [0x41, 0x8f, 0x8f]), // 8F code page 437 byte 8F
    (0x000a, 0x20, [0x45, 0x90, 0x90]), // 90 code page 437 byte 90
    (0x0000, 0x20, [0x61, 0x61, 0x91]), // 91 code page 437 byte 91
    (0x0000, 0x20, [0x41, 0x41, 0x92]), // 92 code page 437 byte 92
    (0x0006, 0x20, [0x6f, 0x93, 0x93]), // 93 code page 437 byte 93
    (0x0006, 0x20, [0x6f, 0x94, 0x94]), // 94 code page 437 byte 94
    (0x0006, 0x20, [0x6f, 0x95, 0x95]), // 95 code page 437 byte 95
    (0x0006, 0x20, [0x75, 0x96, 0x96]), // 96 code page 437 byte 96
    (0x0006, 0x20, [0x75, 0x97, 0x97]), // 97 code page 437 byte 97
    (0x0006, 0x20, [0x79, 0x98, 0x98]), // 98 code page 437 byte 98
    (0x000a, 0x20, [0x4f, 0x99, 0x99]), // 99 code page 437 byte 99
    (0x000a, 0x20, [0x55, 0x9a, 0x9a]), // 9A code page 437 byte 9A
    (0x0000, 0x20, [0x63, 0x63, 0x9b]), // 9B code page 437 byte 9B
    (0x0000, 0x20, [0x4c, 0x4c, 0x9c]), // 9C code page 437 byte 9C
    (0x0000, 0x20, [0x59, 0x59, 0x9d]), // 9D code page 437 byte 9D
    (0x0000, 0x20, [0x50, 0x50, 0x9e]), // 9E code page 437 byte 9E
    (0x0000, 0x20, [0x66, 0x66, 0x9f]), // 9F code page 437 byte 9F
    (0x0006, 0x20, [0x61, 0xa0, 0xa0]), // A0 code page 437 byte A0
    (0x0006, 0x20, [0x69, 0xa1, 0xa1]), // A1 code page 437 byte A1
    (0x0006, 0x20, [0x6f, 0xa2, 0xa2]), // A2 code page 437 byte A2
    (0x0006, 0x20, [0x75, 0xa3, 0xa3]), // A3 code page 437 byte A3
    (0x0006, 0x20, [0x6e, 0xa4, 0xa4]), // A4 code page 437 byte A4
    (0x000a, 0x20, [0x4e, 0xa5, 0xa5]), // A5 code page 437 byte A5
    (0x0000, 0x20, [0x61, 0x61, 0xa6]), // A6 code page 437 byte A6
    (0x0000, 0x20, [0x6f, 0x6f, 0xa7]), // A7 code page 437 byte A7
    (0x0000, 0x00, [0x3f, 0x3f, 0xa8]), // A8 code page 437 byte A8
    (0x0060, 0x14, [0x2b, 0x2b, 0xa9]), // A9 code page 437 byte A9
    (0x0060, 0x14, [0x2b, 0x2b, 0xaa]), // AA code page 437 byte AA
    (0x0000, 0x00, [0x3f, 0x3f, 0xab]), // AB code page 437 byte AB
    (0x0000, 0x00, [0x3f, 0x3f, 0xac]), // AC code page 437 byte AC
    (0x0000, 0x00, [0x21, 0x21, 0xad]), // AD code page 437 byte AD
    (0x0000, 0x00, [0x5b, 0x5b, 0xae]), // AE code page 437 byte AE
    (0x0000, 0x00, [0x5d, 0x5d, 0xaf]), // AF code page 437 byte AF
    (0x0060, 0x14, [0x2b, 0x2b, 0xb0]), // B0 code page 437 byte B0
    (0x0060, 0x14, [0x2b, 0x2b, 0xb1]), // B1 code page 437 byte B1
    (0x0060, 0x14, [0x2b, 0x2b, 0xb2]), // B2 code page 437 byte B2
    (0x0040, 0x04, [0x7c, 0x7c, 0xb3]), // B3 code page 437 byte B3
    (0x0060, 0x14, [0x2b, 0x2b, 0xb4]), // B4 code page 437 byte B4
    (0x0060, 0x14, [0x2b, 0x2b, 0xb5]), // B5 code page 437 byte B5
    (0x0060, 0x14, [0x2b, 0x2b, 0xb6]), // B6 code page 437 byte B6
    (0x0060, 0x14, [0x2b, 0x2b, 0xb7]), // B7 code page 437 byte B7
    (0x0060, 0x14, [0x2b, 0x2b, 0xb8]), // B8 code page 437 byte B8
    (0x0060, 0x14, [0x2b, 0x2b, 0xb9]), // B9 code page 437 byte B9
    (0x0040, 0x04, [0x7c, 0x7c, 0xba]), // BA code page 437 byte BA
    (0x0060, 0x14, [0x2b, 0x2b, 0xbb]), // BB code page 437 byte BB
    (0x0060, 0x14, [0x2b, 0x2b, 0xbc]), // BC code page 437 byte BC
    (0x0060, 0x14, [0x2b, 0x2b, 0xbd]), // BD code page 437 byte BD
    (0x0060, 0x14, [0x2b, 0x2b, 0xbe]), // BE code page 437 byte BE
    (0x0060, 0x14, [0x2b, 0x2b, 0xbf]), // BF code page 437 byte BF
    (0x0060, 0x14, [0x2b, 0x2b, 0xc0]), // C0 code page 437 byte C0
    (0x0060, 0x14, [0x2b, 0x2b, 0xc1]), // C1 code page 437 byte C1
    (0x0060, 0x14, [0x2b, 0x2b, 0xc2]), // C2 code page 437 byte C2
    (0x0060, 0x14, [0x2b, 0x2b, 0xc3]), // C3 code page 437 byte C3
    (0x0020, 0x10, [0x3d, 0x3d, 0xc4]), // C4 code page 437 byte C4
    (0x0060, 0x14, [0x2b, 0x2b, 0xc5]), // C5 code page 437 byte C5
    (0x0060, 0x14, [0x2b, 0x2b, 0xc6]), // C6 code page 437 byte C6
    (0x0060, 0x14, [0x2b, 0x2b, 0xc7]), // C7 code page 437 byte C7
    (0x0060, 0x14, [0x2b, 0x2b, 0xc8]), // C8 code page 437 byte C8
    (0x0060, 0x14, [0x2b, 0x2b, 0xc9]), // C9 code page 437 byte C9
    (0x0060, 0x14, [0x2b, 0x2b, 0xca]), // CA code page 437 byte CA
    (0x0060, 0x14, [0x2b, 0x2b, 0xcb]), // CB code page 437 byte CB
    (0x0060, 0x14, [0x2b, 0x2b, 0xcc]), // CC code page 437 byte CC
    (0x0020, 0x10, [0x3d, 0x3d, 0xcd]), // CD code page 437 byte CD
    (0x0060, 0x14, [0x2b, 0x2b, 0xce]), // CE code page 437 byte CE
    (0x0060, 0x14, [0x2b, 0x2b, 0xcf]), // CF code page 437 byte CF
    (0x0060, 0x14, [0x2b, 0x2b, 0xd0]), // D0 code page 437 byte D0
    (0x0060, 0x14, [0x2b, 0x2b, 0xd1]), // D1 code page 437 byte D1
    (0x0060, 0x14, [0x2b, 0x2b, 0xd2]), // D2 code page 437 byte D2
    (0x0060, 0x14, [0x2b, 0x2b, 0xd3]), // D3 code page 437 byte D3
    (0x0060, 0x14, [0x2b, 0x2b, 0xd4]), // D4 code page 437 byte D4
    (0x0060, 0x14, [0x2b, 0x2b, 0xd5]), // D5 code page 437 byte D5
    (0x0060, 0x14, [0x2b, 0x2b, 0xd6]), // D6 code page 437 byte D6
    (0x0060, 0x14, [0x2b, 0x2b, 0xd7]), // D7 code page 437 byte D7
    (0x0060, 0x14, [0x2b, 0x2b, 0xd8]), // D8 code page 437 byte D8
    (0x0060, 0x14, [0x2b, 0x2b, 0xd9]), // D9 code page 437 byte D9
    (0x0060, 0x14, [0x2b, 0x2b, 0xda]), // DA code page 437 byte DA
    (0x0060, 0x14, [0x2b, 0x2b, 0xdb]), // DB code page 437 byte DB
    (0x0060, 0x14, [0x2b, 0x2b, 0xdc]), // DC code page 437 byte DC
    (0x0040, 0x04, [0x7c, 0x7c, 0xdd]), // DD code page 437 byte DD
    (0x0040, 0x04, [0x7c, 0x7c, 0xde]), // DE code page 437 byte DE
    (0x0020, 0x10, [0x3d, 0x3d, 0xdf]), // DF code page 437 byte DF
    (0x0000, 0x20, [0x61, 0x61, 0xe0]), // E0 code page 437 byte E0
    (0x0000, 0x20, [0x62, 0x62, 0xe1]), // E1 code page 437 byte E1
    (0x0000, 0x20, [0x67, 0x67, 0xe2]), // E2 code page 437 byte E2
    (0x0000, 0x20, [0x70, 0x70, 0xe3]), // E3 code page 437 byte E3
    (0x0000, 0x20, [0x53, 0x53, 0xe4]), // E4 code page 437 byte E4
    (0x0000, 0x20, [0x73, 0x73, 0xe5]), // E5 code page 437 byte E5
    (0x0000, 0x20, [0x6d, 0x6d, 0xe6]), // E6 code page 437 byte E6
    (0x0000, 0x20, [0x74, 0x74, 0xe7]), // E7 code page 437 byte E7
    (0x0000, 0x20, [0x46, 0x46, 0xe8]), // E8 code page 437 byte E8
    (0x0000, 0x20, [0x3f, 0x3f, 0xe9]), // E9 code page 437 byte E9
    (0x0000, 0x20, [0x57, 0x57, 0xea]), // EA code page 437 byte EA
    (0x0000, 0x20, [0x64, 0x64, 0xeb]), // EB code page 437 byte EB
    (0x0000, 0x00, [0x3f, 0x3f, 0xec]), // EC code page 437 byte EC
    (0x0000, 0x20, [0x66, 0x66, 0xed]), // ED code page 437 byte ED
    (0x0000, 0x20, [0x65, 0x65, 0xee]), // EE code page 437 byte EE
    (0x0000, 0x00, [0x3f, 0x3f, 0xef]), // EF code page 437 byte EF
    (0x0000, 0x00, [0x3f, 0x3f, 0xf0]), // F0 code page 437 byte F0
    (0x0000, 0x00, [0x3f, 0x3f, 0xf1]), // F1 code page 437 byte F1
    (0x0000, 0x00, [0x3f, 0x3f, 0xf2]), // F2 code page 437 byte F2
    (0x0000, 0x00, [0x3f, 0x3f, 0xf3]), // F3 code page 437 byte F3
    (0x0000, 0x00, [0x3f, 0x3f, 0xf4]), // F4 code page 437 byte F4
    (0x0000, 0x00, [0x3f, 0x3f, 0xf5]), // F5 code page 437 byte F5
    (0x0000, 0x00, [0x3f, 0x3f, 0xf6]), // F6 code page 437 byte F6
    (0x0020, 0x00, [0x3d, 0x3d, 0xf7]), // F7 code page 437 byte F7
    (0x0060, 0x00, [0x2a, 0xf8, 0xf8]), // F8 code page 437 byte F8
    (0x0060, 0x00, [0x2e, 0x2e, 0xf9]), // F9 code page 437 byte F9
    (0x0060, 0x00, [0x2e, 0x2e, 0xfa]), // FA code page 437 byte FA
    (0x0000, 0x00, [0x3f, 0x3f, 0xfb]), // FB code page 437 byte FB
    (0x0000, 0x00, [0x3f, 0x3f, 0xfc]), // FC code page 437 byte FC
    (0x0000, 0x00, [0x3f, 0x3f, 0xfd]), // FD code page 437 byte FD
    (0x0000, 0x00, [0x3f, 0x3f, 0xfe]), // FE code page 437 byte FE
    (0x0000, 0x00, [0x3f, 0x3f, 0xff]), // FF code page 437 byte FF
];

fn class(b: u8) -> u32 {
    CHARS[b as usize].0 as u32
}

/// Trailing byte counts and offsets for the UTF-8 decoder (no validation: a lone trailing byte is itself, a lead
/// byte takes the next bytes whatever they are).
pub(crate) const TRAILING: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0xc0;
    while i < 256 {
        t[i] = if i < 0xe0 {
            1
        } else if i < 0xf0 {
            2
        } else if i < 0xf8 {
            3
        } else if i < 0xfc {
            4
        } else {
            5
        };
        i += 1;
    }
    t
};
pub(crate) const UTF8_OFFSETS: [u32; 6] = [0, 0x3080, 0xE2080, 0x3C82080, 0xFA082080, 0x82082080];

const ROWS: usize = 100;
const COLS: usize = 200;
const BOXES: usize = 200;

// Box kinds.
const BLOCK: i32 = 0x28;
const COMPUTER: i32 = 0x32;
const CENTRED: i32 = 100;
const LEFT_HEADING: i32 = 0x6e;
const CLOSE: i32 = 0x72;
const SALUTATION: i32 = 0x74;
const PAGE: i32 = 0x78;
const BODY: i32 = 0x82;
const FOOTER: i32 = 0x8c;
const COLUMN: i32 = 0xa0;
const PAGE_NUMBER: i32 = 200;
const RUNNING_FOOT: i32 = 0xd2;
const RUNNING_HEAD: i32 = 0xd6;
const HEADER: i32 = 0xf0;
const TABLE: i32 = 0x104;
const TABLE_COLUMN: i32 = 0x118;
const PARAGRAPH: i32 = 0x140;

/// Codes that differ by the output language (output factor 10: 0 English, 1 French, 2 Spanish).
const CODE_RUN_END: [&str; 3] = [" $tle", " $htt", " $tlp"];
const CODE_RUN_START: [&str; 3] = ["$tls ", "$htc ", "$tlc "];
const CODE_INDENT: [&str; 3] = ["$ind", "$aln", "$ind"];
const CODE_INDENT_1: [&str; 3] = ["$ind1", "$aln1", "$ind1"];
const CODE_KEEP_END: [&str; 3] = ["$kpe", "$rnt", "$rnp"];
const CODE_KEEP_START: [&str; 3] = ["$kps", "$rnc", "$rnc"];
const CODE_SKIP: [&str; 3] = ["$l $sl1", "$l $sl1", "$l $pl1"];
const CODE_PAGE: [&str; 3] = ["$leaf- ", "$pof- ", "$leaf- "];

/// A rectangle of the page and its place in the box tree.
#[derive(Debug, Clone, Copy)]
struct Bx {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
    kind: i32,
    /// Paragraph: continued from the previous page or column. Blocked text: an outline.
    flag: i32,
    parent: i32,
    first: i32,
    prev: i32,
    next: i32,
}

const NO_BOX: Bx = Bx { left: -1, right: -1, top: -1, bottom: -1, kind: -1, flag: -1, parent: -1, first: -1, prev: -1, next: -1 };

/// A rule line pattern: rows of up to five cells. 'c' the character found, 'h' any horizontal rule character, 'e'
/// any vertical rule character, 'r' a character that is both, ' ' a space, '?' anything.
struct Pattern {
    rows: usize,
    cols: usize,
    cells: [&'static [u8]; 4],
}

const fn pat(rows: usize, cols: usize, cells: [&'static [u8]; 4]) -> Pattern {
    Pattern { rows, cols, cells }
}

const HORIZONTAL_PLUS: [Pattern; 5] = [
    pat(1, 4, [b"cccc", b"", b"", b""]),
    pat(2, 3, [b"ccc", b"ccc", b"", b""]),
    pat(1, 5, [b"c c c", b"", b"", b""]),
    pat(1, 4, [b"chhh", b"", b"", b""]),
    pat(1, 4, [b"hhhc", b"", b"", b""]),
];
const HORIZONTAL_DOT: [Pattern; 3] = [
    pat(1, 5, [b"ccccc", b"", b"", b""]),
    pat(2, 4, [b"cccc", b"cccc", b"", b""]),
    pat(1, 5, [b"c c c", b"", b"", b""]),
];
const HORIZONTAL_OTHER: [Pattern; 5] = [
    pat(1, 4, [b"cccc", b"", b"", b""]),
    pat(2, 3, [b"ccc", b"ccc", b"", b""]),
    pat(1, 5, [b"c c c", b"", b"", b""]),
    pat(1, 4, [b"cccr", b"", b"", b""]),
    pat(1, 4, [b"rccc", b"", b"", b""]),
];
const VERTICAL_PLUS: [Pattern; 4] = [
    pat(4, 1, [b"c", b"c", b"c", b"c"]),
    pat(2, 2, [b"cc", b"cc", b"", b""]),
    pat(4, 1, [b"e", b"e", b"e", b"c"]),
    pat(4, 1, [b"c", b"e", b"e", b"e"]),
];
const VERTICAL_DOT: [Pattern; 5] = [
    pat(1, 5, [b"ccccc", b"", b"", b""]),
    pat(2, 3, [b"ccc", b"ccc", b"", b""]),
    pat(3, 2, [b"cc", b"cc", b"cc", b""]),
    pat(1, 5, [b"c c c", b"", b"", b""]),
    pat(2, 3, [b"c c", b"c c", b"", b""]),
];
const VERTICAL_OTHER: [Pattern; 4] = [
    pat(4, 1, [b"c", b"c", b"c", b"c"]),
    pat(2, 2, [b"cc", b"cc", b"", b""]),
    pat(4, 1, [b"c", b"c", b"c", b"r"]),
    pat(4, 1, [b"r", b"c", b"c", b"c"]),
];

struct Engine<'a> {
    /// Input factors (g) and output factors (b).
    g: [i32; 100],
    b: [i32; 100],
    set: usize,
    lang: usize,
    /// Output line width: 0, as the original program passes it.
    out_width: i32,
    input: &'a [u8],
    at: usize,
    // Word reader: the word read ahead, the delimiter to give before it and the one after it.
    word: [i32; 200],
    word_len: i32,
    word_pos: i32,
    word_delim: i32,
    word_next: i32,
    // The page: characters, attributes from reading (bit 1 underlined or the same character struck over, bit 2 a
    // different character struck over, 0x20 accented letter, 0x10 and 0x04 horizontal and vertical rule) and output
    // marks (1 underline starts, 2 double underline starts, 4 underline ends inside a word, 8 rule cell at a line
    // edge, 0x10 accent prefix, 0x20 single quote to write as a backquote).
    chars: Vec<[u16; COLS]>,
    attrs: Vec<[u32; COLS]>,
    marks: Vec<[u32; COLS]>,
    depth: i32,
    width: i32,
    boxes: Vec<Bx>,
    free: i32,
    /// Text columns on this page, and whether the page's lines are often broken with hyphens.
    ncols: i32,
    hyphenated: bool,
    hyphen_lines: i32,
    hyphen_ends: i32,
    /// The last paragraph of the previous column: blank cells after its last line and whether it ended a sentence.
    prev_room: i32,
    prev_sentence: i32,
    salutation_seen: bool,
    /// What the last box written ended with: 1 a skipped line or block end, 2 a page number, 0 anything else.
    last_kind: i32,
    running_head_seen: bool,
    col_level: [i32; COLS + 1],
    row_item: [i32; ROWS + 1],
    out: Vec<u8>,
}

/// Turns a formatted text file into running text with style codes (UTF-8, one line feed after each line).
///
/// `set` is the substitution set for U+0080 to U+00FF (0 accents dropped, 1 code page 437 accented letters kept, 2
/// every byte kept); below 0 it is taken from input factor 36, and outside 0 to 2 it becomes 0. The original program
/// passes the code page its file typing guessed (0 Windows, 1 DOS, 2 Macintosh, -1 not known).
pub fn pimbri(input: &[u8], set: i32, factors: &Factors) -> Vec<u8> {
    let mut e = Engine::new(input, set, factors);
    while e.at < e.input.len() {
        e.page();
    }
    e.out
}

impl<'a> Engine<'a> {
    fn new(input: &'a [u8], set: i32, f: &Factors) -> Engine<'a> {
        let mut set = if set < 0 { f.genl[36] } else { set };
        if !(0..=2).contains(&set) {
            set = 0;
        }
        let lang = if (0..=2).contains(&f.book[10]) { f.book[10] as usize } else { 0 };
        Engine {
            g: f.genl,
            b: f.book,
            set: set as usize,
            lang,
            out_width: 0,
            input,
            at: 0,
            word: [0; 200],
            word_len: 0,
            word_pos: 0,
            word_delim: 0,
            word_next: 0,
            chars: vec![[0; COLS]; ROWS],
            attrs: vec![[0; COLS]; ROWS],
            marks: vec![[0; COLS]; ROWS],
            depth: 0,
            width: 0,
            boxes: vec![NO_BOX; BOXES],
            free: -1,
            ncols: 0,
            hyphenated: false,
            hyphen_lines: 0,
            hyphen_ends: 0,
            prev_room: i32::MAX,
            prev_sentence: 1,
            salutation_seen: false,
            last_kind: 0,
            running_head_seen: false,
            col_level: [-1; COLS + 1],
            row_item: [-1; ROWS + 1],
            out: Vec::new(),
        }
    }

    // ----- Cells -----

    fn ok(r: i32, c: i32) -> bool {
        (0..ROWS as i32).contains(&r) && (0..COLS as i32).contains(&c)
    }

    fn ch(&self, r: i32, c: i32) -> u16 {
        if Self::ok(r, c) {
            self.chars[r as usize][c as usize]
        } else {
            0
        }
    }

    /// The low byte of a cell, which most tests look at (so U+2014 counts as a blank).
    fn lo(&self, r: i32, c: i32) -> u8 {
        self.ch(r, c) as u8
    }

    fn attr(&self, r: i32, c: i32) -> u32 {
        if Self::ok(r, c) {
            self.attrs[r as usize][c as usize]
        } else {
            0
        }
    }

    fn mark(&self, r: i32, c: i32) -> u32 {
        if Self::ok(r, c) {
            self.marks[r as usize][c as usize]
        } else {
            0
        }
    }

    fn add_mark(&mut self, r: i32, c: i32, bits: u32) {
        if Self::ok(r, c) {
            self.marks[r as usize][c as usize] |= bits;
        }
    }

    fn set_mark(&mut self, r: i32, c: i32, v: u32) {
        if Self::ok(r, c) {
            self.marks[r as usize][c as usize] = v;
        }
    }

    fn add_attr(&mut self, r: i32, c: i32, bits: u32) {
        if Self::ok(r, c) {
            self.attrs[r as usize][c as usize] |= bits;
        }
    }

    /// Class flags of a cell whose whole value is below 0x100 (0 otherwise).
    fn class_full(&self, r: i32, c: i32) -> u32 {
        let v = self.ch(r, c);
        if v < 0x100 {
            class(v as u8)
        } else {
            0
        }
    }

    /// Class flags of a cell whose low byte is below 0x80, whatever the high byte (0 otherwise).
    fn class_low(b: u8) -> u32 {
        if b < 0x80 {
            class(b)
        } else {
            0
        }
    }

    fn bx(&self, i: usize) -> Bx {
        self.boxes.get(i).copied().unwrap_or(NO_BOX)
    }

    // ----- Reading -----

    fn read_byte(&mut self) -> i32 {
        match self.input.get(self.at) {
            Some(&b) => {
                self.at += 1;
                b as i32
            }
            None => -1,
        }
    }

    /// One UTF-8 character, or -1 at the end of the input (also when the end cuts a sequence short). Values above
    /// 0x7FFFFFFF become U+FFFD.
    fn decode(&mut self) -> i32 {
        let first = self.read_byte();
        if first < 0 {
            return -1;
        }
        let extra = TRAILING[first as usize] as usize;
        let mut acc: u32 = 0;
        let mut b = first as u32;
        for _ in 0..extra {
            acc = acc.wrapping_add(b).wrapping_mul(64);
            let next = self.read_byte();
            if next < 0 {
                return -1;
            }
            b = next as u32;
        }
        acc = acc.wrapping_add(b);
        let v = acc.wrapping_sub(UTF8_OFFSETS[extra]);
        if v > 0x7fff_ffff {
            0xfffd
        } else {
            v as i32
        }
    }

    /// The word reader: hands out the characters of a word, and before each word the delimiter that preceded it
    /// (space, line feed, carriage return, form feed, -1 at the end). A tab is one space; other control characters
    /// except backspace are dropped. A word holds at most 200 characters (a longer one goes on as a new word after a
    /// line feed); when the word does not fit in `room` cells its delimiter is given as a line feed.
    fn token(&mut self, room: i32) -> i32 {
        loop {
            if self.word_delim == -1 {
                return -1;
            }
            if self.word_next == 0 || self.word_len <= self.word_pos {
                self.word_delim = self.word_next;
                if self.word_delim == -1 {
                    return -1;
                }
                let mut n = 0usize;
                loop {
                    if n >= 200 {
                        self.word_next = 10;
                        break;
                    }
                    let c = self.decode();
                    if c < 0x21 && c != 8 {
                        if c == 0x20 || c == 10 || c == 13 || c == 12 || c == -1 {
                            self.word_next = c;
                            break;
                        }
                        if c == 9 {
                            self.word_next = 0x20;
                            break;
                        }
                        continue;
                    }
                    self.word[n] = c;
                    n += 1;
                }
                self.word_len = n as i32;
                self.word_pos = 0;
                if self.word_delim != 0 {
                    if room < self.word_len {
                        return 10;
                    }
                    return self.word_delim;
                }
            }
            if self.word_pos < self.word_len {
                let c = self.word[self.word_pos as usize];
                self.word_pos += 1;
                return c;
            }
        }
    }

    fn clear_row(&mut self, r: usize) {
        self.chars[r] = [0; COLS];
        self.attrs[r] = [0; COLS];
    }

    /// Reads one page into the grid, then sets the depth and width, removes double spacing and clears the marks.
    fn read_page(&mut self) {
        self.depth = 0;
        self.width = 0;
        let mut ink = [false; ROWS];
        self.clear_row(0);
        let mut row = 0usize;
        let mut col: i32 = 0;
        loop {
            let t = self.token(COLS as i32 - col);
            if t < 0x20 {
                match t {
                    13 => col = 0,
                    10 => {
                        if row + 1 < ROWS {
                            row += 1;
                            self.clear_row(row);
                            col = 0;
                        } else {
                            break;
                        }
                    }
                    8 => {
                        if col != 0 {
                            col -= 1;
                        }
                    }
                    12 | -1 => break,
                    // 0x1A is ignored; nothing else below 0x20 comes from the word reader.
                    _ => {}
                }
                continue;
            }
            if col < COLS as i32 {
                let c = col as usize;
                let cell = self.chars[row][c];
                if t == 0x5f {
                    // An underscore is never stored: it underlines the cell.
                    self.attrs[row][c] |= 1;
                    if cell == 0 {
                        self.chars[row][c] = 0x20;
                    }
                } else if t == 0x20 {
                    if cell == 0 {
                        self.chars[row][c] = 0x20;
                    }
                } else if !(0..=0x100).contains(&t) {
                    ink[row] = true;
                    self.chars[row][c] = t as u16;
                    self.attrs[row][c] = 0;
                } else {
                    ink[row] = true;
                    // The stored value, the value the original compares with for "the same character again" (a
                    // replacement of 0x80 or more never compares equal), and the attributes.
                    let (stored, same, at) = if t == 0x100 {
                        // The original reads one entry past the end of its 256-entry table here, into its own
                        // engine fields: the attributes there are 0 and the replacement is a byte of a pointer, which
                        // cannot be known. The port keeps U+0100.
                        (0x100u16, 0x100i32, 0u32)
                    } else {
                        let (_, a, rep) = CHARS[t as usize];
                        let r = rep[self.set] as i8;
                        (r as i16 as u16, r as i32, a as u32)
                    };
                    if cell == 0 || cell == 0x20 {
                        self.chars[row][c] = stored;
                        self.attrs[row][c] = at;
                    } else if cell as i32 == same {
                        self.attrs[row][c] |= 1;
                    } else {
                        self.attrs[row][c] |= 2;
                    }
                }
                col += 1;
            }
            if self.width < col {
                self.width = col;
            }
        }
        let mut depth = row as i32;
        if self.chars[row][0] != 0 {
            depth += 1;
        }
        let width = self.width;
        for r in 0..depth as usize {
            let mut c = width - 1;
            while c >= 0 && self.chars[r][c as usize] == 0 {
                self.chars[r][c as usize] = 0x20;
                c -= 1;
            }
        }
        // Blank columns at the right and blank rows at the bottom are taken off.
        let mut width = width;
        while width > 0 && (0..depth as usize).all(|r| self.chars[r][(width - 1) as usize] == 0x20) {
            width -= 1;
        }
        while depth > 0 && (0..width as usize).all(|c| self.chars[(depth - 1) as usize][c] == 0x20) {
            depth -= 1;
        }
        // Double spacing: k blank rows go after every row that has text, k being the shortest run of blank rows
        // between two such rows (at most input factor 37; the run before the first counts as long enough).
        let mut k = self.g[37];
        if k > 0 && self.g[38] <= depth {
            let mut run = k;
            for &has_ink in ink.iter().take(depth as usize) {
                if !has_ink {
                    run = run.wrapping_add(1);
                } else {
                    if run < k {
                        k = run;
                        if run == 0 {
                            break;
                        }
                    }
                    run = 0;
                }
            }
            if k > 0 {
                let mut dest = 0usize;
                let mut r = 0usize;
                while r < depth as usize {
                    if dest != r {
                        for c in 0..width as usize {
                            self.chars[dest][c] = self.chars[r][c];
                            self.attrs[dest][c] = self.attrs[r][c];
                        }
                    }
                    dest += 1;
                    if ink[r] {
                        r = r.saturating_add(k as usize);
                    }
                    r = r.saturating_add(1);
                }
                depth = dest as i32;
            }
        }
        for r in 0..depth as usize {
            for c in 0..width as usize {
                self.marks[r][c] = 0;
            }
        }
        self.depth = depth;
        self.width = width;
    }

    // ----- Rule lines -----

    /// Marks the characters that form rule lines: horizontal ones get attribute 0x10, vertical ones 0x04.
    fn find_rules(&mut self) {
        for r in 0..self.depth {
            for c in 0..self.width {
                let b = self.lo(r, c);
                if Self::class_low(b) & 0x20 != 0 && self.attr(r, c) & 0x10 == 0 {
                    let pats: &[Pattern] = match b {
                        b'+' => &HORIZONTAL_PLUS,
                        b'.' => &HORIZONTAL_DOT,
                        _ => &HORIZONTAL_OTHER,
                    };
                    for p in pats {
                        if self.rule_match(1, p, r, c, b) {
                            break;
                        }
                    }
                }
            }
        }
        for c in 0..self.width {
            for r in 0..self.depth {
                let b = self.lo(r, c);
                if Self::class_low(b) & 0x40 != 0 && self.attr(r, c) & 4 == 0 {
                    let pats: &[Pattern] = match b {
                        b'+' => &VERTICAL_PLUS,
                        b'.' => &VERTICAL_DOT,
                        _ => &VERTICAL_OTHER,
                    };
                    for p in pats {
                        if self.rule_match(2, p, r, c, b) {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Tries a pattern with each of its 'c' cells placed on (row, col); on a match marks its cells (direction 1
    /// horizontal, 2 vertical).
    fn rule_match(&mut self, dir: i32, p: &Pattern, row: i32, col: i32, b: u8) -> bool {
        let mut grid = [[b'?'; 5]; 5];
        for (i, line) in p.cells.iter().enumerate().take(p.rows.min(4)) {
            for (j, &x) in line.iter().enumerate().take(p.cols.min(5)) {
                grid[i][j] = x;
            }
        }
        for ar in 0..p.rows {
            for ac in 0..p.cols {
                if grid[ar][ac] != b'c' {
                    continue;
                }
                let mut ok = true;
                'rows: for (pr, prow) in grid.iter().enumerate().take(p.rows) {
                    for (pc, &want) in prow.iter().enumerate().take(p.cols) {
                        if !ok {
                            break 'rows;
                        }
                        if want == b'?' || (pr == ar && pc == ac) {
                            continue;
                        }
                        let rr = row + pr as i32 - ar as i32;
                        let cc = col + pc as i32 - ac as i32;
                        let x = if rr < 0 || rr >= self.depth || cc < 0 || cc >= self.width { 0x20 } else { self.lo(rr, cc) };
                        ok = match want {
                            b' ' => x == 0x20,
                            b'c' => x == b,
                            b'e' => Self::class_low(x) & 0x40 != 0,
                            b'h' => Self::class_low(x) & 0x20 != 0,
                            b'n' => x != 0x20,
                            b'r' => Self::class_low(x) & 0x60 == 0x60,
                            other => x == other,
                        };
                    }
                }
                if ok {
                    for (pr, prow) in grid.iter().enumerate().take(p.rows) {
                        for (pc, &want) in prow.iter().enumerate().take(p.cols) {
                            let rr = row + pr as i32 - ar as i32;
                            let cc = col + pc as i32 - ac as i32;
                            if rr < 0 || rr >= self.depth || cc < 0 || cc >= self.width {
                                continue;
                            }
                            match want {
                                b'c' => self.add_attr(rr, cc, if dir == 1 { 0x10 } else { 4 }),
                                b'e' => self.add_attr(rr, cc, 4),
                                b'h' => self.add_attr(rr, cc, 0x10),
                                b'r' => self.add_attr(rr, cc, 0x14),
                                _ => {}
                            }
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    // ----- Box tree -----

    fn alloc(&mut self) -> Option<usize> {
        if self.free < 0 {
            // The original has no answer for running out of its 200 boxes (it reads before its box array); the port
            // leaves the box out.
            return None;
        }
        let i = self.free as usize;
        self.free = self.bx(i).next;
        Some(i)
    }

    fn release(&mut self, i: usize) {
        if let Some(b) = self.boxes.get_mut(i) {
            b.next = self.free;
            self.free = i as i32;
        }
    }

    fn free_tree(&mut self, i: usize) {
        let mut c = self.bx(i).first;
        while c >= 0 {
            let next = self.bx(c as usize).next;
            self.free_tree(c as usize);
            c = next;
        }
        self.release(i);
    }

    /// Puts `child` under `parent`, after the sibling `after` (or first when `after` is below 0).
    fn link(&mut self, child: usize, parent: usize, after: i32) {
        if after < 0 {
            self.boxes[child].next = self.boxes[parent].first;
            self.boxes[parent].first = child as i32;
        } else {
            self.boxes[child].next = self.boxes[after as usize].next;
            self.boxes[after as usize].next = child as i32;
        }
        self.boxes[child].prev = after;
        let n = self.boxes[child].next;
        if n >= 0 {
            self.boxes[n as usize].prev = child as i32;
        }
        self.boxes[child].parent = parent as i32;
    }

    fn unlink(&mut self, child: usize) {
        let parent = self.boxes[child].parent;
        self.boxes[child].parent = -1;
        let prev = self.boxes[child].prev;
        let next = self.boxes[child].next;
        if next >= 0 {
            self.boxes[next as usize].prev = prev;
        }
        self.boxes[child].prev = -1;
        if prev < 0 {
            if parent >= 0 {
                self.boxes[parent as usize].first = next;
            }
        } else {
            self.boxes[prev as usize].next = next;
        }
        self.boxes[child].next = -1;
    }

    /// A new box with the given rectangle and kind, linked under `parent` after `after`.
    #[allow(clippy::too_many_arguments)]
    fn add_box(&mut self, parent: usize, after: i32, left: i32, right: i32, top: i32, bottom: i32, kind: i32) -> Option<usize> {
        let i = self.alloc()?;
        let b = &mut self.boxes[i];
        b.first = -1;
        b.left = left;
        b.right = right;
        b.top = top;
        b.bottom = bottom;
        b.kind = kind;
        self.link(i, parent, after);
        Some(i)
    }

    /// Adds a box and tests it as its kind; on failure takes it out again.
    #[allow(clippy::too_many_arguments)]
    fn attempt(&mut self, parent: usize, after: i32, left: i32, right: i32, top: i32, bottom: i32, kind: i32) -> Option<usize> {
        let i = self.add_box(parent, after, left, right, top, bottom, kind)?;
        if self.classify(i) {
            Some(i)
        } else {
            self.unlink(i);
            self.free_tree(i);
            None
        }
    }

    // ----- Measures -----

    /// Cells before the first visible character of a row of the box (the box width when there is none).
    fn first_ink(&self, i: usize, r: i32) -> i32 {
        let b = self.bx(i);
        let mut c = b.left;
        while c <= b.right && self.lo(r, c) < 0x21 {
            c += 1;
        }
        c - b.left
    }

    /// Blank cells after the last visible character of a row of the box.
    fn end_room(&self, i: usize, r: i32) -> i32 {
        let b = self.bx(i);
        let mut c = b.right;
        while c >= b.left && self.lo(r, c) < 0x21 {
            c -= 1;
        }
        b.right - c
    }

    /// Length of the visible run at the left edge of a row of the box.
    fn first_word(&self, i: usize, r: i32) -> i32 {
        let b = self.bx(i);
        let mut c = b.left;
        while c <= b.right && self.lo(r, c) > 0x20 {
            c += 1;
        }
        c - b.left
    }

    /// The last visible character of a row of the box ends a sentence.
    fn ends_sentence(&self, i: usize, r: i32) -> bool {
        let b = self.bx(i);
        let mut c = b.right;
        while c >= b.left {
            let x = self.lo(r, c);
            if x > 0x20 {
                return class(x) & 0x10 != 0;
            }
            c -= 1;
        }
        false
    }

    /// The low byte of the last visible character of a row of the box, 0 when there is none.
    fn last_ink(&self, i: usize, r: i32) -> u8 {
        let b = self.bx(i);
        let mut c = b.right;
        while c >= b.left {
            let x = self.lo(r, c);
            if x > 0x20 {
                return x;
            }
            c -= 1;
        }
        0
    }

    /// The first letter of a row of the box is a capital.
    fn row_starts_upper(&self, i: usize, r: i32) -> bool {
        let b = self.bx(i);
        for c in b.left..=b.right {
            let k = self.class_full(r, c);
            if k & 2 != 0 {
                return k & 8 != 0;
            }
        }
        false
    }

    /// The first letter of the box is a capital.
    fn box_starts_upper(&self, i: usize) -> bool {
        let b = self.bx(i);
        for r in b.top..=b.bottom {
            for c in b.left..=b.right {
                let k = self.class_full(r, c);
                if k & 2 != 0 {
                    return k & 8 != 0;
                }
            }
        }
        false
    }

    /// Capital letters per letters, times 10000.
    fn caps(&self, i: usize) -> i32 {
        let b = self.bx(i);
        let (mut letters, mut upper) = (0i32, 0i32);
        for r in b.top..=b.bottom {
            for c in b.left..=b.right {
                let k = self.class_full(r, c);
                if k & 2 != 0 {
                    letters += 1;
                    if k & 8 != 0 {
                        upper += 1;
                    }
                }
            }
        }
        if letters == 0 {
            0
        } else {
            upper * 10000 / letters
        }
    }

    /// The cells from `from` to `to` of a row are all blank.
    fn blank_span(&self, r: i32, from: i32, to: i32) -> bool {
        (from..=to).all(|c| self.lo(r, c) <= 0x20)
    }

    /// The text from `from` to `to` of a row starts with an enumerator: one letter, or digits and punctuation, with
    /// any non-alphanumeric characters before (a., A), 12., (b), 3). With `stop` the enumerator only has to be
    /// followed by a blank; without, nothing else may follow it.
    fn enumerator(&self, r: i32, from: i32, to: i32, stop: bool) -> bool {
        let mut state = 1;
        for c in from..=to {
            let x = self.lo(r, c);
            let k = Self::class_low(x);
            match state {
                1 => {
                    if x > 0x20 {
                        state = if k & 2 != 0 {
                            2
                        } else if k & 1 != 0 {
                            3
                        } else {
                            4
                        };
                    }
                }
                2 => {
                    if x < 0x21 {
                        if stop {
                            return true;
                        }
                        state = 5;
                    } else if k & 2 != 0 {
                        return false;
                    } else {
                        state = 3;
                    }
                }
                3 => {
                    if x < 0x21 {
                        if stop {
                            return true;
                        }
                        state = 5;
                    } else if k & 2 != 0 {
                        state = 2;
                    }
                }
                4 => {
                    if x < 0x21 {
                        return false;
                    }
                    if k & 2 != 0 {
                        state = 2;
                    } else if k & 1 != 0 {
                        state = 3;
                    }
                }
                _ => {
                    if x > 0x20 {
                        return false;
                    }
                }
            }
        }
        matches!(state, 2 | 3 | 5)
    }

    /// The column where a right margin is measured from: the box's right edge, or further right when the box is
    /// narrower than an assumed page width (input factor 42, divided among the text columns).
    fn right_limit(&self, i: usize) -> i32 {
        let b = self.bx(i);
        let mut v = self.g[42];
        if self.ncols > 1 {
            v /= self.ncols;
        }
        v = v.wrapping_sub(1).wrapping_add(b.left);
        v.max(b.right)
    }

    /// The column runs of the box separated by enough blank columns (input factor 33 for boxes less tall than factor
    /// 32, else factor 34) or by one column of vertical rule. The last run ends at the box's right edge.
    fn segments(&self, i: usize) -> Vec<(i32, i32)> {
        let b = self.bx(i);
        let gap = (if b.bottom - b.top < self.g[32] { self.g[33] } else { self.g[34] }).max(1);
        let mut segs: Vec<(i32, i32)> = Vec::new();
        let mut open = false;
        let mut last = 0;
        for c in b.left..=b.right {
            let mut empty = true;
            let mut rule = false;
            for r in b.top..=b.bottom {
                if self.lo(r, c) > 0x20 {
                    empty = false;
                    if self.attr(r, c) & 4 == 0 {
                        rule = false;
                        break;
                    }
                    rule = true;
                }
            }
            if empty || rule {
                if open && (rule || c - last >= gap) {
                    if let Some(s) = segs.last_mut() {
                        s.1 = last;
                    }
                    open = false;
                }
            } else {
                last = c;
                if !open {
                    segs.push((c, 0));
                    open = true;
                }
            }
        }
        if open {
            if let Some(s) = segs.last_mut() {
                s.1 = b.right;
            }
        }
        segs
    }

    /// The row runs of the box separated by blank rows (a row of horizontal rule counts as blank).
    fn row_groups(&self, i: usize) -> Vec<(i32, i32)> {
        let b = self.bx(i);
        let mut groups: Vec<(i32, i32)> = Vec::new();
        let mut open = false;
        let mut last = 0;
        for r in b.top..=b.bottom {
            let mut empty = true;
            let mut rule = false;
            for c in b.left..=b.right {
                if self.lo(r, c) > 0x20 {
                    empty = false;
                    if self.attr(r, c) & 0x10 == 0 {
                        rule = false;
                        break;
                    }
                    rule = true;
                }
            }
            if empty || rule {
                if open && (rule || r - last > 0) {
                    if let Some(g) = groups.last_mut() {
                        g.1 = last;
                    }
                    open = false;
                }
            } else {
                last = r;
                if !open {
                    groups.push((r, 0));
                    open = true;
                }
            }
        }
        if open {
            if let Some(g) = groups.last_mut() {
                g.1 = b.bottom;
            }
        }
        groups
    }

    fn col_has_ink(&self, b: &Bx, c: i32) -> bool {
        (b.top..=b.bottom).any(|r| self.lo(r, c) > 0x20)
    }

    fn row_has_ink(&self, b: &Bx, r: i32) -> bool {
        (b.left..=b.right).any(|c| self.lo(r, c) > 0x20)
    }

    /// Moves the box's left edge to its first column with text. The original searches on without limit; the port
    /// stops at the page's last column and leaves the edge when nothing is found.
    fn trim_left(&mut self, i: usize) {
        let b = self.bx(i);
        let mut c = b.left.max(0);
        while c < COLS as i32 {
            if self.col_has_ink(&b, c) {
                self.boxes[i].left = c;
                return;
            }
            c += 1;
        }
    }

    fn trim_right(&mut self, i: usize) {
        let b = self.bx(i);
        let mut c = b.right.min(COLS as i32 - 1);
        while c >= 0 {
            if self.col_has_ink(&b, c) {
                self.boxes[i].right = c;
                return;
            }
            c -= 1;
        }
    }

    fn trim_top(&mut self, i: usize) {
        let b = self.bx(i);
        let mut r = b.top.max(0);
        while r < ROWS as i32 {
            if self.row_has_ink(&b, r) {
                self.boxes[i].top = r;
                return;
            }
            r += 1;
        }
    }

    fn trim_bottom(&mut self, i: usize) {
        let b = self.bx(i);
        let mut r = b.bottom.min(ROWS as i32 - 1);
        while r >= 0 {
            if self.row_has_ink(&b, r) {
                self.boxes[i].bottom = r;
                return;
            }
            r -= 1;
        }
    }

    // ----- Analysis -----

    fn classify(&mut self, i: usize) -> bool {
        match self.bx(i).kind {
            BLOCK => self.is_block(i),
            COMPUTER => self.is_computer_text(i),
            CENTRED => self.is_centred(i),
            LEFT_HEADING => self.is_left_heading(i),
            CLOSE => self.is_close(i),
            SALUTATION => self.is_salutation(i),
            PAGE => self.analyse_page(i),
            BODY => self.analyse_body(i),
            FOOTER => self.is_header(i, RUNNING_FOOT),
            COLUMN => self.analyse_column(i),
            PAGE_NUMBER => self.is_page_number(i),
            HEADER => self.is_header(i, RUNNING_HEAD),
            TABLE => self.is_table(i),
            _ => true,
        }
    }

    /// The page: a one-line first group may be a header and a one-line last group a footer (each must hold a page
    /// number); the rest is the body.
    fn analyse_page(&mut self, i: usize) -> bool {
        self.trim_top(i);
        self.trim_bottom(i);
        self.trim_left(i);
        self.trim_right(i);
        let groups = self.row_groups(i);
        let n = groups.len();
        if n == 0 {
            // Only rule lines: the original then works with values it never set. The port writes nothing.
            return false;
        }
        let b = self.bx(i);
        let mut header = false;
        let mut footer = false;
        let mut last: i32 = -1;
        if n > 1 {
            if self.g[46] == 0 && groups[0].0 == groups[0].1 {
                if let Some(h) = self.attempt(i, -1, b.left, b.right, groups[0].0, groups[0].1, HEADER) {
                    header = true;
                    last = h as i32;
                }
            }
            let g = groups[n - 1];
            if self.g[47] == 0 && (!header || n > 2) && g.0 == g.1 {
                if let Some(f) = self.attempt(i, last, b.left, b.right, g.0, g.1, FOOTER) {
                    footer = true;
                    last = f as i32;
                }
            }
        }
        let top = if header { groups[1].0 } else { groups[0].0 };
        let bottom = if footer { groups[n - 2].1 } else { groups[n - 1].1 };
        if let Some(body) = self.add_box(i, last, b.left, b.right, top, bottom, BODY) {
            self.classify(body);
        }
        true
    }

    /// A header or footer line: at most two parts, one of them a page number; the other becomes the running head or
    /// foot.
    fn is_header(&mut self, i: usize, running: i32) -> bool {
        self.trim_left(i);
        self.trim_right(i);
        let segs = self.segments(i);
        if segs.len() >= 3 {
            return false;
        }
        let b = self.bx(i);
        let mut found = None;
        for (k, s) in segs.iter().enumerate() {
            if self.attempt(i, -1, s.0, s.1, b.top, b.bottom, PAGE_NUMBER).is_some() {
                found = Some(k);
                break;
            }
        }
        let Some(k) = found else { return false };
        if segs.len() > 1 {
            let other = if k == 0 { 1 } else { 0 };
            self.add_box(i, -1, segs[other].0, segs[other].1, b.top, b.bottom, running);
        }
        true
    }

    /// A page number: "-...-", "(...)" or "[...]" around it is taken off; the text may hold one space, at column 4
    /// or before and after a word starting with "P" or "p" (the number follows it); it must hold a digit.
    fn is_page_number(&mut self, i: usize) -> bool {
        let b = self.bx(i);
        let r = b.top;
        let first = self.lo(r, b.left);
        let last = self.lo(r, b.right);
        if (first == b'-' && last == b'-') || (first == b'(' && last == b')') || (first == b'[' && last == b']') {
            self.boxes[i].left += 1;
            self.boxes[i].right -= 1;
            if self.boxes[i].right < self.boxes[i].left {
                return false;
            }
            self.trim_left(i);
            if self.boxes[i].right < self.boxes[i].left {
                return false;
            }
            self.trim_right(i);
            if self.boxes[i].right < self.boxes[i].left {
                return false;
            }
        }
        let b = self.bx(i);
        let mut space = -1;
        for c in b.left..=b.right {
            if self.lo(r, c) == b' ' {
                if space >= 0 {
                    return false;
                }
                space = c;
            }
        }
        if space >= 0 {
            // The column is counted from the page's left edge, not the box's.
            if space > 4 {
                return false;
            }
            let f = self.lo(r, b.left);
            if f != b'P' && f != b'p' {
                return false;
            }
            self.boxes[i].left = space + 1;
        }
        let b = self.bx(i);
        (b.left..=b.right).any(|c| self.class_full(r, c) & 1 != 0)
    }

    /// The body: text columns when it splits into 2 to factor 44 parts whose widths differ by at most factor 45 and
    /// it has at most factor 43 row groups; otherwise one column.
    fn analyse_body(&mut self, i: usize) -> bool {
        let b = self.bx(i);
        let segs = self.segments(i);
        let mut columns = false;
        if segs.len() > 1 && segs.len() as i64 <= self.g[44] as i64 {
            let widest = segs.iter().map(|s| s.1 - s.0 + 1).max().unwrap_or(0);
            let narrowest = segs.iter().map(|s| s.1 - s.0 + 1).min().unwrap_or(0);
            if (widest - narrowest) as i64 <= self.g[45] as i64 && self.row_groups(i).len() as i64 <= self.g[43] as i64 {
                columns = true;
            }
        }
        if !columns {
            self.ncols = 1;
            if let Some(c) = self.add_box(i, -1, b.left, b.right, b.top, b.bottom, COLUMN) {
                self.classify(c);
            }
        } else {
            self.ncols = segs.len() as i32;
            let mut prev: i32 = -1;
            for s in &segs {
                if let Some(c) = self.add_box(i, prev, s.0, s.1, b.top, b.bottom, COLUMN) {
                    self.classify(c);
                    prev = c as i32;
                }
            }
        }
        true
    }

    /// A column: each row group is classified in turn.
    fn analyse_column(&mut self, i: usize) -> bool {
        let col = self.bx(i);
        let groups = self.row_groups(i);
        let n = groups.len();
        let mut last: i32 = -1;
        let mut k = 0usize;
        while k < n {
            let (top, bottom) = groups[k];
            let mut found = self.attempt(i, last, col.left, col.right, top, bottom, COMPUTER);
            if k + 1 < n && found.is_none() {
                found = self.attempt(i, last, col.left, col.right, top, bottom, CENTRED);
                if found.is_none() {
                    found = self.attempt(i, last, col.left, col.right, top, bottom, LEFT_HEADING);
                }
            }
            if found.is_none() {
                found = self.attempt(i, last, col.left, col.right, top, bottom, TABLE);
            }
            if found.is_none() && top == bottom {
                if !self.salutation_seen {
                    // Every box before it in the column must be centred or blocked, short, and few.
                    let mut count: i32 = 0;
                    let mut c = self.bx(i).first;
                    while c >= 0 {
                        let s = self.bx(c as usize);
                        if !((s.kind == CENTRED || s.kind == BLOCK) && s.bottom - s.top < self.g[18]) {
                            break;
                        }
                        count += 1;
                        if count > self.g[17] {
                            break;
                        }
                        c = s.next;
                    }
                    if c < 0 {
                        found = self.attempt(i, last, col.left, col.right, top, bottom, SALUTATION);
                        if found.is_some() {
                            self.salutation_seen = true;
                        }
                    }
                } else {
                    // A complimentary close takes this group and the next; 1 to factor 19 groups must follow those,
                    // and every group from here on must be at most factor 20 lines.
                    let after = n as i64 - k as i64 - 2;
                    if after > 0 && after <= self.g[19] as i64 {
                        let all_short = groups[k..].iter().all(|g| g.1 - g.0 < self.g[20]);
                        if all_short {
                            let end = groups[k + 1].1;
                            found = self.attempt(i, last, col.left, col.right, top, end, CLOSE);
                            if found.is_some() {
                                self.salutation_seen = false;
                                k += 1;
                            }
                        }
                    }
                }
            }
            if found.is_none() {
                found = self.attempt(i, last, col.left, col.right, top, bottom, BLOCK);
            }
            if found.is_none() {
                if let Some(p) = self.add_box(i, last, col.left, col.right, top, bottom, PARAGRAPH) {
                    self.boxes[p].flag = 0;
                    // The first group of a column continues the last paragraph of the previous column when its first
                    // word would have fitted in the room left on that paragraph's last line (with a tolerance).
                    if k == 0 {
                        let w = self.first_word(p, top);
                        if w > 0 {
                            let tolerance = if self.prev_sentence == 0 { self.g[8] } else { self.g[7] };
                            if self.prev_room <= w.wrapping_add(tolerance) {
                                self.boxes[p].flag = 1;
                            }
                        }
                    }
                    found = Some(p);
                }
            }
            if let Some(f) = found {
                last = f as i32;
            }
            k += 1;
        }
        if last < 0 || self.bx(last as usize).kind != PARAGRAPH {
            self.prev_room = i32::MAX;
            self.prev_sentence = 1;
        } else {
            let b = self.bx(last as usize);
            self.prev_room = self.end_room(last as usize, b.bottom);
            self.prev_sentence = self.ends_sentence(last as usize, b.bottom) as i32;
        }
        true
    }

    /// Computer text (only when input factor 48 is 0): enough characters of the "computer" classes.
    fn is_computer_text(&mut self, i: usize) -> bool {
        if self.g[48] != 0 {
            return false;
        }
        let b = self.bx(i);
        let (mut total, mut hits) = (0i32, 0i32);
        for r in b.top..=b.bottom {
            let mut after_blank = true;
            for c in b.left..=b.right {
                let v = self.ch(r, c);
                if v == 0x20 || self.attr(r, c) & 0x1c != 0 {
                    after_blank = true;
                    continue;
                }
                total += 1;
                let k = self.class_full(r, c);
                let hit = k & 0x80 != 0
                    || (k & 0x200 != 0 && after_blank)
                    || (k & 0x100 != 0 && c < b.right && self.class_full(r, c + 1) & 2 != 0);
                if hit {
                    hits += 1;
                    after_blank = true;
                } else {
                    after_blank = false;
                }
            }
        }
        let pct = if total == 0 { 0 } else { hits * 10000 / total };
        if self.g[27] < total {
            self.g[29] <= pct
        } else {
            self.g[28] <= pct
        }
    }

    /// One part; margins at least factors 0 (left) and 1 (right) that differ by at most factor 2 (factor 6 when at
    /// least factor 5 of the letters are capitals).
    fn is_centred(&mut self, i: usize) -> bool {
        let caps = self.caps(i);
        let tolerance = if self.g[5] <= caps { self.g[6] } else { self.g[2] };
        let segs = self.segments(i);
        if segs.len() != 1 {
            return false;
        }
        let b = self.bx(i);
        let left = segs[0].0 - b.left;
        if left < self.g[0] {
            return false;
        }
        let right = self.right_limit(i) - segs[0].1;
        if self.g[1] <= right && left <= right.wrapping_add(tolerance) && right.wrapping_sub(tolerance) <= left {
            self.trim_left(i);
            self.trim_right(i);
            return true;
        }
        false
    }

    /// One part; left margin at most factor 3, right margin at least factor 4, a capital first letter and at least
    /// factor 9 capitals.
    fn is_left_heading(&mut self, i: usize) -> bool {
        let segs = self.segments(i);
        if segs.len() != 1 {
            return false;
        }
        let b = self.bx(i);
        if segs[0].0 - b.left <= self.g[3]
            && self.g[4] <= self.right_limit(i) - segs[0].1
            && self.box_starts_upper(i)
            && self.g[9] <= self.caps(i)
        {
            self.trim_left(i);
            self.trim_right(i);
            return true;
        }
        false
    }

    /// A complimentary close: one part with a margin of at least factor 23, a first line ending in a comma, every
    /// later line blank or starting with a capital.
    fn is_close(&mut self, i: usize) -> bool {
        let segs = self.segments(i);
        if segs.len() != 1 {
            return false;
        }
        let b = self.bx(i);
        let left = segs[0].0 - b.left;
        let right = self.right_limit(i) - segs[0].1;
        if self.g[23] > right.max(left) || self.last_ink(i, b.top) != b',' {
            return false;
        }
        let mut r = b.top;
        loop {
            r += 1;
            if r > b.bottom {
                self.boxes[i].flag = 0;
                self.trim_left(i);
                self.trim_right(i);
                return true;
            }
            if !(self.last_ink(i, r) == 0 || self.row_starts_upper(i, r)) {
                return false;
            }
        }
    }

    /// A salutation: one part, right margin at least factor 22, left margin at most factor 21, ending in a comma or
    /// colon, with a capital first letter.
    fn is_salutation(&mut self, i: usize) -> bool {
        let segs = self.segments(i);
        if segs.len() != 1 {
            return false;
        }
        let b = self.bx(i);
        let left = segs[0].0 - b.left;
        let right = self.right_limit(i) - segs[0].1;
        if self.g[22] <= right && left <= self.g[21] {
            let x = self.last_ink(i, b.top);
            if (x == b',' || x == b':') && self.box_starts_upper(i) {
                self.boxes[i].flag = 0;
                self.trim_left(i);
                self.trim_right(i);
                return true;
            }
        }
        false
    }

    /// Blocked text: a margin of at least factor 14, or an outline (flag 1): lines indented by different amounts,
    /// or in short blocks any indented line after the first, or in long blocks enough indented lines. A one-line
    /// group with a small left margin that ends a sentence is never blocked.
    fn is_block(&mut self, i: usize) -> bool {
        let segs = self.segments(i);
        let (Some(first), Some(lastseg)) = (segs.first().copied(), segs.last().copied()) else { return false };
        let b = self.bx(i);
        let height = b.bottom - b.top + 1;
        let left = first.0 - b.left;
        let right = self.right_limit(i) - lastseg.1;
        let widest = right.max(left);
        if height == 1 && left <= self.g[15] && self.ends_sentence(i, b.bottom) {
            return false;
        }
        self.boxes[i].flag = 0;
        let mut result = self.g[14] <= widest;
        let mut count = 0i32;
        let mut prev = 0;
        let mut later = false;
        for r in b.top..=b.bottom {
            let indent = self.first_ink(i, r) - left;
            if indent > 0 {
                if r != b.top {
                    later = true;
                }
                if prev > 0 && indent != prev {
                    self.boxes[i].flag = 1;
                    result = true;
                    break;
                }
                count += 1;
                prev = indent;
            }
        }
        if height < self.g[41] {
            if later {
                self.boxes[i].flag = 1;
                result = true;
            }
        } else if self.g[39] <= count {
            let pct = count * 10000 / height;
            if self.g[40] <= pct {
                self.boxes[i].flag = 1;
                result = true;
            }
        }
        if result {
            self.trim_left(i);
            self.trim_right(i);
        }
        result
    }

    /// A table: at least factor 26 lines in more than factor 31 + 1 parts, the first part filled on at least factor
    /// 30 of the lines; with two parts the first must not be all enumerators (that is a list). Each part becomes a
    /// table column.
    fn is_table(&mut self, i: usize) -> bool {
        self.trim_top(i);
        self.trim_bottom(i);
        self.trim_left(i);
        self.trim_right(i);
        let b = self.bx(i);
        let height = b.bottom - b.top + 1;
        if height < self.g[26] {
            return false;
        }
        let segs = self.segments(i);
        let n = segs.len() as i64;
        if n <= self.g[31] as i64 + 1 {
            return false;
        }
        let Some(&(from, to)) = segs.first() else { return false };
        let (mut filled, mut plain) = (0i32, 0i32);
        for r in b.top..=b.bottom {
            if !self.blank_span(r, from, to) {
                filled += 1;
                if n < 3 && !self.enumerator(r, from, to, false) {
                    plain += 1;
                }
            }
        }
        let pct = if height == 0 { 0 } else { filled * 10000 / height };
        if pct < self.g[30] {
            return false;
        }
        if n < 3 {
            let share = if filled == 0 { 0 } else { plain * 10000 / filled };
            if share <= self.g[35] {
                return false;
            }
        }
        let mut prev: i32 = -1;
        for s in &segs {
            if let Some(c) = self.add_box(i, prev, s.0, s.1, b.top, b.bottom, TABLE_COLUMN) {
                prev = c as i32;
            }
        }
        true
    }

    /// Counts, over the leaf boxes of the page, the lines whose last visible character is a hyphen; the page is
    /// hyphenated when they are at least factor 10 of the lines.
    fn hyphenation(&mut self, i: usize) {
        let b = self.bx(i);
        if b.kind == PAGE {
            self.hyphen_ends = 0;
            self.hyphen_lines = 0;
        }
        if b.first < 0 {
            for r in b.top..=b.bottom {
                let mut c = b.right;
                while c >= b.left {
                    let x = self.lo(r, c);
                    if x > 0x20 {
                        self.hyphen_lines += 1;
                        if x == b'-' {
                            self.hyphen_ends += 1;
                        }
                        break;
                    }
                    c -= 1;
                }
            }
        } else {
            let mut c = b.first;
            while c >= 0 {
                self.hyphenation(c as usize);
                c = self.bx(c as usize).next;
            }
        }
        if b.kind == PAGE {
            let pct = if self.hyphen_lines == 0 { 0 } else { self.hyphen_ends * 10000 / self.hyphen_lines };
            self.hyphenated = self.g[10] <= pct;
        }
    }

    // ----- Output marks -----

    /// Underline marks: "_" before the first underlined letter of each word, "&@" before a letter that is not
    /// underlined right after one that is; with four or more underlined words in a row "__" before the first, "_"
    /// before the last and nothing between.
    fn underline_marks(&mut self, i: usize) {
        let b = self.bx(i);
        let mut run = 0usize;
        let mut slots = [(0i32, 0i32); 3];
        for r in b.top..=b.bottom {
            let mut inside = false;
            let mut prev_letter = false;
            for c in b.left..=b.right {
                let letter = self.class_full(r, c) & 2 != 0;
                if letter {
                    if self.attr(r, c) & 1 == 0 {
                        if inside {
                            self.add_mark(r, c, 4);
                            inside = false;
                        }
                        run = 0;
                    } else if !inside {
                        self.add_mark(r, c, 1);
                        inside = true;
                        if !prev_letter {
                            if run < 3 {
                                slots[run] = (r, c);
                                run += 1;
                            } else if run == 3 {
                                self.add_mark(slots[0].0, slots[0].1, 2);
                                self.set_mark(slots[1].0, slots[1].1, 0);
                                self.set_mark(slots[2].0, slots[2].1, 0);
                                slots[2] = (r, c);
                                run = 4;
                            } else {
                                self.set_mark(slots[2].0, slots[2].1, 0);
                                slots[2] = (r, c);
                            }
                        }
                    }
                } else {
                    inside = false;
                }
                prev_letter = letter;
            }
        }
    }

    /// Rule cells at the edges of each line are left out; accented letters get "%" when output factor 11 is 0; a
    /// single quote at a word start paired with one at a word end are both written as backquotes.
    fn other_marks(&mut self, i: usize) {
        let b = self.bx(i);
        for r in b.top..=b.bottom {
            let mut c = b.left;
            while c <= b.right && self.attr(r, c) & 4 != 0 {
                self.add_mark(r, c, 8);
                c += 1;
            }
            if c < b.right {
                c = b.right;
                while c >= b.left && self.attr(r, c) & 4 != 0 {
                    self.add_mark(r, c, 8);
                    c -= 1;
                }
            }
            if self.b[11] == 0 {
                for c in b.left..=b.right {
                    if self.attr(r, c) & 0x20 != 0 {
                        self.add_mark(r, c, 0x10);
                    }
                }
            }
        }
        let mut open: Option<(i32, i32)> = None;
        for r in b.top..=b.bottom {
            let mut state = 1;
            for c in b.left..=b.right {
                let v = self.ch(r, c);
                if v == 0x20 {
                    state = 1;
                } else if v == 0x27 {
                    if state == 1 {
                        open = Some((r, c));
                        state = 2;
                    } else if state == 2 {
                        let word_end = c == b.right || self.class_full(r, c + 1) & 3 == 0;
                        if let (true, Some((orow, ocol))) = (word_end, open) {
                            self.add_mark(orow, ocol, 0x20);
                            self.add_mark(r, c, 0x20);
                            open = None;
                        }
                        state = 0;
                    }
                } else if self.class_full(r, c) & 3 != 0 {
                    state = 2;
                } else if state == 1 {
                    if self.class_full(r, c) & 0x800 == 0 {
                        state = 0;
                    }
                } else if state == 2 && self.class_full(r, c) & 0x400 == 0 {
                    state = 0;
                }
            }
        }
    }

    // ----- Output -----

    fn put_char(&mut self, v: u16) {
        // UTF-8 of the 16-bit value; surrogate values are written as three bytes like any other.
        let v = v as u32;
        if v < 0x80 {
            self.out.push(v as u8);
        } else if v < 0x800 {
            self.out.push(0xc0 | (v >> 6) as u8);
            self.out.push(0x80 | (v & 0x3f) as u8);
        } else {
            self.out.push(0xe0 | (v >> 12) as u8);
            self.out.push(0x80 | ((v >> 6) & 0x3f) as u8);
            self.out.push(0x80 | (v & 0x3f) as u8);
        }
    }

    /// One output line: the prefix, the text of a row from `left` to `right` (none when `row` is below 0), the
    /// suffix and a line feed. Blank and rule-edge cells are cut at the right, and at the left when `trim` is set.
    /// A last character "-" gets a second one when the suffix is empty and the hyphen is a real one: `dash` is 1,
    /// or the page is not hyphenated, or the line has one character, or the character before is a hyphen or not a
    /// letter.
    #[allow(clippy::too_many_arguments)]
    fn emit(&mut self, prefix: &str, row: i32, left: i32, right: i32, trim: bool, dash: i32, suffix: &str) {
        self.out.extend_from_slice(prefix.as_bytes());
        if row >= 0 {
            let r = row;
            let cut = |e: &Engine, c: i32| e.lo(r, c) < 0x21 || e.mark(r, c) & 8 != 0;
            let mut end = right;
            while end >= left && cut(self, end) {
                end -= 1;
            }
            let mut start = left;
            if trim {
                while start <= end && cut(self, start) {
                    start += 1;
                }
            }
            if start <= end {
                for c in start..=end {
                    let mut v = self.ch(r, c);
                    let m = self.mark(r, c);
                    if m != 0 {
                        if m & 2 != 0 {
                            self.out.extend_from_slice(b"__");
                        } else if m & 1 != 0 {
                            self.out.push(b'_');
                        }
                        if m & 4 != 0 {
                            self.out.extend_from_slice(b"&@");
                        }
                        if m & 0x10 != 0 {
                            self.out.push(b'%');
                        }
                        if m & 8 != 0 {
                            v = 0x20;
                        } else if m & 0x20 != 0 {
                            v = 0x60;
                        }
                    }
                    if v == 0x24 {
                        let n = self.ch(r, c + 1);
                        if c >= end || (!(0x30..=0x39).contains(&n) && n != 0x2e && n != 0x2c) {
                            self.out.push(b'$');
                        }
                    }
                    self.put_char(v);
                }
                if self.ch(r, end) == 0x2d && suffix.is_empty() {
                    let real = dash == 1 || !self.hyphenated || end <= start || {
                        let p = self.lo(r, end - 1);
                        p == b'-' || Self::class_low(p) & 2 == 0
                    };
                    if real {
                        self.out.push(b'-');
                    }
                }
            }
        }
        self.out.extend_from_slice(suffix.as_bytes());
        self.out.push(b'\n');
    }

    /// A line holding only a code.
    fn code(&mut self, s: &str) {
        self.emit(s, -1, 0, 0, false, 1, "");
    }

    fn rows(&mut self, i: usize, dash: i32) {
        let b = self.bx(i);
        for r in b.top..=b.bottom {
            self.emit("", r, b.left, b.right, true, dash, "");
        }
    }

    fn output(&mut self, i: usize) {
        match self.bx(i).kind {
            PARAGRAPH => self.write_paragraph(i),
            BLOCK => self.write_block(i),
            COMPUTER => self.write_computer_text(i),
            CENTRED => self.write_centred(i),
            LEFT_HEADING => self.write_left_heading(i),
            CLOSE => self.write_close(i),
            SALUTATION => self.write_salutation(i),
            COLUMN => self.write_column(i),
            PAGE_NUMBER => self.write_page_number(i),
            RUNNING_FOOT | RUNNING_HEAD => self.write_running(i),
            TABLE => self.write_table(i),
            _ => self.write_other(i),
        }
    }

    /// Boxes with no output of their own write their children; a leaf writes its lines plainly.
    fn write_other(&mut self, i: usize) {
        let first = self.bx(i).first;
        if first < 0 {
            self.underline_marks(i);
            self.other_marks(i);
            self.rows(i, 0);
            self.last_kind = 0;
        } else {
            let mut c = first;
            while c >= 0 {
                self.output(c as usize);
                c = self.bx(c as usize).next;
            }
        }
    }

    /// A column: its boxes in order, with "$kps" before the box that comes before a complimentary close and "$kpe"
    /// at the end.
    fn write_column(&mut self, i: usize) {
        let first = self.bx(i).first;
        let mut kept = false;
        if first < 0 {
            self.underline_marks(i);
            self.other_marks(i);
            self.rows(i, 0);
            self.last_kind = 0;
        } else {
            let mut c = first;
            while c >= 0 {
                let next = self.bx(c as usize).next;
                if !kept && next >= 0 && self.bx(next as usize).kind == CLOSE {
                    self.code(CODE_KEEP_START[self.lang]);
                    kept = true;
                }
                self.output(c as usize);
                c = self.bx(c as usize).next;
            }
        }
        if kept {
            self.code(CODE_KEEP_END[self.lang]);
        }
    }

    /// "$es~para." (not when continued), the lines, "$ee~para."; a line indented two or more past the least indented
    /// line starts a new paragraph.
    fn write_paragraph(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        let b = self.bx(i);
        let least = (b.top..=b.bottom).map(|r| self.first_ink(i, r)).min().unwrap_or(0);
        if b.flag == 0 {
            self.code("$es~para.");
        }
        for r in b.top..=b.bottom {
            if self.first_ink(i, r) >= least.wrapping_add(2) {
                self.code("$ee~para.");
                self.code("$es~para.");
            }
            self.emit("", r, b.left, b.right, true, 0, "");
        }
        self.code("$ee~para.");
        self.last_kind = 0;
    }

    /// Blocked text as a poem ("$es~poem.", each line followed by " $l", "$ee~poem."), or an outline.
    fn write_block(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        let b = self.bx(i);
        if b.flag == 1 {
            self.write_outline(i);
        } else {
            self.code("$es~poem.");
            for r in b.top..=b.bottom {
                self.emit("", r, b.left, b.right, true, 1, " $l");
            }
            self.code("$ee~poem.");
        }
        self.last_kind = 1;
    }

    /// An outline: item lines get "$hlN " by the column their text starts in. A line is an item when it starts left
    /// of the line before, starts with an enumerator, or its first word would have fitted on the line before.
    fn write_outline(&mut self, i: usize) {
        let b = self.bx(i);
        let parent = if b.parent < 0 { i } else { b.parent as usize };
        for c in b.left..=b.right {
            if let Some(x) = self.col_level.get_mut(c as usize) {
                *x = -1;
            }
        }
        let mut prev = COLS as i32 + 1;
        for r in b.top..=b.bottom {
            let start = self.first_ink(i, r) + b.left;
            let item = if start < prev || self.enumerator(r, start, b.right, true) {
                true
            } else {
                let room = self.end_room(parent, r - 1);
                let mut c = start;
                while c <= b.right && self.lo(r, c) > 0x20 {
                    c += 1;
                }
                c - start < room
            };
            let slot = r as usize;
            if item {
                if let Some(x) = self.row_item.get_mut(slot) {
                    *x = start;
                }
                if let Some(x) = self.col_level.get_mut(start as usize) {
                    *x = 1;
                }
            } else if let Some(x) = self.row_item.get_mut(slot) {
                *x = -1;
            }
            prev = start;
        }
        // Item columns from left to right get levels 1, 3, 5... capped at the line width less 4 (with width 0 the
        // second and later all get -4).
        let mut level = 1i32;
        for c in b.left..=b.right {
            if let Some(x) = self.col_level.get_mut(c as usize) {
                if *x != -1 {
                    *x = level;
                    level += 2;
                    if self.out_width - 4 < level {
                        level = self.out_width - 4;
                    }
                }
            }
        }
        self.code("$es~outline.");
        for r in b.top..=b.bottom {
            let next_item = self.row_item.get((r + 1) as usize).copied().unwrap_or(-1);
            let dash = if r < b.bottom && next_item == -1 { 0 } else { 1 };
            let item = self.row_item.get(r as usize).copied().unwrap_or(-1);
            if item == -1 {
                self.emit("", r, b.left, b.right, true, dash, "");
            } else {
                let level = self.col_level.get(item as usize).copied().unwrap_or(-1);
                let prefix = format!("$hl{} ", (level - 1) / 2);
                self.emit(&prefix, r, b.left, b.right, true, dash, "");
            }
        }
        self.code("$ee~outline.");
    }

    fn write_computer_text(&mut self, i: usize) {
        let b = self.bx(i);
        self.code("$es~compdisplay.");
        for r in b.top..=b.bottom {
            self.emit("", r, b.left, b.right, false, 1, "");
        }
        self.code("$ee~compdisplay.");
        self.last_kind = 1;
    }

    /// A centred heading: "$es~h1. " before the first line, " $l" after each line but the last, " $ee~h1." after the
    /// last, then a skipped line. A skipped line also goes before it when a running head has been written and the
    /// last box did not end with one.
    fn write_centred(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        if self.b[7] == 1 && self.last_kind != 1 && self.last_kind != 2 && self.running_head_seen {
            self.code(CODE_SKIP[self.lang]);
        }
        let b = self.bx(i);
        for r in b.top..=b.bottom {
            let prefix = if r == b.top { "$es~h1. " } else { "" };
            let suffix = if r == b.bottom { " $ee~h1." } else { " $l" };
            self.emit(prefix, r, b.left, b.right, true, 1, suffix);
        }
        if self.b[8] == 1 {
            self.code(CODE_SKIP[self.lang]);
            self.last_kind = 1;
        } else {
            self.last_kind = 0;
        }
    }

    /// A left heading: "$l $cpN" (keep N lines together), a skipped line, "$ind5", the lines, "$ind1".
    fn write_left_heading(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        let b = self.bx(i);
        let mut d = self.out_width.wrapping_sub(self.b[5]).wrapping_add(1);
        if d < 2 {
            d = 2;
        }
        let n = ((b.bottom - b.top + 1) * (b.right - b.left + 1)) / d;
        if self.b[4] == 1 && self.last_kind != 1 {
            self.code(&format!("$l $cp{}", n.wrapping_add(2)));
            self.code(CODE_SKIP[self.lang]);
        } else {
            self.code(&format!("$l $cp{}", n.wrapping_add(1)));
        }
        self.code(&format!("{}{}", CODE_INDENT[self.lang], self.b[5]));
        self.rows(i, 0);
        self.code(CODE_INDENT_1[self.lang]);
        self.last_kind = 0;
    }

    /// A complimentary close as a poem block (blank lines left out).
    fn write_close(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        let b = self.bx(i);
        self.code("$es~poem.");
        for r in b.top..=b.bottom {
            if self.last_ink(i, r) != 0 {
                self.emit("", r, b.left, b.right, true, 1, " $l");
            }
        }
        self.code("$ee~poem.");
        self.last_kind = 1;
    }

    fn write_salutation(&mut self, i: usize) {
        self.underline_marks(i);
        self.other_marks(i);
        let b = self.bx(i);
        self.code("$es~poem.");
        for r in b.top..=b.bottom {
            self.emit("", r, b.left, b.right, true, 1, " $l");
        }
        self.code("$ee~poem.");
        self.last_kind = 0;
    }

    fn write_page_number(&mut self, i: usize) {
        self.other_marks(i);
        if self.b[6] == 1 {
            let b = self.bx(i);
            self.emit(CODE_PAGE[self.lang], b.top, b.left, b.right, true, 1, "");
            self.last_kind = 2;
        }
    }

    fn write_running(&mut self, i: usize) {
        if self.b[9] == 1 {
            self.other_marks(i);
            let b = self.bx(i);
            self.emit(CODE_RUN_START[self.lang], b.top, b.left, b.right, true, 1, CODE_RUN_END[self.lang]);
            self.running_head_seen = true;
        }
    }

    /// A table in the stacked form: per row, each cell on its own line followed by ":" (first column), ";" (middle)
    /// or "." (last), an empty cell as "$q-- $:", "$q-- ;" or "$q-- .", and "$l" after the row.
    fn write_table(&mut self, i: usize) {
        self.other_marks(i);
        let b = self.bx(i);
        let mut cols: Vec<(i32, i32)> = Vec::new();
        let mut c = b.first;
        while c >= 0 {
            let x = self.bx(c as usize);
            cols.push((x.left, x.right));
            c = x.next;
        }
        let n = cols.len();
        self.code("$es~poem.");
        for r in b.top..=b.bottom {
            for (k, &(l, rt)) in cols.iter().enumerate() {
                let k = k + 1;
                if !self.blank_span(r, l, rt) {
                    let suffix = if k == 1 {
                        ":"
                    } else if k == n {
                        "."
                    } else {
                        ";"
                    };
                    self.emit("", r, l, rt, true, 1, suffix);
                } else {
                    let suffix = if k == 1 {
                        "$q-- $:"
                    } else if k == n {
                        "$q-- ."
                    } else {
                        "$q-- ;"
                    };
                    self.emit("", -1, 0, 0, false, 1, suffix);
                }
            }
            self.code("$l");
        }
        self.code("$ee~poem.");
        if b.bottom - b.top < 2 {
            self.last_kind = 0;
        } else {
            self.last_kind = 1;
            self.code(CODE_SKIP[self.lang]);
        }
    }

    /// One page: read, find rules, analyse, measure hyphenation, write.
    fn page(&mut self) {
        self.ncols = 0;
        self.read_page();
        self.find_rules();
        if self.width <= 0 || self.depth <= 0 {
            return;
        }
        // A page whose cells are all blank by their low byte (for instance only U+2014) sends the original past
        // the end of its page; the port writes nothing for it.
        let any = (0..self.depth).any(|r| (0..self.width).any(|c| self.lo(r, c) > 0x20));
        if !any {
            return;
        }
        self.boxes[0] = Bx {
            left: 0,
            right: self.width - 1,
            top: 0,
            bottom: self.depth - 1,
            kind: PAGE,
            flag: 0,
            parent: -1,
            first: -1,
            prev: -1,
            next: -1,
        };
        for k in 1..BOXES {
            self.boxes[k].next = if k + 1 < BOXES { k as i32 + 1 } else { -1 };
        }
        self.free = 1;
        self.classify(0);
        if self.bx(0).first < 0 {
            return;
        }
        self.hyphenation(0);
        self.output(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factors() -> Factors {
        Factors::shipped().expect("the shipped factor files parse")
    }

    /// Runs the importer on lines joined with CR LF (and a final CR LF), as the reference would get them from a
    /// Windows text file, with the substitution set the input factors give.
    fn run(lines: &[&str]) -> String {
        let mut text = String::new();
        for l in lines {
            text.push_str(l);
            text.push_str("\r\n");
        }
        String::from_utf8_lossy(&pimbri(text.as_bytes(), -1, &factors())).into_owned()
    }

    /// The expected output: each line followed by a line feed.
    fn lines(v: &[&str]) -> String {
        v.iter().map(|l| format!("{l}\n")).collect()
    }

    const LONG: &str = "The rain had stopped by the time we reached the old stone bridge at last.";

    #[test]
    fn shipped_factor_files_parse() {
        // Values from the factor tables: centred heading margins, caps share, hyphenation share, substitution set,
        // assumed page width, computer text off; output gaps, page numbers, language, outline runover, tables.
        let f = factors();
        assert_eq!(f.input(0), 6);
        assert_eq!(f.input(1), 4);
        assert_eq!(f.input(5), 8500);
        assert_eq!(f.input(10), 1000);
        assert_eq!(f.input(12), 0);
        assert_eq!(f.input(36), 1);
        assert_eq!(f.input(42), 65);
        assert_eq!(f.input(48), 1);
        assert_eq!(f.output(1), 2);
        assert_eq!(f.output(3), 0);
        assert_eq!(f.output(6), 1);
        assert_eq!(f.output(10), 0);
        assert_eq!(f.output(12), 4);
        assert_eq!(f.output(13), 1);
    }

    #[test]
    fn factor_file_errors() {
        let book: &[u8] = include_bytes!("../../../tables/import/text-output-factors.txt");
        // The headers are checked: the output file is not an input file.
        assert!(Factors::parse(book, book).is_err());
        // "-1" must be followed by "***"; the end of the file before "-1" is an error (the original loops).
        assert!(Factors::parse(b"-pimbri i 1\r\n0 6\r\n-1 0\r\n", book).is_err());
        assert!(Factors::parse(b"-pimbri i 1\r\n0 6\r\n", book).is_err());
        // Comments, a value after non-digits (the minus is lost), an index of 100 or more ignored, "|065" is "A".
        let f = Factors::parse(b"| c\r\n-pimbri i 1\r\n7 -3 | note\r\n150 9\r\n|0651 2\r\n-1\r\n*** |\r\n", book)
            .expect("parses");
        assert_eq!(f.input(7), 3);
        // "A1 2": atoi of "A1 2" is 0, so factor 0 gets the number after the first non-digits: 1.
        assert_eq!(f.input(0), 1);
    }

    #[test]
    fn worked_example_blocked_heading_and_indented_paragraph() {
        // "Chapter One" is one line, not last: left margin 0 rules out centring, 2 capitals of 10 letters a left
        // heading, one line a table; it does not end in "," or ":" (no salutation); its right margin (76 - 10) is
        // at least 10, so it is blocked text (a poem block). The paragraph's margins are 0 and only its first line
        // is indented (not an outline): a paragraph, with the first line indented 2 or more past the least indented
        // line, so an empty paragraph comes first.
        let out = run(&[
            "Chapter One",
            "",
            "    It was a bright cold day in April, and the clocks were striking thirteen.",
            "Winston Smith, his chin nuzzled into his breast in an effort to escape the",
            "vile wind, slipped quickly through the glass doors of Victory Mansions.",
        ]);
        assert_eq!(
            out,
            lines(&[
                "$es~poem.",
                "Chapter One $l",
                "$ee~poem.",
                "$es~para.",
                "$ee~para.",
                "$es~para.",
                "It was a bright cold day in April, and the clocks were striking thirteen.",
                "Winston Smith, his chin nuzzled into his breast in an effort to escape the",
                "vile wind, slipped quickly through the glass doors of Victory Mansions.",
                "$ee~para.",
            ])
        );
    }

    #[test]
    fn centred_heading() {
        // All capitals: centring tolerance 24; left margin 29 (at least 6), right margin 72 - 39 = 33 (at least 4).
        // No running head was written, so no skipped line before; one after (output factor 8). The last line is a
        // one-line group ending a sentence at the left edge: a paragraph.
        let centred = format!("{}CHAPTER ONE", " ".repeat(29));
        let out = run(&[centred.as_str(), "", LONG]);
        assert_eq!(out, lines(&["$es~h1. CHAPTER ONE $ee~h1.", "$l $sl1", "$es~para.", LONG, "$ee~para."]));
    }

    #[test]
    fn left_heading() {
        // Left margin 0, right margin 72 - 11 = 61 (at least 15), capitals: a left heading. Keep-together count:
        // 1 line x 12 cells / 2 (width 0 makes the divisor 2) + 2, then a skipped line (output factor 4), "$ind5",
        // the line, "$ind1".
        let out = run(&["INTRODUCTION", "", LONG]);
        assert_eq!(
            out,
            lines(&["$l $cp8", "$l $sl1", "$ind5", "INTRODUCTION", "$ind1", "$es~para.", LONG, "$ee~para."])
        );
    }

    #[test]
    fn two_column_table_stacked() {
        // Four lines in two parts (2 blank columns split a box of 4 lines); the first part is filled on every line
        // and is not a list of enumerators: a table. With width 0 it is written stacked: each cell with ":" (first
        // column) or "." (last), an empty last cell as "$q-- .", "$l" after each row, and a skipped line after a
        // table of more than 2 rows. The parts differ in width by more than 2, so they are not text columns.
        let out = run(&[
            "Apples          Red and round",
            "Bananas         Yellow and long",
            "Cherries",
            "Grapes          Green or purple",
        ]);
        assert_eq!(
            out,
            lines(&[
                "$es~poem.",
                "Apples:",
                "Red and round.",
                "$l",
                "Bananas:",
                "Yellow and long.",
                "$l",
                "Cherries:",
                "$q-- .",
                "$l",
                "Grapes:",
                "Green or purple.",
                "$l",
                "$ee~poem.",
                "$l $sl1",
            ])
        );
    }

    #[test]
    fn running_head_and_page_number_on_a_header_line() {
        // The first group is one line in two parts; "CHAPTER ONE" is not a page number (its space is past column
        // 4), "12" is (it holds a digit): the line is a header, the other part its running head, which comes first.
        let head = format!("CHAPTER ONE{}12", " ".repeat(34));
        let out = run(&[
            head.as_str(),
            "",
            "The rain had stopped by the time we reached the old stone bridge at last, and",
            "we sat down on the wet grass to wait for the others to catch up with us.",
        ]);
        assert_eq!(
            out,
            lines(&[
                "$tls CHAPTER ONE $tle",
                "$leaf- 12",
                "$es~para.",
                "The rain had stopped by the time we reached the old stone bridge at last, and",
                "we sat down on the wet grass to wait for the others to catch up with us.",
                "$ee~para.",
            ])
        );
    }

    #[test]
    fn underline_marks() {
        // Underscores overprinted after a carriage return underline the letters above them: "_" before the first
        // underlined letter of a word, "&@" before a letter that is not underlined right after one that is.
        let text = format!("This is important and unhappy news.\r{}{}{}__\r\n", " ".repeat(8), "_".repeat(9), " ".repeat(5));
        let out = String::from_utf8_lossy(&pimbri(text.as_bytes(), -1, &factors())).into_owned();
        assert_eq!(out, lines(&["$es~para.", "This is _important and _un&@happy news.", "$ee~para."]));
        // Four underlined words in a row: "__" before the first, "_" before the last, nothing between.
        let text = format!("One two three four five six.\r{}{}\r\n", " ".repeat(4), "_".repeat(19));
        let out = String::from_utf8_lossy(&pimbri(text.as_bytes(), -1, &factors())).into_owned();
        assert_eq!(out, lines(&["$es~para.", "One __two three four _five six.", "$ee~para."]));
    }

    #[test]
    fn outline_levels_with_width_zero() {
        // Lines indented by different amounts make an outline. An item starts left of the line before or with an
        // enumerator; item columns get levels 1, 3, ... capped at width - 4, so with width 0 the first column is
        // level 1 ("$hl0 ") and every other -4 ("$hl-2 ", (-4 - 1) / 2 rounded towards 0).
        let out = run(&[
            "I. Introduction to the subject of the report and its background",
            "   A. History of the problem as it was first described",
            "   B. Earlier attempts to solve it",
            "      1. The first attempt",
            "II. Method",
        ]);
        assert_eq!(
            out,
            lines(&[
                "$es~outline.",
                "$hl0 I. Introduction to the subject of the report and its background",
                "$hl-2 A. History of the problem as it was first described",
                "$hl-2 B. Earlier attempts to solve it",
                "$hl-2 1. The first attempt",
                "$hl0 II. Method",
                "$ee~outline.",
            ])
        );
    }

    #[test]
    fn dollar_doubling() {
        // "$" is doubled unless a digit, "." or "," follows.
        let out = run(&["It costs $5 or $ more and $$."]);
        assert_eq!(out, lines(&["$es~para.", "It costs $5 or $$ more and $$$.", "$ee~para."]));
    }

    #[test]
    fn line_final_hyphens() {
        // Two of three lines end in "-": the page is hyphenated. Paragraph lines pass 0, so "exam-" (a letter
        // before) stays a broken word, while "--" (a hyphen before) gets a third hyphen.
        let out = run(&[
            "The candidates were told that the written part of the final exam-",
            "ination would be long and difficult, and some of them laughed --",
            "but most of them did not think it was funny at all, not even a bit.",
        ]);
        assert_eq!(
            out,
            lines(&[
                "$es~para.",
                "The candidates were told that the written part of the final exam-",
                "ination would be long and difficult, and some of them laughed ---",
                "but most of them did not think it was funny at all, not even a bit.",
                "$ee~para.",
            ])
        );
        // One line of eleven ending in "-" (909 of 10000, below input factor 10): the page is not hyphenated and
        // the hyphen is a real one, written "--".
        let full = vec!["word"; 14].join(" ");
        let first = format!("{} co-", vec!["word"; 13].join(" "));
        let mut input: Vec<&str> = vec![first.as_str()];
        for _ in 0..10 {
            input.push(&full);
        }
        let out = run(&input);
        let mut expected = vec!["$es~para.".to_string(), format!("{} co--", vec!["word"; 13].join(" "))];
        for _ in 0..10 {
            expected.push(full.clone());
        }
        expected.push("$ee~para.".to_string());
        let expected: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();
        assert_eq!(out, lines(&expected));
    }

    #[test]
    fn double_spacing_is_removed() {
        // Nine lines with one blank line after each: depth 17 (at least input factor 38), the shortest blank run is
        // 1, so one blank row is taken out after each line and the nine lines make one paragraph.
        let full = vec!["word"; 14].join(" ");
        let mut input: Vec<&str> = Vec::new();
        for k in 0..9 {
            if k > 0 {
                input.push("");
            }
            input.push(&full);
        }
        let out = run(&input);
        let mut expected = vec!["$es~para."];
        for _ in 0..9 {
            expected.push(&full);
        }
        expected.push("$ee~para.");
        assert_eq!(out, lines(&expected));
    }

    #[test]
    fn a_last_page_of_one_word_is_lost() {
        // The word reader reads one word ahead; when the input ends inside that read-ahead, the page loop stops
        // and the word is never written. With a second word the last page is read.
        let f = factors();
        let out = String::from_utf8_lossy(&pimbri(b"first page\x0csecond", -1, &f)).into_owned();
        assert_eq!(out, lines(&["$es~poem.", "first page $l", "$ee~poem."]));
        let out = String::from_utf8_lossy(&pimbri(b"first page\x0csecond page", -1, &f)).into_owned();
        assert_eq!(out, lines(&["$es~poem.", "first page $l", "$ee~poem.", "$es~poem.", "second page $l", "$ee~poem."]));
    }

    #[test]
    fn arbitrary_bytes_do_not_panic() {
        // A simple generator: bytes weighted towards line ends, form feeds, backspaces, underscores, hyphens,
        // rule characters, digits and high bytes; also random factor values.
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let shipped = factors();
        let pool: &[u8] = b"\r\n\x0c\x08_-+.|=$', :()[]P12aAbB      \t\xc3\xa9\xe2\x80\x94\xff\x80";
        for round in 0..400i32 {
            let len = (next() % 4000) as usize;
            let mut input = Vec::with_capacity(len);
            for _ in 0..len {
                let r = next();
                input.push(if r % 3 == 0 { (r >> 8) as u8 } else { pool[(r >> 8) as usize % pool.len()] });
            }
            let mut f = shipped.clone();
            if round % 4 == 3 {
                for k in 0..100 {
                    let r = next();
                    f.genl[k] = match r % 5 {
                        0 => (r >> 8) as i32,
                        1 => -((r >> 40) as i32 % 50),
                        _ => ((r >> 8) % 40) as i32,
                    };
                    f.book[k] = ((next() >> 8) % 6) as i32 - 1;
                }
            }
            let set = round % 5 - 1;
            let _ = pimbri(&input, set, &f);
        }
    }
}
