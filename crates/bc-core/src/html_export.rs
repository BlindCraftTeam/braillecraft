//! Save As HTML (spec 21): the document body (internal 8-bit text, codes as 0x1C name 0x1F) through the HTML
//! recipe, which is four rule scripts and two built-in steps run one after another, each step's output the next
//! step's input: the tag script (codes to tags), the italics script (underscore marks to <it> tags), the built-in
//! `html_step` (the html and body tags, non-ASCII text as numeric references), the styles script (style tags to html
//! elements), the fix script (html fixes, maths to LaTeX) and the built-in `fixlinks` (link text to anchors). The
//! recipe's first, optional, translation table is not part of the original's installation, so it is not run.
//!
//! The two built-in steps are ported byte for byte from the original (spec 21, the section of bytes read 2026-10-17).

use crate::rule_script::RuleScript;

/// The rule scripts of the recipe, in running order (tables/rule-scripts).
pub const SCRIPTS: [&str; 4] = ["html-tags.rules", "html-italics.rules", "html-styles.rules", "html-fix.rules"];

/// The head and foot of an HTML with MathJax file, byte for byte as the original writes them: the
/// body the MathJax rule script writes goes between them.
pub const MATHJAX_HEAD: &str = "<!DOCTYPE html>\r\n<html>\r\n<head>\r\n<meta charset = \"utf-8\">\r\n<meta name = \"viewport\" content = \"width=device-width\">\r\n<title>MathJax example</title>\r\n<script src=\"https://polyfill.io/v3/polyfill.min.js?features=es6\"></script>\r\n<script id=\"MathJax-script\" async\r\n        src=\"https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js\">\r\n</script>\r\n</head>\r\n<body>\r\n<p>";
pub const MATHJAX_FOOT: &str = "</p>\r\n</body>\r\n</html>\r\n";

/// An HTML with MathJax file: the script's output between the fixed head and foot.
pub fn with_mathjax(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MATHJAX_HEAD.len() + body.len() + MATHJAX_FOOT.len());
    out.extend_from_slice(MATHJAX_HEAD.as_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(MATHJAX_FOOT.as_bytes());
    out
}

/// The HTML file bytes from the body and the four scripts in `SCRIPTS` order.
pub fn html(body: &[u8], scripts: &mut [RuleScript; 4]) -> Result<Vec<u8>, String> {
    let step = |name: &str, s: &mut RuleScript, text: &[u8]| s.run(text).map_err(|e| format!("{name}: {e}"));
    let text = step(SCRIPTS[0], &mut scripts[0], body)?;
    let text = step(SCRIPTS[1], &mut scripts[1], &text)?;
    let text = html_step(&text);
    let text = step(SCRIPTS[2], &mut scripts[2], &text)?;
    let text = step(SCRIPTS[3], &mut scripts[3], &text)?;
    Ok(fixlinks(&text))
}

/// The reference text for one escape byte: (first byte, page, offset).
const PAGES: [(u8, u16, i32); 66] = [
    (0xB0, 0x28, -0x30), (0xB1, 0x28, 0x50), (0xB2, 0x01, -0x30), (0xB3, 0x01, 0x50), (0xB4, 0x02, -0x30), (0xB5, 0x02, 0x50),
    (0xB6, 0x04, -0x30), (0xB7, 0x04, 0x50), (0xB8, 0x1E, -0x30), (0xB9, 0x1E, 0x50), (0xBA, 0x17, -0x30), (0xBB, 0x17, 0x50),
    (0xBC, 0x12, -0x30), (0xBD, 0x12, 0x50), (0xBE, 0x13, -0x30), (0xBF, 0x13, 0x50), (0xC0, 0x09, -0x30), (0xC1, 0x09, 0x50),
    (0xC2, 0x0A, -0x30), (0xC3, 0x0A, 0x50), (0xC4, 0x0B, -0x30), (0xC5, 0x0B, 0x50), (0xC6, 0x0C, -0x30), (0xC7, 0x0C, 0x50),
    (0xC8, 0x0D, -0x30), (0xC9, 0x0D, 0x50), (0xCA, 0x0E, -0x30), (0xCB, 0x0E, 0x50), (0xCC, 0x0F, -0x30), (0xCD, 0x0F, 0x50),
    (0xCE, 0x10, -0x30), (0xCF, 0x10, 0x50), (0xD0, 0x1D, -0x30), (0xD1, 0x1D, 0x50), (0xD2, 0x07, -0x30), (0xD3, 0x07, 0x50),
    (0xD8, 0x27, -0x30), (0xD9, 0x27, 0x50), (0xDA, 0x29, -0x30), (0xDB, 0x29, 0x50), (0xDC, 0x2A, -0x30), (0xDD, 0x2A, 0x50),
    (0xDE, 0x25, -0x30), (0xDF, 0x25, 0x50), (0xE0, 0x06, -0x30), (0xE1, 0x06, 0x50), (0xE2, 0x03, 0x50), (0xE3, 0x03, -0x30),
    (0xE4, 0x05, -0x30), (0xE5, 0x05, 0x50), (0xE8, 0x30, -0x30), (0xE9, 0x30, 0x50), (0xEA, 0x31, -0x30), (0xEB, 0x31, 0x50),
    (0xEC, 0x20, -0x30), (0xED, 0x20, 0x50), (0xEE, 0x21, -0x30), (0xEF, 0x21, 0x50), (0xF0, 0x22, -0x30), (0xF1, 0x22, 0x50),
    (0xF2, 0x23, -0x30), (0xF3, 0x23, 0x50), (0xF5, 0x00, 0x80), (0xE6, 0x00, -0x30), (0xE7, 0x00, 0x50), (0, 0, 0),
];

/// The code page 437 characters of bytes 0x80 to 0xAF as the step writes them (0x9C is a prefix byte).
const CP437: [u16; 48] = [
    0x00C7, 0x00FC, 0x00E9, 0x00E2, 0x00E4, 0x00E0, 0x00E5, 0x00E7, 0x00EA, 0x00EB, 0x00E8, 0x00EF, 0x00EE, 0x00EC, 0x00C4, 0x00C5,
    0x00C9, 0x00E6, 0x00C6, 0x00F4, 0x00F6, 0x00F2, 0x00FB, 0x00F9, 0x00FF, 0x00D6, 0x00DC, 0x00A2, 0, 0x00A5, 0x20A7, 0x0192,
    0x00E1, 0x00ED, 0x00F3, 0x00FA, 0x00F1, 0x00D1, 0x00AA, 0x00BA, 0x00BF, 0x2310, 0x00AC, 0x00BD, 0x00BC, 0x00A1, 0x00AB, 0x00BB,
];

/// The capital letters a 0x9C prefix gives by its second byte.
const CAPITALS: [(u8, u16); 17] = [
    (0x83, 0xC2), (0x85, 0xC0), (0x88, 0xCA), (0x89, 0xCB), (0x8A, 0xC8), (0x8B, 0xCF), (0x8C, 0xCE), (0x8D, 0xCC), (0x93, 0xD4),
    (0x95, 0xD2), (0x96, 0xDB), (0x97, 0xD9), (0x98, 0x178), (0xA0, 0xC1), (0xA1, 0xCD), (0xA2, 0xD3), (0xA3, 0xDA),
];

/// The entity texts an escape's second byte may be written as, with the value each stands for; tested in this order
/// and the last match wins, as the original tests them.
const ENTITIES: [(&[u8], i32); 17] = [
    (b"&lt;", 60), (b"&gt;", 62), (b"<it>", 95), (b"&rsquo;", 96), (b"&ocirc;", 147), (b"&ograve;", 149), (b"&ucirc;", 150),
    (b"&ugrave;", 151), (b"&ucirc;", 150), (b"&Ouml;", 153), (b"&Uuml;", 154), (b"&yen;", 157), (b"&iacute;", 161),
    (b"&uacute;", 163), (b"&ntilde;", 164), (b"&Ntilde;", 165), (b"&Ccedil;", 199),
];

/// The state of the built-in html step: the Korean jamo being composed (state 1 a lead consonant, 2 with its vowel,
/// 3 with its trailing consonant) and whether a "</it>" is to be skipped.
#[derive(Default)]
struct Step {
    out: Vec<u8>,
    jamo: u8,
    lead: i32,
    vowel: i32,
    trail: i32,
    skip_it_end: bool,
}

impl Step {
    fn reference(&mut self, code: u32) {
        self.out.extend_from_slice(format!("&#x{:04x};", code & 0xFFFF).as_bytes());
    }

    /// Writes the composed jamo, when any, and clears it.
    fn flush(&mut self) {
        match self.jamo {
            1 => self.reference((0x1100 + self.lead) as u32),
            2 => self.reference((0xAC00 + self.lead * 0x24C + self.vowel * 0x1C) as u32),
            3 => self.reference((0xAC01 + self.lead * 0x24C + self.vowel * 0x1C + self.trail) as u32),
            _ => {}
        }
        self.jamo = 0;
        self.lead = 0;
        self.vowel = 0;
        self.trail = 0;
    }

    /// The second byte of an escape, read from the text after it: an entity text stands for its value (and "<it>"
    /// marks the "</it>" to skip), the index moves past it; a comma is the prefix byte. Returns the value and how
    /// many extra bytes were taken.
    fn second(&mut self, rest: &[u8]) -> (i32, usize) {
        let mut c = rest.first().map_or(0, |&b| i32::from(b));
        let mut taken = 0;
        for (text, value) in ENTITIES {
            if rest.starts_with(text) {
                c = value;
                taken = text.len() - 1;
                if text == b"<it>" {
                    self.skip_it_end = true;
                }
            }
        }
        if c == 0x2C {
            c = 0x9C;
        }
        (c, taken)
    }

    /// One Korean jamo escape: the second byte with the entity and comma rules, plus the offset.
    fn jamo(&mut self, rest: &[u8], offset: i32) -> usize {
        let mut c = rest.first().map_or(0, |&b| i32::from(b));
        let mut taken = 0;
        for (text, value) in [(&b"&lt;"[..], 0x3C), (b"&gt;", 0x3E), (b"<it>", 0x5F)] {
            if rest.starts_with(text) {
                c = value;
                taken = 3;
                if value == 0x5F {
                    self.skip_it_end = true;
                }
            }
        }
        if c == 0x2C {
            c = 0x9C;
        }
        c += offset;
        if c < 0x14 {
            self.flush();
            self.jamo = 1;
            self.lead = c;
            self.vowel = 0;
            self.trail = 0;
        } else if (0x61..=0x7D).contains(&c) {
            self.jamo = 2;
            self.vowel = c - 0x61;
            self.trail = 0;
        } else if (0xA8..=0xC2).contains(&c) {
            self.jamo = 3;
            self.trail = c - 0xA8;
        } else {
            self.flush();
            self.reference((0x1100 + c) as u32);
        }
        taken + 1
    }

    /// One gathered line of the input.
    fn line(&mut self, buf: &[u8]) {
        let mut i = 0;
        while i < buf.len() {
            let c = buf[i];
            let mut raw = true;
            if c == 0xE6 || c == 0xE7 {
                i += self.jamo(&buf[i + 1..], if c == 0xE6 { -0x30 } else { 0x50 });
                raw = false;
            } else {
                self.flush();
            }
            match c {
                0xE6 | 0xE7 => {}
                0x1D => {
                    self.out.push(b'-');
                    raw = false;
                }
                0x1E => {
                    self.reference(0xA0);
                    raw = false;
                }
                0x80..=0xAF if c != 0x9C => self.reference(u32::from(CP437[usize::from(c - 0x80)])),
                0x9C => {
                    let next = buf.get(i + 1).copied().unwrap_or(0);
                    if let Some((_, u)) = CAPITALS.iter().find(|(b, _)| *b == next) {
                        self.reference(u32::from(*u));
                    }
                    i += 1;
                }
                0xF6 => self.reference(0xF7),
                0xF7 => self.reference(0xD7),
                0xF8 => self.reference(0xB0),
                0xF9 => self.reference(0xB7),
                _ => {
                    if let Some((_, page, offset)) = PAGES.iter().find(|(b, _, _)| *b == c && *b != 0) {
                        let (second, taken) = self.second(&buf[i + 1..]);
                        self.reference((i32::from(*page) * 0x100 + second + offset) as u32);
                        i += taken + 1;
                        raw = false;
                    }
                }
            }
            if self.skip_it_end && buf[i + 1..].starts_with(b"</it>") {
                i += 4;
                self.skip_it_end = false;
                raw = false;
            }
            i += 1;
            if c < 0x80 && raw {
                self.out.push(c);
            }
        }
        self.out.push(b'\n');
    }
}

/// The built-in html step: "<html>\n<body>\n", each input line (ended by a line feed, the end, or
/// 0x9C0E bytes) with its bytes below 0x80 copied and the rest written as numeric character references, then
/// "\n</body>\n</html>\n". The last line is written even when empty, so an input ending in a line feed gets one more
/// "\n".
pub fn html_step(input: &[u8]) -> Vec<u8> {
    let mut step = Step { out: Vec::with_capacity(input.len() + 64), ..Step::default() };
    step.out.extend_from_slice(b"<html>\n<body>\n");
    let mut buf: Vec<u8> = Vec::new();
    for &b in input {
        if b == 10 || buf.len() > 0x9C0D {
            step.line(&buf);
            buf.clear();
            if b != 10 {
                buf.push(b);
            }
        } else {
            buf.push(b);
        }
    }
    step.line(&buf);
    step.out.extend_from_slice(b"\n</body>\n</html>\n");
    step.out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The built-in link step: the text re-lined; a line holding "<a>TEXT (URL)</a>" is written as
/// PREFIX<a href="URL">TEXT</a> with the byte that followed "</a>" after it; "<a></a>" is taken out; a link runs on
/// across line feeds (each becomes a space) until its "</a>"; lines are also ended after 999 bytes; a last line with
/// no line feed is not written.
pub fn fixlinks(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + 64);
    let mut buf: Vec<u8> = Vec::with_capacity(1000);
    let mut ending = false;
    let mut after: u8 = 0;
    for &byte in input {
        let mut c = byte;
        if c == 10 {
            ending = true;
            c = 0x20;
        }
        let has_a = find(&buf, b"<a>").is_some();
        let has_end = find(&buf, b"</a>").is_some();
        if has_a && !has_end {
            ending = false;
        }
        if has_a && has_end {
            after = c;
            ending = true;
        }
        if ending || buf.len() + 1 > 999 {
            match find(&buf, b"<a></a>") {
                Some(p) => {
                    out.extend_from_slice(&buf[..p]);
                    out.extend_from_slice(&buf[p + 7..]);
                    out.push(b'\n');
                }
                None => match (find(&buf, b"<a>"), find(&buf, b"</a>")) {
                    (Some(start), Some(end)) => {
                        // The URL: from the last "(" before "</a>" (not before "<a>") to the ")" before "</a>".
                        let mut open = end;
                        while open > start && buf[open] != b'(' {
                            open -= 1;
                        }
                        let prefix = &buf[..start];
                        let mut text_end = open;
                        if text_end > 0 && buf[text_end - 1] == b' ' {
                            text_end -= 1;
                        }
                        let text = &buf[start + 3..text_end.max(start + 3)];
                        let url = &buf[open + 1..end.saturating_sub(1).max(open + 1)];
                        out.extend_from_slice(prefix);
                        out.extend_from_slice(b"<a href=\"");
                        out.extend_from_slice(url);
                        out.extend_from_slice(b"\">");
                        out.extend_from_slice(text);
                        out.extend_from_slice(b"</a>\n");
                    }
                    _ => {
                        out.extend_from_slice(&buf);
                        out.push(b'\n');
                    }
                },
            }
            if after != 0 {
                out.push(after);
            }
            buf.clear();
            ending = false;
            after = 0;
        } else {
            buf.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripts() -> [RuleScript; 4] {
        let folder = format!("{}/../../tables/rule-scripts", env!("CARGO_MANIFEST_DIR"));
        SCRIPTS.map(|n| RuleScript::open(std::path::Path::new(&format!("{folder}/{n}"))).unwrap())
    }

    #[test]
    fn the_html_step_writes_references_as_the_original() {
        // Hard space, a hyphen point, a code page 437 letter, a capital through the prefix, an escape with an
        // entity as its second byte, and a plain line.
        let out = html_step(b"a\x1eb\x1dc\x82\x9c\x83\xb0&lt;x\nlast");
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "<html>\n<body>\na&#x00a0;b-c&#x00e9;&#x00c2;&#x280c;x\nlast\n\n</body>\n</html>\n");
        // Korean: lead, vowel and trail compose one syllable; a lone lead is a U+11xx character.
        // Lead 0 (0xE6 takes 0x30 off), vowel 0 (0xE7 adds 0x50: 0x61) and trail 0 (0xA8) make U+AC01; a byte that
        // is none of the three flushes and is written on page 0x11.
        let out = html_step(b"\xe6\x30\xe7\x11\xe7\x58!\xe7\x00");
        assert_eq!(String::from_utf8(out).unwrap(), "<html>\n<body>\n&#xac01;!&#x1150;\n\n</body>\n</html>\n");
    }

    #[test]
    fn the_link_step_relines_and_makes_anchors() {
        let out = fixlinks(b"see <a>Site (http://x.y)</a>, then\nplain\n<a></a>gone\ntail");
        // An empty "<a></a>" leaves an empty line, as the original writes it, and the byte after it starts the next.
        assert_eq!(String::from_utf8(out).unwrap(), "see <a href=\"http://x.y\">Site</a>\n, then\nplain\n\ngone\n");
    }

    #[test]
    fn the_recipe_turns_styles_and_codes_into_html() {
        let body = b"\x1ces~h1.\x1fTitle\x1cee~h1.\x1f\x1cp\x1fText with _one_ word.\x1cl\x1fNext\n";
        let out = html(body, &mut scripts()).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("<html>\n<body>\n"), "{text}");
        assert!(text.contains("<h1>Title</h1>"), "{text}");
        assert!(text.contains("</p><p>"), "{text}");
        assert!(text.contains("_one_"), "{text}");
        assert!(text.ends_with("</body>\n</html>\n"), "{text}");
    }
}
