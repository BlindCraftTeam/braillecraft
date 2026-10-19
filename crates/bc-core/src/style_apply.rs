//! Applying a style to a range, Join Styles, Join Lines and New Paragraph (spec 31). The
//! work is done on the running text (codes marked 0x1C ... 0x1F) with places kept through every change.

use crate::doctext::{CLOSE, OPEN};

/// The running text being changed, with the places (caret, selection ends) moved along with each change.
struct Text {
    c: Vec<char>,
    marks: Vec<usize>,
}

impl Text {
    fn new(running: &str) -> Text {
        Text { c: running.chars().collect(), marks: Vec::new() }
    }

    fn mark(&mut self, at: usize) -> usize {
        self.marks.push(at);
        self.marks.len() - 1
    }

    /// Puts `s` in at `at`; a place at `at` stays before it.
    fn insert(&mut self, at: usize, s: &[char]) {
        self.c.splice(at..at, s.iter().copied());
        for m in &mut self.marks {
            if *m > at {
                *m += s.len();
            }
        }
    }

    fn delete(&mut self, from: usize, to: usize) {
        self.c.drain(from..to);
        for m in &mut self.marks {
            if *m >= to {
                *m -= to - from;
            } else if *m > from {
                *m = from;
            }
        }
    }

    fn insert_code(&mut self, at: usize, code: &str) {
        let mut s = vec![OPEN];
        s.extend(code.chars());
        s.push(CLOSE);
        self.insert(at, &s);
    }

    /// The code that starts at `p`: its text and its whole length (marks included).
    fn code_at(&self, p: usize) -> Option<(String, usize)> {
        if self.c.get(p) != Some(&OPEN) {
            return None;
        }
        let mut depth = 0;
        for (k, &ch) in self.c[p..].iter().enumerate() {
            match ch {
                OPEN => depth += 1,
                CLOSE => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((self.c[p + 1..p + k].iter().collect(), k + 1));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The start of the code that ends just before `p` (the character before `p` is its end mark).
    fn code_ending_at(&self, p: usize) -> Option<usize> {
        if p == 0 || self.c[p - 1] != CLOSE {
            return None;
        }
        let mut depth = 0;
        for k in (0..p).rev() {
            match self.c[k] {
                CLOSE => depth += 1,
                OPEN => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(k);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// For each place, whether it is inside a code other than at the code's own start.
    fn inside_codes(&self) -> Vec<bool> {
        let mut inside = vec![false; self.c.len() + 1];
        let mut p = 0;
        while p < self.c.len() {
            if let Some((_, len)) = self.code_at(p) {
                for v in inside.iter_mut().take(p + len).skip(p + 1) {
                    *v = true;
                }
                p += len;
            } else {
                p += 1;
            }
        }
        inside
    }
}

/// A paragraph style code (es~NAME or ee~NAME with NAME ending in a period): its start or end, and NAME. "es~"
/// and "ee~" are compared exactly, so [ES~X.] is no paragraph style code here (the widening kinds take it in any
/// case).
fn paragraph_style(code: &str) -> Option<(bool, &str)> {
    let name = code.get(3..)?;
    if !name.ends_with('.') {
        return None;
    }
    match code.get(..3)? {
        "es~" => Some((true, name)),
        "ee~" => Some((false, name)),
        _ => None,
    }
}

/// Whether the characters at `p` are `pat`, case ignored.
fn matches_at(t: &Text, p: usize, pat: &[char]) -> bool {
    p + pat.len() <= t.c.len() && pat.iter().zip(&t.c[p..]).all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// The code kinds for widening a paragraph style's range (spec 31 1.3): 1 ind, 4 paragraph openers, 5 l,
/// 8 pg, 9 paragraph closers, 0 any other code.
fn kind(code: &str) -> u8 {
    let mut name = String::new();
    for ch in code.chars() {
        if ch == '~' || ch.is_ascii_digit() {
            break;
        }
        name.push(ch.to_ascii_lowercase());
    }
    if name.ends_with('-') && code.contains('~') {
        name.pop();
    }
    let paragraph = code.get(3..).is_some_and(|n| n.ends_with('.'));
    match name.as_str() {
        "p" | "hds" | "tls" => 4,
        "es" if paragraph => 4,
        "l" => 5,
        "ind" => 1,
        "<" | "hde" | "sk" | "tle" => 9,
        "ee" if paragraph => 9,
        "pg" => 8,
        _ => 0,
    }
}

/// The list of line codes (Join Lines and the cleanup after a paragraph style is applied): the code's name
/// up to "~", compared whole and in any case.
fn line_code(code: &str) -> Option<&'static str> {
    let name = code.split('~').next().unwrap_or("").to_ascii_lowercase();
    ["p", "es", "hds", "tls", "l", "ptye", "ind", "hl", "ptys", "<", "ee", "hde", "tle", "gd"].into_iter().find(|n| *n == name)
}

/// True when nothing but codes lies between `pos` and the paragraph boundary before it (spec 31 1.4).
fn only_codes_before(t: &Text, inside: &[bool], pos: usize) -> bool {
    let mut p = pos;
    while p > 0 {
        p -= 1;
        if inside[p] {
            continue;
        }
        if let Some((code, _)) = t.code_at(p) {
            if matches!(kind(&code), 4 | 8 | 9) {
                return true;
            }
        } else {
            return false;
        }
    }
    true
}

/// Where a paragraph style's range starts when widened back from `s`, and whether text was passed (spec 31 1.5).
fn widen_start(t: &Text, s: usize) -> (usize, bool) {
    let inside = t.inside_codes();
    let mut flag = false;
    let mut p = s.min(t.c.len());
    loop {
        if p < t.c.len() && !inside[p] {
            if let Some((code, len)) = t.code_at(p) {
                let k = match kind(&code) {
                    1 if !only_codes_before(t, &inside, p) => 4,
                    5 if !only_codes_before(t, &inside, p) => 9,
                    k => k,
                };
                match k {
                    4 if p < s => return (p, flag),
                    8 | 9 if p + len <= s => return (p + len, flag),
                    _ => {}
                }
            } else {
                flag = true;
            }
        }
        if p == 0 {
            return (0, flag);
        }
        p -= 1;
    }
}

/// Where a paragraph style's range ends when widened forward from `e` (spec 31 1.6).
fn widen_end(t: &Text, e: usize, mut flag: bool) -> usize {
    let mut p = e;
    while p < t.c.len() {
        if let Some((code, len)) = t.code_at(p) {
            match kind(&code) {
                4 | 8 => return p,
                9 => return p + len,
                1 if flag => return p,
                5 if flag => return p + len,
                _ => {}
            }
            p += len;
        } else {
            flag = true;
            p += 1;
        }
    }
    t.c.len()
}

/// The cleanup after a paragraph style's codes are in (spec 31 2): `s` is the new start code, `e` just after the new
/// end code.
fn clean_up(t: &mut Text, s: usize, e: usize) {
    let ms = t.mark(s);
    let me = t.mark(e);
    // Line codes right before the new end code go (the list check never matches es or ee here).
    if let Some(end_code) = t.code_ending_at(e) {
        if t.code_at(end_code).is_some_and(|(c, _)| paragraph_style(&c).is_some()) {
            let mut at = end_code;
            while let Some(before) = t.code_ending_at(at) {
                let (code, _) = t.code_at(before).unwrap_or_default();
                let deletable = before > t.marks[ms] && line_code(&code).is_some_and(|n| n != "es" && n != "ee");
                if !deletable {
                    break;
                }
                t.delete(before, at);
                at = before;
            }
        }
    }
    // Inside: paragraph style codes go, a run of them leaving one new-line code; a new-line or l code at the start goes.
    let mut from = t.marks[ms];
    if t.code_at(from).is_some_and(|(c, _)| paragraph_style(&c).is_some()) {
        from += t.code_at(from).map(|(_, l)| l).unwrap_or(0);
    }
    let end_code = t.code_ending_at(t.marks[me]).unwrap_or(t.marks[me]);
    let mto = t.mark(end_code);
    let mut p = from;
    let mut last_deleted_at: Option<usize> = None;
    while p < t.marks[mto] {
        match t.code_at(p) {
            Some((code, len)) if paragraph_style(&code).is_some() => {
                t.delete(p, p + len);
                if p > from && p < t.marks[mto] && last_deleted_at != Some(p) {
                    t.insert_code(p, "<");
                    last_deleted_at = Some(p + 3);
                    p += 3;
                } else {
                    last_deleted_at = Some(p);
                }
            }
            Some((code, len)) if p == from && (code == "<" || code.eq_ignore_ascii_case("l")) => {
                t.delete(p, p + len);
                last_deleted_at = Some(p);
            }
            Some((_, len)) => p += len,
            None => p += 1,
        }
    }
    // An enclosing paragraph style the range was cut out of is closed before it and opened again after it.
    let (s, e) = (t.marks[ms], t.marks[me]);
    let next = (e..t.c.len()).find_map(|p| t.code_at(p).and_then(|(c, _)| paragraph_style(&c).map(|(st, n)| (st, n.to_string()))));
    if let Some((false, name)) = next {
        t.insert_code(e, &format!("es~{name}"));
    }
    let prev = (0..s).rev().find_map(|p| t.code_at(p).and_then(|(c, _)| paragraph_style(&c).map(|(st, n)| (st, n.to_string()))));
    if let Some((true, name)) = prev {
        t.insert_code(s, &format!("ee~{name}"));
    }
}

/// Applies style `name` to `from`..`to` (an empty range at the caret when equal): a
/// paragraph style (name ending in a period) is widened to whole paragraphs and cleaned up inside; the codes go in end
/// first. Answers the running text and the caret.
pub fn apply_style(running: &str, from: usize, to: usize, caret: usize, name: &str) -> (String, usize) {
    let mut t = Text::new(running);
    let m0 = t.mark(caret);
    let c = apply_in(&mut t, from, to, m0, name);
    (t.c.iter().collect(), c)
}

/// The style apply on text whose places are kept (`m0` is the caret's mark); answers where the caret goes.
fn apply_in(t: &mut Text, from: usize, to: usize, m0: usize, name: &str) -> usize {
    let widen = name.ends_with('.');
    let (mut s, mut e) = (from.min(to), from.max(to));
    if widen {
        s = widen_start(t, s).0;
        let flag = widen_start(t, e).1;
        e = widen_end(t, e, flag);
    }
    let selection = from != to;
    let start_code = format!("es~{name}");
    let end_code = format!("ee~{name}");
    let start_len = start_code.chars().count() + 2;
    t.insert_code(e, &end_code);
    let after_end = e + end_code.chars().count() + 2;
    t.insert_code(s, &start_code);
    let after_end = after_end + start_len;
    if widen {
        clean_up(t, s, after_end);
        let c = t.marks[m0];
        if c != s {
            return c;
        }
        // The caret at the widened start goes just after the new start code, found forward from it with case
        // ignored (the cleanup may have closed an enclosing style before it).
        let mut whole = vec![OPEN];
        whole.extend(start_code.chars());
        whole.push(CLOSE);
        return (c..t.c.len()).find(|&q| matches_at(t, q, &whole)).map_or(c, |q| q + whole.len());
    }
    if selection {
        after_end
    } else {
        s + start_len
    }
}

/// The paragraph style open at `pos` (spec 31 5.2, A12.7 1 and 2): the nearest paragraph style start found back
/// from `pos` to `page_start` (the first place of the formatted page `pos` is on), unless an end of that style lies
/// between it and `pos`. With none on the page: none on the first page (`page_start` 0); else the paragraph style
/// open at the page's start (style_level::page_style, which any paragraph style end closes), unless its end lies
/// between the page start and the place before `pos`. The searches take "es~" and "ee~" in any case.
pub fn open_paragraph_style(running: &str, pos: usize, page_start: usize) -> Option<String> {
    open_style_in(&Text::new(running), pos, page_start)
}

fn open_style_in(t: &Text, pos: usize, page_start: usize) -> Option<String> {
    let pos = pos.min(t.c.len());
    let start: Vec<char> = format!("{OPEN}es~").chars().collect();
    let ended = |name: &str, from: usize, to: usize| {
        let end: Vec<char> = format!("{OPEN}ee~{name}{CLOSE}").chars().collect();
        (from..to).any(|q| matches_at(t, q, &end))
    };
    let mut p = pos;
    while p > page_start {
        p -= 1;
        if !matches_at(t, p, &start) {
            continue;
        }
        let Some((code, _)) = t.code_at(p) else { continue };
        let Some((true, name)) = paragraph_style(&code) else { continue };
        return (!ended(name, p + 1, pos)).then(|| name.to_string());
    }
    if page_start == 0 {
        return None;
    }
    let running: String = t.c.iter().collect();
    let (name, _) = crate::style_level::page_style(&running, page_start)?;
    (!ended(&name, page_start, pos.saturating_sub(1))).then_some(name)
}

/// Layout, Join Styles (spec 31 5.3): the paragraph style open at the selection's anchor, else at the caret, applied
/// again over the selection. None when there is no selection or no such style. `page_start` gives the first place
/// of the formatted page a place is on (open_paragraph_style). INFERENCE: the page searched for each place is the
/// page that place is on.
pub fn join_styles(
    running: &str,
    anchor: usize,
    caret: usize,
    page_start: &dyn Fn(usize) -> usize,
) -> Option<(String, usize)> {
    if anchor == caret {
        return None;
    }
    let mut t = Text::new(running);
    let mc = t.mark(caret);
    join_styles_in(&mut t, anchor, caret, page_start).then(|| (t.c.iter().collect(), t.marks[mc]))
}

/// Join Styles on text whose places are kept; false when no paragraph style is open at the anchor or the caret.
fn join_styles_in(t: &mut Text, anchor: usize, caret: usize, page_start: &dyn Fn(usize) -> usize) -> bool {
    let found = open_style_in(t, anchor, page_start(anchor)).or_else(|| open_style_in(t, caret, page_start(caret)));
    let Some(name) = found else {
        return false;
    };
    let m0 = t.mark(caret);
    apply_in(t, anchor, caret, m0, &name);
    true
}

/// Layout, Join Lines (spec 31 5.4): Join Styles, then every line code and style code inside the selection taken out
/// (a paragraph style code ending at the selection's end is kept), with a space where neither side has one (the
/// end of the text counts as no space). A code is read from its start mark to the first end mark after it, so a
/// code inside a code ends the outer one there. None when there is no selection.
pub fn join_lines(
    running: &str,
    anchor: usize,
    caret: usize,
    page_start: &dyn Fn(usize) -> usize,
) -> Option<(String, usize)> {
    if anchor == caret {
        return None;
    }
    let (s0, e0) = (anchor.min(caret), anchor.max(caret));
    let mut t = Text::new(running);
    let mc = t.mark(caret);
    let ms = t.mark(s0);
    let me = t.mark(e0);
    // Join Styles first, with the selection ends carried through it.
    join_styles_in(&mut t, anchor, caret, page_start);
    let mut p = t.marks[me];
    while p > t.marks[ms] + 1 {
        p -= 1;
        if t.c[p] != OPEN {
            continue;
        }
        let Some(close) = t.c[p + 1..].iter().position(|&c| c == CLOSE).map(|k| p + 1 + k) else { continue };
        let code: String = t.c[p + 1..close].iter().collect();
        let end = close + 1;
        let deletable = line_code(&code).is_some() && (end < t.marks[me] || paragraph_style(&code).is_none());
        if deletable {
            let before = if p > 0 { Some(t.c[p - 1]) } else { None };
            let after = t.c.get(end).copied();
            t.delete(p, end);
            if before != Some(' ') && after != Some(' ') {
                t.insert(p, &[' ']);
            }
        }
    }
    Some((t.c.iter().collect(), t.marks[mc]))
}

/// Layout, New Paragraph (spec 31, A12.4): inside an open paragraph style it ends the style and starts it again;
/// elsewhere it puts in the paragraph code; with a selection it does so at its end, then at the lesser of the
/// anchor and the caret as they stand then. The anchor does not move with what is put in, but the caret does: with
/// the anchor before the caret the second codes go at the selection's start; with the caret before the anchor the
/// caret stands after the first codes, so the anchor is the lesser and both go at the selection's end, the second
/// before the first. The caret ends just after the codes put in last. Answers the running text and the caret.
pub fn new_paragraph(
    running: &str,
    anchor: usize,
    caret: usize,
    page_start: &dyn Fn(usize) -> usize,
) -> (String, usize) {
    let (s, e) = (anchor.min(caret), anchor.max(caret));
    let points = if s == e {
        vec![e]
    } else if anchor < caret {
        vec![e, s]
    } else {
        vec![e, e]
    };
    let mut t = Text::new(running);
    let mut caret = 0;
    // Each place is before the codes put in earlier (the second one is at or before the first).
    for at in points {
        let codes = match open_style_in(&t, at, page_start(at)) {
            Some(name) => vec![format!("ee~{name}"), format!("es~{name}")],
            None => vec!["p".to_string()],
        };
        let mut q = at;
        for code in codes {
            t.insert_code(q, &code);
            q += code.chars().count() + 2;
        }
        caret = q;
    }
    (t.c.iter().collect(), caret)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> String {
        s.replace('[', "\u{1c}").replace(']', "\u{1f}")
    }
    fn shown(s: &str) -> String {
        s.replace('\u{1c}', "[").replace('\u{1f}', "]")
    }

    #[test]
    fn a_paragraph_style_widens_to_its_paragraph() {
        let text = run("[p]one two three[p]next");
        // Caret in "two", no selection: the style takes the whole paragraph, the paragraph code before it inside.
        let (out, caret) = apply_style(&text, 7, 7, 7, "list.");
        assert_eq!(shown(&out), "[es~list.][p]one two three[ee~list.][p]next");
        assert!(caret > 0);
        // A character style goes exactly round the selection.
        let (out, _) = apply_style(&text, 3, 6, 6, "bold");
        assert_eq!(shown(&out), "[p][es~bold]one[ee~bold] two three[p]next");
    }

    #[test]
    fn open_style_is_found_back_from_the_place() {
        let text = run("[es~quote.]a b[ee~quote.] c");
        assert_eq!(open_paragraph_style(&text, 12, 0).as_deref(), Some("quote."));
        assert_eq!(open_paragraph_style(&text, text.chars().count(), 0), None);
    }

    #[test]
    fn join_lines_takes_out_line_codes_with_a_space() {
        let text = run("one[l]two[l]three");
        let (out, _) = join_lines(&text, 0, text.chars().count(), &|_| 0).unwrap();
        assert_eq!(shown(&out), "one two three");
        assert!(join_lines(&text, 2, 2, &|_| 0).is_none());
    }

    #[test]
    fn new_paragraph_inside_a_style_ends_and_starts_it() {
        let text = run("[es~quote.]ab[ee~quote.]");
        let (out, caret) = new_paragraph(&text, 12, 12, &|_| 0);
        assert_eq!(shown(&out), "[es~quote.]a[ee~quote.][es~quote.]b[ee~quote.]");
        assert_eq!(caret, 12 + 22);
        let (out, _) = new_paragraph(&run("ab"), 1, 1, &|_| 0);
        assert_eq!(shown(&out), "a[p]b");
    }

    #[test]
    fn caret_follows_the_new_start_code_when_a_style_is_closed_before_it() {
        let text = run("[es~a.]one[<]two[ee~a.]");
        let at = "[es~a.]one[<]".chars().count();
        let (out, caret) = apply_style(&text, at, at, at, "b.");
        assert_eq!(shown(&out), "[es~a.]one[<][ee~a.][es~b.]two[ee~b.]");
        assert_eq!(caret, "[es~a.]one[<][ee~a.][es~b.]".chars().count());
    }

    #[test]
    fn joining_two_paragraphs_of_a_style_leaves_one_new_line_code() {
        let text = run("[es~X.]A[ee~X.][es~X.]B[ee~X.]");
        let (anchor, caret) = (7, text.chars().count() - 8);
        let (out, _) = join_styles(&text, anchor, caret, &|_| 0).unwrap();
        assert_eq!(shown(&out), "[es~X.]A[<]B[ee~X.]");
    }

    #[test]
    fn upper_case_style_codes_are_not_paragraph_codes() {
        assert_eq!(open_paragraph_style(&run("[ES~q.]ab"), 9, 0), None);
        assert_eq!(open_paragraph_style(&run("[es~q.]ab"), 9, 0).as_deref(), Some("q."));
    }

    #[test]
    fn join_styles_without_a_selection_does_nothing() {
        let text = run("[es~a.]one two[ee~a.]");
        assert!(join_styles(&text, 9, 9, &|_| 0).is_none());
    }

    #[test]
    fn join_styles_takes_the_style_at_the_anchor_first() {
        let text = run("[es~a.]one[ee~a.][es~b.]two[ee~b.]");
        let (anchor, caret) = (8, "[es~a.]one[ee~a.][es~b.]t".chars().count());
        let (out, _) = join_styles(&text, anchor, caret, &|_| 0).unwrap();
        assert_eq!(shown(&out), "[es~a.]one[<]two[ee~a.]");
    }

    #[test]
    fn the_open_style_search_stays_on_the_page_then_takes_the_page_start_style() {
        // The style started before the page start is still open there: found through the page-start style.
        let text = run("[es~a.]one two three");
        let page = "[es~a.]one ".chars().count();
        assert_eq!(open_paragraph_style(&text, page + 3, page).as_deref(), Some("a."));
        // An end of another paragraph style before the page start closes the page-start style.
        let text = run("[es~a.]one[ee~b.] two three");
        let page = "[es~a.]one[ee~b.] ".chars().count();
        assert_eq!(open_paragraph_style(&text, page + 3, page), None);
        // On the first page the same text still finds "a.", as no end of "a." follows it.
        assert_eq!(open_paragraph_style(&text, page + 3, 0).as_deref(), Some("a."));
    }

    #[test]
    fn join_lines_keeps_a_paragraph_end_at_the_selection_end() {
        let text = run("[es~p.]a[l]b[ee~p.]");
        let (out, _) = join_lines(&text, 0, text.chars().count(), &|_| 0).unwrap();
        assert_eq!(shown(&out), "[es~p.]a b[ee~p.]");
    }

    #[test]
    fn join_lines_removes_character_style_codes() {
        let text = run("a[es~b]x[ee~b][l]y");
        let (out, _) = join_lines(&text, 0, text.chars().count(), &|_| 0).unwrap();
        assert_eq!(shown(&out), "a x y");
    }

    #[test]
    fn join_lines_code_at_text_end_gets_a_space() {
        let text = run("ab[l]");
        let (out, _) = join_lines(&text, 0, text.chars().count(), &|_| 0).unwrap();
        assert_eq!(shown(&out), "ab ");
    }

    #[test]
    fn new_paragraph_backward_selection_puts_both_breaks_at_the_end() {
        let text = run("abcdefgh");
        let (forward, caret) = new_paragraph(&text, 2, 6, &|_| 0);
        assert_eq!(shown(&forward), "ab[p]cdef[p]gh");
        assert_eq!(caret, 5);
        let (backward, caret) = new_paragraph(&text, 6, 2, &|_| 0);
        assert_eq!(shown(&backward), "abcdef[p][p]gh");
        assert_eq!(caret, 9);
    }

    #[test]
    fn new_paragraph_caret_after_the_codes_at_the_start() {
        let text = run("[es~q.]abcd[ee~q.]");
        let (out, caret) = new_paragraph(&text, 8, 10, &|_| 0);
        assert_eq!(shown(&out), "[es~q.]a[ee~q.][es~q.]bc[ee~q.][es~q.]d[ee~q.]");
        assert_eq!(caret, "[es~q.]a[ee~q.][es~q.]".chars().count());
    }
}
