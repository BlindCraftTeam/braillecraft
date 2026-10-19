//! Document text as it is kept and as the editor shows it.
//!
//! A document is kept as running text with its formatting codes inline, each code
//! marked 0x1C code 0x1F (codes can nest: 0x1C hl4 0x1C es~para. 0x1F 0x1F). A bracket in the text is just a
//! character, never a code. A document holds no line breaks of its own: a line ends where a code says so.
//!
//! The editor shows a document as `Shown`: the text with each code written in brackets, [code], and the list of the
//! character ranges that are codes; brackets outside those ranges are text, as typed. The new-line code [<] is
//! shown as a line break, which is also what Enter puts in (Enter inserts [<]), and
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

/// The name Document, Add Style starts with.
pub const NEW_STYLE_NAME: &str = "New Style Name";

/// What BrailleCraft says about a new style's name in Document, Add Style, or None when it is accepted: empty,
/// longer than 31, still the starting name, or a character other than a letter, a digit, period, hyphen or underscore.
pub fn style_name_problem(name: &str) -> Option<String> {
    style_name_problem_with(name, false)
}

/// As style_name_problem; with `bypass` (the setting BypassStyleNameValidityCheck, spec 18) the check of the
/// characters is left out.
pub fn style_name_problem_with(name: &str, bypass: bool) -> Option<String> {
    if name.is_empty() {
        Some("The style name is empty.".into())
    } else if name.chars().count() > 31 {
        Some("The style name is too long.  It must not be greater than 31 characters.".into())
    } else if name.eq_ignore_ascii_case(NEW_STYLE_NAME) {
        Some(format!("You must give the style a name other than \"{NEW_STYLE_NAME}\"."))
    } else if !bypass && !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        Some("The style name contains invalid characters.  Style names may only contain letters (uppercase and lowercase), numbers, the period, the hyphen, and the underscore.".into())
    } else {
        None
    }
}

/// True when the brackets of a codes field are: a code not started inside a code, not ended
/// outside one, none left open.
pub fn codes_properly_marked(text: &str) -> bool {
    let mut open = false;
    for c in text.chars() {
        match c {
            '[' if open => return false,
            '[' => open = true,
            ']' if !open => return false,
            ']' => open = false,
            _ => {}
        }
    }
    !open
}

/// What BrailleCraft says about a style's codes field (Edit Style Codes, codes shown in brackets), or None: longer
/// than 1000, text outside the codes, codes not properly started or ended.
pub fn style_codes_problem(text: &str) -> Option<String> {
    if text.chars().count() > 1000 {
        return Some("The code string for the style is too long.".into());
    }
    let mut open = false;
    for c in text.chars() {
        match c {
            '[' => open = true,
            ']' => open = false,
            _ if !open => {
                return Some("You cannot place text in a style definition.  To include text, use the [d~] or [q~] codes.".into())
            }
            _ => {}
        }
    }
    (!codes_properly_marked(text)).then(|| "Some codes are not properly started or terminated.".into())
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

    /// The place in running text (one character for each byte of a document file's body) of the editor's place
    /// `pos`: the editor's own line break after a [p] or [l] code takes no place, a line break shown for the
    /// new-line code takes the code's three (spec 05 section A10, the caret kept in a document file).
    pub fn running_place(&self, pos: usize) -> usize {
        self.slice(0, pos.min(self.text.chars().count())).to_running().chars().count()
    }

    /// The editor's place for the place `place` in `running` (the reverse of running_place; a place inside a code
    /// goes to the code's end).
    pub fn shown_place(running: &str, place: usize) -> usize {
        let prefix: String = running.chars().take(place).collect();
        // A code cut short is ended so it counts whole.
        let open = prefix.chars().filter(|&c| c == OPEN).count();
        let close = prefix.chars().filter(|&c| c == CLOSE).count();
        let mut whole = prefix;
        if open > close {
            whole.extend(running.chars().skip(place).take_while(|&c| c != CLOSE));
            whole.push(CLOSE);
        }
        Shown::from_running(&whole).text.chars().count()
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

    /// The names of every code, in the order of `codes`, read in one pass over the text (the codes' character
    /// offsets turned into byte offsets as the text is walked once), so a reader of all the codes of a long
    /// document does not walk the text again for each code.
    pub fn code_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.codes.len());
        let mut spans: Vec<(usize, usize, usize)> = self.codes.iter().enumerate().map(|(k, &(s, e))| (s, e, k)).collect(); // (start, end, place in codes)
        spans.sort_unstable();
        let mut bytes_of = vec![(0usize, 0usize); spans.len()];
        let mut next = 0;
        let mut pending_end: Vec<usize> = Vec::new();
        let mut char_index = 0;
        for (byte, _) in self.text.char_indices().chain(std::iter::once((self.text.len(), '\0'))) {
            while next < spans.len() && spans[next].0 == char_index {
                bytes_of[next].0 = byte;
                pending_end.push(next);
                next += 1;
            }
            pending_end.retain(|&k| {
                if spans[k].1 == char_index {
                    bytes_of[k].1 = byte;
                    false
                } else {
                    true
                }
            });
            char_index += 1;
        }
        names.resize(self.codes.len(), String::new());
        for (k, &(_, _, place)) in spans.iter().enumerate() {
            let (from, to) = bytes_of[k];
            let inner = self.text.get(from..to).unwrap_or("");
            let mut chars = inner.chars();
            chars.next();
            chars.next_back();
            names[place] = chars.as_str().to_string();
        }
        names
    }

    /// Layout, Apply Last Style: the name of the nearest paragraph style ended before `pos`, read from the text
    /// (back from the caret or the selection start to the first style end code [ee~name] whose
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

    /// Edit, Change Case: on `from`..`to`, or with no selection on the letters around the caret (a
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

// ------------------------------------------------------------------ the one-line prompts (dialog 30070)

/// Message 3777, the prompt of the insert-code key (F9, Ctrl+[).
pub const PROMPT_CODE: &str = "Enter code and parameters";
/// Message 3803, the prompt of the insert-style key (Ctrl+F9, Ctrl+<).
pub const PROMPT_STYLE: &str = "Enter style name";
/// Message 3805, the prompt of the insert-character key (Alt+F9, Ctrl+]).
pub const PROMPT_CHAR: &str = "Enter Unicode code number";
/// The longest answer each prompt takes: one less than its buffer (2001, 2000 and 49 characters).
pub const PROMPT_CODE_MAX: usize = 2000;
pub const PROMPT_STYLE_MAX: usize = 1999;
pub const PROMPT_CHAR_MAX: usize = 48;
/// Message 2678, for an answer holding a space.
pub const NO_SPACES: &str = "The parameters to a code must not contain spaces.";
/// The message for a character number that is not two or four hexadecimal digits.
pub const HEX_DIGITS: &str = "Value must be either 2 or four hexadecimal digits";

/// The code range holding `pos` and its name. INFERENCE: "the caret is on a code" is taken as the caret at the
/// code's opening bracket or inside it; the exact test is not known.
fn code_holding(doc: &Shown, pos: usize) -> Option<((usize, usize), String)> {
    let span = *doc.codes.iter().find(|c| c.0 <= pos && pos < c.1)?;
    Some((span, doc.code_at(span.0)?))
}

/// The insert-code prompt's default text and, when the caret is on a code, that code's range, which the answer
/// replaces: the code without its brackets. Elsewhere, and in the formatted view (`coded_view` false), the default
/// is empty and the answer is put in at the caret. INFERENCE: the view flag that empties the default is taken to
/// be the coded view off.
pub fn code_prompt_default(doc: &Shown, pos: usize, coded_view: bool) -> (String, Option<(usize, usize)>) {
    match code_holding(doc, pos).filter(|_| coded_view) {
        Some((span, name)) => (name, Some(span)),
        None => (String::new(), None),
    }
}

/// The code the insert-code prompt's answer puts in: the answer closed as a code ("[answer]"); None for an empty
/// answer (nothing is put in); NO_SPACES when it holds a space.
pub fn code_from_answer(answer: &str) -> Result<Option<String>, &'static str> {
    if answer.is_empty() {
        Ok(None)
    } else if answer.contains(' ') {
        Err(NO_SPACES)
    } else {
        Ok(Some(format!("[{answer}]")))
    }
}

/// The insert-style prompt's default text and the range it replaces: a style start [es~NAME] at the caret gives
/// "NAME", a style end [ee~NAME] gives "/NAME"; elsewhere empty, put in at the caret.
pub fn style_prompt_default(doc: &Shown, pos: usize) -> (String, Option<(usize, usize)>) {
    if let Some((span, name)) = code_holding(doc, pos) {
        if let Some(n) = name.strip_prefix("es~") {
            return (n.to_string(), Some(span));
        }
        if let Some(n) = name.strip_prefix("ee~") {
            return (format!("/{n}"), Some(span));
        }
    }
    (String::new(), None)
}

/// The style code the insert-style prompt's answer puts in: "/NAME" ends the style ([ee~NAME]), "NAME" starts it
/// ([es~NAME]); None for an empty answer; NO_SPACES when it holds a space.
pub fn style_code_from_answer(answer: &str) -> Result<Option<String>, &'static str> {
    if answer.is_empty() {
        Ok(None)
    } else if answer.contains(' ') {
        Err(NO_SPACES)
    } else if let Some(name) = answer.strip_prefix('/') {
        Ok(Some(format!("[ee~{name}]")))
    } else {
        Ok(Some(format!("[es~{answer}]")))
    }
}

/// The character the insert-character prompt's answer names: "U+" or "u+" and then, or with no prefix, exactly 2
/// or exactly 4 hexadecimal digits, a Unicode code number. None for an empty answer; HEX_DIGITS for anything
/// else. INFERENCE: a "D+" or "d+" answer (a DUSCI number) is refused with the same message, as the DUSCI rules
/// are not known.
pub fn char_from_hex_answer(answer: &str) -> Result<Option<char>, &'static str> {
    if answer.is_empty() {
        return Ok(None);
    }
    let digits = answer.strip_prefix("U+").or_else(|| answer.strip_prefix("u+")).unwrap_or(answer);
    if !matches!(digits.len(), 2 | 4) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HEX_DIGITS);
    }
    u32::from_str_radix(digits, 16).ok().and_then(char::from_u32).map(Some).ok_or(HEX_DIGITS)
}

// ------------------------------------------------------------------ the style lists of the style dialogs

/// The Show choices of Apply Style, Delete Style, Modify Style and Add Style, in their order (strings 3823, 3821,
/// 3819, 3817).
pub const STYLE_SHOW: [&str; 4] =
    ["Only Recommended Styles", "Styles In-Use", "Recommended and In-Use Styles", "All Styles"];
/// The Show choice a style dialog opens with: "Recommended and In-Use Styles".
pub const STYLE_SHOW_DEFAULT: usize = 2;

/// Whether a style is recommended (the Show filter): its beginning codes (running text) hold no [ypr0] code, the
/// code compared with case ignored.
pub fn style_recommended(begin_running: &str) -> bool {
    !begin_running.to_ascii_lowercase().contains(&format!("{OPEN}ypr0{CLOSE}"))
}

/// The names of the styles in use in a document (running text): every style start or end code [es~NAME] or
/// [ee~NAME] in it, the names in small letters. INFERENCE: the names are matched with case ignored, as every other
/// style name lookup is.
pub fn styles_in_use(running: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let chars: Vec<char> = running.chars().collect();
    let mut p = 0;
    while p + 4 <= chars.len() {
        let kind = (chars[p + 1].to_ascii_lowercase(), chars[p + 2].to_ascii_lowercase());
        if chars[p] == OPEN && chars[p + 3] == '~' && matches!(kind, ('e', 's') | ('e', 'e')) {
            let name: String = chars[p + 4..].iter().take_while(|&&c| c != CLOSE && c != OPEN).collect();
            let name = name.to_lowercase();
            if !names.contains(&name) {
                names.push(name);
            }
            p += 4;
        } else {
            p += 1;
        }
    }
    names
}

/// The order of the style dialogs' lists: by name, byte by byte with A to Z taken as a to z, a name that begins
/// another first; names equal but for case keep their order. Names holding ":" are never listed.
pub fn style_list_order(names: &[String]) -> Vec<String> {
    let mut listed: Vec<String> = names.iter().filter(|n| !n.contains(':')).cloned().collect();
    listed.sort_by_key(|n| n.to_ascii_lowercase().into_bytes());
    listed
}

/// A style dialog's list: the styles (name, beginning codes in running text) in list order, filtered by the Show
/// choice `show` (STYLE_SHOW: recommended; in use; recommended or in use; all), `in_use` as styles_in_use gives it.
pub fn style_list(styles: &[(String, String)], show: usize, in_use: &[String]) -> Vec<String> {
    let used = |n: &str| in_use.iter().any(|u| u.eq_ignore_ascii_case(n));
    let recommended = |n: &str| styles.iter().find(|s| s.0 == n).is_some_and(|s| style_recommended(&s.1));
    let names: Vec<String> = styles.iter().map(|s| s.0.clone()).collect();
    style_list_order(&names)
        .into_iter()
        .filter(|n| match show {
            0 => recommended(n),
            1 => used(n),
            2 => recommended(n) || used(n),
            _ => true,
        })
        .collect()
}

/// The place of the style named `name` (case ignored) among `names`.
pub fn style_index_ignoring_case(names: &[String], name: &str) -> Option<usize> {
    names.iter().position(|n| n.eq_ignore_ascii_case(name))
}

/// Add Style's basis list: the style whose codes item `k` (1 or more; 0 is "(No Style)") puts in the code fields.
/// The item's style is looked up as the place in the whole sorted list (`names` sorted, names with ":" included) of
/// the style listed at `k` (`listed`, without "(No Style)"), less one: so the style before it in the whole list,
/// which is the item's own style when every style is listed; past the last listed style the first style.
pub fn basis_style(names: &[String], listed: &[String], k: usize) -> Option<String> {
    if k == 0 {
        return None;
    }
    let mut all = names.to_vec();
    all.sort_by_key(|n| n.to_ascii_lowercase().into_bytes());
    let place = listed.get(k).and_then(|n| all.iter().position(|a| a == n)).map_or(-1, |p| p as i64);
    let at = place - 1;
    all.get(at.max(0) as usize).cloned()
}

/// Add Style: a name that, with the period added, is 32 characters long runs into the beginning codes typed, which
/// are then empty when Edit Style Codes opens.
pub fn style_name_overruns_codes(name: &str) -> bool {
    name.len() >= 32
}

#[cfg(test)]
mod style_list_tests {
    use super::*;

    fn v(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn style_list_order_ignores_case_and_drops_colons() {
        let names = v(&["b", "A.", "_x", "a-b", "Z", "2a", "has:colon"]);
        assert_eq!(style_list_order(&names), v(&["2a", "_x", "a-b", "A.", "b", "Z"]));
    }

    #[test]
    fn style_list_order_is_stable_for_case_twins() {
        assert_eq!(style_list_order(&v(&["B", "b"])), v(&["B", "b"]));
    }

    #[test]
    fn recommended_unless_ypr0() {
        assert!(!style_recommended("\u{1c}ypr0\u{1f}"));
        assert!(!style_recommended("\u{1c}g1\u{1f}\u{1c}YPR0\u{1f}"));
        assert!(style_recommended("\u{1c}ypr1\u{1f}"));
        assert!(style_recommended(""));
    }

    #[test]
    fn in_use_reads_start_and_end_codes() {
        let running = "a\u{1c}es~Quote.\u{1f}b\u{1c}ee~Quote.\u{1f}\u{1c}EE~bold\u{1f}\u{1c}p\u{1f}";
        assert_eq!(styles_in_use(running), v(&["quote.", "bold"]));
    }

    #[test]
    fn style_list_filters() {
        let ypr = "\u{1c}ypr0\u{1f}".to_string();
        let styles = vec![
            ("b".to_string(), String::new()),
            ("a".to_string(), ypr.clone()),
            ("c".to_string(), ypr),
            ("x:y".to_string(), String::new()),
        ];
        let used = v(&["a"]);
        assert_eq!(style_list(&styles, 0, &used), v(&["b"]));
        assert_eq!(style_list(&styles, 1, &used), v(&["a"]));
        assert_eq!(style_list(&styles, 2, &used), v(&["a", "b"]));
        assert_eq!(style_list(&styles, 3, &used), v(&["a", "b", "c"]));
    }

    #[test]
    fn style_index_ignoring_case_finds_any_case() {
        assert_eq!(style_index_ignoring_case(&v(&["Para.", "x"]), "para."), Some(0));
        assert_eq!(style_index_ignoring_case(&v(&["Para.", "x"]), "y"), None);
    }

    #[test]
    fn basis_item_takes_the_style_before_it_in_the_whole_list() {
        let names = v(&["c", "a", "b"]);
        let all = v(&["a", "b", "c"]);
        assert_eq!(basis_style(&names, &all, 0), None);
        assert_eq!(basis_style(&names, &all, 1).as_deref(), Some("a"));
        assert_eq!(basis_style(&names, &all, 2).as_deref(), Some("b"));
        // The last item: past the listed styles, the first style.
        assert_eq!(basis_style(&names, &all, 3).as_deref(), Some("a"));
        // With a filter hiding "a" the list is "b", "c": item 1 looks up "c" and takes the style before it, "b";
        // item 2 is past the listed styles and takes "a", another style's codes.
        let listed = v(&["b", "c"]);
        assert_eq!(basis_style(&names, &listed, 1).as_deref(), Some("b"));
        assert_eq!(basis_style(&names, &listed, 2).as_deref(), Some("a"));
    }

    #[test]
    fn style_name_problem_with_bypass() {
        assert!(style_name_problem_with("a b", true).is_none());
        assert!(style_name_problem_with("a b", false).is_some());
        assert!(style_name_problem_with("", true).is_some());
        assert!(style_name_problem_with(&"x".repeat(32), true).is_some());
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn code_answers() {
        assert_eq!(code_from_answer("ff"), Ok(Some("[ff]".to_string())));
        assert_eq!(code_from_answer("ff x"), Err(NO_SPACES));
        assert_eq!(code_from_answer(""), Ok(None));
    }

    #[test]
    fn style_answers() {
        assert_eq!(style_code_from_answer("/h1."), Ok(Some("[ee~h1.]".to_string())));
        assert_eq!(style_code_from_answer("h1."), Ok(Some("[es~h1.]".to_string())));
        assert_eq!(style_code_from_answer("h 1."), Err(NO_SPACES));
    }

    #[test]
    fn prompt_defaults() {
        let doc = Shown::marked("[pg]abc");
        assert_eq!(code_prompt_default(&doc, 0, true), ("pg".to_string(), Some((0, 4))));
        assert_eq!(code_prompt_default(&doc, 0, false), (String::new(), None));
        assert_eq!(code_prompt_default(&doc, 5, true), (String::new(), None));
        let doc = Shown::marked("x[ee~x]");
        assert_eq!(style_prompt_default(&doc, 1), ("/x".to_string(), Some((1, 7))));
        let doc = Shown::marked("[es~h1.]");
        assert_eq!(style_prompt_default(&doc, 0).0, "h1.");
        assert_eq!(style_prompt_default(&Shown::plain("abc"), 1), (String::new(), None));
    }

    #[test]
    fn char_answers() {
        assert_eq!(char_from_hex_answer("41"), Ok(Some('A')));
        assert_eq!(char_from_hex_answer("U+0041"), Ok(Some('A')));
        assert_eq!(char_from_hex_answer("u+00e9"), Ok(Some('é')));
        assert_eq!(char_from_hex_answer("123"), Err(HEX_DIGITS));
        assert_eq!(char_from_hex_answer("zz"), Err(HEX_DIGITS));
        assert_eq!(char_from_hex_answer(""), Ok(None));
    }
}

#[cfg(test)]
mod tests {
    /// An editor place and a place in the file body convert both ways, the editor's own line break after [p] taking
    /// no place and a shown new-line code taking three.
    #[test]
    fn editor_places_map_to_body_places_and_back() {
        use super::*;
        let running = format!("ab{OPEN}p{CLOSE}cd{OPEN}<{CLOSE}ef");
        let shown = Shown::from_running(&running);
        assert_eq!(shown.text, "ab[p]\ncd\nef");
        for (pos, place) in [(0, 0), (2, 2), (5, 5), (6, 5), (8, 7), (9, 10), (11, 12)] {
            assert_eq!(shown.running_place(pos), place, "pos {pos}");
        }
        for (place, pos) in [(0, 0), (2, 2), (3, 6), (5, 6), (7, 8), (10, 9), (12, 11)] {
            assert_eq!(Shown::shown_place(&running, place), pos, "place {place}");
        }
    }

    use super::*;

    /// The one-pass code names are the names code_at reads one by one, codes inside codes included, with text
    /// outside ASCII around them.
    #[test]
    fn code_names_match_code_at() {
        let doc = Shown::marked("caf\u{e9} [es~h1.]Title[ee~h1.] \u{2800}x[hl1[es~para.]]y[l]\nz[p]\n[ee~para.]");
        let names = doc.code_names();
        assert_eq!(names.len(), doc.codes.len());
        for (k, &(start, _)) in doc.codes.iter().enumerate() {
            assert_eq!(Some(names[k].clone()), doc.code_at(start), "code {k} at {start}");
        }
        assert!(names.contains(&"es~h1.".to_string()) && names.contains(&"hl1[es~para.]".to_string()), "{names:?}");
        assert_eq!(crate::status::open_styles(&doc, 15), vec!["h1.".to_string()]);
        assert_eq!(crate::status::open_styles(&doc, 1000), Vec::<String>::new());
    }

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
    fn change_case_steps_as_specified() {
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
