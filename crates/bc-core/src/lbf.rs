//! Refreshable braille files (.lbf): a braille document as one running line of braille text and "$" markup for a
//! refreshable braille display, as the reference program's exporter writes it (spec 04d-D).
//!
//! The body is walked once. Text is written as it is (UTF-8, no change of case); codes set layout flags, and before
//! the next text the strongest flag is written as markup: "$h" (centred heading), "$h#a", "$p" (paragraph), "$l"
//! (new line) or a single space. Styles listed in data/lbf-styles.xml write their own markup; other styles are
//! walked through their codes. Running heads, footers and some other regions are left out. Every piece of markup
//! stands with one space on each side, shared with the text's own spaces. There are no line ends, header or trailer.
//! Rules are from the exporter's code (spec 04d-D); its stated unknowns are kept as noted below.

use crate::doctext::{CLOSE, OPEN};

/// A style the exporter writes its own markup for.
#[derive(Debug, Clone, Default)]
struct LbfStyle {
    before: String,
    newline: String,
    after: String,
    hierarchy: bool,
    toplevel: i64,
    require_text: bool,
}

/// The styles of data/lbf-styles.xml, by name in small letters.
fn lbf_styles() -> Vec<(String, LbfStyle)> {
    let text = include_str!("../data/lbf-styles.xml");
    let mut text = text.to_string();
    // Comments are not styles.
    while let Some(a) = text.find("<!--") {
        let b = text[a..].find("-->").map_or(text.len(), |b| a + b + 3);
        text.replace_range(a..b, "");
    }
    let field = |body: &str, tag: &str| -> String {
        let (open, close) = (format!("<{tag}>"), format!("</{tag}>"));
        body.find(&open)
            .and_then(|a| body[a + open.len()..].find(&close).map(|b| body[a + open.len()..a + open.len() + b].to_string()))
            .unwrap_or_default()
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    };
    let mut out = Vec::new();
    for part in text.split("<style").skip(1) {
        let Some(q) = part.find('"') else { continue };
        let Some(e) = part[q + 1..].find('"') else { continue };
        let name = part[q + 1..q + 1 + e].to_lowercase();
        let body = part.split("</style>").next().unwrap_or("");
        let s = LbfStyle {
            before: field(body, "before"),
            newline: field(body, "newline"),
            after: field(body, "after"),
            hierarchy: field(body, "hierarchy").trim() == "True",
            toplevel: field(body, "toplevel").trim().parse().unwrap_or(0),
            require_text: field(body, "requiretext").trim() == "True",
        };
        out.push((name, s));
    }
    out
}

/// The exporter's code table: code name, layout flag, whether the code may carry a number (spec 04d-D).
const CODES: &[(&str, u32, bool)] = &[
    ("<", 2, false), ("#", 1, true), ("\u{b0}", 1, true), (">", 1, false), ("cbc", 0x10, true), ("cbp", 2, false),
    ("cd", 1, true), ("cp", 2, true), ("es", 2, false), ("ee", 1, false), ("fr", 1, true), ("gd", 2, true),
    ("hds", 0x10, true), ("hde", 2, false), ("hi", 2, true), ("hl", 2, true), ("ind", 2, true), ("kps", 2, true),
    ("kpe", 2, false), ("l", 2, false), ("ld", 2, true), ("lg", 2, true), ("lpr", 1, false), ("music", 2, false),
    ("p", 4, false), ("pg", 4, true), ("pl", 2, true), ("pm", 4, true), ("psac", 2, true), ("psap", 2, false),
    ("psc", 2, true), ("pst", 2, false), ("ptys", 2, true), ("ptye", 2, false), ("rd", 1, true), ("rfe", 1, false),
    ("run", 1, false), ("sc", 2, true), ("sd", 2, true), ("sk", 2, true), ("sl", 2, true), ("st", 2, true),
    ("taa", 1, true), ("tab", 1, true), ("tas", 1, true), ("tle", 1, false), ("tlp", 1, false), ("htt", 1, false),
    ("top", 2, false), ("ttc", 0x10, true), ("ttt", 2, false),
];

/// The layout flag a code adds: the code is cut at the first of "-0123456789~;:" (a "-" only when a digit follows
/// it; a code that is just ";" or ":" stays itself), the part before looked up in small letters; a code with a
/// number adds its flag only when the table allows one.
fn code_flag(code: &str) -> u32 {
    let chars: Vec<char> = code.chars().collect();
    let cut = if code == ";" || code == ":" {
        chars.len()
    } else {
        (0..chars.len())
            .find(|&i| {
                let c = chars[i];
                (c == '-' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) || "0123456789~;:".contains(c)
            })
            .unwrap_or(chars.len())
    };
    let base: String = chars[..cut].iter().collect::<String>().to_lowercase();
    let number = chars.get(cut).is_some_and(|c| c.is_ascii_digit() || *c == '-');
    CODES
        .iter()
        .find(|(n, _, _)| *n == base)
        .filter(|(_, _, allowed)| !number || *allowed)
        .map_or(0, |(_, f, _)| *f)
}

/// The writer's state while it walks the body.
struct Walk<'a> {
    out: String,
    styles: &'a [(String, LbfStyle)],
    doc_styles: &'a [(String, String, String)],
    /// Layout flags waiting for the next text.
    flags: u32,
    /// Regions whose text is left out: 2 running head, 4 running foot, 0x20, 8 and 0x10 the "meg" region.
    regions: u32,
    region_count: usize,
    /// A page-number run: open, characters written, ended by a code.
    run: bool,
    run_count: usize,
    run_ended: bool,
    /// Line numbering: 0x20 on, 0x40 its second part; characters counted.
    numbering: u32,
    numbering_count: usize,
    /// The style with its own markup that is open, whether its text has started, whether its "before" waits for
    /// text, and its level counter and offset.
    special: Option<usize>,
    text_seen: bool,
    start_pending: bool,
    level: i64,
    offset: i64,
}

impl Walk<'_> {
    fn last_is_space(&self) -> bool {
        self.out.ends_with(' ')
    }

    /// A piece of markup with one space on each side: a space first unless the last character written is one, then
    /// the markup trimmed, then a space unless the next text character is one.
    fn token(&mut self, token: &str, next: Option<char>) {
        if !self.last_is_space() {
            self.out.push(' ');
        }
        self.out.push_str(token.trim());
        if next != Some(' ') {
            self.out.push(' ');
        }
    }

    /// A single space, unless the last character written or the next one is a space.
    fn space(&mut self, next: Option<char>) {
        if !self.last_is_space() && next != Some(' ') {
            self.out.push(' ');
        }
    }

    /// The markup written before a text character (spec 04d-D, token emission).
    fn emit(&mut self, next: Option<char>) {
        let special = self.special.map(|i| self.styles[i].1.clone());
        if self.start_pending {
            self.start_pending = false;
            if let Some(s) = &special {
                if s.before.trim().is_empty() {
                    self.space(next);
                } else if s.require_text {
                    self.token(&s.before, next);
                }
            }
            // The waiting layout flags are dropped.
        } else if let Some(s) = special.as_ref().filter(|s| !s.newline.trim().is_empty() && self.flags >= 2) {
            if s.hierarchy && self.flags & 0x80 != 0 {
                let token = format!("{}#{}", s.newline.trim_end(), level_letters(self.level + self.offset));
                self.token(&token, next);
            } else {
                self.token(&s.newline.clone(), next);
            }
        } else if self.flags & 0x10 != 0 {
            self.token("$h", next);
        } else if self.flags & 8 != 0 {
            self.token("$h#a", next);
        } else if self.flags & 4 != 0 {
            self.token("$p", next);
        } else if self.flags & 2 != 0 {
            self.token("$l", next);
        } else if self.flags & 1 != 0 {
            self.space(next);
        }
        if self.numbering & 0x20 != 0 && self.numbering_count == 0 {
            self.space(next);
            self.out.push_str("$l");
            if next != Some('#') {
                self.out.push('#');
            }
        }
        self.flags = 0;
    }

    /// One character of text (spec 04d-D, body walk).
    fn text(&mut self, c: char) {
        if self.regions != 0 {
            if self.regions & 8 != 0 {
                self.region_count += 1;
            }
            return;
        }
        if c == ' ' && (self.run && self.run_count == 0 || self.numbering & 0x20 != 0 && self.numbering_count == 0) {
            return;
        }
        self.text_seen = true;
        if self.run && self.run_count > 0 && (c == ' ' || self.run_ended) {
            self.out.push_str("31");
            self.run = false;
        }
        // The markup goes before this character, so this character is the one after it.
        if self.flags != 0 || self.start_pending || self.numbering & 0x20 != 0 {
            self.emit(Some(c));
        }
        if self.run && self.run_count == 0 && c != ' ' {
            self.out.push_str("\"3");
        }
        self.out.push(c);
        if self.run {
            self.run_count += 1;
        }
        if self.numbering & 0x20 != 0 {
            self.numbering_count += 1;
        }
    }

    fn find_style(&self, name: &str) -> Option<usize> {
        let name = name.to_lowercase();
        self.styles.iter().position(|(n, _)| *n == name)
    }

    /// The codes of a document style's beginning or ending codes, walked for their layout only.
    fn style_codes(&mut self, name: &str, end: bool, next: Option<char>) {
        let codes = self
            .doc_styles
            .iter()
            .find(|s| s.0.eq_ignore_ascii_case(name))
            .map(|s| if end { s.2.clone() } else { s.1.clone() })
            .unwrap_or_default();
        for code in codes_in(&codes) {
            self.code(&code, next, true);
        }
    }

    /// One code (spec 04d-D, code handling; the first rule that applies ends it). `codes_only` is set while a
    /// document style's own codes are walked.
    fn code(&mut self, code: &str, next: Option<char>, codes_only: bool) {
        let lower = code.to_lowercase();
        if self.regions & 8 != 0 && code == ":" && self.region_count > 0 {
            self.regions &= !8;
            return;
        }
        if self.run {
            self.run_ended = true;
        }
        if self.numbering & 0x20 != 0 && code == ":" && self.numbering_count > 0 {
            self.numbering = 0;
            self.space(next);
            return;
        }
        if let Some(name) = lower.strip_prefix("es~") {
            match self.find_style(name) {
                Some(i) => {
                    let s = self.styles[i].1.clone();
                    self.special = Some(i);
                    self.text_seen = false;
                    if s.hierarchy {
                        self.level = 1;
                        self.offset = s.toplevel - 1;
                    }
                    if !s.before.trim().is_empty() && !s.require_text {
                        self.token(&s.before, next);
                    } else {
                        self.start_pending = true;
                    }
                }
                None if !codes_only => self.style_codes(name, false, next),
                None => {}
            }
            return;
        }
        if let Some(name) = lower.strip_prefix("ee~") {
            match self.find_style(name) {
                Some(i) => {
                    let s = self.styles[i].1.clone();
                    if s.after.trim().is_empty() {
                        self.flags |= 1;
                    } else if !s.require_text || self.text_seen {
                        self.token(&s.after, next);
                    }
                    self.special = None;
                    self.start_pending = false;
                }
                None if !codes_only => self.style_codes(name, true, next),
                None => {}
            }
            return;
        }
        if lower == "ind5" {
            self.flags |= 8;
            return;
        }
        if let (Some(i), Some(n)) = (self.special, lower.strip_prefix("hl")) {
            if self.styles[i].1.hierarchy {
                if let Ok(n) = n.parse::<i64>() {
                    self.level = n;
                    self.flags |= 0x80;
                    return;
                }
            }
        }
        if lower.starts_with("lea") || lower.starts_with("po") {
            self.run = true;
            self.run_count = 0;
            self.run_ended = false;
            self.flags |= code_flag(code);
            return;
        }
        if lower.starts_with("linenum") {
            self.numbering |= 0x20;
            self.numbering_count = 0;
            return;
        }
        if self.numbering & 0x20 != 0 && code == ":" && self.numbering_count == 0 {
            self.numbering |= 0x40;
            return;
        }
        if self.numbering & 0x40 != 0 && code == ";" {
            self.numbering = 0;
            self.space(next);
            return;
        }
        match lower.as_str() {
            "tls" | "tlc" => self.regions |= 2,
            "tle" | "tlp" | "htt" => {
                self.regions &= !2;
                self.flags |= code_flag(code);
            }
            "rfs" => self.regions |= 4,
            "rfe" => {
                self.regions &= !4;
                self.flags |= code_flag(code);
            }
            "vss" => self.regions |= 0x20,
            "vse" => self.regions &= !0x20,
            "meg" | "mec" => {
                self.regions |= 8;
                self.region_count = 0;
            }
            ":" if self.regions & 8 != 0 && self.region_count == 0 => self.regions = (self.regions & !8) | 0x10,
            ";" if self.regions & 0x10 != 0 => self.regions &= !0x10,
            _ => self.flags |= code_flag(code),
        }
    }
}

/// The codes in running text (marked 0x1C ... 0x1F), in order.
fn codes_in(running: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = running;
    while let Some(a) = rest.find(OPEN) {
        let after = &rest[a + OPEN.len_utf8()..];
        let Some(b) = after.find(CLOSE) else { break };
        out.push(after[..b].to_string());
        rest = &after[b + CLOSE.len_utf8()..];
    }
    out
}

/// A level as letters, most significant digit first: 1 a ... 9 i, 0 j.
fn level_letters(n: i64) -> String {
    n.max(0).to_string().chars().map(|d| if d == '0' { 'j' } else { (b'a' + (d as u8 - b'1')) as char }).collect()
}

/// A refreshable braille file from a braille document's running text (codes marked 0x1C ... 0x1F, braille in North
/// American ASCII braille) and its styles (name, beginning codes, ending codes, in running text). A hard space is
/// written as a space (spec 04d-D, inferred).
pub fn write(running: &str, styles: &[(String, String, String)]) -> Vec<u8> {
    let table = lbf_styles();
    let mut w = Walk {
        out: String::new(),
        styles: &table,
        doc_styles: styles,
        flags: 0,
        regions: 0,
        region_count: 0,
        run: false,
        run_count: 0,
        run_ended: false,
        numbering: 0,
        numbering_count: 0,
        special: None,
        text_seen: false,
        start_pending: false,
        level: 1,
        offset: 0,
    };
    let chars: Vec<char> = running.chars().map(|c| if c == '\u{a0}' { ' ' } else { c }).collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == OPEN {
            if let Some(len) = chars[i + 1..].iter().position(|&x| x == CLOSE) {
                let code: String = chars[i + 1..i + 1 + len].iter().collect();
                let next = chars.get(i + len + 2).copied();
                w.code(&code, next, false);
                i += len + 2;
                continue;
            }
        }
        w.text(c);
        i += 1;
    }
    w.out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> String {
        crate::doctext::Shown::marked(s).to_running()
    }

    #[test]
    fn the_style_data_is_read_whole() {
        let s = lbf_styles();
        assert_eq!(s.len(), 29);
        let list = &s.iter().find(|(n, _)| n == "l-list.").unwrap().1;
        assert_eq!((list.before.trim(), list.newline.trim(), list.hierarchy, list.toplevel), ("$\\#a", "$\\", true, 1));
        assert!(!s.iter().find(|(n, _)| n == "bb-boxbegin.").unwrap().1.require_text);
    }

    #[test]
    fn codes_add_their_flags() {
        assert_eq!(code_flag("p"), 4);
        assert_eq!(code_flag("hds2"), 0x10);
        assert_eq!(code_flag("l"), 2);
        // A number where the table allows none: no flag.
        assert_eq!(code_flag("p3"), 0);
        assert_eq!(code_flag("unknown"), 0);
    }

    #[test]
    fn paragraphs_and_lines_become_markup_between_the_text() {
        assert_eq!(write(&r("[p],! cat[l]sat[p]on"), &[]), b" $p ,! cat $l sat $p on".to_vec());
        // A running head is left out.
        assert_eq!(write(&r("a[tls]head[tle] b"), &[]), b"a b".to_vec());
    }

    #[test]
    fn listed_styles_write_their_own_markup() {
        assert_eq!(write(&r("x[es~Footnote.]note[ee~Footnote.] y"), &[]), b"x $n note $n= y".to_vec());
        // A style not listed is walked through its codes.
        let styles = vec![("mine".to_string(), r("[p]"), String::new())];
        assert_eq!(write(&r("a[es~mine]b[ee~mine]"), &styles), b"a $p b".to_vec());
    }
}
