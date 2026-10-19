//! The formatted view of a document (re/spec/27-formatted-view.md): the laid-out page lines as the editing area shows
//! them, and the maps between a place in that text and a place in the document; and the coded view: the document's
//! text with its codes in their display forms, in rows.
//!
//! The formatted text is every line of every page, each followed by a line feed, the pages one after another; the
//! page break between pages is drawn, not written, so it is not in the text, as in the text for
//! screen readers (spec 27 section 6). Blank lines of a page are empty lines.

use crate::doctext::Shown;
use crate::format::{locate, position_of, Page};

/// The formatted text of `pages` and where each line starts in it; or, for the coded view, the document's text in
/// rows (View::coded).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    pub text: String,
    /// The number of characters of `text`.
    pub len: usize,
    /// For each line in order, its page and line on the page (0-based) and the character index where it starts.
    pub lines: Vec<(usize, usize, usize)>,
    /// For each line in order, the byte index of `text` where it starts, so a row of a long document is reached
    /// without counting the characters before it (plan line L1).
    pub line_bytes: Vec<usize>,
    /// The coded view: for each character index of the view (0 to its length), the document place it stands for: a
    /// character's own place, a code's start for the characters showing the code, the place a row break was added
    /// before for the line feed showing the break. Four bytes an entry: a book of millions of characters costs
    /// each map four bytes a character, not eight (plan line L1).
    pub doc_of_view: Vec<u32>,
    /// The coded view: for each document place (0 to the document's length), the character index of the view where
    /// it shows (the start of a code's display form for a place inside the code).
    pub view_of_doc: Vec<u32>,
    /// The coded view: the character ranges of the view showing a code in its display form, in order; drawn in the
    /// code font inside a box (spec 27 4a). A hard space and a marked hyphen are shown as codes too.
    pub codes: Vec<(usize, usize)>,
    pub coded: bool,
}

/// How a code breaks the coded view's rows (spec 27 4b): a new row before it when the row holds anything, before
/// it when the row holds text (not only codes), after it when the row holds text, or always after it.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Break {
    None,
    BeforeIfAny,
    BeforeIfText,
    AfterIfText,
    AfterAlways,
}

/// The row rule of the code named `name` (without its brackets). A code matches a name in the lists when it is that
/// name, or that name followed by `~` or a number.
fn break_of(name: &str) -> Break {
    let n = name.to_lowercase();
    let is = |e: &str| n == e || n.strip_prefix(e).and_then(|r| r.chars().next()).is_some_and(|c| c == '~' || c.is_ascii_digit());
    let style = |prefix: &str| n.starts_with(prefix) && n.ends_with('.');
    if ["p", "pg", "pm", "hds", "mfcs", "tls", "rfs", "top"].iter().any(|e| is(e)) || style("es~") {
        Break::BeforeIfAny
    } else if ["kps", "ind", "hl", "ptys", "sc", "sd", "sk", "st", "sl", "cp", "hi"].iter().any(|e| is(e)) {
        Break::BeforeIfText
    } else if ["l", "ptye", "kpe"].iter().any(|e| is(e)) {
        Break::AfterIfText
    } else if ["<", "hde", "mfce", "tle", "rfe", "gd", "music", "lg"].iter().any(|e| is(e)) || style("ee~") {
        Break::AfterAlways
    } else {
        Break::None
    }
}

/// The coded view's display form of the code `code` (its text with the brackets), spec 27 4a: a style begin
/// [es~name] is shown as "<name>", a style end [ee~name] as "</name>", a [gd~...] code cut to "[gd]"; any other code
/// as it is.
pub fn code_form(code: &str) -> String {
    let inner = code.strip_prefix('[').and_then(|c| c.strip_suffix(']')).unwrap_or(code);
    if let Some(name) = inner.strip_prefix("es~") {
        format!("<{name}>")
    } else if let Some(name) = inner.strip_prefix("ee~") {
        format!("</{name}>")
    } else if inner.starts_with("gd~") {
        "[gd]".to_string()
    } else {
        code.to_string()
    }
}

impl View {
    pub fn new(pages: &[Page]) -> View {
        let mut text = String::new();
        let mut lines = Vec::new();
        let mut line_bytes = Vec::new();
        let mut at = 0;
        for (p, page) in pages.iter().enumerate() {
            for (l, line) in page.lines.iter().enumerate() {
                lines.push((p, l, at));
                line_bytes.push(text.len());
                text.push_str(&line.text);
                text.push('\n');
                at += line.text.chars().count() + 1;
            }
        }
        View { text, len: at, lines, line_bytes, doc_of_view: Vec::new(), view_of_doc: Vec::new(), codes: Vec::new(), coded: false }
    }

    /// The coded view of `doc` (spec 27 4a and 4b): its text with its codes in their display forms, in rows of at
    /// most `width` characters (65 for a print document, 42 for a braille one), the codes' text counted. A row ends at
    /// a line break of the document, at the codes that force one, and before it would pass the width: at the last
    /// break opportunity of the row (after a space, before a code, after a code), or in the middle of a word when it
    /// has none. A code is never cut.
    ///
    /// Built in one pass over the document (plan line L1): each unit (a character, or a code shown whole in its
    /// display form) is placed as it comes, and only the units of the row being built are kept, so a row break put
    /// in at an earlier opportunity moves no more than one row of text.
    pub fn coded(doc: &Shown, width: usize) -> View {
        let len = doc.text.chars().count();
        let width = width.max(1);

        // A unit placed on the row being built: where it is in the view (character and byte), what it shows, and
        // its document places.
        struct Placed {
            view_at: usize,
            byte_at: usize,
            shown: usize,
            code: bool,
            start: usize,
            end: usize,
        }

        let mut text = String::with_capacity(doc.text.len() + 16);
        let mut lines = vec![(0, 0, 0)];
        let mut line_bytes = vec![0];
        let mut doc_of_view: Vec<u32> = Vec::with_capacity(len + 16);
        let mut view_of_doc = vec![0u32; len + 1];
        let mut codes: Vec<(usize, usize)> = Vec::new();
        let mut at = 0usize;
        let mut row: Vec<Placed> = Vec::new();
        let mut col = 0usize;
        let mut has_any = false;
        let mut has_text = false;
        // A break opportunity: before the unit at this index of the row (the row's length means before the unit
        // that comes next).
        let mut opportunity: Option<usize> = None;
        // The unit before asked for a break after it (before the next unit, unless that is a line break).
        let mut break_before_next = false;

        // A row break put in at the view's end (before the unit about to be placed).
        let push_break = |text: &mut String, doc_of_view: &mut Vec<u32>, lines: &mut Vec<(usize, usize, usize)>, line_bytes: &mut Vec<usize>, at: &mut usize, place: usize| {
            text.push('\n');
            doc_of_view.push(place as u32);
            *at += 1;
            lines.push((0, lines.len(), *at));
            line_bytes.push(text.len());
        };

        let mut chars = doc.text.chars();
        let mut ci = 0;
        let mut i = 0;
        while i < len {
            while ci < doc.codes.len() && doc.codes[ci].1 <= i {
                ci += 1;
            }
            // The unit at i: its document places, its display form, its measured length, and its row rule.
            let (start, end, unit_text, raw, code, rule): (usize, usize, String, usize, bool, Option<Break>) =
                if ci < doc.codes.len() && doc.codes[ci].0 == i {
                    let (s, e) = doc.codes[ci];
                    let e = e.max(s + 1).min(len);
                    let raw: String = chars.by_ref().take(e - s).collect();
                    let name: String = raw.chars().skip(1).take((e - s).saturating_sub(2)).collect();
                    ci += 1;
                    (s, e, code_form(&raw), e - s, true, Some(break_of(&name)))
                } else {
                    let c = chars.next().unwrap_or(' ');
                    match c {
                        '\u{a0}' => (i, i + 1, "[']".to_string(), 3, true, None),
                        '\u{1d}' => (i, i + 1, "[-]".to_string(), 3, true, None),
                        c => (i, i + 1, c.to_string(), 1, false, None),
                    }
                };
            i = end;
            let shown = unit_text.chars().count();
            let newline = !code && unit_text == "\n";

            if newline {
                // The document's own line break: placed, and the row starts over after it.
                view_of_doc[start] = at as u32;
                text.push('\n');
                doc_of_view.push(start as u32);
                at += 1;
                lines.push((0, lines.len(), at));
                line_bytes.push(text.len());
                row.clear();
                col = 0;
                has_any = false;
                has_text = false;
                opportunity = None;
                break_before_next = false;
                continue;
            }
            if break_before_next {
                push_break(&mut text, &mut doc_of_view, &mut lines, &mut line_bytes, &mut at, start);
                row.clear();
                col = 0;
                has_any = false;
                has_text = false;
                opportunity = None;
                break_before_next = false;
            }
            let forced = match rule {
                Some(Break::BeforeIfAny) => has_any,
                Some(Break::BeforeIfText) => has_text,
                _ => false,
            };
            if rule.is_some() {
                // A break may go before a code.
                opportunity = Some(row.len());
            }
            if forced {
                push_break(&mut text, &mut doc_of_view, &mut lines, &mut line_bytes, &mut at, start);
                row.clear();
                col = 0;
                has_text = false;
                opportunity = None;
            } else if col + raw > width && has_any {
                let k = opportunity.filter(|&o| o > 0).unwrap_or(row.len());
                if k >= row.len() {
                    // Before the unit about to be placed.
                    push_break(&mut text, &mut doc_of_view, &mut lines, &mut line_bytes, &mut at, start);
                    row.clear();
                } else {
                    // At an earlier opportunity of the row: the line feed goes in there, and what follows it on the
                    // row (at most a row of text) moves one place on.
                    let (view_at, byte_at) = (row[k].view_at, row[k].byte_at);
                    text.insert(byte_at, '\n');
                    doc_of_view.insert(view_at, row[k].start as u32);
                    at += 1;
                    for unit in &mut row[k..] {
                        unit.view_at += 1;
                        unit.byte_at += 1;
                        for place in unit.start..unit.end {
                            view_of_doc[place] += 1;
                        }
                    }
                    for c in codes.iter_mut().rev().take_while(|c| c.0 >= view_at) {
                        c.0 += 1;
                        c.1 += 1;
                    }
                    lines.push((0, lines.len(), view_at + 1));
                    line_bytes.push(byte_at + 1);
                    row.drain(..k);
                }
                col = row.iter().map(|x| x.shown).sum();
                has_text = row.iter().any(|x| !x.code);
                opportunity = None;
            }
            // The unit is on the row now, whichever row that is.
            for place in start..end {
                view_of_doc[place] = at as u32;
            }
            if code {
                codes.push((at, at + shown));
            }
            row.push(Placed { view_at: at, byte_at: text.len(), shown, code, start, end });
            for c in unit_text.chars() {
                text.push(c);
                doc_of_view.push(start as u32);
                at += 1;
            }
            col += shown;
            has_any = true;
            if !code {
                has_text = true;
            }
            let after = match rule {
                Some(Break::AfterAlways) => true,
                Some(Break::AfterIfText) => has_text,
                _ => false,
            };
            let space = !code && unit_text == " ";
            if after {
                break_before_next = true;
            } else if rule.is_some() || space {
                // A break may go after a code, or after a space.
                opportunity = Some(row.len());
            }
        }
        view_of_doc[len] = at as u32;
        doc_of_view.push(len as u32);
        View { text, len: at, lines, line_bytes, doc_of_view, view_of_doc, codes, coded: true }
    }

    /// The line (an index into `lines`) holding character index `pos`, and the column in it.
    pub fn line_at(&self, pos: usize) -> Option<(usize, usize)> {
        let i = self.lines.partition_point(|l| l.2 <= pos).checked_sub(1)?;
        Some((i, pos - self.lines[i].2))
    }

    /// The character index where line `line` (an index into `lines`) ends: where the next line starts, or the end
    /// of the text.
    pub fn line_end(&self, line: usize) -> usize {
        self.lines.get(line + 1).map_or(self.len, |l| l.2)
    }

    /// The text of line `line` (an index into `lines`), with its line feed when it ends in one.
    pub fn line_text(&self, line: usize) -> &str {
        let start = self.line_bytes.get(line).copied().unwrap_or(self.text.len());
        let end = self.line_bytes.get(line + 1).copied().unwrap_or(self.text.len());
        &self.text[start.min(end)..end]
    }

    /// The first character index of page `page` (0-based), or the end of the text past the last page.
    pub fn page_start(&self, page: usize) -> usize {
        self.lines.iter().find(|l| l.0 == page).map_or(self.len, |l| l.2)
    }

    /// The document position a caret at character index `pos` of the view stands for: the cell's source there, else
    /// the first cell after it on that line, else just after the line's last cell, else the first text after the
    /// line (spec 27 5c, through format::position_of). In the coded view, the place the character stands for.
    pub fn to_document(&self, pages: &[Page], pos: usize) -> usize {
        if self.coded {
            return self.doc_of_view.get(pos).or(self.doc_of_view.last()).map_or(0, |&p| p as usize);
        }
        let Some((i, col)) = self.line_at(pos) else { return 0 };
        let (p, l, _) = self.lines[i];
        position_of(pages, p + 1, l + 1, col + 1).unwrap_or_else(|| pages.last().map_or(0, end_of))
    }

    /// The character index of the view where the document position `doc_pos` shows: its cell, or for a position that
    /// has no cell, just after the nearest earlier cell (spec 27 5b, through format::locate). In the coded view, the
    /// character showing the place (the start of the new row for a place a row break was added before).
    pub fn to_view(&self, pages: &[Page], doc_pos: usize) -> usize {
        if self.coded {
            return self.view_of_doc.get(doc_pos).or(self.view_of_doc.last()).map_or(0, |&p| p as usize);
        }
        if pages.is_empty() {
            return 0;
        }
        let (p, l, c) = locate(pages, doc_pos);
        match self.lines.iter().find(|x| x.0 == p - 1 && x.1 == l - 1) {
            Some(&(_, _, start)) => {
                let len = pages[p - 1].lines[l - 1].text.chars().count();
                start + (c - 1).min(len)
            }
            None => 0,
        }
    }
}

/// The position just after the last cell of a page that has a source.
fn end_of(page: &Page) -> usize {
    page.lines.iter().flat_map(|l| l.sources.iter().flatten()).last().map_or(page.start, |s| s + 1)
}

/// Whether the editing area keeps at least one text row (spec 27 section 9, unknown 1): a view switch, a font
/// change or turning the translated line on is refused when the area, less `extra_h` (the translated line's height
/// when it is being turned on, else 0), is lower than one row of the new font.
pub fn one_row_fits(area_h: i32, row_h: i32, extra_h: i32) -> bool {
    row_h + extra_h <= area_h
}

/// View, Translated Line (spec 27 section 9): turning the line off (`on_now`) is never refused; turning it on is
/// refused (message 2636) when the editing area, less the line's height `line_h`, would not hold one row.
pub fn translated_line_allowed(on_now: bool, area_h: i32, row_h: i32, line_h: i32) -> bool {
    on_now || one_row_fits(area_h, row_h, line_h)
}

/// The caret's screen row kept through a view switch or a page move (spec 27 section 9, unknowns 2 and 7): its
/// row less the top row, only when the caret is on screen (top <= row < top + rows, at least one row); None scrolls
/// only as far as needed to show the caret.
pub fn kept_row(caret_row: usize, top: usize, rows: usize) -> Option<usize> {
    (caret_row >= top && caret_row < top + rows.max(1)).then(|| caret_row - top)
}

/// The Full-Page font size (spec 27 section 9, unknown 3): sizes 5 to 48 are tried in turn, `measure` giving the
/// font's character width and row height at a size (None when the font cannot be made). The first size at which
/// the frame's width holds no more than `cells` characters, or the area's height no more than `lines` rows, stops
/// the search and the size before it is taken, never less than 5; 47 when no size stops it; 6 when the font
/// cannot be made.
pub fn full_page_size(measure: impl Fn(i32) -> Option<(i32, i32)>, frame_w: i32, area_h: i32, cells: i32, lines: i32) -> i32 {
    for size in 5..=48 {
        let Some((char_w, row_h)) = measure(size) else { return 6 };
        let cols = frame_w / char_w.max(1);
        let rows = area_h / row_h.max(1);
        if cols <= cells || rows <= lines {
            return (size - 1).max(5);
        }
    }
    47
}

/// The caret's column for a click in a formatted row (spec 27 section 9, unknown 4): the click is moved left by
/// half a character width (`half`), cells with no source (`has_source` false: filler, page numbers, running
/// heads) are skipped, and the caret goes before the first source cell whose left edge (`lefts`) is right of that
/// point; past the middle of the last source cell it goes after that cell. None for a row with no source cell.
pub fn click_column(lefts: &[i32], has_source: &[bool], x: i32, half: i32) -> Option<usize> {
    let at = x - half;
    let mut last = None;
    for (i, &left) in lefts.iter().enumerate() {
        if !has_source.get(i).copied().unwrap_or(false) {
            continue;
        }
        if left > at {
            return Some(i);
        }
        last = Some(i);
    }
    last.map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctext::Shown;
    use crate::format::{format, Layout};

    #[test]
    fn formatted_text_and_both_maps() {
        // Two pages of 3 lines; "[p]" is a code and takes no cell.
        let doc = Shown::marked("[p]one two[p]three");
        let pages = format(&doc, &Layout::braille(10, 3), None);
        let v = View::new(&pages);
        let lines: Vec<&str> = v.text.split('\n').collect();
        assert_eq!(lines.len(), pages.iter().map(|p| p.lines.len()).sum::<usize>() + 1);
        assert!(!v.text.contains('['), "codes are hidden: {:?}", v.text);
        // Every cell with a source maps to the view and back to itself.
        let chars: Vec<char> = doc.text.chars().collect();
        for pos in 0..chars.len() {
            let shown = v.to_view(&pages, pos);
            let (i, col) = v.line_at(shown).unwrap();
            let (p, l, _) = v.lines[i];
            if pages[p].lines[l].sources.get(col) == Some(pos) {
                assert_eq!(v.to_document(&pages, shown), pos);
            }
        }
        // The 'o' of "one" is at the start of the first text line.
        let o = doc.text.find('o').unwrap();
        let at = v.to_view(&pages, o);
        assert_eq!(v.text.chars().nth(at), Some('o'));
    }

    #[test]
    fn coded_rows() {
        // Rows of at most 10: a break after the last space that fits.
        let v = View::coded(&Shown::marked("aaa bbb ccc ddd"), 10);
        assert_eq!(v.text, "aaa bbb \nccc ddd");
        // A word longer than the row is cut where the row is full.
        assert_eq!(View::coded(&Shown::marked("abcdefghijkl"), 5).text, "abcde\nfghij\nkl");
        // [p] starts a new row when the row holds anything; [hde] ends one; the codes' text counts.
        let v = View::coded(&Shown::marked("x[p]y[hds]Head[hde]z"), 40);
        // (the editor's own line break after [p] is in the document, so [p] ends its row as well).
        assert_eq!(v.text, "x\n[p]\ny\n[hds]Head[hde]\nz");
        // The maps: every document place that is not inside a code goes to the view and back.
        let doc = Shown::marked("x[p]y[hds]Head[hde]z");
        let v = View::coded(&doc, 40);
        for pos in 0..=doc.text.chars().count() {
            if !doc.codes.iter().any(|c| c.0 < pos && pos < c.1) {
                assert_eq!(v.to_document(&[], v.to_view(&[], pos)), pos, "{pos}");
            }
        }
        // A place where a break was added shows at the start of the new row.
        assert_eq!(v.text.chars().nth(v.to_view(&[], 1)), Some('['));
        // The codes of the view, by their characters.
        assert_eq!(v.codes, vec![(2, 5), (8, 13), (17, 22)]);
    }

    #[test]
    fn codes_are_shown_in_their_display_forms() {
        // A style begin as <name>, a style end as </name>, a gd~ code cut to [gd]; the document keeps its codes.
        let doc = Shown::marked("ab[es~h1.]Head[ee~h1.][gd~xy]c");
        let v = View::coded(&doc, 42);
        assert_eq!(v.text, "ab\n<h1.>Head</h1.>\n[gd]\nc");
        assert_eq!(v.codes, vec![(3, 8), (12, 18), (19, 23)]);
        // A place inside a code shows at the code's start; the view's code characters stand for the code's start.
        let code = doc.codes[0];
        assert_eq!(v.to_view(&[], code.0), 3);
        assert_eq!(v.to_view(&[], code.0 + 3), 3);
        assert_eq!(v.to_document(&[], 5), code.0);
        assert_eq!(v.to_document(&[], 8), code.1);
        // Every place that is not inside a code goes to the view and back to itself.
        for pos in 0..=doc.text.chars().count() {
            if !doc.codes.iter().any(|c| c.0 < pos && pos < c.1) {
                assert_eq!(v.to_document(&[], v.to_view(&[], pos)), pos, "{pos}");
            }
        }
        // A hard space and a marked hyphen are shown as the codes ['] and [-].
        let v = View::coded(&Shown::marked("a\u{a0}b\u{1d}c"), 42);
        assert_eq!(v.text, "a[']b[-]c");
        assert_eq!(v.codes, vec![(1, 4), (5, 8)]);
        assert_eq!(v.to_document(&[], 2), 1);
        assert_eq!(v.to_view(&[], 2), 4);
    }

    #[test]
    fn a_row_is_measured_with_a_codes_own_text() {
        // Rows of 10: [es~x] counts 6 while the row is measured, so it does not fit after "abcdef" and starts a new
        // row (before itself, the last break opportunity), where it is shown as <x>.
        assert_eq!(View::coded(&Shown::marked("abcdef[es~x]g"), 10).text, "abcdef\n<x>g");
        // After "abcd" it fits (4 + 6), is shown as <x> (3), and the row goes on; "ghij" would pass the width at its
        // last letter, so the row breaks at the last opportunity, after the code.
        assert_eq!(View::coded(&Shown::marked("abcd[es~x]ghij"), 10).text, "abcd<x>\nghij");
        // A code at the start of a row is never cut, however long.
        assert_eq!(View::coded(&Shown::marked("[abcdefghijkl]m"), 5).text, "[abcdefghijkl]\nm");
    }

    #[test]
    fn one_row_fits_rule() {
        assert!(one_row_fits(20, 20, 0));
        assert!(!one_row_fits(19, 20, 0));
        assert!(one_row_fits(30, 20, 10));
        assert!(!one_row_fits(30, 20, 11));
    }

    #[test]
    fn turning_off_is_never_refused() {
        assert!(translated_line_allowed(true, 0, 20, 30));
        assert!(!translated_line_allowed(false, 40, 20, 30));
        assert!(translated_line_allowed(false, 50, 20, 30));
    }

    #[test]
    fn kept_row_only_on_screen() {
        assert_eq!(kept_row(5, 0, 10), Some(5));
        assert_eq!(kept_row(10, 0, 10), None);
        assert_eq!(kept_row(3, 4, 10), None);
        assert_eq!(kept_row(4, 4, 0), Some(0));
    }

    #[test]
    fn full_page_size_rule() {
        // A font whose character is `size` wide and `2 * size` high, in a frame 400 wide and an area 600 high.
        let m = |s: i32| Some((s, 2 * s));
        // Size 5 already fails: 400 / 5 = 80 columns <= 80 cells.
        assert_eq!(full_page_size(m, 400, 600, 80, 25), 5);
        // Size 10 first gives 40 columns == 40 cells (equality stops).
        assert_eq!(full_page_size(m, 400, 6000, 40, 25), 9);
        // No size stops the search.
        assert_eq!(full_page_size(m, 100_000, 100_000, 40, 25), 47);
        assert_eq!(full_page_size(|_| None, 400, 600, 40, 25), 6);
    }

    #[test]
    fn click_column_rule() {
        // Cells 10 wide at 0, 10, 20, 30; the cell at 20 has no source.
        let lefts = [0, 10, 20, 30];
        let src = [true, true, false, true];
        // Left half of cell 1 gives cell 1; right half gives cell 2's place, skipped to the next source cell 3.
        assert_eq!(click_column(&lefts, &src, 12, 5), Some(1));
        assert_eq!(click_column(&lefts, &src, 17, 5), Some(3));
        // Exactly half goes right.
        assert_eq!(click_column(&lefts, &src, 15, 5), Some(3));
        assert_eq!(click_column(&lefts, &src, 5, 5), Some(1));
        // Past the middle of the last cell: after it.
        assert_eq!(click_column(&lefts, &src, 36, 5), Some(4));
        assert_eq!(click_column(&lefts, &[false; 4], 5, 5), None);
    }
}
