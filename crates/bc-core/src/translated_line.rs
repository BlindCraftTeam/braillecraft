//! View, Translated Line (command 1310): what the translated line translates. In the formatted view it is the
//! laid-out line the caret is on, as it stands on the page (the document slice of that line, not the whole
//! paragraph it comes from); in the coded view the coded row the caret is on. The codes in force at the line's start
//! (grade, emphasis, language switches) apply to it: every code of the document before the line is carried in
//! ahead of it, and what those codes write is left off the translation. Leading spaces of the line are skipped.
//!
//! Only the codes are carried in, never the text before the line, so moving the caret in a long document translates
//! one line and the codes before it.

use crate::doctext::{Shown, CLOSE, OPEN};
use crate::format::{locate, Page};
use crate::fview::View;

/// The document places of the laid-out line that place `pos` is on: from its first cell's place to just after its
/// last cell's place; an empty span at `pos` for a line with no cell from the document (a blank line).
pub fn page_line_span(pages: &[Page], pos: usize) -> (usize, usize) {
    if pages.is_empty() {
        return (pos, pos);
    }
    let (p, l, _) = locate(pages, pos);
    let Some(line) = pages.get(p - 1).and_then(|pg| pg.lines.get(l - 1)) else { return (pos, pos) };
    let mut places = line.sources.iter().flatten();
    match places.next() {
        Some(first) => {
            let (lo, hi) = places.fold((first, first), |(lo, hi), s| (lo.min(s), hi.max(s)));
            (lo, hi + 1)
        }
        None => (pos, pos),
    }
}

/// The document places of the coded view's row that place `pos` is on: from the place its first character stands
/// for to the place the next row starts at (the document's end after the last row).
pub fn coded_row_span(view: &View, doc_len: usize, pos: usize) -> (usize, usize) {
    let at = view.to_view(&[], pos);
    let Some((row, _)) = view.line_at(at) else { return (pos, pos) };
    let from = view.to_document(&[], view.lines[row].2);
    let to = if row + 1 < view.lines.len() { view.to_document(&[], view.lines[row + 1].2) } else { doc_len };
    (from.min(to), to)
}

/// What the translated line translates for the line `from`..`to` of `doc`: the codes before the line (each code
/// of the document that ends at or before its start, with the line breaks between them, so a new-line code goes
/// too; no other text), and the line with its leading spaces skipped. The text before the line is read once.
pub fn line_parts(doc: &Shown, from: usize, to: usize) -> (Shown, Shown) {
    let mut inner: Vec<(usize, usize)> = doc.inner.clone();
    inner.sort_unstable();
    let mut next_inner = 0;
    let mut carried = Shown::default();
    let mut n = 0usize;
    let mut spans = doc.codes.iter().copied().peekable();
    let mut current: Option<(usize, usize, usize)> = None;
    let mut chars = doc.text.chars().enumerate();
    for (k, c) in chars.by_ref() {
        if k >= from {
            break;
        }
        if current.is_none() {
            if let Some(&(s, e)) = spans.peek() {
                if s == k {
                    spans.next();
                    current = Some((s, e, n));
                }
            }
        }
        match current {
            Some((s, e, at)) => {
                if e > from {
                    break;
                }
                carried.text.push(c);
                n += 1;
                if k + 1 == e {
                    carried.codes.push((at, n));
                    while next_inner < inner.len() && inner[next_inner].0 < e {
                        let (is, ie) = inner[next_inner];
                        if is >= s && ie <= e {
                            carried.inner.push((is - s + at, ie - s + at));
                        }
                        next_inner += 1;
                    }
                    current = None;
                }
            }
            None if c == '\n' => {
                carried.text.push(c);
                n += 1;
            }
            None => {}
        }
    }
    let len = doc.text.chars().count();
    let to = to.min(len);
    let start = doc.text.chars().skip(from).take(to.saturating_sub(from)).take_while(|&c| c == ' ').count() + from;
    (carried, doc.slice(start.min(to), to))
}

/// The translation of a line that was translated after the codes carried in (`whole`, running text), with what
/// the carried codes alone translate to (`lead`) left off its start. When the two part ways inside the lead, the
/// part they share is left off up to the end of its last whole code (INFERENCE: what the codes write is codes and
/// the indicators they bring, which the line's own text does not change).
pub fn leave_off_lead(lead: &str, whole: &str) -> String {
    if let Some(rest) = whole.strip_prefix(lead) {
        return rest.to_string();
    }
    let w: Vec<char> = whole.chars().collect();
    let shared = lead.chars().zip(&w).take_while(|(a, b)| a == *b).count();
    let mut depth = 0usize;
    let mut cut = 0;
    for (k, &c) in w[..shared].iter().enumerate() {
        match c {
            OPEN => depth += 1,
            CLOSE if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    cut = k + 1;
                }
            }
            _ if depth == 0 => cut = k + 1,
            _ => {}
        }
    }
    w[cut..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{format, Layout};

    #[test]
    fn the_second_laid_out_line_of_a_paragraph_is_its_own_words() {
        let doc = Shown::marked("[p]one two three four five six seven eight nine ten");
        let pages = format(&doc, &Layout::braille(16, 25), None);
        let lines: Vec<&str> = pages[0].lines.iter().map(|l| l.text.trim()).filter(|t| !t.is_empty()).collect();
        assert!(lines.len() >= 3, "{lines:?}");
        let second = doc.text.find(lines[1].split(' ').next().unwrap()).unwrap() + 1;
        let (from, to) = page_line_span(&pages, second);
        let (carried, line) = line_parts(&doc, from, to);
        assert_eq!(line.text, lines[1]);
        assert_eq!(carried.text, "[p]\n");
        assert_eq!(carried.codes, vec![(0, 3)]);
    }

    #[test]
    fn a_grade_code_earlier_in_the_paragraph_is_carried_in() {
        let doc = Shown::marked("[p]one [g1l]two three four five six seven eight nine ten");
        let pages = format(&doc, &Layout::braille(16, 25), None);
        let at = doc.text.find("seven").unwrap();
        let (from, to) = page_line_span(&pages, at);
        assert!(from > doc.text.find("[g1l]").unwrap());
        let (carried, line) = line_parts(&doc, from, to);
        assert_eq!(carried.text, "[p]\n[g1l]");
        assert_eq!(carried.codes.len(), 2);
        assert!(line.text.contains("seven") && !line.text.contains("one"), "{}", line.text);
    }

    #[test]
    fn leading_spaces_are_skipped() {
        let doc = Shown::marked("[b]  two words");
        let (carried, line) = line_parts(&doc, 3, doc.text.chars().count());
        assert_eq!(line.text, "two words");
        assert_eq!(carried.text, "[b]");
    }

    #[test]
    fn the_coded_row_is_taken_in_the_coded_view() {
        let doc = Shown::marked("[p]first[p]second");
        let view = View::coded(&doc, 65);
        let len = doc.text.chars().count();
        let at = doc.text.find("second").unwrap() + 2;
        let (from, to) = coded_row_span(&view, len, at);
        let (carried, line) = line_parts(&doc, from, to);
        assert!(line.text.contains("second") && !line.text.contains("first"), "{}", line.text);
        assert!(carried.text.starts_with("[p]"));
    }

    #[test]
    fn what_the_carried_codes_write_is_left_off() {
        assert_eq!(leave_off_lead("\u{1c}g1l\u{1f}", "\u{1c}g1l\u{1f}abc"), "abc");
        assert_eq!(leave_off_lead("\u{1c}g1l\u{1f};x", "\u{1c}g1l\u{1f};yabc"), "yabc");
        assert_eq!(leave_off_lead("\u{1c}es~X\u{1f}", "\u{1c}es~x\u{1f}abc"), "\u{1c}es~x\u{1f}abc");
        assert_eq!(leave_off_lead("", "abc"), "abc");
    }
}
