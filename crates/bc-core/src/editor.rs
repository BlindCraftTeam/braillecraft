//! The editing area's model, as the reference program's editor works (spec 27 section 5): the caret is a place in
//! the document, not in the text on the screen, and the screen shows either the formatted page lines (each page's
//! lines, then a separator row) or the coded rows. Where a place shows is found through the laid-out pages: the cell
//! that came from it, or where the formatter's pen stood at it (format::Line::marks), so a caret can stand on a space
//! the line does not show or on an empty last line, and what is typed goes in at that place.
//!
//! A code is one unit for the caret and for deleting: the caret goes over it at once, and Backspace after it or
//! Delete before it takes the whole code (with the editor's own line break after a [p] or [l]), though the formatted
//! view does not show it (spec 27 5a).

use crate::doctext::Shown;
use crate::engine::StyleTable;
use crate::format::{format_traced, locate, position_of, reformat, Layout, Page, Trail};
use crate::fview::View;

/// A caret movement (spec 27 5a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Right,
    Left,
    Up,
    Down,
    /// To the first cell of the line (the row in the coded view).
    Home,
    /// To just after the last cell of the line (the row's end in the coded view).
    End,
    WordRight,
    WordLeft,
    /// The start of the document, past the codes at its start.
    DocStart,
    DocEnd,
    /// To the previous or next paragraph code.
    ParagraphUp,
    ParagraphDown,
}

/// What a screen row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// Line `line` (0-based) of page `page`.
    Line { page: usize, line: usize },
    /// The page break row after page `page`'s lines (formatted view).
    Break { page: usize },
    /// Row `row` of the coded view.
    Coded { row: usize },
}

#[derive(Debug, Clone)]
pub struct Editor {
    pub doc: Shown,
    pub layout: Layout,
    pub styles: Option<StyleTable>,
    /// The coded view (View, Codes) and its row width (65 for a print document, 42 for a braille one).
    pub coded: bool,
    pub coded_width: usize,
    /// The laid-out pages (formatted view; empty in the coded view).
    pub pages: Vec<Page>,
    /// The text screen readers get: the formatted page lines, or the coded rows (spec 27 section 6).
    pub view: View,
    pub caret: usize,
    pub anchor: usize,
    /// The column Up and Down keep to (spec 27 5a, view+0x50).
    want: Option<usize>,
    /// The first screen row of each page (formatted view): its lines, then its page break row.
    page_rows: Vec<usize>,
    len: usize,
    /// What lets the formatted view be laid out again from the page an edit is on.
    trail: Trail,
}

impl Editor {
    pub fn new(doc: Shown, layout: Layout, styles: Option<StyleTable>, coded: bool, coded_width: usize) -> Editor {
        let mut e = Editor {
            doc,
            layout,
            styles,
            coded,
            coded_width,
            pages: Vec::new(),
            view: View::default(),
            caret: 0,
            anchor: 0,
            want: None,
            page_rows: Vec::new(),
            len: 0,
            trail: Trail::default(),
        };
        e.relayout();
        e
    }

    /// Lays the document out again (or its coded rows) after a change.
    pub fn relayout(&mut self) {
        self.relayout_edit(None);
    }

    /// Lays the document out again; after an edit that replaced places `from` to `old_to` with `from` to `new_to`,
    /// from the page the edit is on (spec 27 5d).
    fn relayout_edit(&mut self, edit: Option<(usize, usize, usize)>) {
        self.len = self.doc.text.chars().count();
        if self.coded {
            self.pages = Vec::new();
            self.trail = Trail::default();
            self.view = View::coded(&self.doc, self.coded_width);
            self.page_rows = Vec::new();
        } else {
            let (pages, trail) = match edit {
                Some((from, old_to, new_to)) => reformat(
                    std::mem::take(&mut self.pages),
                    std::mem::take(&mut self.trail),
                    &self.doc,
                    &self.layout,
                    self.styles.as_ref(),
                    from,
                    old_to,
                    new_to,
                ),
                None => format_traced(&self.doc, &self.layout, self.styles.as_ref()),
            };
            self.pages = pages;
            self.trail = trail;
            self.view = View::new(&self.pages);
            let mut at = 0;
            self.page_rows = self
                .pages
                .iter()
                .map(|p| {
                    let first = at;
                    at += p.lines.len() + 1;
                    first
                })
                .collect();
        }
        self.caret = self.caret.min(self.len);
        self.anchor = self.anchor.min(self.len);
    }

    /// Puts another document in, the caret at `caret`.
    pub fn set_doc(&mut self, doc: Shown, caret: usize) {
        self.doc = doc;
        self.relayout();
        self.set_caret(caret);
    }

    pub fn set_view(&mut self, coded: bool) {
        self.coded = coded;
        self.want = None;
        self.relayout();
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The selection, start before end (equal when there is none).
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn set_caret(&mut self, pos: usize) {
        self.caret = self.out_of_code(pos.min(self.len));
        self.anchor = self.caret;
        self.want = None;
    }

    pub fn select(&mut self, from: usize, to: usize) {
        self.anchor = self.out_of_code(from.min(self.len));
        self.caret = self.out_of_code(to.min(self.len));
        self.want = None;
    }

    /// The number of screen rows.
    pub fn rows(&self) -> usize {
        if self.coded {
            self.view.lines.len()
        } else {
            self.pages.iter().map(|p| p.lines.len() + 1).sum()
        }
    }

    /// What screen row `row` shows.
    pub fn row(&self, row: usize) -> Option<Row> {
        if self.coded {
            return (row < self.view.lines.len()).then_some(Row::Coded { row });
        }
        let page = self.page_rows.partition_point(|&r| r <= row).checked_sub(1)?;
        let line = row - self.page_rows[page];
        let lines = self.pages[page].lines.len();
        match line.cmp(&lines) {
            std::cmp::Ordering::Less => Some(Row::Line { page, line }),
            std::cmp::Ordering::Equal => Some(Row::Break { page }),
            std::cmp::Ordering::Greater => None,
        }
    }

    /// The text of screen row `row` (empty for a page break row).
    pub fn row_text(&self, row: usize) -> String {
        match self.row(row) {
            Some(Row::Line { page, line }) => self.pages[page].lines[line].text.clone(),
            Some(Row::Coded { row }) => {
                let start = self.view.lines[row].2;
                let end = self.view.lines.get(row + 1).map_or(self.view.text.chars().count(), |l| l.2);
                self.view.text.chars().skip(start).take(end - start).filter(|&c| c != '\n').collect()
            }
            _ => String::new(),
        }
    }

    /// The document place that each character of row `row` came from (None for cells the formatter added). In the
    /// coded view every character is the document's own.
    pub fn row_sources(&self, row: usize) -> Vec<Option<usize>> {
        match self.row(row) {
            Some(Row::Line { page, line }) => self.pages[page].lines[line].sources.clone(),
            Some(Row::Coded { row }) => {
                let start = self.view.lines[row].2;
                let n = self.row_text(row).chars().count();
                (0..n).map(|k| Some(self.view.to_document(&[], start + k))).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The first screen row of page `page` (formatted view).
    pub fn page_row(&self, page: usize) -> usize {
        self.page_rows.get(page).copied().unwrap_or(0)
    }

    /// Where document place `pos` shows: its screen row and column (0-based).
    pub fn place(&self, pos: usize) -> (usize, usize) {
        if self.coded {
            let at = self.view.to_view(&[], pos);
            return self.view.line_at(at).unwrap_or((0, 0));
        }
        if self.pages.is_empty() {
            return (0, 0);
        }
        let (p, l, c) = locate(&self.pages, pos);
        (self.page_rows[p - 1] + l - 1, c - 1)
    }

    /// The caret's screen row and column.
    pub fn caret_place(&self) -> (usize, usize) {
        self.place(self.caret)
    }

    /// The document place at screen row `row`, column `col` (a click, or Up and Down to the wanted column, spec 27
    /// 5c): the cell there, else the first cell after it on the row, else where the line ends. A page break row
    /// counts as the line above it.
    pub fn pos_at(&self, row: usize, col: usize) -> usize {
        match self.row(row) {
            Some(Row::Coded { row }) => {
                let start = self.view.lines[row].2;
                let n = self.row_text(row).chars().count();
                self.view.to_document(&[], start + col.min(n))
            }
            Some(Row::Line { page, line }) => position_of(&self.pages, page + 1, line + 1, col + 1).unwrap_or(self.len),
            Some(Row::Break { page }) => {
                let line = self.pages[page].lines.len().saturating_sub(1);
                position_of(&self.pages, page + 1, line + 1, col + 1).unwrap_or(self.len)
            }
            None => self.len,
        }
    }

    /// The code (range from its opening bracket to just after its closing one, with the editor's own line break
    /// after a [p] or [l]) that starts at `pos`.
    fn unit_from(&self, pos: usize) -> Option<(usize, usize)> {
        let &(s, e) = self.doc.codes.iter().find(|c| c.0 == pos)?;
        Some((s, if self.doc.is_own_break(e) { e + 1 } else { e }))
    }

    /// The code that ends at `pos` (its own line break included).
    fn unit_to(&self, pos: usize) -> Option<(usize, usize)> {
        if self.doc.is_own_break(pos.checked_sub(1)?) {
            let &(s, _) = self.doc.codes.iter().find(|c| c.1 == pos - 1)?;
            return Some((s, pos));
        }
        let &(s, e) = self.doc.codes.iter().find(|c| c.1 == pos)?;
        Some((s, e))
    }

    /// A place inside a code goes to the code's start (the caret never stands inside a code).
    fn out_of_code(&self, pos: usize) -> usize {
        if let Some(&(s, _)) = self.doc.codes.iter().find(|c| c.0 < pos && pos < c.1) {
            return s;
        }
        // Between a [p] or [l] and the editor's own line break after it: the code's start.
        if self.doc.is_own_break(pos) {
            if let Some(&(s, _)) = self.doc.codes.iter().find(|c| c.1 == pos) {
                return s;
            }
        }
        pos
    }

    fn step_right(&self, pos: usize) -> usize {
        match self.unit_from(pos) {
            Some((_, e)) => e,
            None => (pos + 1).min(self.len),
        }
    }

    fn step_left(&self, pos: usize) -> usize {
        match self.unit_to(pos) {
            Some((s, _)) => s,
            None => pos.saturating_sub(1),
        }
    }

    fn char_at(&self, pos: usize) -> Option<char> {
        self.doc.text.chars().nth(pos)
    }

    fn is_code(&self, pos: usize) -> bool {
        self.doc.codes.iter().any(|c| c.0 <= pos && pos < c.1)
    }

    /// Whether row `row` has a cell from the document (Up and Down pass over the others, spec 27 5a).
    fn row_has_text(&self, row: usize) -> bool {
        match self.row(row) {
            Some(Row::Line { page, line }) => self.pages[page].lines[line].sources.iter().any(Option::is_some),
            Some(Row::Coded { .. }) => true,
            _ => false,
        }
    }

    /// Moves the caret; with `extend`, the selection's other end stays where it was (Shift).
    pub fn move_caret(&mut self, m: Move, extend: bool) {
        let keep_want = matches!(m, Move::Up | Move::Down);
        let to = match m {
            Move::Right => self.step_right(self.caret),
            Move::Left => self.step_left(self.caret),
            Move::Up | Move::Down => {
                let (row, col) = self.caret_place();
                let want = *self.want.get_or_insert(col);
                let rows = self.rows();
                let mut r = row as i64;
                let target = loop {
                    r += if m == Move::Up { -1 } else { 1 };
                    if r < 0 || r as usize >= rows {
                        break None;
                    }
                    if self.row_has_text(r as usize) {
                        break Some(r as usize);
                    }
                };
                match target {
                    Some(r) => self.pos_at(r, want),
                    None => self.caret,
                }
            }
            Move::Home => {
                let (row, _) = self.caret_place();
                match self.row(row) {
                    Some(Row::Line { page, line }) => {
                        self.pages[page].lines[line].sources.iter().flatten().next().copied().unwrap_or(self.caret)
                    }
                    _ => self.pos_at(row, 0),
                }
            }
            Move::End => {
                let (row, _) = self.caret_place();
                self.pos_at(row, self.row_text(row).chars().count())
            }
            Move::WordRight => {
                let mut p = self.caret;
                while p < self.len && !self.char_at(p).is_some_and(char::is_whitespace) {
                    p = self.step_right(p);
                }
                while p < self.len && (self.char_at(p).is_some_and(char::is_whitespace) || self.is_code(p)) {
                    p = self.step_right(p);
                }
                p
            }
            Move::WordLeft => {
                let mut p = self.caret;
                while p > 0 && (self.char_at(p - 1).is_some_and(char::is_whitespace) || self.unit_to(p).is_some()) {
                    p = self.step_left(p);
                }
                while p > 0 && !self.char_at(p - 1).is_some_and(char::is_whitespace) && self.unit_to(p).is_none() {
                    p = self.step_left(p);
                }
                p
            }
            Move::DocStart => {
                let mut p = 0;
                while let Some((_, e)) = self.unit_from(p) {
                    p = e;
                }
                p
            }
            Move::DocEnd => self.len,
            Move::ParagraphDown => self.paragraph_codes().into_iter().find(|&(s, _)| s >= self.caret).map_or(self.len, |(_, e)| e),
            Move::ParagraphUp => {
                // The paragraph code before the one the caret is just after.
                let before: Vec<(usize, usize)> = self.paragraph_codes().into_iter().filter(|&(_, e)| e < self.caret).collect();
                before.last().map_or(0, |&(_, e)| e)
            }
        };
        self.caret = to.min(self.len);
        if !extend {
            self.anchor = self.caret;
        }
        if !keep_want {
            self.want = None;
        }
    }

    /// The paragraph codes ([p], and style begin codes), each as the unit the caret goes over. INFERENCE (spec 27
    /// 5a: which codes the reference counts as paragraph codes is not traced).
    fn paragraph_codes(&self) -> Vec<(usize, usize)> {
        self.doc
            .codes
            .iter()
            .filter(|c| self.doc.code_at(c.0).is_some_and(|n| n == "p" || n.starts_with("es~")))
            .filter_map(|c| self.unit_from(c.0))
            .collect()
    }

    /// Replaces document places `from` to `to` with `put`; a code the range cuts into goes as a whole, with its own
    /// line break. The caret goes after `put`. Returns what was taken out.
    pub fn replace(&mut self, from: usize, to: usize, put: &Shown) -> Shown {
        let (mut from, mut to) = (from.min(self.len), to.min(self.len));
        for &(s, e) in &self.doc.codes {
            if s < from && e > from {
                from = s;
            }
            if s < to && e > to {
                to = e;
            }
        }
        // A code's own line break goes with it, and the code with its own line break.
        if to > 0 && self.doc.is_own_break(to) {
            to += 1;
        }
        if let Some((s, _)) = self.unit_to(from + 1).filter(|_| self.doc.is_own_break(from)) {
            from = s;
        }
        let removed = self.doc.slice(from, to);
        let mut doc = self.doc.slice(0, from);
        doc.push(put);
        doc.push(&self.doc.slice(to, self.len));
        self.doc = doc;
        self.relayout_edit(Some((from, to, from + put.text.chars().count())));
        self.caret = from + put.text.chars().count();
        self.anchor = self.caret;
        self.want = None;
        removed
    }

    /// Types `put` at the caret, over the selection.
    pub fn insert(&mut self, put: &Shown) -> Shown {
        let (from, to) = self.selection();
        self.replace(from, to, put)
    }

    /// Backspace (spec 27 5a): the selection; else the code ending at the caret, whole; else the character before
    /// it; with `word`, back to the start of the word.
    pub fn backspace(&mut self, word: bool) -> Shown {
        let (from, to) = self.selection();
        if from != to {
            return self.replace(from, to, &Shown::default());
        }
        if self.caret == 0 {
            return Shown::default();
        }
        let start = if word {
            let at = self.caret;
            self.move_caret(Move::WordLeft, false);
            let s = self.caret;
            self.caret = at;
            s
        } else {
            self.step_left(self.caret)
        };
        self.replace(start, self.caret, &Shown::default())
    }

    /// Delete (spec 27 5a): the selection; else the code starting at the caret, whole; else the character there;
    /// with `word`, to the start of the next word.
    pub fn delete(&mut self, word: bool) -> Shown {
        let (from, to) = self.selection();
        if from != to {
            return self.replace(from, to, &Shown::default());
        }
        if self.caret >= self.len {
            return Shown::default();
        }
        let end = if word {
            let at = self.caret;
            self.move_caret(Move::WordRight, false);
            let e = self.caret;
            self.caret = at;
            self.anchor = at;
            e
        } else {
            self.step_right(self.caret)
        };
        let at = self.caret;
        let removed = self.replace(at, end, &Shown::default());
        self.caret = at.min(self.len);
        self.anchor = self.caret;
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(text: &str, cells: usize, lines: usize) -> Editor {
        let mut e = Editor::new(Shown::marked(text), Layout::braille(cells, lines), None, false, 42);
        e.set_caret(e.len());
        e
    }

    fn typed(e: &mut Editor, s: &str) {
        for c in s.chars() {
            e.insert(&Shown::plain(&c.to_string()));
        }
    }

    #[test]
    fn spaces_typed_at_a_line_end_stay() {
        // Lines of 5 cells: "one" fills the first line and the space after it is not shown.
        let mut e = ed("", 5, 4);
        typed(&mut e, "one two three");
        assert_eq!(e.doc.text, "one two three");
        // After typing "one ", the caret stands after "one" on its line, and the next word goes after the space.
        let mut e = ed("", 5, 4);
        typed(&mut e, "one ");
        assert_eq!(e.caret_place(), (0, 3));
        typed(&mut e, "xy");
        assert_eq!(e.doc.text, "one xy");
        assert_eq!(e.row_text(1), "xy");
        // The caret moved away and back to the space keeps it: Left from the "x" stands on the space, at the end
        // of the first line, and what is typed goes before the space.
        e.move_caret(Move::Left, false);
        e.move_caret(Move::Left, false);
        assert_eq!(e.caret, 4);
        assert_eq!(e.caret_place(), (1, 0));
        e.move_caret(Move::Left, false);
        assert_eq!(e.caret, 3);
        assert_eq!(e.caret_place(), (0, 3));
        typed(&mut e, "z");
        assert_eq!(e.doc.text, "onez xy");
    }

    #[test]
    fn an_empty_last_line_takes_the_caret() {
        let mut e = ed("abc", 10, 3);
        typed(&mut e, "\n");
        assert_eq!(e.doc.text, "abc\n");
        assert_eq!(e.caret_place(), (1, 0));
        typed(&mut e, "d");
        assert_eq!(e.doc.text, "abc\nd");
        // Two empty lines at the end.
        let mut e = ed("abc", 10, 4);
        typed(&mut e, "\n\n");
        assert_eq!(e.caret_place(), (2, 0));
    }

    #[test]
    fn a_code_is_one_unit() {
        // "a[p]\nb": the [p] code with the editor's own line break after it.
        let mut e = ed("a[p]\nb", 10, 3);
        e.set_caret(1);
        e.move_caret(Move::Right, false);
        assert_eq!(e.caret, 5);
        e.move_caret(Move::Left, false);
        assert_eq!(e.caret, 1);
        // Backspace after it takes the code and its line break; Delete before it the same.
        e.set_caret(5);
        e.backspace(false);
        assert_eq!(e.doc.text, "ab");
        let mut e = ed("a[p]\nb", 10, 3);
        e.set_caret(1);
        e.delete(false);
        assert_eq!(e.doc.text, "ab");
        // Typing over a selection that cuts into a code takes the whole code.
        let mut e = ed("a[hds]b", 10, 3);
        e.replace(0, 3, &Shown::plain("x"));
        assert_eq!(e.doc.text, "xb");
    }

    #[test]
    fn up_and_down_keep_the_column_and_pass_blank_lines() {
        // Lines "aaa bbb", "ccc", then a blank line before "ddd" ([<] twice).
        let mut e = ed("aaa bbb ccc\n\nddd", 8, 5);
        e.set_caret(5);
        assert_eq!(e.caret_place(), (0, 5));
        e.move_caret(Move::Down, false);
        // "ccc" is shorter: the caret goes to its end.
        assert_eq!(e.caret_place(), (1, 3));
        e.move_caret(Move::Down, false);
        // The blank line is passed; the wanted column 5 is kept, past the end of "ddd".
        assert_eq!(e.caret_place(), (3, 3));
        e.move_caret(Move::Up, false);
        e.move_caret(Move::Up, false);
        assert_eq!(e.caret_place(), (0, 5));
    }

    #[test]
    fn rows_include_a_page_break_row() {
        let e = ed("a\nb\nc", 10, 2);
        // Two lines per page: page 1 "a" "b", break, page 2 "c" "", break.
        assert_eq!(e.rows(), 6);
        assert_eq!(e.row(2), Some(Row::Break { page: 0 }));
        assert_eq!(e.row_text(3), "c");
        assert_eq!(e.place(4), (3, 0));
    }

    #[test]
    fn the_coded_view_moves_on_document_places() {
        let mut e = Editor::new(Shown::marked("aaa bbb ccc ddd"), Layout::braille(40, 25), None, true, 10);
        assert_eq!(e.row_text(0), "aaa bbb ");
        e.set_caret(0);
        e.move_caret(Move::Down, false);
        assert_eq!(e.caret, 8);
        e.move_caret(Move::End, false);
        assert_eq!(e.caret, 15);
        assert_eq!(e.pos_at(0, 2), 2);
    }
}
