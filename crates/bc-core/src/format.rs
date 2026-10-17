//! The page formatter: a document laid out into pages of lines of cells, from its codes (spec 19).
//!
//! The reference program keeps a document as running text with codes and lays it out page by page when the view,
//! Print or Emboss asks for a page. This formatter reads the document in the editor's form (bc_core::doctext: codes
//! in brackets, [<] shown as a line break) and records, for every cell it places, the character position in that
//! text it came from, so a caret position in the editor can be found on its page, line and column.
//!
//! Rules (spec 19, from the reference program's documentation unless marked):
//! - margins: left (where a line starts after a forced new line), runover (where it starts after a word wrap) and a
//!   right margin counted in from the full width; at first left = runover = the first cell, right margin 0.
//!   [indP:F] new line if not at the start of one, left margin at cell P counted from 1 (1 when not given; from the
//!   current margin when F is 1 or 3), runover keeping its offset; [inmP:F] the same without a new line; [ptysN]
//!   runover = left + N (2 when not given); [ptye] runover = left; [rmN] right margin.
//! - [<] new line always (a blank line when already at the start of one); [l] new line unless at the start of one;
//!   [p] new paragraph: new line, then (print documents) one blank line, the first line starting at the third cell of
//!   a braille document and the first of a print document, counted from the edge of the page (F0b, from the braille
//!   and print formatting files); [skN] [l] then N blank lines (0 when not given; a negative N to the line -N from
//!   the page end); [svsblN] N blank lines after every line (1 when not given).
//! - Words that do not fit go to the next line at the runover margin. Extra spaces are left out.
//! - [pg] new page always (two in a row give a blank page); [top] new page if the page has text; [cpN:L] new page if
//!   fewer than N + 1 lines remain, else a new line and L blank lines (braille 1 and 0, print 2 and 1, F0b).
//! - [hdsN:L] ... [hde] centred heading: on a new page if fewer than N + 1 lines remain (braille 1, print 2), else
//!   after L blank lines (braille 0, print 1); each heading line is centred on the whole line, (cells - length) / 2,
//!   its words wrapping inside the right margin.
//! - [fr] the next word flush right.
//! - [es~name] ... [ee~name]: the style's begin and end codes from the document's styles, in place.
//! - Braille page numbers: in the corner the document's page numbering names for even and odd pages, on a line of
//!   their own at the top or bottom of the page, written with the numeric indicator and the letters a to j.
//! INTERIM (spec 19 section 5 UNKNOWN, until traced): a word longer than a whole line is broken at the line end with
//! no mark; a page number takes its line alone; codes this formatter does not lay out take no room.

use crate::doctext::Shown;
use crate::engine::StyleTable;

/// Where a page number goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Corner {
    #[default]
    None,
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
}

impl Corner {
    /// The value a document file and the Page Numbering dialog use: 0 none, 1 upper left, 2 upper right, 3 lower
    /// left, 4 lower right.
    pub fn from_code(n: u16) -> Corner {
        match n {
            1 => Corner::UpperLeft,
            2 => Corner::UpperRight,
            3 => Corner::LowerLeft,
            4 => Corner::LowerRight,
            _ => Corner::None,
        }
    }

    fn top(self) -> bool {
        matches!(self, Corner::UpperLeft | Corner::UpperRight)
    }

    fn bottom(self) -> bool {
        matches!(self, Corner::LowerLeft | Corner::LowerRight)
    }

    fn right(self) -> bool {
        matches!(self, Corner::UpperRight | Corner::LowerRight)
    }
}

/// The page a document is laid out on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub cells: usize,
    pub lines: usize,
    /// A braille document (paragraphs start at the third cell, page numbers in braille).
    pub braille: bool,
    /// Braille page numbers on even pages and on odd pages.
    pub numbers_even: Corner,
    pub numbers_odd: Corner,
    /// The first page whose number is shown.
    pub first_number_shown: usize,
}

impl Layout {
    pub fn braille(cells: usize, lines: usize) -> Layout {
        Layout {
            cells,
            lines,
            braille: true,
            numbers_even: Corner::None,
            numbers_odd: Corner::None,
            first_number_shown: 1,
        }
    }

    pub fn print(cells: usize, lines: usize) -> Layout {
        Layout { braille: false, ..Layout::braille(cells, lines) }
    }
}

/// One line of a page: its cells, and for each cell the character position in the document it came from (None for
/// cells the formatter added: indentation, the space between words, page numbers).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub sources: Vec<Option<usize>>,
    /// Document positions that have no cell (a space, a line break, a code, the end of the document), each with the
    /// column (0-based) where the formatter's pen stood when it came to it: the reference's formatter records the
    /// same for a position it is asked about (spec 27 5b), so a caret can stand after the last letter
    /// of a line on a space the line does not show, or on an empty last line.
    pub marks: Vec<(usize, usize)>,
}

/// One laid-out page: all its lines (blank ones included, up to the lines per page) and the document position its
/// text starts at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub lines: Vec<Line>,
    pub start: usize,
}

/// A piece of the document: a character with its position (in characters), a space, or a code.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Char(char, usize),
    Space,
    Code(String),
    /// A document position that gets no cell: where the formatter's pen stands there is recorded (Line::marks).
    Mark(usize),
}

/// The document in pieces: its codes (the ranges the document marks as codes; a bracket elsewhere is text), with
/// style codes as their begin or end codes; the editor's line breaks become [<] codes, or nothing after [p] and [l].
fn pieces(doc: &Shown, styles: Option<&StyleTable>) -> Vec<Piece> {
    let mut out = Vec::new();
    let chars: Vec<char> = doc.text.chars().collect();
    let mut spans = doc.codes.iter().peekable();
    let mut i = 0;
    let mut after_break_code = false;
    while i < chars.len() {
        if let Some(&&(start, end)) = spans.peek().filter(|s| s.0 == i) {
            spans.next();
            let end = end.min(chars.len());
            let code: String = chars[(start + 1).min(end)..end.saturating_sub(1).max(start + 1).min(end)].iter().collect();
            after_break_code = crate::doctext::breaks_line(&code);
            expand(&code, styles, &mut out, 0, start);
            // A code's place is where the pen stands once the code has done its work (a [p] at the start stands
            // where the paragraph's text starts). INFERENCE: the reference records a place when its input reaches it
            // (spec 27 5b); whether that is before or after a code's own effect is not traced.
            out.push(Piece::Mark(start));
            i = end;
            continue;
        }
        let c = chars[i];
        if c == '\n' || c == ' ' {
            out.push(Piece::Mark(i));
        }
        match c {
            '\n' => {
                if !after_break_code {
                    out.push(Piece::Code("<".to_string()));
                }
                after_break_code = false;
                i += 1;
            }
            ' ' => {
                out.push(Piece::Space);
                after_break_code = false;
                i += 1;
            }
            c => {
                out.push(Piece::Char(c, i));
                after_break_code = false;
                i += 1;
            }
        }
    }
    out.push(Piece::Mark(chars.len()));
    out
}

/// A code as pieces: a style code becomes its begin or end codes (themselves codes in brackets); the text of a [q~X]
/// code is cells of the word it is in, and that of a [d~X] code cells that end the word (the reference's word
/// reader takes both into the word); anything else stays a code. `depth` stops a style that names itself; `pos` is
/// the code's place in the document, which its cells come from.
fn expand(code: &str, styles: Option<&StyleTable>, out: &mut Vec<Piece>, depth: usize, pos: usize) {
    if let Some(text) = code.strip_prefix("q~").or_else(|| code.strip_prefix("d~")) {
        out.extend(text.chars().map(|c| Piece::Char(c, pos)));
        if code.starts_with('d') {
            out.push(Piece::Space);
        }
        return;
    }
    let style = code.strip_prefix("es~").map(|n| (n, 0)).or_else(|| code.strip_prefix("ee~").map(|n| (n, 1)));
    if let (Some((name, which)), Some(table), true) = (style, styles, depth < 4) {
        if let Some(codes) = table.lookup(name.as_bytes(), which) {
            let text: String = codes.iter().map(|&b| match b {
                0x1c => '[',
                0x1f => ']',
                b => b as char,
            }).collect();
            let mut rest = text.as_str();
            while let Some(s) = rest.find('[') {
                let Some(e) = rest[s..].find(']') else { break };
                expand(&rest[s + 1..s + e], styles, out, depth + 1, pos);
                rest = &rest[s + e + 1..];
            }
        }
        return;
    }
    out.push(Piece::Code(code.to_string()));
}

/// A code's name and its numbers: "ind5" is ("ind", [5]), "hds1:0" is ("hds", [1, 0]).
fn split_code(code: &str) -> (&str, Vec<i64>) {
    let end = code.find(|c: char| c.is_ascii_digit() || c == '-' || c == ':').unwrap_or(code.len());
    let (name, rest) = code.split_at(end);
    let numbers = rest
        .split(':')
        .map(|n| n.split(|c: char| !(c.is_ascii_digit() || c == '-')).next().unwrap_or(""))
        .filter_map(|n| n.parse().ok())
        .collect();
    (name, numbers)
}

/// A page number as the page shows it: the prefix [pgN~X] set, then in a braille document the numeric indicator
/// and the digits as the letters a to j, in a print document the digits.
fn page_number(n: usize, prefix: Option<char>, braille: bool) -> String {
    let mut s: String = prefix.into_iter().collect();
    if braille {
        s.push('#');
        for d in n.to_string().chars() {
            s.push(b"jabcdefghi"[d.to_digit(10).unwrap_or(0) as usize] as char);
        }
    } else {
        s.push_str(&n.to_string());
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Formatter {
    layout: Layout,
    pages: Vec<Page>,
    /// Lines of the page being filled (its number is put on its line when the page is finished).
    page: Vec<Line>,
    /// The page counter: the number of the page being filled; [pgN] sets the next page's; the prefix [pgN~X] sets.
    counter: usize,
    next_counter: Option<usize>,
    prefix: Option<char>,
    /// Where page numbers go on odd and even pages and the first number shown; [svpnp] and [svpfd] change them from
    /// the next page.
    numbers_odd: Corner,
    numbers_even: Corner,
    first_number_shown: usize,
    next_numbers: Option<(Corner, Corner)>,
    next_first_shown: Option<usize>,
    page_start: Option<usize>,
    /// Where the last page finished starts (a page with no text starts there too).
    last_start: usize,
    line: Line,
    /// Where the next text on the line starts.
    start_col: usize,
    left: usize,
    runover_offset: usize,
    right: usize,
    blank_between: usize,
    flush_right: bool,
    heading: Option<(Vec<(String, Vec<Option<usize>>)>, i64, i64)>,
    /// The last thing done was a page break by [pg] (a second one gives a blank page).
    after_page_code: bool,
    /// Between [tls] and [tle] (or [rfs] and [rfe]): the words of a running head (or foot), which are not body text
    /// (spec 19, page numbers and running heads). INTERIM: where the reference places it on later pages is not traced
    /// (spec 19 section 4 UNKNOWN), so it is kept and not shown.
    running: Option<Vec<String>>,
    pub running_head: Option<String>,
    /// Marks met while a word was being collected, each with the index in the word of the cell after it: they are
    /// placed with the word (Formatter::word).
    word_marks: Vec<(usize, usize)>,
    /// A code ended the last word: the next one follows it with no space (a line may still end between them).
    touching: bool,
}

impl Formatter {
    fn new(layout: &Layout) -> Formatter {
        Formatter {
            layout: layout.clone(),
            pages: Vec::new(),
            page: Vec::new(),
            counter: 1,
            next_counter: None,
            prefix: None,
            numbers_odd: layout.numbers_odd,
            numbers_even: layout.numbers_even,
            first_number_shown: layout.first_number_shown,
            next_numbers: None,
            next_first_shown: None,
            page_start: None,
            last_start: 0,
            line: Line::default(),
            start_col: 0,
            left: 0,
            runover_offset: 0,
            right: 0,
            blank_between: 0,
            flush_right: false,
            heading: None,
            after_page_code: false,
            running: None,
            running_head: None,
            word_marks: Vec::new(),
            touching: false,
        }
    }

    /// Records document position `pos` at the pen: after the line's last cell, or where the line's text will start.
    fn mark(&mut self, pos: usize) {
        let used = self.line.text.chars().count();
        let col = if self.line_has_text() { used } else { self.start_col.max(used).max(self.rooms().0) };
        self.line.marks.push((pos, col));
    }

    /// Places the marks of the word being placed whose cell index is `k`, at column `col`.
    fn place_word_marks(&mut self, k: usize, col: usize) {
        let marks = &mut self.word_marks;
        let mut i = 0;
        while i < marks.len() {
            if marks[i].1 == k {
                let (pos, _) = marks.remove(i);
                self.line.marks.push((pos, col));
            } else {
                i += 1;
            }
        }
    }

    /// The left margin a [indP:F] or [inmP:F] sets: cell P counted from 1 (1 when not given), from the current
    /// margin when F is 1 or 3, within the line.
    fn margin(&self, p: Option<i64>, f: Option<i64>) -> usize {
        let mut cell = p.unwrap_or(1) - 1;
        if matches!(f, Some(1) | Some(3)) {
            cell += self.left as i64 + 1;
        }
        cell.clamp(0, self.layout.cells.saturating_sub(1) as i64) as usize
    }

    /// Where the line's words wrap: the right margin, or the room a page number on the line keeps.
    fn width(&self) -> usize {
        self.layout.cells.saturating_sub(self.right.max(self.rooms().1)).max(1)
    }

    /// The corner the number of the page being filled goes in (none before the first page shown). INFERENCE: odd
    /// and even are those of the page counter (the reference's parity flag is not traced to its source).
    fn corner(&self) -> Corner {
        if self.counter < self.first_number_shown {
            return Corner::None;
        }
        if self.counter % 2 == 0 {
            self.numbers_even
        } else {
            self.numbers_odd
        }
    }

    /// The page number of the page being filled, as shown.
    fn number_text(&self) -> String {
        page_number(self.counter, self.prefix, self.layout.braille)
    }

    /// The lines a page holds. A page number takes no line of its own: it goes on the first or last line, beside the
    /// text (rooms).
    fn capacity(&self) -> usize {
        self.layout.lines.max(1)
    }

    /// The cells the page number keeps on the line being filled, at its start and at its end: the number's length
    /// and three more on the first line for a number at the top, on the last line for one at the bottom.
    fn rooms(&self) -> (usize, usize) {
        let c = self.corner();
        let at = self.page.len();
        if (c.top() && at == 0) || (c.bottom() && at + 1 == self.capacity()) {
            let room = self.number_text().chars().count() + 3;
            if c.right() {
                (0, room)
            } else {
                (room, 0)
            }
        } else {
            (0, 0)
        }
    }

    fn line_has_text(&self) -> bool {
        self.line.sources.iter().any(Option::is_some)
    }

    fn page_has_text(&self) -> bool {
        self.line_has_text() || self.page.iter().any(|l| l.sources.iter().any(Option::is_some))
    }

    fn put(&mut self, c: char, source: Option<usize>) {
        if source.is_some() && self.page_start.is_none() {
            self.page_start = source;
        }
        self.line.text.push(c);
        self.line.sources.push(source);
    }

    /// Ends the line; the next one starts at `next_col`.
    fn end_line(&mut self, next_col: usize) {
        let mut line = std::mem::take(&mut self.line);
        while line.text.ends_with(' ') {
            line.text.pop();
            line.sources.pop();
        }
        self.push_line(line);
        // Line spacing follows every line end, a blank line's too.
        for _ in 0..self.blank_between {
            self.push_line(Line::default());
        }
        self.start_col = next_col;
        self.after_page_code = false;
    }

    fn push_line(&mut self, line: Line) {
        if self.page.len() >= self.capacity() {
            self.finish_page();
        }
        // Blank lines at the top of a page are left out.
        if line.text.is_empty() && self.page.is_empty() && !self.pages.is_empty() && !self.after_page_code {
            // Its marks go to the line that takes its place.
            self.line.marks.extend(line.marks);
            return;
        }
        self.page.push(line);
        if self.page.len() >= self.capacity() {
            self.finish_page();
        }
    }

    fn finish_page(&mut self) {
        let corner = self.corner();
        let number = self.number_text();
        let mut lines = std::mem::take(&mut self.page);
        let room = self.capacity();
        lines.resize(room, Line::default());
        // The number goes on the first line (at the top) or the last (at the bottom), at its start or its end; the
        // text on that line was kept clear of it.
        let at = if corner.top() { Some(0) } else if corner.bottom() { Some(room - 1) } else { None };
        if let Some(line) = at.and_then(|k| lines.get_mut(k)) {
            let len = number.chars().count();
            let col = if corner.right() { self.layout.cells.saturating_sub(len) } else { 0 };
            let mut text: Vec<char> = line.text.chars().collect();
            if text.len() < col + len {
                text.resize(col + len, ' ');
                line.sources.resize(col + len, None);
            }
            for (k, c) in number.chars().enumerate() {
                text[col + k] = c;
                line.sources[col + k] = None;
            }
            line.text = text.into_iter().collect();
        }
        let start = self.page_start.take().unwrap_or(self.last_start);
        self.last_start = start;
        self.pages.push(Page { lines, start });
        // The next page: its counter, and the number settings changed on this one.
        self.counter = self.next_counter.take().unwrap_or(self.counter + 1);
        if let Some((odd, even)) = self.next_numbers.take() {
            self.numbers_odd = odd;
            self.numbers_even = even;
        }
        if let Some(first) = self.next_first_shown.take() {
            self.first_number_shown = first;
        }
    }

    fn new_page(&mut self) {
        if self.line_has_text() {
            self.end_line(self.left);
        }
        // A page its last line already finished is not finished again (that would be a blank page).
        if !self.page.is_empty() || self.pages.is_empty() {
            self.finish_page();
        }
        self.start_col = self.left;
    }

    /// Places one word (its cells and their sources), wrapping it to the runover margin when it does not fit.
    fn word(&mut self, cells: &[(char, Option<usize>)]) {
        if let Some(r) = &mut self.running {
            r.push(cells.iter().map(|c| c.0).collect());
            self.word_marks.clear();
            return;
        }
        if let Some(h) = &mut self.heading {
            // The heading's words are centred later; places in them with no cell fall back to the cell before.
            h.0.push((cells.iter().map(|c| c.0).collect(), cells.iter().map(|c| c.1).collect()));
            self.word_marks.clear();
            return;
        }
        let width = self.width();
        if self.flush_right {
            self.flush_right = false;
            let len = cells.len();
            let used = self.line.text.chars().count();
            if used + 1 + len > self.layout.cells.saturating_sub(self.rooms().1) {
                self.end_line(self.left + self.runover_offset);
            }
            let used = self.line.text.chars().count();
            for _ in used..self.layout.cells.saturating_sub(self.rooms().1).saturating_sub(len) {
                self.put(' ', None);
            }
            for (k, &(c, s)) in cells.iter().enumerate() {
                let col = self.line.text.chars().count();
                self.place_word_marks(k, col);
                self.put(c, s);
            }
            let col = self.line.text.chars().count();
            self.place_word_marks(cells.len(), col);
            return;
        }
        let mut rest = cells;
        let mut k = 0;
        while !rest.is_empty() {
            let used = self.line.text.chars().count();
            let has_text = self.line_has_text();
            let gap = usize::from(has_text && !self.touching);
            let start = if has_text { used + gap } else { self.start_col.max(used).max(self.rooms().0) };
            if start + rest.len() > width && has_text {
                self.touching = false;
                self.end_line(self.left + self.runover_offset);
                continue;
            }
            let fits = width.saturating_sub(start).max(1);
            if has_text {
                if gap == 1 {
                    self.put(' ', None);
                }
            } else {
                for _ in used..start {
                    self.put(' ', None);
                }
            }
            let (now, later) = rest.split_at(rest.len().min(fits));
            for &(c, s) in now {
                let col = self.line.text.chars().count();
                self.place_word_marks(k, col);
                self.put(c, s);
                k += 1;
            }
            rest = later;
            if !rest.is_empty() {
                self.end_line(self.left + self.runover_offset);
            }
        }
        let col = self.line.text.chars().count();
        self.place_word_marks(k, col);
        self.touching = false;
    }

    fn code(&mut self, code: &str) {
        let (name, n) = split_code(code);
        let first = n.first().copied();
        match name {
            "<" => {
                let next = self.left;
                self.end_line(next);
            }
            "l" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
            }
            "p" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                if !self.layout.braille && self.page_has_text() {
                    self.push_line(Line::default());
                }
                // The paragraph column (spec 19 F0b, the braille and print formatting files): the third cell in
                // braille, the first in print, counted from the edge of the page, not from the left margin.
                self.start_col = if self.layout.braille { 2 } else { 0 };
            }
            "cp" => {
                // New page if fewer than N + 1 lines remain, else a new line and L blank lines (braille 1 and 0,
                // print 2 and 1).
                let (need, skip) = if self.layout.braille { (1, 0) } else { (2, 1) };
                let need = first.unwrap_or(need);
                let skip = n.get(1).copied().unwrap_or(skip);
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let remaining = self.capacity().saturating_sub(self.page.len()) as i64;
                if remaining < need + 1 && !self.page.is_empty() {
                    self.finish_page();
                } else if !self.page.is_empty() {
                    for _ in 0..skip.max(0) {
                        self.push_line(Line::default());
                    }
                }
            }
            "sk" => {
                // [skN]: [l], then N lines (0 when not given, at most three pages' lines); a negative N goes to the
                // line -N from the page end.
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let n = first.unwrap_or(0);
                let count = if n < 0 { self.capacity() as i64 - self.page.len() as i64 + n } else { n };
                for _ in 0..count.clamp(0, 3 * self.layout.lines as i64) {
                    self.push_line(Line::default());
                }
            }
            "ind" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                self.left = self.margin(first, n.get(1).copied());
                self.start_col = self.left;
            }
            "inm" => {
                self.left = self.margin(first, n.get(1).copied());
                if !self.line_has_text() {
                    self.start_col = self.left;
                }
            }
            // [ptys] alone is a runover two cells in.
            "ptys" => self.runover_offset = first.unwrap_or(2).max(0) as usize,
            "ptye" => self.runover_offset = 0,
            "rm" => self.right = first.unwrap_or(0).max(0) as usize,
            // [svsblN]: N blank lines after every line (1 when not given), at most (lines - 4) / 2.
            "svsbl" => {
                let most = (self.layout.lines as i64 - 4).max(0) / 2;
                self.blank_between = first.unwrap_or(1).clamp(0, most) as usize;
            }
            "pg" => {
                // [pgN~X]: the next page is numbered N, with X before its number.
                if let Some(n) = first.filter(|&n| n > 0) {
                    self.next_counter = Some(n as usize);
                }
                if self.after_page_code && !self.page_has_text() {
                    // Two page codes in a row: a blank page.
                    self.finish_page();
                } else {
                    self.new_page();
                }
                if let Some((_, x)) = code.split_once('~') {
                    self.prefix = x.chars().next().filter(|&c| c != ' ');
                }
                self.after_page_code = true;
            }
            // [svpnpA:B] where page numbers go from the next page, A on odd and B on even pages (0 none, 1 upper left,
            // 2 upper right, 3 lower left, 4 lower right); alone, lower right on both.
            "svpnp" => {
                let (odd, even) = if n.is_empty() { (4, 4) } else { (n[0], n.get(1).copied().unwrap_or(0)) };
                let corner = |v: i64| Corner::from_code(v.clamp(0, 4) as u16);
                self.next_numbers = Some((corner(odd), corner(even)));
            }
            // [svpfdN] the first page whose number is shown, from the next page.
            "svpfd" => self.next_first_shown = Some(first.unwrap_or(1).max(1) as usize),
            "top" => {
                if self.page_has_text() {
                    self.new_page();
                }
            }
            "fr" => self.flush_right = true,
            // [kps] ... [kpe] keep a block together, with a new line implied at both ends (spec 19, Pages).
            // INTERIM: moving a block that would cross a page to the next page is not done yet (queue line F6).
            "kps" | "kpe" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
            }
            // [scL] skip L lines (1 when not given), only after a line that is not blank (spec 19, Lines).
            "sc" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let previous = self.page.last().or_else(|| self.pages.last().and_then(|p| p.lines.last()));
                if previous.is_some_and(|l| l.sources.iter().any(Option::is_some)) {
                    for _ in 0..first.unwrap_or(1).max(0) {
                        self.push_line(Line::default());
                    }
                }
            }
            "tls" | "rfs" => self.running = Some(Vec::new()),
            "tle" | "rfe" => {
                if let Some(words) = self.running.take() {
                    if name == "tle" {
                        self.running_head = Some(words.join(" "));
                    }
                }
            }
            "tld" => self.running_head = None,
            "hds" => {
                let (need, skip) = if self.layout.braille { (1, 0) } else { (2, 1) };
                let need = first.unwrap_or(need);
                let skip = n.get(1).copied().unwrap_or(skip);
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                self.heading = Some((Vec::new(), need, skip));
            }
            "hde" => {
                if let Some((words, need, skip)) = self.heading.take() {
                    self.heading_lines(&words, need, skip);
                }
            }
            _ => {}
        }
    }

    fn heading_lines(&mut self, words: &[(String, Vec<Option<usize>>)], need: i64, skip: i64) {
        // On a page number's line a centred line keeps twice the number's room, so it stays centred on the line.
        let (l, r) = self.rooms();
        let width = self.layout.cells.saturating_sub(self.right.max(2 * l.max(r))).max(1);
        let mut lines: Vec<Line> = Vec::new();
        let mut line = Line::default();
        for (w, s) in words {
            let len = w.chars().count();
            if !line.text.is_empty() && line.text.chars().count() + 1 + len > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.text.is_empty() {
                line.text.push(' ');
                line.sources.push(None);
            }
            line.text.push_str(w);
            line.sources.extend(s.iter().copied());
        }
        if !line.text.is_empty() {
            lines.push(line);
        }
        let remaining = self.capacity().saturating_sub(self.page.len()) as i64;
        if remaining < need + 1 && !self.page.is_empty() {
            self.finish_page();
        } else if !self.page.is_empty() {
            for _ in 0..skip.max(0) {
                self.push_line(Line::default());
            }
        }
        for l in lines {
            let len = l.text.chars().count();
            // A centred line is centred on the whole line, rounding down; the right margin only narrows where its
            // words wrap.
            let pad = self.layout.cells.saturating_sub(len) / 2;
            let mut centred = Line { text: " ".repeat(pad), sources: vec![None; pad], marks: Vec::new() };
            centred.text.push_str(&l.text);
            centred.sources.extend(l.sources);
            if let Some(s) = centred.sources.iter().flatten().next() {
                if self.page_start.is_none() {
                    self.page_start = Some(*s);
                }
            }
            self.push_line(centred);
        }
        self.start_col = self.left;
    }

    /// Lays out one piece; `word` is the word being collected.
    fn step(&mut self, p: &Piece, word: &mut Vec<(char, Option<usize>)>) {
        match p {
            Piece::Char(c, at) => word.push((if *c == '\u{a0}' { ' ' } else { *c }, Some(*at))),
            Piece::Space => {
                if !word.is_empty() {
                    self.word(&std::mem::take(word));
                }
                self.touching = false;
            }
            Piece::Code(code) => {
                // A code in a word ends the word, and what follows it is placed against it, except the codes that
                // belong to a word (grouping, word break, verse and font marks). Codes the formatter does not lay
                // out take no room.
                let in_word = matches!(code.as_str(), ":" | ";")
                    || ["wb", "vss", "vse", "fts~", "fte~"].iter().any(|w| code.starts_with(w));
                if !in_word && !word.is_empty() {
                    self.word(&std::mem::take(word));
                    self.touching = true;
                }
                if is_layout(code) {
                    self.code(code);
                }
            }
            Piece::Mark(pos) => {
                if !word.is_empty() {
                    self.word_marks.push((*pos, word.len()));
                } else if self.heading.is_none() && self.running.is_none() {
                    self.mark(*pos);
                }
            }
        }
    }

    /// The formatter as it stands with every document place in it moved by `map`; None when a place has nowhere to
    /// go (it was in the text an edit replaced).
    fn moved(&self, map: &impl Fn(usize) -> Option<usize>) -> Option<Formatter> {
        let mut f = Formatter { pages: Vec::new(), ..self.clone() };
        for line in f.page.iter_mut().chain(std::iter::once(&mut f.line)) {
            move_line(line, map)?;
        }
        if let Some(s) = f.page_start {
            f.page_start = Some(map(s)?);
        }
        f.last_start = map(f.last_start)?;
        if let Some(h) = &mut f.heading {
            for (_, sources) in &mut h.0 {
                for s in sources.iter_mut().flatten() {
                    *s = map(*s)?;
                }
            }
        }
        for m in &mut f.word_marks {
            m.0 = map(m.0)?;
        }
        Some(f)
    }

    /// The formatter with no pages, to keep beside the pages it has made.
    fn snapshot(&self) -> Formatter {
        Formatter { pages: Vec::new(), ..self.clone() }
    }

    /// Lays out `pieces` from piece `from`, `word` being the word collected so far. After each piece that finished a
    /// page, the state is kept in `checkpoints`; with `join`, a state that matches the old layout's at the same page
    /// past the edit takes the old layout's later pages and stops.
    fn run_from(
        mut self,
        pieces: &[Piece],
        from: usize,
        mut word: Vec<(char, Option<usize>)>,
        checkpoints: &mut Vec<Checkpoint>,
        mut join: Option<Join>,
    ) -> Vec<Page> {
        let mut made = self.pages.len();
        for (i, p) in pieces.iter().enumerate().skip(from) {
            self.step(p, &mut word);
            if self.pages.len() == made {
                continue;
            }
            made = self.pages.len();
            let at = i + 1;
            checkpoints.push(Checkpoint { piece: at, pages: made, state: self.snapshot(), word: word.clone() });
            if let Some(j) = &mut join {
                if let Some(tail) = j.try_join(at, made, &self.snapshot(), &word) {
                    let mut pages = self.pages;
                    pages.extend(tail.0);
                    checkpoints.extend(tail.1);
                    return pages;
                }
            }
        }
        self.end(word)
    }

    /// Lays out the last word and what is still open, and gives the pages.
    fn end(mut self, word: Vec<(char, Option<usize>)>) -> Vec<Page> {
        if !word.is_empty() {
            self.word(&word);
        }
        if let Some((words, need, skip)) = self.heading.take() {
            self.heading_lines(&words, need, skip);
        }
        // A line holding only marks is kept too (the end of a document whose last line is empty), but never so as to
        // start a page: marks do not change the pages. On a page not yet started they go on the last line before it.
        if !self.line_has_text() && self.line.text.is_empty() && self.page.is_empty() && !self.pages.is_empty() {
            let marks = std::mem::take(&mut self.line.marks);
            // The last line of the last page that has text (or the page's first line).
            if let Some(page) = self.pages.last_mut() {
                let at = page.lines.iter().rposition(|l| l.sources.iter().any(Option::is_some)).unwrap_or(0);
                if let Some(last) = page.lines.get_mut(at) {
                    let col = last.text.chars().count();
                    last.marks.extend(marks.into_iter().map(|(pos, _)| (pos, col)));
                }
            }
        }
        if self.line_has_text() || !self.line.text.is_empty() || !self.line.marks.is_empty() {
            self.end_line(self.left);
        }
        if !self.page.is_empty() || self.pages.is_empty() {
            self.finish_page();
        }
        self.pages
    }
}

fn is_layout(code: &str) -> bool {
    matches!(
        split_code(code).0,
        "<" | "l" | "p" | "sk" | "ind" | "inm" | "ptys" | "ptye" | "rm" | "svsbl" | "pg" | "top" | "cp" | "fr" | "hds" | "hde"
            | "svpnp" | "svpfd"
            | "tls" | "tle" | "tld" | "rfs" | "rfe" | "kps" | "kpe" | "sc"
    )
}

/// Lays out a document (in the editor's form) into pages, with the document's styles.
pub fn format(doc: &Shown, layout: &Layout, styles: Option<&StyleTable>) -> Vec<Page> {
    format_traced(doc, layout, styles).0
}

/// Moves every document place on a line by `map`; None when one has nowhere to go.
fn move_line(line: &mut Line, map: &impl Fn(usize) -> Option<usize>) -> Option<()> {
    for s in line.sources.iter_mut().flatten() {
        *s = map(*s)?;
    }
    for m in &mut line.marks {
        m.0 = map(m.0)?;
    }
    Some(())
}

/// The formatter's state after the piece that finished a page: the piece to go on from, the pages made, the state
/// and the word being collected.
#[derive(Debug, Clone)]
struct Checkpoint {
    piece: usize,
    pages: usize,
    state: Formatter,
    word: Vec<(char, Option<usize>)>,
}

/// What a layout keeps so that it can be laid out again after an edit from the page the edit is on (spec 27 5d: the
/// reference drops its pages from the edit on and lays them out again; the ones before are kept).
#[derive(Debug, Clone, Default)]
pub struct Trail {
    pieces: Vec<Piece>,
    checkpoints: Vec<Checkpoint>,
    layout: Option<Layout>,
}

/// The old layout's pages and states after the edit, to take over once the new layout comes to the same state at
/// the same page.
struct Join {
    /// Where the pieces after the edit start, in the new pieces; the new index less the old one.
    same_from: usize,
    shift: isize,
    /// The old pages from `base` on, and the old checkpoints after the restart.
    base: usize,
    pages: Vec<Page>,
    checkpoints: Vec<Checkpoint>,
    total_pages: usize,
    map: Box<dyn Fn(usize) -> Option<usize>>,
}

impl Join {
    /// When the new layout, at piece `at` with `made` pages, stands where the old one stood: the old pages after
    /// `made` and the old checkpoints after it, moved into the new document.
    fn try_join(
        &mut self,
        at: usize,
        made: usize,
        state: &Formatter,
        word: &[(char, Option<usize>)],
    ) -> Option<(Vec<Page>, Vec<Checkpoint>)> {
        // Only past the edit, and not at the old layout's last page (the end of the layout can still change it).
        if at < self.same_from || made >= self.total_pages {
            return None;
        }
        let k = self.checkpoints.iter().position(|c| c.pages == made)?;
        let old = &self.checkpoints[k];
        if old.piece as isize + self.shift != at as isize {
            return None;
        }
        let map = &self.map;
        if old.state.moved(map).as_ref() != Some(state) {
            return None;
        }
        let old_word: Option<Vec<_>> = old.word.iter().map(|&(c, s)| Some((c, match s {
            Some(s) => Some(map(s)?),
            None => None,
        }))).collect();
        if old_word.as_deref() != Some(word) {
            return None;
        }
        let mut pages: Vec<Page> = self.pages.split_off(made - self.base);
        for page in &mut pages {
            page.start = map(page.start)?;
            for line in &mut page.lines {
                move_line(line, map)?;
            }
        }
        let mut later = self.checkpoints.split_off(k + 1);
        for c in &mut later {
            c.piece = (c.piece as isize + self.shift) as usize;
            c.state = c.state.moved(map)?;
            for w in c.word.iter_mut() {
                if let Some(s) = w.1 {
                    w.1 = Some(map(s)?);
                }
            }
        }
        Some((pages, later))
    }
}

/// Whether document place `p` is inside a code (`codes` in order).
fn in_code(codes: &[(usize, usize)], p: usize) -> bool {
    let k = codes.partition_point(|c| c.0 <= p);
    k > 0 && p < codes[k - 1].1
}

/// The index of the first piece at or after document place `pos`; a piece with no place of its own (a code, a
/// space) counts at the place of the piece before it.
fn first_at(pieces: &[Piece], pos: usize) -> usize {
    let key = |mut i: usize| loop {
        match pieces[i] {
            Piece::Char(_, at) | Piece::Mark(at) => return at,
            _ if i == 0 => return 0,
            _ => i -= 1,
        }
    };
    let (mut lo, mut hi) = (0, pieces.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if key(mid) < pos {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Lays out a document and keeps what lets it be laid out again quickly after an edit (reformat).
pub fn format_traced(doc: &Shown, layout: &Layout, styles: Option<&StyleTable>) -> (Vec<Page>, Trail) {
    let pieces = pieces(doc, styles);
    let mut checkpoints = Vec::new();
    let pages = Formatter::new(layout).run_from(&pieces, 0, Vec::new(), &mut checkpoints, None);
    (pages, Trail { pieces, checkpoints, layout: Some(layout.clone()) })
}

/// Lays a document out again after an edit replaced its places `from` to `old_to` with what is now `from` to
/// `new_to`: from the last page finished before the edit, the pages before it kept, and stopping at the first page
/// after the edit where the layout stands as it stood before, the old pages after it kept with their places moved.
/// The result is the same as format gives.
pub fn reformat(
    pages: Vec<Page>,
    trail: Trail,
    doc: &Shown,
    layout: &Layout,
    styles: Option<&StyleTable>,
    from: usize,
    old_to: usize,
    new_to: usize,
) -> (Vec<Page>, Trail) {
    if trail.layout.as_ref() != Some(layout) || trail.pieces.is_empty() {
        return format_traced(doc, layout, styles);
    }
    let delta = new_to as isize - old_to as isize;
    // Only the stretch around the edit is cut into pieces again: from the last plain letter before the edit (the
    // same letter in the old text) to the first plain letter at or after its end. A letter's piece does not depend
    // on what came before it, so the pieces before and after that stretch are the old ones (those after it with
    // their places moved).
    let len = doc.text.chars().count();
    let plain = |c: char, p: usize| c != ' ' && c != '\n' && !in_code(&doc.codes, p);
    let lo = from.saturating_sub(64);
    let before = |lo: usize| {
        doc.text.chars().enumerate().skip(lo).take(from - lo).filter(|&(p, c)| plain(c, p)).last().map(|(p, _)| p)
    };
    let start = before(lo).or_else(|| before(0)).unwrap_or(0);
    let end = doc.text.chars().enumerate().skip(new_to).find(|&(p, c)| plain(c, p)).map_or(len, |(p, _)| p);
    let mut old = trail.pieces;
    let cut_from = if start == 0 { 0 } else { first_at(&old, start) };
    let cut_to = if end == len { old.len() } else { first_at(&old, (end as isize - delta) as usize) };
    let mut mid = pieces(&doc.slice(start, end), styles);
    if end < len {
        mid.pop();
    }
    for p in &mut mid {
        match p {
            Piece::Char(_, at) | Piece::Mark(at) => *at += start,
            _ => {}
        }
    }
    let mid_len = mid.len();
    old.splice(cut_from..cut_to, mid);
    if delta != 0 {
        for p in &mut old[cut_from + mid_len..] {
            match p {
                Piece::Char(_, at) | Piece::Mark(at) => *at = (*at as isize + delta) as usize,
                _ => {}
            }
        }
    }
    let new = old;
    let same_before = cut_from;
    let same_from = cut_from + mid_len;
    let shift = mid_len as isize - (cut_to - cut_from) as isize;
    let map = move |p: usize| -> Option<usize> {
        if p < from {
            Some(p)
        } else if p >= old_to {
            Some((p as isize + delta) as usize)
        } else {
            None
        }
    };
    // The last state kept before the first changed piece, never at the last page (the end of a layout changes it).
    let restart = trail
        .checkpoints
        .iter()
        .rposition(|c| c.piece <= same_before && c.pages < pages.len());
    let mut checkpoints = trail.checkpoints;
    let later = match restart {
        Some(k) => checkpoints.split_off(k + 1),
        None => std::mem::take(&mut checkpoints),
    };
    let total_pages = pages.len();
    let (formatter, start, word, base) = match restart.map(|k| checkpoints[k].clone()) {
        Some(c) => {
            let mut f = c.state;
            f.pages = pages;
            (f, c.piece, c.word, c.pages)
        }
        None => {
            let mut f = Formatter::new(layout);
            f.pages = pages;
            (f, 0, Vec::new(), 0)
        }
    };
    let mut formatter = formatter;
    let old_pages = formatter.pages.split_off(base);
    let join = Join {
        same_from,
        shift,
        base,
        pages: old_pages,
        checkpoints: later,
        total_pages,
        map: Box::new(map),
    };
    let pages = formatter.run_from(&new, start, word, &mut checkpoints, Some(join));
    (pages, Trail { pieces: new, checkpoints, layout: Some(layout.clone()) })
}

/// The character position at page `page`, line `line`, column `column` (all 1-based): the cell there, else the
/// first cell after it on that line, else just after the line's last cell; on a line with no text, the first text
/// after it. None for a page or line the document does not have.
pub fn position_of(pages: &[Page], page: usize, line: usize, column: usize) -> Option<usize> {
    let pg = pages.get(page.checked_sub(1)?)?;
    let ln = pg.lines.get(line.checked_sub(1)?)?;
    if let Some(s) = ln.sources.iter().skip(column.saturating_sub(1)).flatten().next() {
        return Some(*s);
    }
    // A place with no cell where the pen stood at or after the column (the first such), else the line's last.
    let col = column.saturating_sub(1);
    let mark = ln.marks.iter().filter(|m| m.1 >= col).min_by_key(|m| (m.1, m.0)).or_else(|| ln.marks.iter().max_by_key(|m| (m.1, m.0)));
    if let Some(m) = mark {
        return Some(m.0);
    }
    if let Some(s) = ln.sources.iter().flatten().last() {
        return Some(s + 1);
    }
    let later = pages[page - 1].lines[line..].iter().chain(pages[page..].iter().flat_map(|p| &p.lines));
    later.flat_map(|l| l.sources.iter().flatten()).next().copied()
}

/// The page, line and column (all 1-based, the line counted on its page) of a character position in the document:
/// the cell that came from that position; for a position that is no cell (a space, a line end, a code, the end), where
/// the formatter's pen stood at it (Line::marks); failing both, the place just after the last cell before it, as a
/// caret at the end of a line stands after its last cell; before any cell, the first cell.
pub fn locate(pages: &[Page], pos: usize) -> (usize, usize, usize) {
    let mut before: Option<(usize, usize, usize)> = None;
    for (p, page) in pages.iter().enumerate() {
        for (l, line) in page.lines.iter().enumerate() {
            // A place with no cell: where the formatter's pen stood (spec 27 5b).
            if let Some(&(_, col)) = line.marks.iter().find(|m| m.0 == pos) {
                return (p + 1, l + 1, col + 1);
            }
            for (c, s) in line.sources.iter().enumerate() {
                match s {
                    Some(s) if *s == pos => return (p + 1, l + 1, c + 1),
                    Some(s) if *s < pos => before = Some((p + 1, l + 1, c + 2)),
                    Some(_) => return before.unwrap_or((p + 1, l + 1, c + 1)),
                    None => {}
                }
            }
        }
    }
    before.unwrap_or((1, 1, 1))
}

/// The text of a formatted file (.brf, .prf) from laid-out pages, as the reference program's page writer writes it
/// (spec 04d-B): every line of every page with its trailing spaces left off and a line end after it, and a form feed
/// after every page, the last one included; nothing before the first page or after the last form feed. The line end
/// is CR LF: the reference opens the file in Windows text mode, which writes each line end as CR LF (spec 04d-B
/// item 1, inferred from the open mode). The characters are the cells as the pages hold them; the caller maps them
/// to the chosen encoding.
pub fn formatted_text(pages: &[Page]) -> String {
    let mut out = String::new();
    for page in pages {
        for line in &page.lines {
            out.push_str(line.text.trim_end_matches(' '));
            out.push_str("\r\n");
        }
        out.push('\u{c}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(page: &Page) -> Vec<&str> {
        page.lines.iter().map(|l| l.text.as_str()).collect()
    }

    /// A document whose bracketed runs are codes.
    fn m(text: &str) -> Shown {
        Shown::marked(text)
    }

    #[test]
    fn places_with_no_cell_stand_where_the_pen_was() {
        // A space at the end of a line (the line does not show it) stands after the line's last letter; so does a
        // second space; the next word is on the next line.
        let pages = format(&m("one  two"), &Layout::braille(5, 4), None);
        assert_eq!(texts(&pages[0]), ["one", "two", "", ""]);
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 4), (1, 1, 4));
        assert_eq!(locate(&pages, 5), (1, 2, 1));
        assert_eq!(position_of(&pages, 1, 1, 4), Some(3));
        // The end of a document whose last line is empty stands on that empty line, and back.
        let pages = format(&m("abc\n"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0]), ["abc", "", ""]);
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 4), (1, 2, 1));
        assert_eq!(position_of(&pages, 1, 2, 1), Some(4));
        // A code inside a word stands before the letter after it.
        let pages = format(&m("ab[x]cd"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0])[0], "abcd");
        assert_eq!(locate(&pages, 2), (1, 1, 3));
        assert_eq!(locate(&pages, 7), (1, 1, 5));
        // Marks never add a page: a document ending at a page code keeps its one page.
        let pages = format(&m("a[pg]"), &Layout::braille(10, 3), None);
        assert_eq!(pages.len(), 1);
        assert_eq!(locate(&pages, 5), (1, 1, 2));
    }

    #[test]
    fn words_wrap_at_the_width_and_a_new_line_code_ends_a_line() {
        let pages = format(&m("aaa bbb ccc\nddd"), &Layout::braille(8, 4), None);
        assert_eq!(pages.len(), 1);
        assert_eq!(texts(&pages[0]), ["aaa bbb", "ccc", "ddd", ""]);
    }

    #[test]
    fn a_braille_paragraph_starts_at_the_third_cell_and_runover_follows_the_margin() {
        let pages = format(&m("[p]\naaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "ccc", ""]);
        // Margins count cells from 1: [ind3] is the third cell; [ptys] alone runs over two cells further in.
        let pages = format(&m("[ind3][ptys2]aaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "    ccc", ""]);
        let pages = format(&m("[ind1][ptys]aaa bbb ccc"), &Layout::braille(8, 3), None);
        assert_eq!(texts(&pages[0]), ["aaa bbb", "  ccc", ""]);
        // [ind2:1] is two cells on from the current margin.
        let pages = format(&m("[ind3]a[ind2:1]b"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["  a", "    b"]);
    }

    #[test]
    fn pages_break_when_full_and_at_a_page_code_with_the_number_in_its_corner() {
        let mut layout = Layout::braille(10, 3);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::LowerRight;
        // The number shares the last line with text, which keeps three cells clear of it.
        let pages = format(&m("a\nb\nc[pg]d"), &layout, None);
        assert_eq!(pages.len(), 2);
        assert_eq!(texts(&pages[0]), ["a", "b", "c       #a"]);
        assert_eq!(texts(&pages[1]), ["d", "", "        #b"]);
        let pages = format(&m("a\nb\nccc dd"), &layout, None);
        assert_eq!(texts(&pages[0]), ["a", "b", "ccc     #a"]);
        assert_eq!(texts(&pages[1])[0], "dd");
        // At the upper left, text on the first line starts three cells after the number; [pgN~X] numbers the next
        // page N with X before it; a print document's number is in digits.
        layout.numbers_odd = Corner::UpperLeft;
        layout.numbers_even = Corner::UpperLeft;
        let pages = format(&m("a[pg9~p]b"), &layout, None);
        assert_eq!(texts(&pages[0])[0], "#a   a");
        assert_eq!(texts(&pages[1])[0], "p#i   b");
        let mut print = Layout::print(10, 2);
        print.numbers_odd = Corner::LowerRight;
        assert_eq!(texts(&format(&m("a"), &print, None)[0]), ["a", "         1"]);
        // [svpnp] changes where numbers go from the next page.
        let pages = format(&m("a [svpnp0:0]b[pg]c"), &layout, None);
        assert_eq!(texts(&pages[0])[0], "#a   a b");
        assert_eq!(texts(&pages[1])[0], "c");
    }

    #[test]
    fn a_heading_is_centred_and_a_word_longer_than_a_line_is_broken() {
        let pages = format(&m("[hds]ab[hde]abcdefghij"), &Layout::braille(6, 4), None);
        assert_eq!(texts(&pages[0]), ["  ab", "abcdef", "ghij", ""]);
    }

    #[test]
    fn styles_expand_to_their_codes() {
        let styles = StyleTable::from_text([("para.", "[p]", ""), ("h1.", "[hds]", "[hde]")]);
        let pages = format(&m("[es~h1.]Title[ee~h1.][es~para.]aa bb[ee~para.]"), &Layout::braille(9, 3), Some(&styles));
        assert_eq!(texts(&pages[0]), ["  Title", "  aa bb", ""]);
    }

    #[test]
    fn a_bracket_cell_is_text_and_a_paragraph_column_ignores_the_margin_and_centring_rounds_down() {
        // Brackets in the text, even ones that look like a code, are text.
        let pages = format(&Shown::plain("d[n by] \"[hds2]\" x"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "d[n by] \"[hds2]\" x");
        let pages = format(&m("[ind4][p]\nab"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "  ab");
        let pages = format(&m("[hds]abc[hde]"), &Layout::braille(6, 2), None);
        assert_eq!(texts(&pages[0])[0], " abc");
        // Centred on the whole line, not inside the right margin.
        let pages = format(&m("[rm4][hds]ab[hde]"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "    ab");
    }

    /// The shipped sample documents (when a folder of them is named in BC_DX_SAMPLES) lay out, as print and as
    /// braille, with every character of their text in one cell, in order, on full pages no wider than the page.
    #[test]
    fn every_sample_document_lays_out_with_all_its_text_in_order() {
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext != "dxp" && ext != "dxb" {
                continue;
            }
            let doc = crate::dxfile::read(&std::fs::read(&p).unwrap()).unwrap();
            let doc = Shown::from_running(&doc.text);
            // Every character of the text, except a running head's or foot's, which is not body text.
            let mut running = false;
            let expected: Vec<usize> = pieces(&doc, None)
                .iter()
                .filter_map(|piece| match piece {
                    Piece::Code(c) if matches!(split_code(c).0, "tls" | "rfs") => {
                        running = true;
                        None
                    }
                    Piece::Code(c) if matches!(split_code(c).0, "tle" | "rfe") => {
                        running = false;
                        None
                    }
                    Piece::Char(_, at) if !running => Some(*at),
                    _ => None,
                })
                .collect();
            for layout in [Layout::print(80, 50), Layout::braille(40, 25)] {
                let pages = format(&doc, &layout, None);
                let placed: Vec<usize> =
                    pages.iter().flat_map(|pg| &pg.lines).flat_map(|l| l.sources.iter().flatten().copied()).collect();
                if placed != expected {
                    let k = placed.iter().zip(&expected).position(|(a, b)| a != b).unwrap_or(placed.len().min(expected.len()));
                    let at = expected.get(k).or(placed.get(k)).copied().unwrap_or(0);
                    let around: String = doc.text.chars().skip(at.saturating_sub(40)).take(80).collect();
                    panic!(
                        "{}: {} placed, {} expected; first difference at item {k} (placed {:?}, expected {:?}) near {around:?}",
                        p.display(),
                        placed.len(),
                        expected.len(),
                        placed.get(k),
                        expected.get(k)
                    );
                }
                for page in &pages {
                    assert_eq!(page.lines.len(), layout.lines, "{}", p.display());
                    assert!(page.lines.iter().all(|l| l.text.chars().count() <= layout.cells), "{}", p.display());
                }
            }
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn a_position_is_found_on_its_page_line_and_column() {
        let pages = format(&m("aaa bbb ccc\nddd"), &Layout::braille(8, 2), None);
        assert_eq!(locate(&pages, 0), (1, 1, 1));
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 5), (1, 1, 6));
        assert_eq!(locate(&pages, 8), (1, 2, 1));
        // The line end after "ccc": after its last cell, not at the start of the next page.
        assert_eq!(locate(&pages, 11), (1, 2, 4));
        assert_eq!(locate(&pages, 12), (2, 1, 1));
        assert_eq!(locate(&pages, 99), (2, 1, 4));
        let pages = format(&m("[p]\nab"), &Layout::braille(8, 2), None);
        assert_eq!(locate(&pages, 0), (1, 1, 3));
    }

    #[test]
    fn a_formatted_file_has_trimmed_lines_and_a_form_feed_after_every_page() {
        let pages = format(&m("[p]\nab cd[pg]ef"), &Layout::braille(8, 2), None);
        assert_eq!(formatted_text(&pages), "  ab cd\r\n\r\n\u{c}ef\r\n\r\n\u{c}");
    }

    #[test]
    fn keep_blocks_start_and_end_lines_and_skip_follows_text_only() {
        let pages = format(&m("ab[kps]cd[kpe]ef"), &Layout::braille(20, 5), None);
        assert_eq!(&texts(&pages[0])[..3], ["ab", "cd", "ef"]);
        // [sc1] skips a line after text, none after a blank line or at the top.
        let pages = format(&m("[sc1]ab[sc1]cd[sk1][sc1]ef"), &Layout::braille(20, 8), None);
        assert_eq!(&texts(&pages[0])[..5], ["ab", "", "cd", "", "ef"]);
    }

    /// Edits laid out again from their page give the same pages as laying the whole document out.
    fn check_edits(doc: &Shown, layout: &Layout, edits: usize) {
        let (mut pages, mut trail) = format_traced(doc, layout, None);
        let mut doc = doc.clone();
        let puts = ["x", " ", "\n", "[p]\n", "[pg]", "word ", "[hds]Head[hde]", "", "  ", "[ind4]a b c d e f g"];
        let mut seed: u64 = 12345;
        for k in 0..edits {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let len = doc.text.chars().count();
            let from = (seed >> 33) as usize % (len + 1);
            let take = if k % 3 == 0 { (seed >> 20) as usize % 6 } else { 0 };
            let mut to = (from + take).min(len);
            // Keep the codes whole, as the editor does.
            let mut from = from;
            for &(s, e) in &doc.codes {
                if s < from && e > from {
                    from = s;
                }
                if s < to && e > to {
                    to = e;
                }
            }
            let put = Shown::marked(puts[k % puts.len()]);
            let mut next = doc.slice(0, from);
            next.push(&put);
            next.push(&doc.slice(to, len));
            let new_to = from + put.text.chars().count();
            let (p, t) = reformat(pages, trail, &next, layout, None, from, to, new_to);
            let whole = format(&next, layout, None);
            assert_eq!(t.pieces, pieces(&next, None), "edit {k}: {from}..{to} -> {:?}", put.text);
            assert_eq!(p, whole, "edit {k}: {from}..{to} -> {:?}", put.text);
            pages = p;
            trail = t;
            doc = next;
        }
    }

    #[test]
    fn an_edit_laid_out_from_its_page_gives_the_whole_layout() {
        let text: String = (0..300)
            .map(|i| match i % 17 {
                0 => "[p]\n".to_string(),
                5 => "[hds]Part[hde]".to_string(),
                9 => "[sk1]".to_string(),
                _ => format!("word{} ", i % 7),
            })
            .collect();
        let mut layout = Layout::braille(20, 6);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::UpperLeft;
        check_edits(&m(&text), &layout, 300);
        check_edits(&m(&text), &Layout::print(30, 8), 200);
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        for entry in std::fs::read_dir(dir).unwrap().flatten().take(3) {
            let p = entry.path();
            if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dxb") || e.eq_ignore_ascii_case("dxp")) {
                continue;
            }
            let doc = crate::dxfile::read(&std::fs::read(&p).unwrap()).unwrap();
            check_edits(&Shown::from_running(&doc.text), &Layout::braille(40, 25), 30);
        }
    }

    #[test]
    fn code_text_is_laid_out_and_a_code_ends_a_word() {
        // [q~X] is cells of its word, [d~X] cells that end the word.
        let pages = format(&m("aa [q~,']b [d~xx]yy"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "aa ,'b xx yy");
        // The two parts of a word a code ends touch, but a line may end between them.
        let pages = format(&m("abc[x]de"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "abcde");
        let pages = format(&m("abc[x]de"), &Layout::braille(4, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["abc", "de"]);
    }

    #[test]
    fn running_head_text_is_not_body_text() {
        let pages = format(&m("ab [tls]Head Line[tle] cd ef[rfs]Foot[rfe]"), &Layout::braille(20, 3), None);
        assert_eq!(texts(&pages[0])[0], "ab cd ef");
        assert!(texts(&pages[0]).iter().all(|l| !l.contains("Head") && !l.contains("Foot")));
    }
}
