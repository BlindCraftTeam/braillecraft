//! Edit, Find, Find Again and Replace, as the original searches (spec 32). The text is the running text (codes marked
//! 0x1C ... 0x1F) with the two codes [-] and ['] taken as one unit each (0x1D and 0x1E, as the original keeps them);
//! places given to and answered by this module are places in the running text.

use crate::doctext::{CLOSE, OPEN};

const HYPHEN: char = '\u{1d}';
const APOSTROPHE: char = '\u{1e}';
const WILD: char = '\u{1b}';

/// A search: what is looked for and how, with the state the original keeps between searches of one window.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Search {
    /// The text looked for, in running form (codes marked).
    pub text: String,
    pub match_case: bool,
    pub backward: bool,
    pub whole_words: bool,
    pub stylename: bool,
    pub wildcard: bool,
    /// The search is for a style's start code (Find stylename with a bare name).
    pub style: bool,
    /// The selection at the time of the command: whether there was one, the caret and the anchor.
    pub had_selection: bool,
    pub caret: usize,
    pub anchor: usize,
    pub wrapped: bool,
    pub selection_done: bool,
    pub found: bool,
}

/// The questions a search can ask (spec 32 2.2 and 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Not found in the selection: search the rest of the document?
    RestOfDocument,
    /// The end was reached: continue at the beginning?
    WrapToStart,
    /// The beginning was reached: continue at the end?
    WrapToEnd,
}

/// The text in units: [-] and ['] codes as one unit each; `at[k]` is the running-text place of unit k.
pub struct Units {
    pub u: Vec<char>,
    pub at: Vec<usize>,
    inside: Vec<bool>,
}

impl Units {
    pub fn new(running: &str) -> Units {
        let r: Vec<char> = running.chars().collect();
        let (mut u, mut at) = (Vec::with_capacity(r.len()), Vec::with_capacity(r.len() + 1));
        let mut i = 0;
        while i < r.len() {
            if r[i] == OPEN && i + 2 < r.len() && r[i + 2] == CLOSE && (r[i + 1] == '-' || r[i + 1] == '\'') {
                u.push(if r[i + 1] == '-' { HYPHEN } else { APOSTROPHE });
                at.push(i);
                i += 3;
            } else {
                u.push(r[i]);
                at.push(i);
                i += 1;
            }
        }
        at.push(r.len());
        // A place after a code's start mark, up to and including its end mark, is inside the code.
        let mut inside = vec![false; u.len() + 1];
        let mut depth = 0usize;
        for (k, &c) in u.iter().enumerate() {
            if depth > 0 {
                inside[k] = true;
            }
            match c {
                OPEN => depth += 1,
                CLOSE if depth > 0 => depth -= 1,
                _ => {}
            }
        }
        Units { u, at, inside }
    }

    /// The unit at or after running-text place `p`.
    pub fn unit_of(&self, p: usize) -> usize {
        self.at.partition_point(|&a| a < p)
    }
}

/// The pattern in units, as the original makes it from the search: codes [-] and ['] as single units, a "*" right
/// before a code's end mark a wildcard when wildcards are on, and for a style search the style's start code.
fn pattern(s: &Search) -> Vec<char> {
    if s.style {
        let mut p = vec![OPEN];
        p.extend("ES~".chars());
        p.extend(s.text.chars().flat_map(char::to_uppercase));
        p.push(CLOSE);
        return p;
    }
    let mut p = Units::new(&s.text).u;
    if s.wildcard {
        for k in 0..p.len().saturating_sub(1) {
            if p[k] == '*' && p[k + 1] == CLOSE {
                p[k] = WILD;
            }
        }
    }
    p
}

fn fold(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// A match: its end unit, and the spans (start, end) in units: literal, wildcard, literal, ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub spans: Vec<(usize, usize)>,
}

fn match_at(t: &Units, pos: usize, pat: &[char], exact: bool) -> Option<Match> {
    let u = &t.u;
    let skip_hyphen = !pat.contains(&HYPHEN);
    let same = |a: char, b: char| if exact { a == b } else { fold(a) == fold(b) };
    let (mut i, mut j) = (pos, 0);
    let mut spans = Vec::new();
    let mut span_start = pos;
    while j < pat.len() {
        if pat[j] == WILD {
            spans.push((span_start, i));
            let start = i;
            let term = pat.get(j + 1).copied().unwrap_or(CLOSE);
            while i < u.len() && fold(u[i]) != fold(term) {
                i += 1;
            }
            spans.push((start, i));
            span_start = i;
            j += 1;
            continue;
        }
        if skip_hyphen {
            while i < u.len() && u[i] == HYPHEN {
                i += 1;
            }
        }
        if i >= u.len() || !same(u[i], pat[j]) {
            return None;
        }
        i += 1;
        j += 1;
    }
    spans.push((span_start, i));
    Some(Match { start: pos, end: i, spans })
}

/// The original's whole-word test on [start, start + pattern length) (spec 32 2.4); `print` is a print document.
fn whole_word(t: &Units, start: usize, plen: usize, print: bool) -> bool {
    let u = &t.u;
    let mut k = start;
    loop {
        if k == 0 {
            break;
        }
        k -= 1;
        let c = u[k];
        if c == CLOSE || c == ' ' || c == APOSTROPHE || (print && c == '@') {
            break;
        }
        if print && (c == '\'' || c == '"') {
            continue;
        }
        return false;
    }
    let end = start + plen;
    if end >= u.len() {
        return true;
    }
    for &c in &u[end..] {
        if c == OPEN || c == ' ' || c == APOSTROPHE || (print && (c == '@' || c == '.')) {
            return true;
        }
        if print && ",?!;:-'\"".contains(c) {
            continue;
        }
        return false;
    }
    false
}

/// One search from unit `from` up to `limit` (spec 32 2.1): backward when the limit is before `from` (the limit
/// included), else forward (the limit excluded; None is the end of the text).
fn search_once(t: &Units, from: usize, limit: Option<usize>, s: &Search, print: bool) -> Option<Match> {
    let pat = pattern(s);
    let exact = s.match_case && !s.style;
    let whole = s.whole_words && !s.style;
    let fits = |p: usize| -> Option<Match> {
        if t.inside[p] && p != from {
            return None;
        }
        let m = match_at(t, p, &pat, exact)?;
        (!whole || whole_word(t, p, pat.len(), print)).then_some(m)
    };
    match limit {
        Some(l) if l < from => (l..=from.min(t.u.len())).rev().find_map(fits),
        _ => {
            let end = limit.unwrap_or(t.u.len() + 1).min(t.u.len() + 1);
            (from..end).find_map(|p| if p <= t.u.len() { fits(p) } else { None })
        }
    }
}

/// The limits of spec 32 2.1 (mode 1 the selection, mode 2 after wrapping), in units.
fn limit(t: &Units, s: &Search, mode: u8) -> Option<usize> {
    let (c, a) = (t.unit_of(s.caret), t.unit_of(s.anchor));
    let (lo, hi) = (c.min(a), c.max(a));
    match (mode, s.backward) {
        (1, false) => s.had_selection.then_some(hi),
        (1, true) => Some(if s.had_selection { lo } else { 0 }),
        (2, false) => Some(if s.had_selection { lo } else { c }),
        (2, true) => Some(if s.had_selection { hi } else { c } + 1),
        (_, false) => None,
        (_, true) => Some(0),
    }
}

/// A search as the original makes it from the caret (running-text place) with its questions (spec 32 2.2, 2.3):
/// the selection first, then the rest of the document, then the other end. `replace_all` is the mode of Replace All
/// (its questions differ). Answers the match in running-text places (start, end) and the spans, and keeps the
/// search's state up to date.
pub fn find(
    running: &str,
    caret: usize,
    s: &mut Search,
    print: bool,
    replace_all: bool,
    ask: &mut dyn FnMut(Question) -> bool,
) -> Option<(usize, usize, Vec<(usize, usize)>)> {
    let t = Units::new(running);
    let to_running = |m: Match| {
        let spans = m.spans.iter().map(|&(a, b)| (t.at[a], t.at[b])).collect();
        (t.at[m.start], t.at[m.end], spans)
    };
    let mut from = t.unit_of(caret);
    if s.had_selection && !s.selection_done && !s.found {
        let (c, a) = (t.unit_of(s.caret), t.unit_of(s.anchor));
        from = if s.backward { c.max(a) } else { c.min(a) };
        if let Some(m) = search_once(&t, from, limit(&t, s, 1), s, print) {
            s.found = true;
            return Some(to_running(m));
        }
        if replace_all || !ask(Question::RestOfDocument) {
            return None;
        }
        s.selection_done = true;
    }
    let mode = if s.wrapped { 2 } else { 0 };
    if let Some(m) = search_once(&t, from, limit(&t, s, mode), s, print) {
        s.found = true;
        return Some(to_running(m));
    }
    if s.wrapped {
        return None;
    }
    s.wrapped = true;
    let l2 = limit(&t, s, 2).unwrap_or(0);
    let again = if !s.backward {
        (l2 > 0 && ask(Question::WrapToStart)).then_some(0)
    } else {
        (l2 < t.u.len() && ask(Question::WrapToEnd)).then_some(t.u.len())
    };
    let m = search_once(&t, again?, limit(&t, s, 2), s, print)?;
    s.found = true;
    Some(to_running(m))
}

/// The replacement of a plain or wildcard match (spec 32 3.3): each literal part becomes the next part of `with`
/// (split at its wildcards), each wildcard span is kept when `with` has that many wildcards, else taken out. In a
/// braille document `with` is put in lower case first. Answers the running text that replaces the match.
pub fn replacement(running: &str, spans: &[(usize, usize)], with: &str, wildcard: bool, braille: bool) -> String {
    let r: Vec<char> = running.chars().collect();
    let mut with = if braille { with.to_lowercase() } else { with.to_string() };
    let mut parts: Vec<String> = vec![with.clone()];
    if wildcard {
        let marked = with.replace(&format!("*{CLOSE}"), &format!("{WILD}{CLOSE}"));
        with = marked;
        parts = with.split(WILD).map(str::to_string).collect();
    }
    let stars = parts.len() - 1;
    let mut out = String::new();
    for (p, &(a, b)) in spans.iter().enumerate() {
        if p % 2 == 0 {
            out.push_str(parts.get(p / 2).map(String::as_str).unwrap_or(""));
        } else if (p + 1) / 2 <= stars {
            out.extend(&r[a..b]);
        }
    }
    out
}

/// The end code of the style whose start code is at `start` (running text): the first [ee~NAME] after it, in any case.
pub fn style_end_after(running: &str, start: usize, name: &str) -> Option<(usize, usize)> {
    let r: Vec<char> = running.chars().collect();
    let code: Vec<char> = std::iter::once(OPEN).chain(format!("ee~{name}").chars()).chain(std::iter::once(CLOSE)).collect();
    (start..r.len().saturating_sub(code.len() - 1))
        .find(|&p| r[p..p + code.len()].iter().zip(&code).all(|(a, b)| fold(*a) == fold(*b)))
        .map(|p| (p, p + code.len()))
}

/// The running text of a codes field (codes written in brackets).
pub fn field_to_running(field: &str) -> String {
    field.chars().map(|c| match c {
        '[' => OPEN,
        ']' => CLOSE,
        c => c,
    }).collect()
}

/// A remembered search text as the dialog's codes field shows it: codes in brackets, or left out when the document
/// does not show codes (spec 32 1.1).
pub fn running_to_field(running: &str, codes: bool) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in running.chars() {
        match c {
            OPEN => {
                depth += 1;
                if codes {
                    out.push('[');
                }
            }
            CLOSE if depth > 0 => {
                depth -= 1;
                if codes {
                    out.push(']');
                }
            }
            c if codes || depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// What the original says about a find or replace field (spec 32 1.1, 1.3), or None: the find text 1 to 100
/// characters (2748, 2750), the replace text at most 100 (2726), codes properly started and ended (2264).
pub fn field_problem(field: &str, replace: bool) -> Option<String> {
    let n = field.chars().count();
    if !replace && n == 0 {
        Some("The search string is empty.".into())
    } else if n > 100 {
        Some(if replace {
            "The replace string must not be greater than 100 characters.".into()
        } else {
            "The search text must not be greater than 100 characters.".into()
        })
    } else if !crate::doctext::codes_properly_marked(field) {
        Some("Some codes are not properly started or terminated.".into())
    } else {
        None
    }
}

/// The style name of a running text that is one whole style start or end code ([es~NAME] or [ee~NAME], any case),
/// as the original tells a typed code from a bare name in stylename mode.
pub fn style_code_name(running: &str) -> Option<String> {
    let r: Vec<char> = running.chars().collect();
    let inner = r.strip_prefix(&[OPEN])?.strip_suffix(&[CLOSE])?;
    if inner.contains(&OPEN) || inner.len() < 3 || inner[2] != '~' {
        return None;
    }
    let ok = matches!(inner[0], 'e' | 'E') && matches!(inner[1], 's' | 'S' | 'e' | 'E');
    ok.then(|| inner[3..].iter().collect())
}

/// One replacement made (spec 32 3.3): the new running text, its edits (place, length taken out, length put in, in
/// the old text's places, left to right) and the caret after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    pub text: String,
    pub edits: Vec<(usize, usize, usize)>,
    pub caret: usize,
}

impl Replaced {
    /// Where an old place is after the edits: moved by the change in length of the edits before it; a place inside
    /// a taken-out part goes to its start.
    pub fn moved(&self, p: usize) -> usize {
        let mut shift = 0isize;
        for &(at, old, new) in &self.edits {
            if p >= at + old {
                shift += new as isize - old as isize;
            } else if p > at {
                return (at as isize + shift) as usize;
            }
        }
        (p as isize + shift) as usize
    }
}

/// Replaces match `m` (start, end and spans as `find` answers them) by `with` (running text): a plain or wildcard
/// match by `replacement`; a style search's start code and the end code after it by [es~WITH] and [ee~WITH] in
/// lower case, both taken out when `with` is empty. The caret goes after what was put in when searching forward
/// (after the new start code for a style), one unit before the match when searching backward.
pub fn replace_match(running: &str, m: &(usize, usize, Vec<(usize, usize)>), s: &Search, with: &str, braille: bool) -> Replaced {
    let r: Vec<char> = running.chars().collect();
    let (start, end) = (m.0, m.1);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    if s.style {
        let new = with.to_lowercase();
        let code = |kind: &str| if new.is_empty() { String::new() } else { format!("{OPEN}{kind}~{new}{CLOSE}") };
        edits.push((start, end - start, code("es")));
        if let Some((a, b)) = style_end_after(running, end, &s.text) {
            edits.push((a, b - a, code("ee")));
        }
    } else {
        edits.push((start, end - start, replacement(running, &m.2, with, s.wildcard, braille)));
    }
    let mut text = String::new();
    let mut at = 0;
    for (p, old, new) in &edits {
        text.extend(&r[at..*p]);
        text.push_str(new);
        at = p + old;
    }
    text.extend(&r[at..]);
    let caret = if s.backward {
        let u = Units::new(&text);
        u.at[u.unit_of(start).saturating_sub(1)]
    } else {
        start + edits[0].2.chars().count()
    };
    Replaced { edits: edits.into_iter().map(|(p, old, new)| (p, old, new.chars().count())).collect(), text, caret }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> String {
        s.replace('[', "\u{1c}").replace(']', "\u{1f}")
    }
    fn plain(text: &str) -> Search {
        Search { text: text.into(), ..Search::default() }
    }

    #[test]
    fn plain_search_folds_case_and_skips_starts_inside_codes() {
        let r = run("the [es~cat.]Cat sat");
        let mut s = plain("cat");
        let (a, b, _) = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        assert_eq!((a, b), (13, 16));
        let mut s = Search { match_case: true, ..plain("cat") };
        assert!(find(&r, 0, &mut s, true, false, &mut |_| false).is_none());
    }

    #[test]
    fn whole_words_in_print_documents() {
        let r = run("cats, cat. concat");
        let mut s = Search { whole_words: true, ..plain("cat") };
        let (a, _, _) = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        assert_eq!(a, 6);
    }

    #[test]
    fn wrap_asks_and_continues_at_the_beginning() {
        let r = run("cat and dog");
        let mut s = Search { caret: 5, anchor: 5, ..plain("cat") };
        let mut asked = Vec::new();
        let m = find(&r, 5, &mut s, true, false, &mut |q| {
            asked.push(q);
            true
        });
        assert_eq!(asked, vec![Question::WrapToStart]);
        assert_eq!(m.map(|m| m.0), Some(0));
    }

    #[test]
    fn stylename_finds_the_start_code() {
        let r = run("x[es~quote.]y[ee~quote.]");
        let mut s = Search { style: true, stylename: true, ..plain("quote.") };
        let (a, b, _) = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        assert_eq!((a, b), (1, 12));
        assert_eq!(style_end_after(&r, b, "quote."), Some((13, 24)));
    }

    #[test]
    fn replacing_moves_later_places_and_renames_styles() {
        let r = run("a cat, a cat");
        let mut s = plain("cat");
        let m = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        let done = replace_match(&r, &m, &s, "dog", false);
        assert_eq!((done.text.as_str(), done.caret, done.moved(12)), ("a dog, a cat", 5, 12));
        let r = run("x[es~quote.]y[ee~quote.]z");
        let mut s = Search { style: true, stylename: true, ..plain("quote.") };
        let m = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        assert_eq!(replace_match(&r, &m, &s, "Block", false).text, run("x[es~block]y[ee~block]z"));
        assert_eq!(replace_match(&r, &m, &s, "", false).text, "xyz");
    }

    #[test]
    fn dialog_fields_are_checked_as_the_original() {
        assert_eq!(field_problem("", false).as_deref(), Some("The search string is empty."));
        assert!(field_problem("", true).is_none());
        assert_eq!(field_problem("[es~x", false).as_deref(), Some("Some codes are not properly started or terminated."));
        assert_eq!(style_code_name(&run("[ES~Quote.]")).as_deref(), Some("Quote."));
        assert_eq!(style_code_name(&run("[p]")), None);
        assert_eq!(running_to_field(&run("a[p]b"), false), "ab");
    }

    #[test]
    fn wildcard_keeps_or_drops_its_span() {
        let r = run("a[fs12]b");
        let mut s = Search { wildcard: true, ..plain(&run("[fs*]")) };
        let (_, _, spans) = find(&r, 0, &mut s, true, false, &mut |_| false).unwrap();
        assert_eq!(replacement(&r, &spans, &run("[ft*]"), true, false), run("[ft12]"));
        assert_eq!(replacement(&r, &spans, &run("[ft]"), true, false), run("[ft]"));
    }
}
