//! The formatted view of a document (re/spec/27-formatted-view.md): the laid-out page lines as the editing area shows
//! them, and the maps between a place in that text and a place in the document; and the coded view: the document's
//! text with its codes in their display forms, in rows.
//!
//! The formatted text is every line of every page, each followed by a line feed, the pages one after another; the
//! page break between pages is drawn, not written, so it is not in the text, as in the reference program's text for
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
    /// The coded view: for each character index of the view (0 to its length), the document place it stands for: a
    /// character's own place, a code's start for the characters showing the code, the place a row break was added
    /// before for the line feed showing the break.
    pub doc_of_view: Vec<usize>,
    /// The coded view: for each document place (0 to the document's length), the character index of the view where
    /// it shows (the start of a code's display form for a place inside the code).
    pub view_of_doc: Vec<usize>,
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

/// One unit of the coded view: a character of the document, or a code shown whole in its display form.
struct Unit {
    /// Its document places.
    start: usize,
    end: usize,
    /// What the view shows for it.
    text: Vec<char>,
    /// How many characters it counts as when a row is measured: a code counts its own text, with the brackets
    /// (spec 27 4b); the display form is put in after the row was measured, as the reference does.
    raw: usize,
    /// Drawn as a code (in the code font, inside a box).
    code: bool,
    /// A code's row rule; None for a character (a hard space and a marked hyphen are shown as codes but give no
    /// break opportunity of their own, as in the reference's display buffer builder).
    rule: Option<Break>,
}

impl View {
    pub fn new(pages: &[Page]) -> View {
        let mut text = String::new();
        let mut lines = Vec::new();
        let mut at = 0;
        for (p, page) in pages.iter().enumerate() {
            for (l, line) in page.lines.iter().enumerate() {
                lines.push((p, l, at));
                text.push_str(&line.text);
                text.push('\n');
                at += line.text.chars().count() + 1;
            }
        }
        View { text, len: at, lines, doc_of_view: Vec::new(), view_of_doc: Vec::new(), codes: Vec::new(), coded: false }
    }

    /// The coded view of `doc` (spec 27 4a and 4b): its text with its codes in their display forms, in rows of at
    /// most `width` characters (65 for a print document, 42 for a braille one), the codes' text counted. A row ends at
    /// a line break of the document, at the codes that force one, and before it would pass the width: at the last
    /// break opportunity of the row (after a space, before a code, after a code), or in the middle of a word when it
    /// has none. A code is never cut.
    pub fn coded(doc: &Shown, width: usize) -> View {
        let chars: Vec<char> = doc.text.chars().collect();
        let len = chars.len();
        let width = width.max(1);
        let mut units: Vec<Unit> = Vec::with_capacity(len);
        let mut ci = 0;
        let mut i = 0;
        while i < len {
            while ci < doc.codes.len() && doc.codes[ci].1 <= i {
                ci += 1;
            }
            if ci < doc.codes.len() && doc.codes[ci].0 == i {
                let (s, e) = doc.codes[ci];
                let e = e.max(s + 1).min(len);
                let raw: String = chars[s..e].iter().collect();
                let name: String = chars[s + 1..(e - 1).max(s + 1)].iter().collect();
                units.push(Unit { start: s, end: e, text: code_form(&raw).chars().collect(), raw: e - s, code: true, rule: Some(break_of(&name)) });
                i = e;
                ci += 1;
                continue;
            }
            let (text, code): (Vec<char>, bool) = match chars[i] {
                '\u{a0}' => ("[']".chars().collect(), true),
                '\u{1d}' => ("[-]".chars().collect(), true),
                c => (vec![c], false),
            };
            units.push(Unit { start: i, end: i + 1, raw: text.len(), text, code, rule: None });
            i += 1;
        }

        // Where the rows break: `breaks[u]` is a row break before unit u that is not in the document.
        let mut breaks = vec![false; units.len() + 1];
        let mut row_start = 0usize;
        let mut col = 0usize;
        let mut has_any = false;
        let mut has_text = false;
        let mut opportunity: Option<usize> = None;
        let is_newline = |u: &Unit| !u.code && u.text[0] == '\n';
        let mut u = 0;
        while u < units.len() {
            if is_newline(&units[u]) {
                u += 1;
                row_start = u;
                col = 0;
                has_any = false;
                has_text = false;
                opportunity = None;
                continue;
            }
            let (rule, raw, shown, code) = (units[u].rule, units[u].raw, units[u].text.len(), units[u].code);
            let forced = match rule {
                Some(Break::BeforeIfAny) => has_any,
                Some(Break::BeforeIfText) => has_text,
                _ => false,
            };
            if rule.is_some() {
                // A break may go before a code.
                opportunity = Some(u);
            }
            if forced {
                breaks[u] = true;
                row_start = u;
                col = 0;
                has_text = false;
                opportunity = None;
            } else if col + raw > width && has_any {
                let at = opportunity.filter(|&o| o > row_start).unwrap_or(u);
                breaks[at] = true;
                row_start = at;
                col = units[at..u].iter().map(|x| x.text.len()).sum();
                has_text = units[at..u].iter().any(|x| !x.code);
                opportunity = None;
            }
            // The unit is on the row now, whichever row that is.
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
            let space = !code && units[u].text[0] == ' ';
            u += 1;
            if after && u < units.len() && !is_newline(&units[u]) {
                breaks[u] = true;
                row_start = u;
                col = 0;
                has_any = false;
                has_text = false;
                opportunity = None;
            } else if rule.is_some() || space {
                // A break may go after a code, or after a space.
                opportunity = Some(u);
            }
        }

        let mut text = String::with_capacity(len + 16);
        let mut lines = vec![(0, 0, 0)];
        let mut doc_of_view: Vec<usize> = Vec::with_capacity(len + 16);
        let mut view_of_doc = vec![0usize; len + 1];
        let mut codes = Vec::new();
        let mut at = 0usize;
        for (u, unit) in units.iter().enumerate() {
            if breaks[u] {
                text.push('\n');
                doc_of_view.push(unit.start);
                at += 1;
                lines.push((0, lines.len(), at));
            }
            for place in unit.start..unit.end {
                view_of_doc[place] = at;
            }
            if unit.code {
                codes.push((at, at + unit.text.len()));
            }
            for &c in &unit.text {
                text.push(c);
                doc_of_view.push(unit.start);
                at += 1;
                if c == '\n' {
                    lines.push((0, lines.len(), at));
                }
            }
        }
        view_of_doc[len] = at;
        doc_of_view.push(len);
        View { text, len: at, lines, doc_of_view, view_of_doc, codes, coded: true }
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

    /// The first character index of page `page` (0-based), or the end of the text past the last page.
    pub fn page_start(&self, page: usize) -> usize {
        self.lines.iter().find(|l| l.0 == page).map_or(self.len, |l| l.2)
    }

    /// The document position a caret at character index `pos` of the view stands for: the cell's source there, else
    /// the first cell after it on that line, else just after the line's last cell, else the first text after the
    /// line (spec 27 5c, through format::position_of). In the coded view, the place the character stands for.
    pub fn to_document(&self, pages: &[Page], pos: usize) -> usize {
        if self.coded {
            return self.doc_of_view.get(pos).or(self.doc_of_view.last()).copied().unwrap_or(0);
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
            return self.view_of_doc.get(doc_pos).or(self.view_of_doc.last()).copied().unwrap_or(0);
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
            if pages[p].lines[l].sources.get(col) == Some(&Some(pos)) {
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
}
