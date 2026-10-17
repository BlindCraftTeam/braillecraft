//! What kind of file is being opened, decided as the reference program decides it: first by a short table of
//! extensions, then by the file's content: a document file by its signature, the containers and binary formats by
//! their marks, and a text file by counts over its first 512 bytes (coded text, coded braille, formatted braille,
//! formatted text). A sample holding a control byte other than tab, line feed, carriage return or form feed is not
//! a supported file.

/// The kinds of file an import can be (the reference's import methods, by its numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A print or braille document file (.dxp, .dxb, .dxt).
    Document,
    /// Text with $ codes, as Save As coded text writes it (2).
    CodedText,
    /// Braille with | codes, as Save As coded braille writes it (3).
    CodedBraille,
    /// Formatted text: plain text with its own lines and pages (5).
    FormattedText,
    /// Formatted braille (BRF, 6).
    FormattedBraille,
    /// SGML, ICADD or HTML (8).
    Sgml,
    /// XML DAISY or NISO (9).
    Daisy,
    /// TeX or LaTeX (10).
    Tex,
    /// Word 2007 and later (11), Excel 2007 and later (12), OpenDocument (13), Word 6 to 2003 (1), WordPerfect (0),
    /// Hangul Word Processor (16), Orbit Reader (4).
    Docx,
    Xlsx,
    OpenDocument,
    Word,
    WordPerfect,
    Hangul,
    Orbit,
    /// Portable Document Format (17).
    Pdf,
    /// Portable Embosser Format (BrailleCraft's own addition; the reference has no PEF).
    Pef,
    /// Rich Text Format: not supported, with its own message.
    Rtf,
    /// Music files (.lim, .mid): not supported by BrailleCraft.
    Music,
    /// Not a supported file (18).
    Unsupported,
}

impl FileKind {
    /// Whether the file is braille (imported as a braille document).
    pub fn is_braille(self) -> bool {
        matches!(self, FileKind::CodedBraille | FileKind::FormattedBraille | FileKind::Orbit | FileKind::Pef)
    }

    /// The kinds the import dialog lists, in its order, with their names.
    pub fn filters() -> &'static [(FileKind, &'static str)] {
        &[
            (FileKind::WordPerfect, "WordPerfect 3.x, 5.x, or 6.x"),
            (FileKind::Word, "Microsoft Word 6-2003"),
            (FileKind::Pdf, "Portable Document Format (PDF)"),
            (FileKind::CodedText, "Coded text (Windows)"),
            (FileKind::CodedBraille, "Coded braille"),
            (FileKind::Orbit, "Orbit Reader"),
            (FileKind::FormattedText, "Formatted text"),
            (FileKind::FormattedBraille, "Formatted braille"),
            (FileKind::Sgml, "SGML/ICADD/HTML"),
            (FileKind::Daisy, "XML-Daisy/Niso"),
            (FileKind::Tex, "TeX or LaTeX"),
            (FileKind::Docx, "OpenXML / Word 2007-2019, 365"),
            (FileKind::Xlsx, "Excel 2007-2019, 365"),
            (FileKind::OpenDocument, "OpenDocument"),
            (FileKind::Hangul, "Hangul Word Processor"),
            (FileKind::Unsupported, "Unsupported file format"),
        ]
    }

    /// Whether the import dialog shows a code page choice (text) or a braille encoding choice (braille) for it.
    pub fn asks_code_page(self) -> bool {
        matches!(self, FileKind::CodedText | FileKind::FormattedText)
    }

    pub fn asks_braille_encoding(self) -> bool {
        matches!(self, FileKind::CodedBraille | FileKind::Orbit | FileKind::FormattedBraille)
    }
}

/// What the counts over a text file's first 512 bytes found (the reference's file statistics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStats {
    /// A form feed was seen.
    pub form_feed: bool,
    /// "|code" starts.
    pub bars: i32,
    /// The braille score: braille-like patterns add, print-like ones take away.
    pub braille: i32,
    /// "$code" starts.
    pub dollars: i32,
    /// The longest line.
    pub longest: i32,
    /// Which family of code page the upper-half bytes look like (0, 1 or 2; -1 when none looks like any; 2 for a
    /// file with carriage returns and no line feeds).
    pub code_page: i32,
}

/// The extensions a file's kind is taken from before its content is looked at (the reference's extension table).
fn by_extension(ext: &str) -> Option<FileKind> {
    Some(match ext.to_ascii_uppercase().as_str() {
        "BRF" | "BFM" => FileKind::FormattedBraille,
        "PRF" | "PIM" => FileKind::FormattedText,
        "LIM" | "MID" => FileKind::Music,
        _ => return None,
    })
}

/// The kind of file `name` holding `bytes`.
pub fn detect(name: &str, bytes: &[u8]) -> FileKind {
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    if let Some(kind) = by_extension(ext) {
        return kind;
    }
    let ext = ext.to_ascii_lowercase();
    let starts = |m: &[u8]| bytes.starts_with(m);
    // Containers and binary formats.
    if starts(&[0xFF, b'W', b'P', b'C']) {
        return FileKind::WordPerfect;
    }
    if starts(b"HWP Document File") {
        return FileKind::Hangul;
    }
    if starts(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        // A compound file: Word 6 to 2003 (an old Excel or a Hangul file is not supported here).
        return if ext == "hwp" { FileKind::Hangul } else { FileKind::Word };
    }
    if starts(b"PK\x03\x04") {
        let has = |s: &[u8]| bytes.windows(s.len()).any(|w| w == s);
        return if has(b"word/") {
            FileKind::Docx
        } else if has(b"xl/") {
            FileKind::Xlsx
        } else if has(b"mimetypeapplication/vnd.oasis.opendocument") || has(b"content.xml") {
            FileKind::OpenDocument
        } else {
            FileKind::Unsupported
        };
    }
    if starts(b"%PDF") {
        return FileKind::Pdf;
    }
    if matches!(ext.as_str(), "dxp" | "dxb" | "dxt")
        || (bytes.len() >= 6 && bytes[..4] == crate::dxfile::SIGNATURE.to_be_bytes() && bytes[4..6] == [0, 2])
    {
        return FileKind::Document;
    }
    // BrailleCraft's own addition: PEF, by extension or by its root element.
    let head = &bytes[..bytes.len().min(512)];
    let has_head = |s: &[u8]| head.windows(s.len()).any(|w| w == s);
    if ext == "pef" || has_head(b"<pef") {
        return FileKind::Pef;
    }
    if has_head(b"<dtbook") || has_head(b"DTD DTBook") {
        return FileKind::Daisy;
    }
    if has_head(b"<!DOCTYPE") || has_head(b"<html") || has_head(b"<HTML") {
        return FileKind::Sgml;
    }
    if has_head(b"\\documentclass") || has_head(b"\\documentstyle") {
        return FileKind::Tex;
    }
    if starts(b"{\\rtf1") {
        return FileKind::Rtf;
    }
    let Some(s) = text_stats(head) else { return FileKind::Unsupported };
    kind_of_text(&s)
}

/// A text file's kind from its counts, in the reference's order: coded text, coded braille, formatted braille,
/// formatted text.
pub fn kind_of_text(s: &TextStats) -> FileKind {
    if !s.form_feed && s.dollars >= 3 && s.dollars > s.bars {
        FileKind::CodedText
    } else if !s.form_feed && s.braille >= 3 && s.bars >= 4 && s.bars > s.dollars {
        FileKind::CodedBraille
    } else if s.dollars + s.bars <= 2 && s.longest <= 45 && s.braille >= 3 {
        FileKind::FormattedBraille
    } else if s.dollars + s.bars <= 2 {
        FileKind::FormattedText
    } else {
        FileKind::Unsupported
    }
}

/// Counts over a sample of a text file; None when the sample holds a control byte other than tab, line feed, carriage
/// return or form feed (not a text file).
pub fn text_stats(sample: &[u8]) -> Option<TextStats> {
    // The sample ends at its first NUL (the reference reads it as a C string after the control byte check).
    let mut s = TextStats::default();
    let (mut c, mut w, mut m) = (0i32, 0i32, 0i32);
    let (mut crs, mut lfs) = (0, 0);
    for (i, &b) in sample.iter().enumerate() {
        let next = sample.get(i + 1).copied().unwrap_or(0);
        for (family, count) in [(1, &mut c), (0, &mut w), (2, &mut m)] {
            let table = &BYTE_CLASSES[family];
            let (x, y) = (table[b as usize], table[next as usize]);
            match (x, y) {
                (1, 2) | (2, 1) => *count += 1,
                (3, 3) if family == 1 => *count += 1,
                (2, 2) => *count -= 1,
                _ => {}
            }
            if x == 4 {
                *count -= 1;
            }
        }
        match b {
            b'\r' => crs += 1,
            b'\n' => lfs += 1,
            0x0C => s.form_feed = true,
            b'\t' => {}
            0..=0x1F => return None,
            _ => {}
        }
    }
    s.code_page = if w < m {
        if m < c { 1 } else { 2 }
    } else if w < c {
        1
    } else {
        0
    };
    if c < 1 && m < 1 && w < 1 {
        s.code_page = -1;
    }
    if crs != 0 && lfs == 0 {
        s.code_page = 2;
    }
    let text = match sample.iter().position(|&b| b == 0) {
        Some(n) => &sample[..n],
        None => sample,
    };
    s.dollars = code_starts(text, b'$');
    s.bars = code_starts(text, b'|');
    s.longest = longest_line(text);
    s.braille = braille_score(text);
    Some(s)
}

/// "$code" or "|code" starts: the mark at the start or after a space or control (or an upper-half byte), followed by
/// one of # ' / : ; < > or a letter.
fn code_starts(text: &[u8], mark: u8) -> i32 {
    let starts_code = |c: u8| c.is_ascii_alphabetic() || b"#'/:;<>".contains(&c);
    let mut n = 0;
    for (i, &b) in text.iter().enumerate() {
        if b != mark {
            continue;
        }
        let before_ok = i == 0 || text[i - 1] < b'!' || text[i - 1] >= 0x80;
        let next = text.get(i + 1).copied().unwrap_or(0);
        if before_ok && next < 0x80 && starts_code(next) {
            n += 1;
        }
    }
    n
}

/// The longest line, in bytes (line feeds, form feeds and carriage returns end lines).
fn longest_line(text: &[u8]) -> i32 {
    let (mut longest, mut now) = (0, 0);
    for &b in text {
        if matches!(b, b'\n' | 0x0C | b'\r') {
            longest = longest.max(now);
            now = 0;
        } else {
            now += 1;
        }
    }
    longest
}

/// The braille score: ",," and " ," (or " ?;") patterns and "#" followed by a letter a to j add one; "#" then a digit
/// adds one when an underscore comes before the next space, else takes one; a number of three or more digits between
/// spaces, a capital letter before a small one or a small one before a capital take one; "$ " after a letter adds one.
/// "#[style=Section Info" lines and "#[ ... ]#" marks are passed over.
fn braille_score(text: &[u8]) -> i32 {
    let at = |i: usize| text.get(i).copied().unwrap_or(0);
    let span = |i: usize, stop: &[u8]| text[i.min(text.len())..].iter().position(|b| stop.contains(b)).unwrap_or(text.len() - i.min(text.len()));
    let digits = |i: usize| text[i.min(text.len())..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut score = 0;
    let mut i = 0;
    while i < text.len() {
        let c0 = at(i);
        if c0 == b'#' && at(i + 1) == b'[' {
            let rest = &text[(i + 2).min(text.len())..];
            if rest.len() >= 18 && rest[..18].eq_ignore_ascii_case(b"style=Section Info") {
                i += span(i, b"\r\n");
            } else if let Some(k) = text[i..].windows(2).position(|w| w == b"]#") {
                i += k + 2;
            } else {
                i += span(i, b" \r\n");
            }
            continue;
        }
        let (c1, c2) = (at(i + 1), at(i + 2));
        if c0 == b' ' && c1 != 0 && (c1 == b',' || c2 == b';') {
            let k = usize::from(c2 == b',');
            if at(i + k + 2) != b' ' {
                score += 1;
            }
        } else if c0 == b',' && c1 == b',' {
            score += 1;
        } else if c0 == b'#' {
            if (b'a'..=b'j').contains(&c1) || (b'A'..=b'J').contains(&c1) {
                score += 1;
            } else if c1.is_ascii_digit() {
                if span(i, b"_") < span(i, b" ") {
                    score += 1;
                } else {
                    score -= 1;
                }
            }
        } else if c0 == b' ' && c1.is_ascii_digit() && c2.is_ascii_digit() && at(i + 3).is_ascii_digit() && {
            let n = digits(i + 4);
            at(i + 4 + n) == b' '
        } {
            score -= 1;
        } else if (c0.is_ascii_uppercase() && c1.is_ascii_lowercase()) || (c0.is_ascii_lowercase() && c1.is_ascii_uppercase()) {
            score -= 1;
        } else if c0 == b'$' && c1 == b' ' && i > 0 && at(i - 1).is_ascii_alphabetic() {
            score += 1;
        }
        i += 1;
    }
    score
}

// The byte-class rows of the three code page families, taken from the reference program's data.
include!("../data/byteclasses.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_by_extension_then_content() {
        assert_eq!(detect("a.brf", b"anything"), FileKind::FormattedBraille);
        assert_eq!(detect("a.prf", b"anything"), FileKind::FormattedText);
        assert_eq!(detect("a.txt", b"%PDF-1.4"), FileKind::Pdf);
        assert_eq!(detect("a.txt", b"Plain text, with words.\r\nAnd lines.\r\n"), FileKind::FormattedText);
        // Coded text: three or more $codes and more than | codes, no form feed.
        assert_eq!(detect("a.txt", b"$p Text $l more $p end"), FileKind::CodedText);
        // Formatted braille by content: braille-like patterns, short lines.
        assert_eq!(detect("a.txt", b",! cat sat4\n,,big #abc ,dog\n"), FileKind::FormattedBraille);
        // A control byte other than tab, line feed, carriage return or form feed: not supported.
        assert_eq!(detect("a.txt", b"ab\x01cd"), FileKind::Unsupported);
        assert_eq!(detect("a.rtf", b"{\\rtf1\\ansi"), FileKind::Rtf);
    }
}
