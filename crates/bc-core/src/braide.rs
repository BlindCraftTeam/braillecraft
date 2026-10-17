//! Formatted braille back into running braille with codes, as the reference program's formatted braille importer
//! does it (spec 28 sections 2c, 2d and 13): the file is read page by page into a grid; the braille page numbers in
//! the corners become page numbering codes, the print page numbers print page codes, the first and last lines running
//! head and foot codes when the settings say so; then each line is classed by where its text starts: a blank run is a
//! skip, a line of dashes and a number a print page change, a line holding two spaces (or a space and a quoted word)
//! as-is text, and the rest continuation lines, paragraphs, indented, centred or tabbed lines.
//!
//! The output is the scanner's coded form ("|CODE" words), which the import scanner and the import rule scripts turn
//! into a document.

/// The formatted braille import settings (Global, Formatted Braille Importer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Lines per page and cells per line of the file (when formatting is not preserved).
    pub lines: usize,
    pub cells: usize,
    /// Where braille page numbers stand on odd and even pages, and print page numbers (0 none, 1 upper left, 2 upper
    /// right, 3 lower left, 4 lower right).
    pub odd_main: i32,
    pub even_main: i32,
    pub odd_reference: i32,
    pub even_reference: i32,
    /// The first line is a running head (and the last a running foot) on: 0 no pages, 1 odd, 2 even, 3 both.
    pub header: i32,
    pub footer: i32,
    /// Keep the file's lines and pages as they are.
    pub preserve: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            lines: 30,
            cells: 40,
            odd_main: 4,
            even_main: 4,
            odd_reference: 2,
            even_reference: 2,
            header: 0,
            footer: 0,
            preserve: false,
        }
    }
}

/// What the import found to warn about: a page longer than the lines per page, a line wider than the cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Warnings {
    pub long_page: bool,
    pub wide_line: bool,
}

struct Braide<'a> {
    s: Settings,
    input: &'a [u8],
    at: usize,
    out: Vec<u8>,
    rows: Vec<Vec<u8>>,
    /// Rows read on this page and the page's width (after trailing blank columns are taken off).
    depth: usize,
    width: usize,
    /// The line being classed: its first and last-plus-one text columns.
    first: i32,
    last: i32,
    /// The text cut out last.
    token: Vec<u8>,
    parity: i32,
    counter: i32,
    roman: bool,
    page_char: u8,
    /// The print page number as the next page should show it (continuation letter first), the running head and foot
    /// last written (a backquote: none yet).
    print: Vec<u8>,
    head: Vec<u8>,
    foot: Vec<u8>,
    /// Room left on the last line written, and the rooms the page numbers on the first and last row take.
    room: i32,
    room_top: i32,
    room_bottom: i32,
    as_is: bool,
    page_pending: bool,
    other_control: bool,
    warnings: Warnings,
    overflow: bool,
}

const DIGITS: &[u8; 11] = b"JABCDEFGHI#";

impl Braide<'_> {
    fn getc(&mut self) -> Option<u8> {
        let c = self.input.get(self.at).copied();
        self.at += 1;
        c
    }

    fn eof(&self) -> bool {
        self.at >= self.input.len()
    }

    fn cell(&self, row: usize, col: i32) -> u8 {
        match self.rows.get(row).and_then(|r| r.get(col as usize)) {
            Some(0) | None => b' ',
            Some(&c) => c,
        }
    }

    /// From column `from` of `row`, forward or back, the first column whose cell is (`is`) or is not `c`; -1 when
    /// there is none.
    fn scan(&self, row: i32, from: i32, forward: bool, is: bool, c: u8) -> i32 {
        if row < 0 || row as usize >= self.s.lines {
            return -1;
        }
        let mut k = from;
        while k >= 0 && (k as usize) < self.s.cells {
            if (self.cell(row as usize, k) == c) == is {
                return k;
            }
            k += if forward { 1 } else { -1 };
        }
        -1
    }

    /// Columns `from` to `to` of `row` as the token (blanked in the grid with `blank`); its length, -1 when the
    /// range is empty.
    fn cut(&mut self, row: usize, from: i32, to: i32, blank: bool) -> i32 {
        self.token.clear();
        if from < 0 || to < 0 || to < from {
            return -1;
        }
        let mut k = from;
        while (k as usize) < self.s.cells && k <= to {
            let c = self.cell(row, k);
            self.token.push(c);
            if blank && c != b' ' {
                self.rows[row][k as usize] = b' ';
            }
            k += 1;
        }
        self.token.len() as i32
    }

    /// The column of the space before (back) or after (forward) a corner token; -2 when the token does not stand
    /// alone, two spaces from other text.
    fn corner_space(&self, row: i32, col: i32, forward: bool) -> i32 {
        let k = self.scan(row, col, forward, true, b' ');
        if k < 0 {
            return -2;
        }
        if !forward {
            if k == 0 || self.cell(row as usize, k - 1) > b' ' {
                return -2;
            }
        } else if k < self.s.cells as i32 - 1 {
            if self.cell(row as usize, k + 1) > b' ' {
                return -2;
            }
        } else {
            return -2;
        }
        k
    }

    /// Reads one page into the grid; false when the file has ended with nothing more than a line.
    fn read_page(&mut self) -> bool {
        self.depth = 0;
        self.width = 0;
        for r in &mut self.rows {
            r.iter_mut().for_each(|c| *c = 0);
        }
        let (mut row, mut col) = (0usize, 0usize);
        let mut last: i32;
        loop {
            last = self.getc().map_or(-1, i32::from);
            if !(0x20..=0x7e).contains(&last) {
                match last {
                    0x0D => {}
                    0x0A => {
                        if self.overflow {
                            self.warnings.long_page = true;
                            self.overflow = false;
                        }
                        if row + 1 < self.s.lines {
                            row += 1;
                            col = 0;
                        } else {
                            self.overflow = true;
                            break;
                        }
                    }
                    0x0C => {
                        if !self.overflow {
                            break;
                        }
                        self.overflow = false;
                    }
                    -1 | 0x1A => {
                        self.overflow = false;
                        break;
                    }
                    _ => self.other_control = true,
                }
            } else {
                if self.overflow {
                    self.warnings.long_page = true;
                    self.overflow = false;
                }
                let c = if last > 0x5f { last - 0x20 } else { last } as u8;
                if col < self.s.cells {
                    self.rows[row][col] = c;
                    col += 1;
                } else {
                    self.warnings.wide_line = true;
                }
                self.width = self.width.max(col);
            }
        }
        self.depth = row + usize::from(self.rows[row][0] != 0);
        for r in self.rows.iter_mut().take(self.depth) {
            let mut k = self.width;
            while k > 0 && r[k - 1] == 0 {
                r[k - 1] = b' ';
                k -= 1;
            }
        }
        // Blank columns at the right of every row are taken off.
        while self.width > 0 && (0..self.depth).all(|r| self.rows[r][self.width - 1] == b' ') {
            self.width -= 1;
        }
        !((last == 0x1A || self.eof()) && self.depth < 2)
    }

    /// The token at a page number corner (1 upper left .. 4 upper right .. lower right), blanked out of its row; its
    /// length, or 0 or less when there is none. The right-hand corners keep its room on their row.
    fn corner_token(&mut self, corner: i32) -> i32 {
        let w = self.s.cells as i32;
        let bottom = self.depth as i32 - 1;
        match corner {
            1 | 3 => {
                let row = if corner == 1 { 0 } else { bottom };
                let k = self.corner_space(row, 0, true);
                if row < 0 {
                    return -1;
                }
                self.cut(row as usize, 0, k - 1, true)
            }
            2 | 4 => {
                let row = if corner == 2 { 0 } else { bottom };
                if row < 0 {
                    return -1;
                }
                let k = self.corner_space(row, w - 1, false);
                let n = self.cut(row as usize, k + 1, w - 1, true);
                if n > 0 {
                    if corner == 2 {
                        self.room_top = n + 3;
                    } else {
                        self.room_bottom = n + 3;
                    }
                }
                n
            }
            _ => -1,
        }
    }

    fn put(&mut self, s: &[u8]) {
        self.out.extend_from_slice(s);
    }

    fn end_as_is(&mut self) {
        if self.as_is {
            self.put(b"|SPACING |PAR-IND |COMPRESS\n");
            self.as_is = false;
        }
    }

    /// The braille page number in the token (the digit letters after '#'); -2 when it holds something else, -1 when
    /// it is too large.
    fn digits_value(token: &[u8]) -> i64 {
        let mut value: i64 = 0;
        let mut place: i64 = 1;
        for &c in token.iter().rev() {
            if c == DIGITS[10] {
                break;
            }
            let Some(d) = DIGITS[..10].iter().position(|&x| x == c) else { return -2 };
            if place < 0 || (i32::MAX as i64 / place) < d as i64 {
                return -1;
            }
            if i32::MAX as i64 - d as i64 * place < value {
                return -1;
            }
            value += d as i64 * place;
            place = if place < 0xccccccd { place * 10 } else { -1 };
        }
        value
    }

    /// A Roman number (I V X L C D M, a smaller one before a larger one taken off); -2 for another letter.
    fn roman_value(token: &[u8]) -> i64 {
        const LETTERS: &[u8] = b"IVXLCDM";
        const VALUES: [i64; 7] = [1, 5, 10, 50, 100, 500, 1000];
        let (mut value, mut largest) = (0i64, 0i64);
        for &c in token.iter().rev() {
            let Some(k) = LETTERS.iter().position(|&x| x == c.to_ascii_uppercase()) else { return -2 };
            let v = VALUES[k];
            if v < largest {
                value -= v;
            } else {
                largest = v;
                if i32::MAX as i64 - largest < value {
                    return -1;
                }
                value += v;
            }
        }
        value
    }

    /// Where line `row`'s text starts and ends; its class: 10 blank, 0x14 a print page change line, 0x28 as-is,
    /// 0x1e an ordinary line, 0x32 preserved as it is.
    fn class(&mut self, row: i32) -> u8 {
        self.first = self.scan(row, 0, true, false, b' ');
        let k = self.scan(row, self.s.cells as i32 - 1, false, false, b' ');
        self.last = if k < 0 { -1 } else { k + 1 };
        if self.s.preserve {
            return 0x32;
        }
        if self.first < 0 {
            return 10;
        }
        let r = row as usize;
        if self.cell(r, self.first) == b'-' {
            let k = self.scan(row, self.first + 1, true, false, b'-');
            if k - self.first > 2 && self.cell(r, k) != b' ' {
                let next = self.scan(row, k + 1, true, true, b' ');
                if next == self.last || next == -1 {
                    return 0x14;
                }
            }
        }
        for k in self.first..self.last {
            if self.cell(r, k) == b' ' {
                if self.cell(r, k + 1) == b' ' {
                    return 0x28;
                }
                if self.cell(r, k + 1) == b'"' {
                    let q = self.scan(row, k + 1, true, false, b'"');
                    if self.cell(r, q) == b' ' {
                        return 0x28;
                    }
                }
            }
        }
        0x1e
    }

    fn run(&mut self) {
        if !self.s.preserve {
            self.put(b"|WB-NO ");
            self.put(b"|PAR-IND\n");
            let opening = format!(
                "|SVRHP{} |SVPNP{}:{}:{}:{} |SVRFP{}\n",
                self.s.header, self.s.odd_main, self.s.even_main, self.s.odd_reference, self.s.even_reference, self.s.footer
            );
            self.put(opening.as_bytes());
        }
        while !self.eof() {
            self.room_top = 0;
            self.room_bottom = 0;
            if !self.read_page() {
                return;
            }
            if self.page_pending {
                self.put(b"|PG\n");
                self.page_pending = false;
            }
            // The braille page number in its corner.
            let main = if self.parity == 1 { self.s.odd_main } else { self.s.even_main };
            if !self.s.preserve && main > 0 {
                let mut value = i64::from(self.corner_token(main));
                if value > 0 {
                    if self.token.first() == Some(&b'#') {
                        value = Self::digits_value(&self.token);
                        if value > 0 && self.roman {
                            self.put(b"|PNTA\n");
                            self.roman = false;
                        }
                        let t0 = self.token[0];
                        if (t0 != b'#' || self.page_char != b' ') && t0 != self.page_char {
                            self.page_char = if t0 == b'#' { b' ' } else { t0 };
                            let s = format!("|PV:{}\n", self.page_char as char);
                            self.put(s.as_bytes());
                        }
                    } else if self.token.first() == Some(&b';') {
                        value = Self::roman_value(&self.token[1..]);
                        if value > 0 && !self.roman {
                            self.put(b"|PNTR\n");
                            self.roman = true;
                        }
                        if value <= 0 {
                            value = -1;
                        }
                    } else {
                        value = -1;
                    }
                }
                if value < 1 {
                    if self.counter != i32::MAX {
                        let s = format!("|SVPFD{}\n", self.counter + 1);
                        self.put(s.as_bytes());
                    }
                } else if value != i64::from(self.counter) {
                    let s = format!("|PV{value}\n");
                    self.put(s.as_bytes());
                    self.counter = value as i32;
                }
            }
            // The print page number in its corner.
            let reference = if self.parity == 1 { self.s.odd_reference } else { self.s.even_reference };
            if !self.s.preserve && reference > 0 {
                let n = self.corner_token(reference);
                if n < 1 {
                    if self.print != b"`" {
                        self.put(b"|LED\n");
                        self.print = b"`".to_vec();
                    }
                } else if self.token != self.print {
                    let s = [b"|LEAF- ".as_slice(), &self.token, b" |L\n"].concat();
                    self.put(&s);
                    self.print = [b" ".as_slice(), &self.token].concat();
                }
            }
            // The running head on the first row.
            let mut from = 0usize;
            let parity = self.parity;
            if (!self.s.preserve && self.s.header == 3) || self.s.header == parity {
                from = 1;
                let last = self.scan(0, self.s.cells as i32 - 1, false, false, b' ');
                let first = self.scan(0, 0, true, false, b' ');
                let n = self.cut(0, first, last, false);
                if n < 1 {
                    if !self.s.preserve && !self.head.is_empty() {
                        self.put(b"|TLS |' |TLE\n");
                        self.head.clear();
                    }
                } else if self.token != self.head {
                    let s = [b"|TLS ".as_slice(), &self.token, b" |TLE\n"].concat();
                    self.put(&s);
                    self.head = self.token.clone();
                }
            }
            // The running foot on the last row.
            let mut to = self.depth;
            if (!self.s.preserve && self.s.footer == 3) || self.s.footer == parity {
                to -= 1;
                let row = self.depth as i32 - 1;
                let last = self.scan(row, self.s.cells as i32 - 1, false, false, b' ');
                let first = self.scan(row, 0, true, false, b' ');
                let n = self.cut(row.max(0) as usize, first, last, false);
                if n < 1 {
                    if !self.foot.is_empty() {
                        self.put(b"|RFS |' |RFE\n");
                        self.foot.clear();
                    }
                } else if self.token != self.foot {
                    let s = [b"|RFS ".as_slice(), &self.token, b" |RFE\n"].concat();
                    self.put(&s);
                    self.foot = self.token.clone();
                }
            }
            self.lines(from, to);
            self.end_as_is();
            self.put(b"\x0c");
            if self.s.preserve {
                self.page_pending = true;
            }
            self.counter = self.counter.saturating_add(1);
            self.parity = if self.parity >= 2 { 1 } else { self.parity + 1 };
            // The continuation letter of the print page number the next page shows.
            if self.print.first() != Some(&b'`') {
                if let Some(c) = self.print.first_mut() {
                    *c = match *c {
                        b' ' | b'Z' => b'A',
                        c => c + 1,
                    };
                }
                if self.s.preserve {
                    self.page_pending = true;
                }
            }
        }
    }

    /// The body rows `from` to `to` (the head and foot rows left out), each by its class.
    fn lines(&mut self, from: usize, to: usize) {
        let w = self.s.cells as i32;
        let mut row = from;
        while row < to {
            match self.class(row as i32) {
                10 => {
                    self.end_as_is();
                    let mut next = row;
                    loop {
                        next += 1;
                        if next >= to || self.class(next as i32) != 10 {
                            break;
                        }
                    }
                    if next < to {
                        let s = format!("|SK{}\n", next - row);
                        self.put(s.as_bytes());
                        row = next - 1;
                    } else {
                        self.page_pending = true;
                        row = to - 1;
                    }
                    self.room = 0;
                }
                0x14 => {
                    self.end_as_is();
                    let start = self.scan(row as i32, self.first, true, false, b'-');
                    let n = self.cut(row, start, self.last - 1, false);
                    if n < 1 {
                        self.cut(row, self.first, self.last - 1, false);
                        let s = [b"|L ".as_slice(), &self.token, b" |L\n"].concat();
                        self.put(&s);
                    } else {
                        let s = [b"|LEAF- ".as_slice(), &self.token, b" |L\n"].concat();
                        self.put(&s);
                        self.print = [b" ".as_slice(), &self.token].concat();
                    }
                    self.room = 0;
                }
                0x1e => {
                    self.end_as_is();
                    let (first, last) = (self.first, self.last);
                    if first == 0 || first == 2 {
                        if first == 0 {
                            // A line whose first word would have fit on the line before was a new line there.
                            let k = self.scan(row as i32, 1, true, true, b' ');
                            if k > 0 && k + 1 <= self.room {
                                self.put(b"|L ");
                            }
                        }
                        self.cut(row, 0, last - 1, false);
                        let mut s = self.token.clone();
                        s.extend_from_slice(if self.token.last() == Some(&b'-') { b" |L\n" } else { b"\n" });
                        self.put(&s);
                        self.room = w - last;
                    } else if first == 4 {
                        self.cut(row, first, last - 1, false);
                        let s = [b"|IND5 ".as_slice(), &self.token, b" |IND1\n"].concat();
                        self.put(&s);
                        self.room = 0;
                    } else if first < 3 || (first - (w - last)).abs() > 1 {
                        self.cut(row, first, last - 1, false);
                        let s = [format!("|TAB{} ", first + 1).as_bytes(), &self.token, b" |L\n"].concat();
                        self.put(&s);
                        self.room = 0;
                    } else {
                        self.cut(row, first, last - 1, false);
                        let s = [b"|HDS ".as_slice(), &self.token, b" |HDE\n"].concat();
                        self.put(&s);
                        self.room = 0;
                    }
                }
                0x28 => {
                    if !self.as_is {
                        self.put(b"|L |AS-IS |NO-PAR |MARKING\n");
                        self.as_is = true;
                    }
                    let last = self.last;
                    self.cut(row, 0, last - 1, false);
                    let mut s = self.token.clone();
                    s.extend_from_slice(if self.token.last() == Some(&b'-') { b"|<-\n" } else { b"\n" });
                    self.put(&s);
                    self.room = 0;
                }
                _ => {
                    // Preserved as it is: a line code between lines, every space kept.
                    if row > from {
                        self.put(b"|<\n");
                    }
                    let last = self.last;
                    self.cut(row, 0, last - 1, false);
                    let t = self.token.clone();
                    for c in t {
                        if c == b' ' {
                            self.put(b"|'");
                        } else {
                            self.out.push(c);
                        }
                    }
                }
            }
            if row == 0 {
                self.room -= self.room_top;
            } else if row + 1 == self.depth {
                self.room -= self.room_bottom;
            }
            self.room = self.room.max(0);
            row += 1;
        }
    }
}

/// Formatted braille (in North American braille ASCII) as coded braille text, with what to warn about.
pub fn braide(input: &[u8], settings: Settings) -> (Vec<u8>, Warnings) {
    let mut b = Braide {
        s: settings,
        input,
        at: 0,
        out: Vec::new(),
        rows: vec![vec![0u8; settings.cells.max(1)]; settings.lines.max(1)],
        depth: 0,
        width: 0,
        first: 0,
        last: 0,
        token: Vec::new(),
        parity: 1,
        counter: 1,
        roman: false,
        page_char: b' ',
        print: b"`".to_vec(),
        head: b"`".to_vec(),
        foot: b"`".to_vec(),
        room: 0,
        room_top: 0,
        room_bottom: 0,
        as_is: false,
        page_pending: false,
        other_control: false,
        warnings: Warnings::default(),
        overflow: false,
    };
    b.run();
    (b.out, b.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, s: Settings) -> String {
        String::from_utf8(braide(text.as_bytes(), s).0).unwrap()
    }

    #[test]
    fn pages_become_codes() {
        let s = Settings { lines: 6, cells: 20, ..Settings::default() };
        // A centred heading, a paragraph (two cells in), a line whose first word would have fit on the line before
        // (a new line there), two blank lines, and the page number #a in the lower right corner, flush right.
        let page = "       ,title\n  ,! cat sat\non ! mat4\n\n\nsome text         #a\n\x0c";
        let out = run(page, s);
        assert!(out.starts_with("|WB-NO |PAR-IND\n|SVRHP0 |SVPNP4:4:2:2 |SVRFP0\n"), "{out}");
        assert!(out.contains("|HDS ,TITLE |HDE\n"), "{out}");
        assert!(out.contains("  ,! CAT SAT\n|L ON ! MAT4\n"), "{out}");
        assert!(out.contains("|SK2\n"), "{out}");
        assert!(out.ends_with("SOME TEXT\n\x0c"), "{out}");
        // The page number #a is page 1, the counter's: no page code.
        assert!(!out.contains("|PV"), "{out}");
    }

    #[test]
    fn a_page_number_that_differs_and_a_print_page_change() {
        let s = Settings { lines: 4, cells: 20, ..Settings::default() };
        let out = run("text\n-------#ab\nmore              #e\n\x0c", s);
        assert!(out.contains("|LEAF- #AB |L\n"), "{out}");
        assert!(out.contains("|PV5\n"), "{out}");
    }
}
