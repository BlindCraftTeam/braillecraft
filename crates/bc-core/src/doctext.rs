//! Document text as it is kept and as the editor shows it.
//!
//! A document is kept as the reference program keeps it: running text with its formatting codes inline, each code
//! marked 0x1C code 0x1F (codes can nest: 0x1C hl4 0x1C es~para. 0x1F 0x1F). A bracket in the text is just a
//! character, never a code. A document holds no line breaks of its own: a line ends where a code says so.
//!
//! The editor shows a document as `Shown`: the text with each code written in brackets, [code], and the list of the
//! character ranges that are codes; brackets outside those ranges are text, as typed. The new-line code [<] is
//! shown as a line break, which is also what Enter puts in (Enter inserts [<], as in the reference program), and
//! the codes that start a new line of their own ([p] new paragraph, [l] new line if not at the start of one) are
//! followed by a line break that is the editor's own, not part of the document.

/// Opens a code in running text.
pub const OPEN: char = '\u{1c}';
/// Closes a code in running text.
pub const CLOSE: char = '\u{1f}';

/// Codes after which the editor starts a new line.
pub fn breaks_line(code: &str) -> bool {
    matches!(code, "p" | "l")
}

/// The idle code, [idle] or [idle~...]: the one code the translated line's "Hide Idle" setting leaves out (spec 24
/// 2d.2: the test is the code's first letters "idle" followed by its end or "~").
pub fn is_idle_code(code: &str) -> bool {
    code == "idle" || code.starts_with("idle~")
}

/// The new-line code, shown as a line break.
const NEW_LINE: &str = "<";

/// A document as the editor shows it: the text, and the character ranges (start, end) of its codes, each range
/// running from the opening bracket to just after the closing one, in order and not overlapping; `inner` holds the
/// same for codes inside a code ([hl4[es~para.]]). Other brackets, inside a code's text too ([q~@]] holds the
/// braille cell "]"), are text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown {
    pub text: String,
    pub codes: Vec<(usize, usize)>,
    pub inner: Vec<(usize, usize)>,
}

impl Shown {
    /// Text with no codes.
    pub fn plain(text: &str) -> Shown {
        Shown { text: text.to_string(), ..Shown::default() }
    }

    /// Text in which every bracketed run with no space or line break in it ([p], [hds2], [hl4[es~para.]]) is a
    /// code, every bracket inside it a code bracket: for code text the program writes itself (menu codes, styles)
    /// and for tests.
    pub fn marked(text: &str) -> Shown {
        let chars: Vec<char> = text.chars().collect();
        let mut running = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '[' {
                if let Some(end) = matching(&chars, i) {
                    for &c in &chars[i..=end] {
                        running.push(match c {
                            '[' => OPEN,
                            ']' => CLOSE,
                            c => c,
                        });
                    }
                    let name: String = chars[i + 1..end].iter().collect();
                    i = end + 1;
                    // The line break after [p] or [l] is the editor's own.
                    if breaks_line(&name) && chars.get(i) == Some(&'\n') {
                        i += 1;
                    }
                    continue;
                }
            }
            running.push(chars[i]);
            i += 1;
        }
        // A line break typed in it is the new-line code, as in the editor.
        let running = Shown::from_running(&running).to_running();
        Shown::from_running(&running)
    }

    /// Running text (codes marked 0x1C ... 0x1F) to the editor's form.
    pub fn from_running(running: &str) -> Shown {
        let mut text = String::with_capacity(running.len() + running.len() / 16);
        let mut codes = Vec::new();
        let mut inner = Vec::new();
        let mut len = 0usize;
        // Character positions of the open codes, and the byte position of the outermost one.
        let mut open: Vec<usize> = Vec::new();
        let mut outer_byte = 0usize;
        for c in running.chars() {
            match c {
                OPEN => {
                    if open.is_empty() {
                        outer_byte = text.len();
                    }
                    open.push(len);
                    text.push('[');
                    len += 1;
                }
                CLOSE if !open.is_empty() => {
                    let start = open.pop().unwrap_or(0);
                    if !open.is_empty() {
                        text.push(']');
                        len += 1;
                        inner.push((start, len));
                        continue;
                    }
                    let name = &text[outer_byte + 1..];
                    if name == NEW_LINE {
                        text.truncate(outer_byte);
                        text.push('\n');
                        len = start + 1;
                    } else {
                        let breaks = breaks_line(name);
                        text.push(']');
                        len += 1;
                        codes.push((start, len));
                        if breaks {
                            text.push('\n');
                            len += 1;
                        }
                    }
                }
                CLOSE => {}
                c => {
                    text.push(c);
                    len += 1;
                }
            }
        }
        Shown { text, codes, inner }
    }

    /// The editor's form to running text: each code's brackets (and those of codes inside it) as 0x1C and 0x1F; a
    /// line break right after a
    /// [p] or [l] code is the editor's own, any other line break is the new-line code.
    pub fn to_running(&self) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        let mut out = String::with_capacity(self.text.len() + self.text.len() / 16);
        let mut spans = self.codes.iter().peekable();
        let mut after_break_code = false;
        let mut i = 0;
        while i < chars.len() {
            if let Some(&&(start, end)) = spans.peek().filter(|s| s.0 == i) {
                spans.next();
                let end = end.min(chars.len());
                for (k, &c) in chars[start..end].iter().enumerate() {
                    let at = start + k;
                    let opens = at == start || self.inner.iter().any(|s| s.0 == at);
                    let closes = at + 1 == end || self.inner.iter().any(|s| s.1 == at + 1);
                    out.push(match c {
                        '[' if opens => OPEN,
                        ']' if closes => CLOSE,
                        c => c,
                    });
                }
                let name: String = chars[(start + 1).min(end)..end.saturating_sub(1).max(start + 1).min(end)].iter().collect();
                after_break_code = breaks_line(&name);
                i = end;
                continue;
            }
            let c = chars[i];
            i += 1;
            match c {
                '\n' if after_break_code => after_break_code = false,
                '\n' => {
                    out.push(OPEN);
                    out.push_str(NEW_LINE);
                    out.push(CLOSE);
                }
                c => {
                    out.push(c);
                    after_break_code = false;
                }
            }
        }
        out
    }

    /// The part from character `from` to character `to`, with the codes wholly inside it.
    pub fn slice(&self, from: usize, to: usize) -> Shown {
        let text: String = self.text.chars().skip(from).take(to.saturating_sub(from)).collect();
        let within = |v: &[(usize, usize)]| -> Vec<(usize, usize)> {
            v.iter().filter(|c| c.0 >= from && c.1 <= to).map(|c| (c.0 - from, c.1 - from)).collect()
        };
        Shown { text, codes: within(&self.codes), inner: within(&self.inner) }
    }

    /// This followed by `other`.
    pub fn push(&mut self, other: &Shown) {
        let at = self.text.chars().count();
        self.text.push_str(&other.text);
        self.codes.extend(other.codes.iter().map(|c| (c.0 + at, c.1 + at)));
        self.inner.extend(other.inner.iter().map(|c| (c.0 + at, c.1 + at)));
    }

    /// The code whose range starts at character `pos`: its name, without the brackets.
    pub fn code_at(&self, pos: usize) -> Option<String> {
        let span = self.codes.iter().find(|c| c.0 == pos)?;
        let name: String = self.text.chars().skip(span.0 + 1).take(span.1 - span.0 - 2).collect();
        Some(name)
    }

    /// Layout, Apply Last Style: the name of the nearest paragraph style ended before `pos`, read from the text as
    /// the original does (back from the caret or the selection start to the first style end code [ee~name] whose
    /// name ends in "."; codes inside a code count too). None when there is none.
    pub fn last_paragraph_style_before(&self, pos: usize) -> Option<String> {
        let mut spans: Vec<(usize, usize)> = self.codes.iter().chain(self.inner.iter()).copied().filter(|c| c.0 < pos).collect();
        spans.sort_by(|a, b| b.0.cmp(&a.0));
        spans.into_iter().find_map(|(from, to)| {
            let name: String = self.text.chars().skip(from + 1).take(to - from - 2).collect();
            let style = name.strip_prefix("ee~")?;
            style.ends_with('.').then(|| style.to_string())
        })
    }

    /// Edit, Change Case, as the original: on `from`..`to`, or with no selection on the letters around the caret (a
    /// code ends the word). With no lower-case letter in the range the letters go to lower case; with only lower-case
    /// letters, and a character that is not a letter in the range or the range starting a word, each letter after a
    /// character that is not a letter (a code's end bracket counts) takes a capital; otherwise all go to upper case.
    /// Text inside codes is left as it is. Answers the range and the characters that change (place, new character),
    /// or None when the range has no cased letter.
    pub fn change_case(&self, from: usize, to: usize) -> Option<(usize, usize, Vec<(usize, char)>)> {
        let chars: Vec<char> = self.text.chars().collect();
        let in_code = |p: usize| self.codes.iter().any(|&(s, e)| s <= p && p < e);
        let letter = |p: usize| chars.get(p).is_some_and(|c| c.is_alphabetic()) && !in_code(p);
        let (from, to) = if from != to {
            (from.min(to), from.max(to))
        } else {
            let mut s = from;
            while s > 0 && letter(s - 1) {
                s -= 1;
            }
            let mut e = from;
            while e < chars.len() && letter(e) {
                e += 1;
            }
            (s, e)
        };
        let word_start = from == 0 || !chars[from - 1].is_alphabetic();
        let (mut upper, mut lower, mut other) = (false, false, false);
        for p in (from..to).filter(|&p| !in_code(p)) {
            let c = chars[p];
            if c.is_uppercase() {
                upper = true;
            } else if c.is_lowercase() {
                lower = true;
            } else if !c.is_alphabetic() {
                other = true;
            }
        }
        if !upper && !lower {
            return None;
        }
        enum Mode {
            Lower,
            Words,
            Upper,
        }
        let mode = if !lower {
            Mode::Lower
        } else if !upper && (other || word_start) {
            Mode::Words
        } else {
            Mode::Upper
        };
        // A case change that is one character, else the character as it is (the text keeps its length).
        fn upper_of(c: char) -> char {
            let mut it = c.to_uppercase();
            match (it.next(), it.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        }
        fn lower_of(c: char) -> char {
            let mut it = c.to_lowercase();
            match (it.next(), it.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        }
        let mut after_other = word_start;
        let mut changes = Vec::new();
        for p in from..to {
            let c = chars[p];
            if !in_code(p) {
                let new = match mode {
                    Mode::Lower => lower_of(c),
                    Mode::Upper => upper_of(c),
                    Mode::Words if after_other => upper_of(c),
                    Mode::Words => c,
                };
                if new != c {
                    changes.push((p, new));
                }
            }
            after_other = !c.is_alphabetic();
        }
        Some((from, to, changes))
    }

    /// The text with each code whose name `keep` refuses taken out, brackets and all, with the editor's own line
    /// break after it; the other codes stay as shown.
    pub fn text_keeping_codes(&self, keep: impl Fn(&str) -> bool) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        let mut out = String::new();
        let mut at = 0;
        for &(start, end) in &self.codes {
            if start < at || end > chars.len() {
                continue;
            }
            let name: String = chars[start + 1..end.saturating_sub(1).max(start + 1)].iter().collect();
            if !keep(&name) {
                out.extend(&chars[at..start]);
                at = if self.is_own_break(end) { end + 1 } else { end };
            }
        }
        out.extend(&chars[at..]);
        out
    }

    /// Whether the line break at character `pos` is the editor's own, after a [p] or [l] code.
    pub fn is_own_break(&self, pos: usize) -> bool {
        pos > 0
            && self.codes.iter().find(|c| c.1 == pos).is_some_and(|c| {
                let name: String = self.text.chars().skip(c.0 + 1).take(c.1 - c.0 - 2).collect();
                breaks_line(&name)
            })
    }
}

/// What an edit of the editor's text did to its codes (`after_edit`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Edited {
    /// The text after the edit, with its codes.
    pub shown: Shown,
    /// What is left of codes the edit broke (it took one of their brackets), in the new text's positions, last
    /// first: to be removed, since a code goes as a whole.
    pub remains: Vec<(usize, usize)>,
    /// The text the edit took out, when it held whole codes: kept so that pasting the same text puts its codes back.
    pub removed: Option<Shown>,
    /// Codes that have gone, each with the place it stood (a code with the editor's line break after it): when the
    /// same text stands at that place again (an undo puts it back, perhaps a piece at a time), it is a code again.
    pub taken: Vec<(usize, Shown)>,
}

/// The codes of `before` after its text became `text`: the edit is the one changed stretch between the two texts.
/// Codes before it stay, codes after it move with the text, a code edited inside (both its brackets kept) stays a
/// code, and a code that lost a bracket is listed in `remains`. When the stretch put in is the text of one of
/// `known` (text taken out or copied earlier, with codes), those codes come back with it; when the text of a code in
/// `taken` stands at its place again, it is a code again.
pub fn after_edit(before: &Shown, text: &str, known: &[Shown], taken: &[(usize, Shown)]) -> Edited {
    let a: Vec<char> = before.text.chars().collect();
    let b: Vec<char> = text.chars().collect();
    let p = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - p;
    let s = a.iter().rev().zip(b.iter().rev()).take(max_suffix).take_while(|(x, y)| x == y).count();
    let (old_end, new_end) = (a.len() - s, b.len() - s);
    let delta = b.len() as i64 - a.len() as i64;
    let shift = |x: usize| (x as i64 + delta) as usize;
    let mut remains = Vec::new();
    let mut broken: Vec<(usize, usize)> = Vec::new();
    let mut fix = |spans: &[(usize, usize)], top: bool| -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for &(st, en) in spans {
            if en <= p {
                out.push((st, en));
            } else if st >= old_end {
                out.push((shift(st), shift(en)));
            } else if st < p && en > old_end {
                out.push((st, shift(en)));
            } else if top {
                if st < p {
                    remains.push((st, p));
                }
                if en > old_end {
                    remains.push((new_end, shift(en)));
                }
                broken.push((st, en));
            }
        }
        out
    };
    let mut codes = fix(&before.codes, true);
    let mut inner = fix(&before.inner, false);
    // Codes taken earlier keep their place as the text moves.
    // Text put in exactly at the place leaves the place where it is: that is where an undo puts the code back.
    let mut gone: Vec<(usize, Shown)> = taken
        .iter()
        .map(|(at, s)| (if *at > old_end || (*at == old_end && old_end > p) { shift(*at) } else { *at }, s.clone()))
        .collect();
    // A [p] or [l] code that goes takes the editor's line break after it along; each code that goes is kept with
    // its place.
    for &(st, en) in &broken {
        let name: String = a[(st + 1).min(en)..en.saturating_sub(1).max(st + 1)].iter().collect();
        let own_break = breaks_line(&name) && a.get(en) == Some(&'\n');
        if own_break && en >= old_end {
            remains.push((shift(en), shift(en) + 1));
        }
        let at = if st >= old_end { shift(st) } else { st };
        gone.push((at, before.slice(st, if own_break { en + 1 } else { en })));
    }
    // Codes inside a broken code go with it.
    inner.retain(|&(st, en)| {
        !broken.iter().any(|&(bs, be)| {
            let (bs, be) = (if bs < p { bs } else { shift(bs) }, if be > old_end { shift(be) } else { be });
            st >= bs && en <= be
        })
    });
    let removed = (old_end > p).then(|| before.slice(p, old_end)).filter(|r| !r.codes.is_empty());
    if new_end > p && !codes.iter().any(|&(st, en)| st < p && en > p) {
        let put: String = b[p..new_end].iter().collect();
        if let Some(k) = known.iter().find(|k| k.text == put && !k.codes.is_empty()) {
            codes.extend(k.codes.iter().map(|c| (c.0 + p, c.1 + p)));
            inner.extend(k.inner.iter().map(|c| (c.0 + p, c.1 + p)));
        }
    }
    // A code taken earlier whose text stands at its place again, where no code is, is a code again; the ones that
    // came back this edit (in `broken`) wait for the next.
    let fresh = gone.len() - broken.len();
    let mut kept = Vec::new();
    for (k, (at, frag)) in gone.into_iter().enumerate() {
        let len = frag.text.chars().count();
        let back = k < fresh
            && at + len <= b.len()
            && b[at..at + len].iter().copied().eq(frag.text.chars())
            && !codes.iter().any(|&(st, en)| st < at + len && at < en);
        if back {
            codes.extend(frag.codes.iter().map(|c| (c.0 + at, c.1 + at)));
            inner.extend(frag.inner.iter().map(|c| (c.0 + at, c.1 + at)));
        } else {
            kept.push((at, frag));
        }
    }
    if kept.len() > 32 {
        kept.drain(..kept.len() - 32);
    }
    codes.sort_unstable();
    inner.sort_unstable();
    remains.sort_unstable_by(|x, y| y.cmp(x));
    Edited { shown: Shown { text: text.to_string(), codes, inner }, remains, removed, taken: kept }
}

/// The closing bracket matching the one at `i`, when the run between them holds no space or line break.
fn matching(chars: &[char], i: usize) -> Option<usize> {
    let mut depth = 0;
    for (j, &c) in chars.iter().enumerate().skip(i) {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return (j > i + 1).then_some(j);
                }
            }
            ' ' | '\n' => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_kept_or_taken_out() {
        let s = Shown::marked("a[p]b[idle~x]c[idle]d");
        // [p] is followed by the editor's own line break, which goes with it.
        assert_eq!(s.text_keeping_codes(|_| true), "a[p]\nb[idle~x]c[idle]d");
        assert_eq!(s.text_keeping_codes(|_| false), "abcd");
        assert_eq!(s.text_keeping_codes(|c| !is_idle_code(c)), "a[p]\nbcd");
        // A bracket that is text stays.
        let s = Shown { text: "x[y".into(), ..Shown::default() };
        assert_eq!(s.text_keeping_codes(|_| false), "x[y");
        assert!(!is_idle_code("idler"));
    }

    #[test]
    fn change_case_steps_as_the_original() {
        let apply = |s: &Shown, from: usize, to: usize| {
            let (_, _, ch) = s.change_case(from, to).unwrap();
            let mut t: Vec<char> = s.text.chars().collect();
            for (p, c) in ch {
                t[p] = c;
            }
            t.into_iter().collect::<String>()
        };
        // The word at the caret: lower, then capitals, then upper, then lower again.
        let s = Shown::plain("say hello now");
        assert_eq!(apply(&s, 6, 6), "say Hello now");
        assert_eq!(apply(&Shown::plain("say Hello now"), 6, 6), "say HELLO now");
        assert_eq!(apply(&Shown::plain("say HELLO now"), 6, 6), "say hello now");
        // A selection over words takes a capital at each word.
        assert_eq!(apply(&s, 0, 13), "Say Hello Now");
        // Lower-case letters inside a word, with nothing else selected, go to upper case.
        assert_eq!(apply(&s, 5, 7), "say hELlo now");
        // Codes are left as they are, and a code's end starts a word.
        let s = Shown::marked("[es~para.]one two");
        let n = s.text.chars().count();
        assert_eq!(apply(&s, 0, n), "[es~para.]One Two");
        // Nothing with a case: no change.
        assert!(Shown::plain("12 3").change_case(0, 4).is_none());
    }

    #[test]
    fn last_paragraph_style_is_read_back_from_the_text() {
        let s = Shown::marked("[es~para.]one[ee~para.] [es~em]two[ee~em] [es~h1.]x[ee~h1.] three");
        let end = s.text.chars().count();
        // The nearest end code before the place whose name ends in "."; a nested style ([ee~em]) is passed over.
        assert_eq!(s.last_paragraph_style_before(end).as_deref(), Some("h1."));
        let before_h1 = s.text.find("[es~h1.]").unwrap();
        assert_eq!(s.last_paragraph_style_before(before_h1).as_deref(), Some("para."));
        assert_eq!(s.last_paragraph_style_before(3), None);
        // Inside a code that holds a code.
        let s = Shown::marked("[hl4[ee~list.]]a");
        assert_eq!(s.last_paragraph_style_before(s.text.chars().count()).as_deref(), Some("list."));
        // Text that only looks like a code is not one.
        assert_eq!(Shown::plain("[ee~para.]").last_paragraph_style_before(10), None);
    }

    fn run(s: &str) -> String {
        s.replace('[', "\u{1c}").replace(']', "\u{1f}")
    }

    #[test]
    fn new_line_codes_are_line_breaks_in_the_editor() {
        let shown = Shown::from_running(&run("One[<]Two[<][<]Three"));
        assert_eq!(shown.text, "One\nTwo\n\nThree");
        assert!(shown.codes.is_empty());
        assert_eq!(shown.to_running(), run("One[<]Two[<][<]Three"));
    }

    #[test]
    fn paragraph_and_line_codes_are_followed_by_a_line_break_that_is_not_a_code() {
        let running = run("[es~h1.]Title[ee~h1.][p]First[l]Second[p][<]Third");
        let shown = Shown::from_running(&running);
        assert_eq!(shown.text, "[es~h1.]Title[ee~h1.][p]\nFirst[l]\nSecond[p]\n\nThird");
        assert_eq!(shown.codes, [(0, 8), (13, 21), (21, 24), (30, 33), (40, 43)]);
        assert_eq!(shown.to_running(), running);
        assert!(shown.is_own_break(24) && shown.is_own_break(43) && !shown.is_own_break(44));
        assert_eq!(shown.code_at(21).as_deref(), Some("p"));
    }

    #[test]
    fn brackets_in_the_text_are_text_and_nested_codes_are_one_code() {
        let running = format!("{}Entry{}See \"[hds2]\" and d[n ,*apt]", run("[hl4[es~para.]]"), run("[<]"));
        let shown = Shown::from_running(&running);
        assert_eq!(shown.text, "[hl4[es~para.]]Entry\nSee \"[hds2]\" and d[n ,*apt]");
        assert_eq!(shown.codes, [(0, 15)]);
        assert_eq!(shown.to_running(), running);
        // A bracket in a code's own text is text: [q~@]] is the direct braille "@]".
        let running = "\u{1c}q~@]\u{1f}1.";
        let shown = Shown::from_running(running);
        assert_eq!(shown.text, "[q~@]]1.");
        assert_eq!(shown.to_running(), running);
    }

    #[test]
    fn code_text_the_program_writes_is_marked_and_slices_keep_whole_codes() {
        let s = Shown::marked("[tx][g1]a [b");
        assert_eq!(s.codes, [(0, 4), (4, 8)]);
        let s = Shown::marked("x[<]y");
        assert_eq!(s.text, "x\ny");
        let mut t = Shown::plain("ab ");
        t.push(&Shown::marked("[p]"));
        assert_eq!(t.codes, [(3, 6)]);
        assert_eq!(t.slice(1, 6).codes, [(2, 5)]);
        assert!(t.slice(4, 6).codes.is_empty());
    }

    #[test]
    fn codes_follow_edits_and_go_as_a_whole() {
        let doc = Shown::marked("ab[ind5]cd");
        // Typing before a code moves it; typing a bracket makes no code.
        let e = after_edit(&doc, "x[ab[ind5]cd", &[], &[]);
        assert_eq!(e.shown.codes, [(4, 10)]);
        assert!(e.remains.is_empty());
        // Changing the number inside keeps the code.
        let e = after_edit(&doc, "ab[ind7]cd", &[], &[]);
        assert_eq!(e.shown.codes, [(2, 8)]);
        // Deleting its closing bracket breaks it: what is left of it is to go, and the deleted text is kept.
        let e = after_edit(&doc, "ab[ind5cd", &[], &[]);
        assert!(e.shown.codes.is_empty());
        assert_eq!(e.remains, [(2, 7)]);
        assert!(e.removed.is_none());
        // Deleting the whole code keeps it, and putting the same text back (undo, paste) brings the code back.
        let e = after_edit(&doc, "abcd", &[], &[]);
        let taken = e.removed.clone().unwrap();
        assert_eq!(taken.text, "[ind5]");
        let back = after_edit(&e.shown, "ab[ind5]cd", &[taken], &[]);
        assert_eq!(back.shown.codes, [(2, 8)]);
        // The same text typed by hand, with nothing known, is text.
        assert!(after_edit(&e.shown, "ab[ind5]cd", &[], &[]).shown.codes.is_empty());
        // Deleting a paragraph code's closing bracket takes the code and the line break after it.
        let doc = Shown::marked("Title[p]\nNext");
        assert_eq!(doc.text, "Title[p]\nNext");
        let e = after_edit(&doc, "Title[p\nNext", &[], &[]);
        assert_eq!(e.remains, [(7, 8), (5, 7)]);
        // The program takes what is left; an undo then puts the pieces back one at a time, and the code is back.
        let steps = ["Title[pNext", "TitleNext", "Title[pNext", "Title[p\nNext", "Title[p]\nNext"];
        let mut now = e;
        for s in steps {
            now = after_edit(&now.shown, s, &[], &now.taken);
        }
        assert_eq!(now.shown.codes, [(5, 8)]);
        assert!(now.shown.is_own_break(8));
    }
}
