//! The formatted view of a document (re/spec/27-formatted-view.md): the laid-out page lines as the editing area shows
//! them, and the maps between a place in that text and a place in the document.
//!
//! The text is every line of every page, each followed by a line feed, the pages one after another; the page break
//! between pages is drawn, not written, so it is not in the text, as in the reference program's text for screen
//! readers (spec 27 section 6). Blank lines of a page are empty lines.

use crate::doctext::Shown;
use crate::format::{locate, position_of, Page};

/// The formatted text of `pages` and where each line starts in it; or, for the coded view, the document's text in
/// rows (View::coded).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    pub text: String,
    /// For each line in order, its page and line on the page (0-based) and the character index where it starts.
    pub lines: Vec<(usize, usize, usize)>,
    /// The coded view: the document places before which a row break is shown that is not in the document.
    pub breaks: Vec<usize>,
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
        View { text, lines, breaks: Vec::new(), coded: false }
    }

    /// The coded view of `doc` (spec 27 4b): its text with its codes, in rows of at most `width` characters (65 for a
    /// print document, 42 for a braille one), the codes' text counted. A row ends at a line break of the document, at
    /// the codes that force one, and before it would pass the width: after its last space or code, or in the middle
    /// of a word when it has none.
    pub fn coded(doc: &Shown, width: usize) -> View {
        let chars: Vec<char> = doc.text.chars().collect();
        let width = width.max(1);
        let mut breaks: Vec<usize> = Vec::new();
        let mut start = 0usize; // where the row starts
        let mut opportunity: Option<usize> = None; // the last place in the row a break may go
        let is_text = |from: usize, to: usize| (from..to).any(|k| !doc.codes.iter().any(|c| c.0 <= k && k < c.1));
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\n' {
                i += 1;
                start = i;
                opportunity = None;
                continue;
            }
            if let Some(&(s, e)) = doc.codes.iter().find(|c| c.0 == i) {
                let name: String = chars[s + 1..e.saturating_sub(1).max(s + 1)].iter().collect();
                let rule = break_of(&name);
                let forced = match rule {
                    Break::BeforeIfAny => i > start,
                    Break::BeforeIfText => is_text(start, i),
                    _ => false,
                };
                if forced {
                    breaks.push(i);
                    start = i;
                } else if e - start > width && i > start {
                    let at = opportunity.filter(|&o| o > start).unwrap_or(i);
                    breaks.push(at);
                    start = at;
                }
                i = e;
                let after = match rule {
                    Break::AfterAlways => true,
                    Break::AfterIfText => is_text(start, i),
                    _ => false,
                };
                if after && i < chars.len() && chars[i] != '\n' {
                    breaks.push(i);
                    start = i;
                    opportunity = None;
                } else {
                    // A break may go before or after a code.
                    opportunity = Some(i);
                }
                continue;
            }
            if i + 1 - start > width && i > start {
                let at = opportunity.filter(|&o| o > start).unwrap_or(i);
                breaks.push(at);
                start = at;
                opportunity = None;
            }
            if chars[i] == ' ' {
                opportunity = Some(i + 1);
            }
            i += 1;
        }
        breaks.dedup();
        let mut text = String::with_capacity(doc.text.len() + breaks.len());
        let mut lines = vec![(0, 0, 0)];
        let mut b = breaks.iter().peekable();
        let mut at = 0usize;
        for (k, &c) in chars.iter().enumerate() {
            while b.peek().is_some_and(|&&x| x == k) {
                b.next();
                text.push('\n');
                at += 1;
                lines.push((0, lines.len(), at));
            }
            text.push(c);
            at += 1;
            if c == '\n' {
                lines.push((0, lines.len(), at));
            }
        }
        View { text, lines, breaks, coded: true }
    }

    /// The line (an index into `lines`) holding character index `pos`, and the column in it.
    pub fn line_at(&self, pos: usize) -> Option<(usize, usize)> {
        let i = self.lines.partition_point(|l| l.2 <= pos).checked_sub(1)?;
        Some((i, pos - self.lines[i].2))
    }

    /// The first character index of page `page` (0-based), or the end of the text past the last page.
    pub fn page_start(&self, page: usize) -> usize {
        self.lines.iter().find(|l| l.0 == page).map_or(self.text.chars().count(), |l| l.2)
    }

    /// The document position a caret at character index `pos` of the view stands for: the cell's source there, else
    /// the first cell after it on that line, else just after the line's last cell, else the first text after the
    /// line (spec 27 5c, through format::position_of).
    pub fn to_document(&self, pages: &[Page], pos: usize) -> usize {
        if self.coded {
            // Each added break before the place is one character of the view that is not in the document.
            let added = self.breaks.iter().enumerate().take_while(|(n, &b)| b + n < pos).count();
            return pos - added;
        }
        let Some((i, col)) = self.line_at(pos) else { return 0 };
        let (p, l, _) = self.lines[i];
        position_of(pages, p + 1, l + 1, col + 1).unwrap_or_else(|| pages.last().map_or(0, end_of))
    }

    /// The character index of the view where the document position `doc_pos` shows: its cell, or for a position that
    /// has no cell, just after the nearest earlier cell (spec 27 5b, through format::locate).
    pub fn to_view(&self, pages: &[Page], doc_pos: usize) -> usize {
        if self.coded {
            // A place where a row break was added shows at the start of the new row.
            return doc_pos + self.breaks.partition_point(|&b| b <= doc_pos);
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
        // The maps: every document place goes to the view and back.
        let doc = Shown::marked("x[p]y[hds]Head[hde]z");
        let v = View::coded(&doc, 40);
        for pos in 0..=doc.text.chars().count() {
            assert_eq!(v.to_document(&[], v.to_view(&[], pos)), pos, "{pos}");
        }
        // A place where a break was added shows at the start of the new row.
        assert_eq!(v.text.chars().nth(v.to_view(&[], 1)), Some('['));
    }
}
