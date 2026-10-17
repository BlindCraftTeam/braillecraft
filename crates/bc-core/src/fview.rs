//! The formatted view of a document (re/spec/27-formatted-view.md): the laid-out page lines as the editing area shows
//! them, and the maps between a place in that text and a place in the document.
//!
//! The text is every line of every page, each followed by a line feed, the pages one after another; the page break
//! between pages is drawn, not written, so it is not in the text, as in the reference program's text for screen
//! readers (spec 27 section 6). Blank lines of a page are empty lines.

use crate::format::{locate, position_of, Page};

/// The formatted text of `pages` and where each line starts in it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct View {
    pub text: String,
    /// For each line in order, its page and line on the page (0-based) and the character index where it starts.
    pub lines: Vec<(usize, usize, usize)>,
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
        View { text, lines }
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
        let Some((i, col)) = self.line_at(pos) else { return 0 };
        let (p, l, _) = self.lines[i];
        position_of(pages, p + 1, l + 1, col + 1).unwrap_or_else(|| pages.last().map_or(0, end_of))
    }

    /// The character index of the view where the document position `doc_pos` shows: its cell, or for a position that
    /// has no cell, just after the nearest earlier cell (spec 27 5b, through format::locate).
    pub fn to_view(&self, pages: &[Page], doc_pos: usize) -> usize {
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
}
