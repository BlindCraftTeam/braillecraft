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
//!   [indP] new line if not at the start of one, left margin P, runover keeping its offset; [inmP] left margin
//!   without a new line; [ptysN] runover = left + N; [ptye] runover = left; [rmN] right margin.
//! - [<] new line always (a blank line when already at the start of one); [l] new line unless at the start of one;
//!   [p] new paragraph: new line, then (print documents) one blank line, the first line starting at the third cell of
//!   a braille document and the first of a print document, counted from the edge of the page (F0b, from the braille
//!   and print formatting files); [skN] [l] then N blank lines; [svsblN] N blank lines between text lines.
//! - Words that do not fit go to the next line at the runover margin. Extra spaces are left out.
//! - [pg] new page always (two in a row give a blank page); [top] new page if the page has text; [cpN:L] new page if
//!   fewer than N + 1 lines remain, else a new line and L blank lines (braille 1 and 0, print 2 and 1, F0b).
//! - [hdsN:L] ... [hde] centred heading: on a new page if fewer than N + 1 lines remain (braille 1, print 2), else
//!   after L blank lines (braille 0, print 1); each heading line starts at (width - length + 1) / 2 (F0b).
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
#[derive(Debug, Clone)]
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
}

/// One laid-out page: all its lines (blank ones included, up to the lines per page) and the document position its
/// text starts at.
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub lines: Vec<Line>,
    pub start: usize,
}

/// A piece of the document: a character with its position (in characters), a space, or a code.
#[derive(Debug, Clone)]
enum Piece {
    Char(char, usize),
    Space,
    Code(String),
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
            expand(&code, styles, &mut out, 0);
            i = end;
            continue;
        }
        let c = chars[i];
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
    out
}

/// A code as pieces: a style code becomes its begin or end codes (themselves codes in brackets), anything else
/// stays a code. `depth` stops a style that names itself.
fn expand(code: &str, styles: Option<&StyleTable>, out: &mut Vec<Piece>, depth: usize) {
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
                expand(&rest[s + 1..s + e], styles, out, depth + 1);
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

/// The braille page number: the numeric indicator and the digits as the letters a to j.
fn braille_number(n: usize) -> String {
    let mut s = String::from("#");
    for d in n.to_string().chars() {
        s.push(b"jabcdefghi"[d.to_digit(10).unwrap_or(0) as usize] as char);
    }
    s
}

struct Formatter<'a> {
    layout: &'a Layout,
    pages: Vec<Page>,
    /// Lines of the page being filled (text lines only; number lines are added when the page is finished).
    page: Vec<Line>,
    page_start: Option<usize>,
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
}

impl<'a> Formatter<'a> {
    fn new(layout: &'a Layout) -> Formatter<'a> {
        Formatter {
            layout,
            pages: Vec::new(),
            page: Vec::new(),
            page_start: None,
            line: Line::default(),
            start_col: 0,
            left: 0,
            runover_offset: 0,
            right: 0,
            blank_between: 0,
            flush_right: false,
            heading: None,
            after_page_code: false,
        }
    }

    fn width(&self) -> usize {
        self.layout.cells.saturating_sub(self.right).max(1)
    }

    /// The number of the page being filled (1-based) and the corner its number goes in.
    fn corner(&self) -> Corner {
        let n = self.pages.len() + 1;
        if n < self.layout.first_number_shown {
            return Corner::None;
        }
        if n % 2 == 0 {
            self.layout.numbers_even
        } else {
            self.layout.numbers_odd
        }
    }

    /// Text lines a page holds: the lines per page less a line for a number at the top or bottom.
    fn capacity(&self) -> usize {
        let c = self.corner();
        self.layout.lines.saturating_sub(usize::from(c.top()) + usize::from(c.bottom())).max(1)
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
        let had_text = line.sources.iter().any(Option::is_some);
        self.push_line(line);
        if had_text {
            for _ in 0..self.blank_between {
                self.push_line(Line::default());
            }
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
            return;
        }
        self.page.push(line);
        if self.page.len() >= self.capacity() {
            self.finish_page();
        }
    }

    fn finish_page(&mut self) {
        let corner = self.corner();
        let number = self.pages.len() + 1;
        let mut lines = std::mem::take(&mut self.page);
        let room = self.capacity();
        lines.resize(room, Line::default());
        let number_line = || {
            let text = if self.layout.braille { braille_number(number) } else { number.to_string() };
            let pad = if corner.right() { self.layout.cells.saturating_sub(text.chars().count()) } else { 0 };
            let text = format!("{}{text}", " ".repeat(pad));
            let sources = vec![None; text.chars().count()];
            Line { text, sources }
        };
        if corner.top() {
            lines.insert(0, number_line());
        }
        if corner.bottom() {
            lines.push(number_line());
        }
        let start = self.page_start.take().unwrap_or_else(|| self.pages.last().map_or(0, |p| p.start));
        self.pages.push(Page { lines, start });
    }

    fn new_page(&mut self) {
        if self.line_has_text() {
            self.end_line(self.left);
        }
        self.finish_page();
        self.start_col = self.left;
    }

    /// Places one word (its cells and their sources), wrapping it to the runover margin when it does not fit.
    fn word(&mut self, cells: &[(char, Option<usize>)]) {
        if let Some(h) = &mut self.heading {
            h.0.push((cells.iter().map(|c| c.0).collect(), cells.iter().map(|c| c.1).collect()));
            return;
        }
        let width = self.width();
        if self.flush_right {
            self.flush_right = false;
            let len = cells.len();
            let used = self.line.text.chars().count();
            if used + 1 + len > self.layout.cells {
                self.end_line(self.left + self.runover_offset);
            }
            let used = self.line.text.chars().count();
            for _ in used..self.layout.cells.saturating_sub(len) {
                self.put(' ', None);
            }
            for &(c, s) in cells {
                self.put(c, s);
            }
            return;
        }
        let mut rest = cells;
        while !rest.is_empty() {
            let used = self.line.text.chars().count();
            let has_text = self.line_has_text();
            let start = if has_text { used + 1 } else { self.start_col.max(used) };
            if start + rest.len() > width && has_text {
                self.end_line(self.left + self.runover_offset);
                continue;
            }
            let fits = width.saturating_sub(start).max(1);
            if has_text {
                self.put(' ', None);
            } else {
                for _ in used..start {
                    self.put(' ', None);
                }
            }
            let (now, later) = rest.split_at(rest.len().min(fits));
            for &(c, s) in now {
                self.put(c, s);
            }
            rest = later;
            if !rest.is_empty() {
                self.end_line(self.left + self.runover_offset);
            }
        }
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
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                for _ in 0..first.unwrap_or(1).max(0) {
                    self.push_line(Line::default());
                }
            }
            "ind" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                self.left = first.unwrap_or(0).max(0) as usize;
                self.start_col = self.left;
            }
            "inm" => {
                self.left = first.unwrap_or(0).max(0) as usize;
                if !self.line_has_text() {
                    self.start_col = self.left;
                }
            }
            "ptys" => self.runover_offset = first.unwrap_or(0).max(0) as usize,
            "ptye" => self.runover_offset = 0,
            "rm" => self.right = first.unwrap_or(0).max(0) as usize,
            "svsbl" => self.blank_between = first.unwrap_or(0).max(0) as usize,
            "pg" => {
                if self.after_page_code && !self.page_has_text() {
                    // Two page codes in a row: a blank page.
                    self.finish_page();
                } else {
                    self.new_page();
                }
                self.after_page_code = true;
            }
            "top" => {
                if self.page_has_text() {
                    self.new_page();
                }
            }
            "fr" => self.flush_right = true,
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
        let width = self.width();
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
            // A centred line starts at (width - length + 1) / 2 (spec 19 F0b).
            let pad = (width + 1).saturating_sub(len) / 2;
            let mut centred = Line { text: " ".repeat(pad), sources: vec![None; pad] };
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

    fn run(mut self, pieces: &[Piece]) -> Vec<Page> {
        let mut word: Vec<(char, Option<usize>)> = Vec::new();
        for p in pieces {
            match p {
                Piece::Char(c, at) => word.push((if *c == '\u{a0}' { ' ' } else { *c }, Some(*at))),
                Piece::Space => {
                    if !word.is_empty() {
                        self.word(&std::mem::take(&mut word));
                    }
                }
                Piece::Code(code) => {
                    // Codes the formatter does not lay out take no room and do not break a word.
                    if is_layout(code) {
                        if !word.is_empty() {
                            self.word(&std::mem::take(&mut word));
                        }
                        self.code(code);
                    }
                }
            }
        }
        if !word.is_empty() {
            self.word(&word);
        }
        if let Some((words, need, skip)) = self.heading.take() {
            self.heading_lines(&words, need, skip);
        }
        if self.line_has_text() || !self.line.text.is_empty() {
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
    )
}

/// Lays out a document (in the editor's form) into pages, with the document's styles.
pub fn format(doc: &Shown, layout: &Layout, styles: Option<&StyleTable>) -> Vec<Page> {
    Formatter::new(layout).run(&pieces(doc, styles))
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
    if let Some(s) = ln.sources.iter().flatten().last() {
        return Some(s + 1);
    }
    let later = pages[page - 1].lines[line..].iter().chain(pages[page..].iter().flat_map(|p| &p.lines));
    later.flat_map(|l| l.sources.iter().flatten()).next().copied()
}

/// The page, line and column (all 1-based, the line counted on its page) of a character position in the document:
/// the cell that came from that position; for a position that is no cell (a space, a line end, a code), the place
/// just after the last cell before it, as a caret at the end of a line stands after its last cell; before any cell,
/// the first cell.
pub fn locate(pages: &[Page], pos: usize) -> (usize, usize, usize) {
    let mut before: Option<(usize, usize, usize)> = None;
    for (p, page) in pages.iter().enumerate() {
        for (l, line) in page.lines.iter().enumerate() {
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
    fn words_wrap_at_the_width_and_a_new_line_code_ends_a_line() {
        let pages = format(&m("aaa bbb ccc\nddd"), &Layout::braille(8, 4), None);
        assert_eq!(pages.len(), 1);
        assert_eq!(texts(&pages[0]), ["aaa bbb", "ccc", "ddd", ""]);
    }

    #[test]
    fn a_braille_paragraph_starts_at_the_third_cell_and_runover_follows_the_margin() {
        let pages = format(&m("[p]\naaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "ccc", ""]);
        let pages = format(&m("[ind2][ptys2]aaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "    ccc", ""]);
    }

    #[test]
    fn pages_break_when_full_and_at_a_page_code_with_the_number_in_its_corner() {
        let mut layout = Layout::braille(10, 3);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::LowerRight;
        let pages = format(&m("a\nb\nc[pg]d"), &layout, None);
        assert_eq!(pages.len(), 3);
        assert_eq!(texts(&pages[0]), ["a", "b", "        #a"]);
        assert_eq!(texts(&pages[1]), ["c", "", "        #b"]);
        assert_eq!(texts(&pages[2]), ["d", "", "        #c"]);
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
    fn a_bracket_cell_is_text_and_a_paragraph_column_ignores_the_margin_and_centring_rounds_up() {
        // Brackets in the text, even ones that look like a code, are text.
        let pages = format(&Shown::plain("d[n by] \"[hds2]\" x"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "d[n by] \"[hds2]\" x");
        let pages = format(&m("[ind4][p]\nab"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "  ab");
        let pages = format(&m("[hds]abc[hde]"), &Layout::braille(6, 2), None);
        assert_eq!(texts(&pages[0])[0], "  abc");
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
            let expected: Vec<usize> = pieces(&doc, None)
                .iter()
                .filter_map(|piece| if let Piece::Char(_, at) = piece { Some(*at) } else { None })
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
}
