//! What kind of file is being opened, decided first by a short table of
//! extensions, then by the file's content: a document file by its signature, the containers and binary formats by
//! their marks, HTML or SGML by its end tags and comments, and a text file by counts over its first 512 bytes (coded
//! text, coded braille, formatted braille, formatted text). A sample holding a control byte other than tab, line feed, carriage return or form feed is not
//! a supported file.

/// The kinds of file an import can be (the import methods, by their numbers).
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
    /// Portable Embosser Format (BrailleCraft's own addition).
    Pef,
    /// Markdown (BrailleCraft's own addition): by the extension .md or .markdown; imported through the HTML chain.
    Markdown,
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
            (FileKind::Markdown, "Markdown"),
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

/// What the counts over a text file's first 512 bytes found (the file statistics).
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

/// The extensions a file's kind is taken from before its content is looked at (the extension table).
/// The file kind override table shipped as tables/file-kinds.txt, in the form the built-in extension table
/// has (spec 18 A3: lines "EXT" : METHOD_IMPORT_<kind>): an extension named there is read
/// as that kind before the file's bytes are looked at.
const FILE_KINDS: &str = include_str!("../../../tables/file-kinds.txt");

/// The kind a method name of the override table stands for; None for a name the program has no kind for.
fn kind_of_method(method: &str) -> Option<FileKind> {
    Some(match method.strip_prefix("METHOD_IMPORT_")? {
        "BRF" | "BRF_FOR_UEB" => FileKind::FormattedBraille,
        "BRU" => FileKind::CodedBraille,
        "OBF" => FileKind::Orbit,
        "FORMATTED_TEXT" | "TXT" => FileKind::FormattedText,
        "TXX" => FileKind::CodedText,
        "WORD" | "WORDPORT" => FileKind::Word,
        "OPENXML" => FileKind::Docx,
        "SGML" => FileKind::Sgml,
        "PDF" => FileKind::Pdf,
        "DAISY" => FileKind::Daisy,
        "OPENDOCUMENT" => FileKind::OpenDocument,
        "EXCEL2007" => FileKind::Xlsx,
        "TEX" => FileKind::Tex,
        "HWP" => FileKind::Hangul,
        "WP" => FileKind::WordPerfect,
        "MIDI" | "LIME" => FileKind::Music,
        "OTHER" | "UNSUPPORTED" => FileKind::Unsupported,
        _ => return None,
    })
}

/// The override table's entries: (extension in capitals, kind).
fn file_kind_table() -> &'static [(String, FileKind)] {
    static TABLE: std::sync::OnceLock<Vec<(String, FileKind)>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        FILE_KINDS
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.starts_with('#') {
                    return None;
                }
                let (ext, method) = line.split_once(':')?;
                let ext = ext.trim().trim_matches('"').to_ascii_uppercase();
                let kind = kind_of_method(method.trim().trim_end_matches(';').trim())?;
                (!ext.is_empty()).then_some((ext, kind))
            })
            .collect()
    })
}

fn by_extension(ext: &str) -> Option<FileKind> {
    let upper = ext.to_ascii_uppercase();
    if let Some((_, kind)) = file_kind_table().iter().find(|(e, _)| *e == upper) {
        return Some(*kind);
    }
    Some(match upper.as_str() {
        "LIM" | "MID" => FileKind::Music,
        "MD" | "MARKDOWN" => FileKind::Markdown,
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
    if starts(&COMPOUND) {
        // A compound file: Word 6 to 2003; one with a Workbook stream is an older Excel workbook, not supported
        // (refusal gives it its own message).
        return if ext == "hwp" {
            FileKind::Hangul
        } else if old_excel(bytes) {
            FileKind::Unsupported
        } else {
            FileKind::Word
        };
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
    // HTML or SGML written in UTF-16: more than 3 "</" or "<!--" among its first UTF-16 units.
    if crate::sgml::is_utf16_markup(bytes) {
        return FileKind::Sgml;
    }
    if has_head(b"<dtbook") || has_head(b"DTD DTBook") {
        return FileKind::Daisy;
    }
    // HTML or SGML: text with more than 3 "</" or "<!--" in its first 2048 bytes.
    if crate::sgml::is_markup(bytes) {
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

/// The signature of a compound file (OLE structured storage).
const COMPOUND: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The names of the streams and storages in a compound file's directory (empty when the file does not add up).
fn compound_names(bytes: &[u8]) -> Vec<String> {
    let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if !bytes.starts_with(&COMPOUND) {
        return Vec::new();
    }
    let Some(shift) = u16_at(0x1E).filter(|s| (7..=16).contains(s)) else { return Vec::new() };
    let size = 1usize << shift;
    let sector = |n: u32| 512usize.max(size) + n as usize * size;
    // The allocation table: the sectors the header lists (the first 109; enough for a file of several megabytes).
    let fat: Vec<u32> = (0..109)
        .filter_map(|i| u32_at(0x4C + 4 * i))
        .take_while(|&n| n < 0xFFFF_FFFA)
        .flat_map(|n| (0..size / 4).map(move |i| (n, i)))
        .map_while(|(n, i)| u32_at(sector(n) + 4 * i))
        .collect();
    let mut names = Vec::new();
    let mut at = u32_at(0x30).unwrap_or(u32::MAX);
    let mut seen = 0;
    while at < 0xFFFF_FFFA && seen < 4096 {
        let start = sector(at);
        for e in 0..size / 128 {
            let entry = start + e * 128;
            let Some(len) = u16_at(entry + 0x40) else { break };
            let units: Vec<u16> = (0..(len as usize / 2).saturating_sub(1).min(31)).filter_map(|i| u16_at(entry + 2 * i)).collect();
            if !units.is_empty() {
                names.push(String::from_utf16_lossy(&units));
            }
        }
        at = fat.get(at as usize).copied().unwrap_or(u32::MAX);
        seen += 1;
    }
    names
}

/// Whether the file is a compound file with a "Workbook" stream: an Excel workbook older than Excel 2007.
pub fn old_excel(bytes: &[u8]) -> bool {
    compound_names(bytes).iter().any(|n| n.eq_ignore_ascii_case("Workbook"))
}

/// The message for a file the import refuses, by its kind: an older Excel workbook, Rich Text Format, and any other
/// unsupported file (also music files). None for a kind that has an importer, or that BrailleCraft does not import
/// yet (not_yet).
pub fn refusal(kind: FileKind, bytes: &[u8]) -> Option<String> {
    match kind {
        FileKind::Unsupported | FileKind::Music if old_excel(bytes) => Some(OLD_EXCEL.to_string()),
        FileKind::Unsupported | FileKind::Music => Some(UNSUPPORTED.to_string()),
        FileKind::Rtf => Some(RTF.to_string()),
        _ => None,
    }
}

/// The message for a kind BrailleCraft has no importer for yet (BrailleCraft's own wording).
pub fn not_yet(kind: FileKind) -> String {
    let name = FileKind::filters().iter().find(|f| f.0 == kind).map_or("", |f| f.1);
    format!("BrailleCraft cannot import {name} files yet.")
}

pub const OLD_EXCEL: &str = "BrailleCraft only supports EXCEL 2007 documents, but this document was saved with an older version of Excel.  Try saving the document in the newer format if possible or pasting its contents into a Word document.";
pub const UNSUPPORTED: &str = "This document cannot be imported into BrailleCraft.  It appears to be in an unsupported file format.";
pub const RTF: &str = "This document cannot be imported into BrailleCraft.  It appears to be in Rich Text Format (RTF).  If Microsoft\u{ae} Word is installed, you can use it to convert your document to a native Word format and import that into BrailleCraft.";

/// A text file's kind from its counts, in this order: coded text, coded braille, formatted braille,
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
    // The sample ends at its first NUL (it is read as a C string after the control byte check).
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

// The byte-class rows of the three code page families.
include!("../data/byteclasses.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_by_extension_then_content() {
        assert_eq!(detect("a.brf", b"anything"), FileKind::FormattedBraille);
        assert_eq!(detect("a.prf", b"anything"), FileKind::FormattedText);
        assert_eq!(detect("notes.md", b"# Title\n\nText."), FileKind::Markdown);
        assert_eq!(detect("notes.MARKDOWN", b"text"), FileKind::Markdown);
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

    /// A compound file of one allocation sector and one directory sector, with these entries after the root.
    fn compound(names: &[&str]) -> Vec<u8> {
        let mut d = vec![0u8; 512 * 3];
        d[..8].copy_from_slice(&COMPOUND);
        d[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
        d[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        d[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
        d[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        d[0x2C..0x30].copy_from_slice(&1u32.to_le_bytes());
        d[0x30..0x34].copy_from_slice(&1u32.to_le_bytes());
        for i in 0..109 {
            d[0x4C + 4 * i..0x50 + 4 * i].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        d[0x4C..0x50].copy_from_slice(&0u32.to_le_bytes());
        // The allocation sector (sector 0): itself, then the directory's one sector.
        for i in 0..128 {
            d[512 + 4 * i..516 + 4 * i].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        d[512..516].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        d[516..520].copy_from_slice(&0xFFFF_FFFEu32.to_le_bytes());
        for (e, name) in std::iter::once("Root Entry").chain(names.iter().copied()).enumerate() {
            let at = 1024 + 128 * e;
            let units: Vec<u16> = name.encode_utf16().collect();
            for (i, u) in units.iter().enumerate() {
                d[at + 2 * i..at + 2 * i + 2].copy_from_slice(&u.to_le_bytes());
            }
            d[at + 0x40..at + 0x42].copy_from_slice(&((units.len() as u16 + 1) * 2).to_le_bytes());
            d[at + 0x42] = if e == 0 { 5 } else { 2 };
        }
        d
    }

    #[test]
    fn refusal_old_excel() {
        let xls = compound(&["Workbook", "\u{5}SummaryInformation"]);
        assert!(old_excel(&xls));
        assert_eq!(detect("sheet.xls", &xls), FileKind::Unsupported);
        assert_eq!(refusal(detect("sheet.xls", &xls), &xls).as_deref(), Some(OLD_EXCEL));
        // A Word 6 to 2003 document is still Word.
        let doc = compound(&["WordDocument", "1Table"]);
        assert!(!old_excel(&doc));
        assert_eq!(detect("letter.doc", &doc), FileKind::Word);
    }

    #[test]
    fn refusal_rtf() {
        let rtf = b"{\\rtf1\\ansi x}";
        assert_eq!(refusal(detect("a.rtf", rtf), rtf).as_deref(), Some(RTF));
    }

    #[test]
    fn refusal_unsupported() {
        let junk = b"ab\x01\x02cd";
        assert_eq!(refusal(detect("a.bin", junk), junk).as_deref(), Some(UNSUPPORTED));
    }

    #[test]
    fn refusal_none_for_docx() {
        assert_eq!(refusal(FileKind::Docx, b"PK\x03\x04word/"), None);
        assert_eq!(refusal(FileKind::Sgml, b"<html></html>"), None);
    }

    /// An eBraille or EPUB 3 package is a zip with no word/, xl/ or OpenDocument part: it is not opened.
    #[test]
    fn an_ebrl_or_epub_zip_is_unsupported() {
        let meta = crate::ebraille::Metadata::default();
        let kinds = |name: &str| crate::ebraille::kind_of(name, "");
        let cell = |_: char| 0u8;
        for braille in [true, false] {
            let input = crate::ebraille::Input { running: "\x1ces~para.\x1fText\x1cee~para.\x1f", braille, kinds: &kinds, cell: &cell, meta: &meta, default_language: None, default_system: None, name: "book" };
            let bytes = crate::ebraille::package(&input);
            assert_eq!(detect("book.ebrl", &bytes), FileKind::Unsupported);
            assert_eq!(detect("book.epub", &bytes), FileKind::Unsupported);
        }
    }
}
