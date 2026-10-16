//! Reading .dxp (print) and .dxb (braille) document files, format versions 0 and 1.
//!
//! Layout (all numbers big-endian):
//! - 10-byte signature: u32 0xFF445349, u16 2, u16 document type (1 print, 0 braille), u16 format version.
//! - 224-byte settings block at offset 10: five u16 page-numbering settings (braille numbers on even pages, on odd
//!   pages, print numbers on even pages, on odd pages: 0 none, 1 upper left, 2 upper right, 3 lower left, 4 lower
//!   right; then the first page number shown); at +18 a u32 body length; at +26 the translated line (u16, 0 or 1);
//!   at +28 six-key entry (u16, 0 or 1); at +32 four 8-byte table names, NUL padded (print-to-braille table, its
//!   scan table, braille-to-print table, its scan table).
//! - The body at offset 234: 8-bit text in code page 437, with each formatting code as the byte 0x1C, the code, the
//!   byte 0x1F; 0x1E is a hard space.
//!
//! - After the body, two blocks hold the document's styles (spec 05-B (c)), each: u16 entry count, u16 0, u16 total
//!   bytes of its strings (each counted with its NUL), u16 0, then one value per entry (2 bytes in the first block,
//!   none in the second), then the strings, NUL terminated. The first block holds the style names, sorted, with a u16
//!   index for each; the second holds two strings per style, its beginning codes and its ending codes, codes written
//!   0x1C code 0x1F.
//! - Settings +64 to +87 are the Table of Contents settings (spec 05-B (d)). Settings +88, +98, +133, +143 and +153
//!   are 10-byte descriptors of optional sections after the styles (u16 kind, u32 file offset, u32 length; all zero
//!   for a section that is absent): +88 kind 1 is the name of the document's embosser setup, NUL terminated; the
//!   others (the extra block and its second copy, embedded objects) are kept as they are read and written back.
//! - Settings +108 to +128 hold the document's embosser page layout (DxLayout, spec 25 section 10).
//!
//! The text is returned as running text (bc_core::doctext): codes marked 0x1C ... 0x1F as in the file, a hard space
//! as U+00A0, every other byte its code page 437 character; brackets in the text stay text. Writing does the reverse
//! and keeps the settings bytes a document was read with, so a document saved again keeps what BrailleCraft does not
//! use itself.

use crate::DocumentKind;

pub const SIGNATURE: u32 = 0xFF44_5349;
const HEADER: usize = 10;
const SETTINGS: usize = 224;
const BODY: usize = HEADER + SETTINGS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxDocument {
    pub kind: DocumentKind,
    pub version: u16,
    /// Braille page numbers on even and odd pages, print page numbers on even and odd pages, first page number.
    pub page_numbering: [u16; 5],
    pub translated_line: bool,
    pub six_key: bool,
    /// Print-to-braille table, its scan table, braille-to-print table, its scan table.
    pub tables: [String; 4],
    /// The running text, codes marked 0x1C ... 0x1F (bc_core::doctext).
    pub text: String,
    /// The styles stored with the document, in the file's order: name, beginning codes, ending codes, each in running
    /// text (codes marked 0x1C ... 0x1F; a bracket is text).
    pub styles: Vec<(String, String, String)>,
    /// The index value the file gives each style name (written back as read).
    pub style_index: Vec<u16>,
    /// The 224-byte settings block as read, for writing back what BrailleCraft does not use itself.
    pub settings: Vec<u8>,
    /// The name of the document's embosser setup (settings +88); empty for none.
    pub embosser: String,
    /// The other optional sections as read: their descriptor's place in the settings block, kind and bytes.
    pub sections: Vec<(usize, u16, Vec<u8>)>,
    /// The document's embosser page layout (settings +108); None when the file holds none.
    pub layout: Option<DxLayout>,
}

/// The embosser page layout a document keeps in 21 bytes at settings +108 (spec 25 section 10): u16 lines per page,
/// top margin, cells per line, binding margin; u8 interpoint, banner page, interline, a byte of unknown meaning; the
/// paper form name in 9 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DxLayout {
    pub lines: u16,
    pub top_margin: u16,
    pub cells: u16,
    pub binding: u16,
    pub interpoint: bool,
    pub banner: u8,
    /// 0 off, 1 ink and braille, 2 braille-only pass (read back as 1, as the reference loader does).
    pub interline: u8,
    /// Byte +11, meaning unknown (spec 25 10.3); kept as read.
    pub unknown: u8,
    /// The form name, at most 9 characters.
    pub form: String,
}

const LAYOUT_AT: usize = 108;
const LAYOUT_LEN: usize = 21;

/// The layout in a settings block. As the reference loader takes it (spec 25 10.1): only when lines, cells and the
/// form name's first byte are all non-zero; interpoint and interline come back as 0 or 1.
fn read_layout(s: &[u8]) -> Option<DxLayout> {
    let b = &s[LAYOUT_AT..LAYOUT_AT + LAYOUT_LEN];
    let at = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
    if at(0) == 0 || at(4) == 0 || b[12] == 0 {
        return None;
    }
    Some(DxLayout {
        lines: at(0),
        top_margin: at(2),
        cells: at(4),
        binding: at(6),
        interpoint: b[8] != 0,
        banner: b[9],
        interline: u8::from(b[10] != 0),
        unknown: b[11],
        form: b[12..21].iter().take_while(|&&c| c != 0).map(|&c| cp437(c)).collect(),
    })
}

/// The 21 bytes the reference writer stores for a layout (spec 25 10.1: big-endian, the form name's first 9 bytes,
/// no terminator).
fn layout_bytes(l: &DxLayout) -> [u8; LAYOUT_LEN] {
    let mut b = [0u8; LAYOUT_LEN];
    for (i, v) in [l.lines, l.top_margin, l.cells, l.binding].iter().enumerate() {
        b[2 * i..2 * i + 2].copy_from_slice(&v.to_be_bytes());
    }
    b[8] = u8::from(l.interpoint);
    b[9] = l.banner;
    b[10] = l.interline;
    b[11] = l.unknown;
    for (j, c) in l.form.chars().take(9).enumerate() {
        b[12 + j] = to_cp437(c);
    }
    b
}

/// The places of the optional section descriptors in the settings block (spec 05-B).
const SECTION_SLOTS: [usize; 5] = [88, 98, 133, 143, 153];
/// The descriptor of the embosser setup name, and its kind.
const EMBOSSER_SLOT: usize = 88;
const EMBOSSER_KIND: u16 = 1;

/// The settings block of a new document. Fields BrailleCraft sets itself (page numbering, body length, translated
/// line, six-key entry, tables) are filled in when the document is written. The others:
/// - +10 1 and +12 8: constants the reference writes (spec 05).
/// - +16 2 and +30 1201: the values every sample holds; what the reference gives a new document is not traced
///   (spec 05-B (d), UNKNOWN), so the sample values are used.
/// - +64 to +87: the Table of Contents settings the reference's loader sets for a document that has none (spec 05-B
///   (d), observed), the same values every sample holds: contents style 1, the three switches off, number top level
///   on, runover 4 and 2, left margins 1 and 1, increment 2, right margins 6 and 6, guide dots '"', maximum runover
///   off, 0, levels one through 99.
pub fn new_settings() -> Vec<u8> {
    let mut s = vec![0u8; SETTINGS];
    let mut put = |at: usize, v: u16| s[at..at + 2].copy_from_slice(&v.to_be_bytes());
    put(10, 1);
    put(12, 8);
    put(16, 2);
    put(30, 1201);
    put(64, 1);
    put(68, 4);
    put(70, 2);
    put(72, 1);
    put(74, 1);
    put(76, 2);
    put(78, 6);
    put(80, 6);
    put(84, 0);
    put(86, 99);
    s[66] = 0;
    s[67] = 1;
    s[82] = b'"';
    s[83] = 0;
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxError {
    TooShort,
    NotADocument,
    NewerVersion(u16),
    BodyOutOfRange,
}

impl std::fmt::Display for DxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxError::TooShort => write!(f, "The file is too short to be a print or braille document."),
            DxError::NotADocument => write!(f, "The file is not a print or braille document (.dxp or .dxb)."),
            DxError::NewerVersion(v) => write!(
                f,
                "The document was saved in format version {v}; this version of BrailleCraft opens versions 0 and 1."
            ),
            DxError::BodyOutOfRange => write!(f, "The document says its text is longer than the file."),
        }
    }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Code page 437, bytes 0x80 to 0xFF.
pub(crate) const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û',
    'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡',
    '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─',
    '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█',
    '▄', '▌', '▐', '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±', '≥',
    '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

fn cp437(b: u8) -> char {
    if b < 0x80 {
        b as char
    } else {
        CP437_HIGH[(b - 0x80) as usize]
    }
}

/// Reads a .dxp or .dxb file.
pub fn read(d: &[u8]) -> Result<DxDocument, DxError> {
    if d.len() < BODY {
        return Err(DxError::TooShort);
    }
    if u32_at(d, 0) != SIGNATURE || u16_at(d, 4) != 2 {
        return Err(DxError::NotADocument);
    }
    let kind = match u16_at(d, 6) {
        1 => DocumentKind::Print,
        0 => DocumentKind::Braille,
        _ => return Err(DxError::NotADocument),
    };
    let version = u16_at(d, 8);
    if version > 1 {
        return Err(DxError::NewerVersion(version));
    }
    let s = HEADER;
    let page_numbering = [u16_at(d, s), u16_at(d, s + 2), u16_at(d, s + 4), u16_at(d, s + 6), u16_at(d, s + 8)];
    let length = u32_at(d, s + 18) as usize;
    let tables = std::array::from_fn(|i| {
        let name = &d[s + 32 + i * 8..s + 40 + i * 8];
        let end = name.iter().position(|&b| b == 0).unwrap_or(8);
        name[..end].iter().map(|&b| cp437(b)).collect::<String>()
    });
    let body = d.get(BODY..BODY + length).ok_or(DxError::BodyOutOfRange)?;

    // The running text, codes kept as the file marks them (doctext::OPEN and CLOSE are 0x1C and 0x1F).
    let text: String = body
        .iter()
        .map(|&b| match b {
            0x1C => crate::doctext::OPEN,
            0x1F => crate::doctext::CLOSE,
            0x1E => '\u{a0}',
            b => cp437(b),
        })
        .collect();
    let (styles, style_index) = read_styles(d, BODY + length).unwrap_or_default();
    // The optional sections the descriptors point to; one that lies outside the file is left out.
    let mut embosser = String::new();
    let mut sections = Vec::new();
    for slot in SECTION_SLOTS {
        let at = s + slot;
        let kind = u16_at(d, at);
        let (offset, len) = (u32_at(d, at + 2) as usize, u32_at(d, at + 6) as usize);
        if kind == 0 && offset == 0 && len == 0 {
            continue;
        }
        let Some(bytes) = d.get(offset..offset + len) else { continue };
        if slot == EMBOSSER_SLOT && kind == EMBOSSER_KIND {
            embosser = bytes.iter().take_while(|&&b| b != 0).map(|&b| cp437(b)).collect();
        } else {
            sections.push((slot, kind, bytes.to_vec()));
        }
    }
    Ok(DxDocument {
        kind,
        version,
        page_numbering,
        translated_line: u16_at(d, s + 26) != 0,
        six_key: u16_at(d, s + 28) != 0,
        tables,
        text,
        styles,
        style_index,
        settings: d[HEADER..BODY].to_vec(),
        embosser,
        sections,
        layout: read_layout(&d[HEADER..BODY]),
    })
}

/// One block after the body: its values (`width` bytes each) and its strings. None when the file ends early or the
/// block does not add up.
fn read_block(d: &[u8], at: &mut usize, width: usize) -> Option<(Vec<u16>, Vec<Vec<u8>>)> {
    let head = d.get(*at..*at + 8)?;
    let (n, size) = (u16_at(head, 0) as usize, u16_at(head, 4) as usize);
    *at += 8;
    let values = (0..n).map(|i| if width == 2 { d.get(*at + 2 * i..*at + 2 * i + 2).map(|v| u16_at(v, 0)) } else { Some(0) }).collect::<Option<Vec<_>>>()?;
    *at += width * n;
    let strings: Vec<Vec<u8>> = d.get(*at..*at + size)?.split(|&b| b == 0).map(<[u8]>::to_vec).collect();
    *at += size;
    // The strings end in a NUL, so splitting leaves one empty piece after the last.
    (strings.len() == n + 1 && strings.last().is_some_and(Vec::is_empty)).then(|| (values, strings[..n].to_vec()))
}

/// A stored style string to running text: codes marked 0x1C ... 0x1F as in the file, so a bracket in a style's text
/// (",*apt]" in a contents title) stays text.
fn codes_text(b: &[u8]) -> String {
    b.iter().map(|&b| match b {
        0x1C => crate::doctext::OPEN,
        0x1F => crate::doctext::CLOSE,
        b => cp437(b),
    }).collect()
}

/// The two style blocks after the body.
fn read_styles(d: &[u8], at: usize) -> Option<(Vec<(String, String, String)>, Vec<u16>)> {
    let mut at = at;
    let (index, names) = read_block(d, &mut at, 2)?;
    let (_, codes) = read_block(d, &mut at, 0)?;
    if codes.len() != 2 * names.len() {
        return None;
    }
    let styles = names
        .iter()
        .enumerate()
        .map(|(i, n)| (codes_text(n), codes_text(&codes[2 * i]), codes_text(&codes[2 * i + 1])))
        .collect();
    Some((styles, index))
}

/// A character in code page 437; '?' for one it does not have.
fn to_cp437(c: char) -> u8 {
    if (c as u32) < 0x80 {
        return c as u8;
    }
    CP437_HIGH.iter().position(|&h| h == c).map_or(b'?', |i| 0x80 + i as u8)
}

/// A style string in running text to stored bytes.
fn codes_bytes(t: &str) -> Vec<u8> {
    t.chars().map(|c| match c {
        crate::doctext::OPEN => 0x1C,
        crate::doctext::CLOSE => 0x1F,
        c => to_cp437(c),
    }).collect()
}

/// One block after the body (the layout read_block reads).
fn write_block(out: &mut Vec<u8>, values: Option<&[u16]>, strings: &[Vec<u8>]) {
    let size: usize = strings.iter().map(|s| s.len() + 1).sum();
    for v in [strings.len() as u16, 0, size as u16, 0] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    for v in values.unwrap_or(&[]) {
        out.extend_from_slice(&v.to_be_bytes());
    }
    for s in strings {
        out.extend_from_slice(s);
        out.push(0);
    }
}

/// Running text (codes marked 0x1C ... 0x1F) as a document file stores its body: 8-bit code page 437, codes as the
/// bytes 0x1C and 0x1F, a hard space as 0x1E.
pub fn body_bytes(running: &str) -> Vec<u8> {
    running
        .chars()
        .map(|c| match c {
            crate::doctext::OPEN => 0x1C,
            crate::doctext::CLOSE => 0x1F,
            '\u{a0}' => 0x1E,
            c => to_cp437(c),
        })
        .collect()
}

/// The table names a document file in the older format (version 0) gives in place of newer ones (spec 04d-C, from
/// the reference program's compatibility list): older print-to-braille table, its older scan table (empty: kept),
/// the newer table, its newer scan table (empty: any), and the code that must be in force for the older name to
/// apply.
const OLDER_TABLES: [(&str, &str, &str, &str, &str); 8] = [
    ("NLAAUP", "", "NLAACP", "", "g1l"),
    ("BRITIC", "ENGDXN", "BRITIC", "ENGDXP", "caplv1"),
    ("ENMOCP", "GENDXN", "ENMOCP", "GENDXP", "caplv1"),
    ("FRANCU", "", "FRANC", "", "g1l"),
    ("DEUTUP", "", "DEUTCP", "", "g1l"),
    ("GAELCP", "GENDXN", "GAELCP", "GENDXP", "caplv1"),
    ("SPANU", "", "SPAN", "", "g1l"),
    ("SVENUP", "GENDXN", "SVENUP", "GENDXP", "caplv1"),
];

/// The tables as a document file in the older format names them: the print-to-braille table and its scan table
/// changed to their older names where the compatibility list has them and its code is in force (`in_force` says
/// whether a code word, such as "g1l", is among the document's initial codes; inferred from the list, the matcher is
/// not traced, spec 04d-C); the braille-to-print pair is written as it is.
pub fn older_table_names(tables: &[String; 4], in_force: impl Fn(&str) -> bool) -> [String; 4] {
    let mut out = tables.clone();
    for (old, old_scan, new, new_scan, code) in OLDER_TABLES {
        if tables[0].eq_ignore_ascii_case(new) && (new_scan.is_empty() || tables[1].eq_ignore_ascii_case(new_scan)) && in_force(code) {
            out[0] = old.to_string();
            if !old_scan.is_empty() {
                out[1] = old_scan.to_string();
            }
            break;
        }
    }
    out
}

/// Writes a .dxp or .dxb file in the document's format version: 1, the version a document without embedded
/// objects is saved in (spec 05-B), or 0, the older format, which differs only in that number (spec 04d-C; its table
/// names come from older_table_names). The settings block the document was read with (new_settings for a new one)
/// with BrailleCraft's own fields filled in, the body, and the styles.
pub fn write(doc: &DxDocument) -> Vec<u8> {
    let body = body_bytes(&doc.text);
    let mut out = Vec::with_capacity(BODY + body.len() + 2048);
    out.extend_from_slice(&SIGNATURE.to_be_bytes());
    out.extend_from_slice(&2u16.to_be_bytes());
    out.extend_from_slice(&(if doc.kind == DocumentKind::Print { 1u16 } else { 0 }).to_be_bytes());
    out.extend_from_slice(&doc.version.min(1).to_be_bytes());
    let mut s = if doc.settings.len() == SETTINGS { doc.settings.clone() } else { new_settings() };
    for (i, v) in doc.page_numbering.iter().enumerate() {
        s[2 * i..2 * i + 2].copy_from_slice(&v.to_be_bytes());
    }
    s[18..22].copy_from_slice(&(body.len() as u32).to_be_bytes());
    s[26..28].copy_from_slice(&u16::from(doc.translated_line).to_be_bytes());
    s[28..30].copy_from_slice(&u16::from(doc.six_key).to_be_bytes());
    for (i, t) in doc.tables.iter().enumerate() {
        let mut name = [0u8; 8];
        for (j, c) in t.chars().take(8).enumerate() {
            name[j] = to_cp437(c);
        }
        s[32 + i * 8..40 + i * 8].copy_from_slice(&name);
    }
    // The embosser layout; with none, the bytes stay as the document was read (zero for a new one).
    if let Some(l) = &doc.layout {
        s[LAYOUT_AT..LAYOUT_AT + LAYOUT_LEN].copy_from_slice(&layout_bytes(l));
    }
    out.extend_from_slice(&s);
    out.extend_from_slice(&body);
    // The names as stored are sorted (spec 05-B (c)); a document read from a file keeps its order and index values,
    // others are sorted by name with each name's place as its index (INTERIM: the reference lists the index values in
    // its name table's own order, which is not traced).
    let mut styles: Vec<&(String, String, String)> = doc.styles.iter().collect();
    let kept = doc.style_index.len() == styles.len();
    if !kept {
        styles.sort_by(|a, b| codes_bytes(&a.0).cmp(&codes_bytes(&b.0)));
    }
    let index: Vec<u16> = if kept { doc.style_index.clone() } else { (0..styles.len() as u16).collect() };
    write_block(&mut out, Some(&index), &styles.iter().map(|s| codes_bytes(&s.0)).collect::<Vec<_>>());
    let codes: Vec<Vec<u8>> = styles.iter().flat_map(|s| [codes_bytes(&s.1), codes_bytes(&s.2)]).collect();
    write_block(&mut out, None, &codes);
    // The optional sections after the styles, each with its descriptor pointing at where it now lies.
    let mut sections: Vec<(usize, u16, Vec<u8>)> = doc.sections.iter().filter(|s| s.0 != EMBOSSER_SLOT).cloned().collect();
    if !doc.embosser.is_empty() {
        let mut name: Vec<u8> = doc.embosser.chars().map(to_cp437).collect();
        name.push(0);
        sections.push((EMBOSSER_SLOT, EMBOSSER_KIND, name));
    }
    sections.sort_by_key(|s| s.0);
    for slot in SECTION_SLOTS {
        out[HEADER + slot..HEADER + slot + 10].fill(0);
    }
    for (slot, kind, bytes) in sections {
        let at = HEADER + slot;
        let offset = out.len() as u32;
        out[at..at + 2].copy_from_slice(&kind.to_be_bytes());
        out[at + 2..at + 6].copy_from_slice(&offset.to_be_bytes());
        out[at + 6..at + 10].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
        out.extend_from_slice(&bytes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(kind: u16, body: &[u8]) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&SIGNATURE.to_be_bytes());
        d.extend_from_slice(&2u16.to_be_bytes());
        d.extend_from_slice(&kind.to_be_bytes());
        d.extend_from_slice(&1u16.to_be_bytes());
        let mut s = vec![0u8; SETTINGS];
        s[0..2].copy_from_slice(&2u16.to_be_bytes());
        s[18..22].copy_from_slice(&(body.len() as u32).to_be_bytes());
        s[26..28].copy_from_slice(&1u16.to_be_bytes());
        s[32..37].copy_from_slice(b"amerc");
        d.extend_from_slice(&s);
        d.extend_from_slice(body);
        d
    }

    #[test]
    fn reads_codes_hard_spaces_and_code_page_437() {
        let doc = read(&sample(1, b"\x1ces~h1.\x1fCaf\x82\x1cee~h1.\x1f\x1cp\x1fPage\x1eUp\x1chl4\x1ces~para.\x1f\x1f6\x1c<\x1fEnd [x]")).unwrap();
        assert_eq!(doc.kind, DocumentKind::Print);
        assert_eq!(doc.text, "\x1ces~h1.\x1fCafé\x1cee~h1.\x1f\x1cp\x1fPage\u{a0}Up\x1chl4\x1ces~para.\x1f\x1f6\x1c<\x1fEnd [x]");
        // In the editor's form the codes are known as codes and the brackets typed in the text stay text.
        let shown = crate::doctext::Shown::from_running(&doc.text);
        assert_eq!(shown.text, "[es~h1.]Café[ee~h1.][p]\nPage\u{a0}Up[hl4[es~para.]]6\nEnd [x]");
        assert_eq!(shown.codes.len(), 4);
        assert!(doc.translated_line);
        assert!(!doc.six_key);
        assert_eq!(doc.tables[0], "amerc");
        assert_eq!(doc.page_numbering[0], 2);
    }

    #[test]
    fn refuses_other_files_and_newer_versions() {
        assert_eq!(read(b"short"), Err(DxError::TooShort));
        let mut d = sample(0, b"x");
        d[0] = 0;
        assert_eq!(read(&d), Err(DxError::NotADocument));
        let mut d = sample(0, b"x");
        d[9] = 2;
        assert_eq!(read(&d), Err(DxError::NewerVersion(2)));
        let mut d = sample(0, b"xyz");
        d.truncate(d.len() - 1);
        assert_eq!(read(&d), Err(DxError::BodyOutOfRange));
    }

    /// The shipped sample documents, when a folder of them is named in BC_DX_SAMPLES.
    #[test]
    fn reads_every_sample_document() {
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext != "dxp" && ext != "dxb" {
                continue;
            }
            let doc = read(&std::fs::read(&p).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(!doc.text.is_empty(), "{}", p.display());
            // The editor's form gives back the same running text.
            let back = crate::doctext::Shown::from_running(&doc.text).to_running();
            if back != doc.text {
                let (a, b): (Vec<char>, Vec<char>) = (back.chars().collect(), doc.text.chars().collect());
                let k = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                let near = |v: &[char]| v[k.saturating_sub(30)..(k + 30).min(v.len())].iter().collect::<String>();
                panic!("{}: differs at {k}: back {:?} file {:?}", p.display(), near(&a), near(&b));
            }
            // Written back, the file is the same, byte for byte.
            let bytes = std::fs::read(&p).unwrap();
            assert!(!doc.styles.is_empty(), "{}", p.display());
            let again = write(&doc);
            if again != bytes {
                let k = again.iter().zip(&bytes).position(|(x, y)| x != y).unwrap_or(again.len().min(bytes.len()));
                panic!("{}: written file differs at byte {k} (lengths {} and {})", p.display(), again.len(), bytes.len());
            }
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn a_new_document_is_written_and_read_back() {
        let doc = DxDocument {
            kind: DocumentKind::Braille,
            version: 1,
            page_numbering: [4, 4, 2, 2, 1],
            translated_line: true,
            six_key: false,
            tables: ["amerc".into(), "AMEDXP".into(), "AMEBPC".into(), "AMEDXB".into()],
            text: "\x1cp\x1f,! cat\u{a0}sat [x]".into(),
            styles: vec![("para.".into(), "\x1cp\x1f".into(), String::new()), ("bold".into(), "\x1ccs~b\x1f".into(), "\x1cce~b\x1f".into())],
            style_index: Vec::new(),
            settings: Vec::new(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
        };
        let bytes = write(&doc);
        assert_eq!(&bytes[..10], b"\xFFDSI\x00\x02\x00\x00\x00\x01");
        let back = read(&bytes).unwrap();
        assert_eq!(back.text, doc.text);
        assert_eq!(back.tables, doc.tables);
        assert_eq!(back.page_numbering, doc.page_numbering);
        assert!(back.translated_line && !back.six_key);
        // Styles are stored sorted by name.
        assert_eq!(back.styles[0], ("bold".into(), "\x1ccs~b\x1f".into(), "\x1cce~b\x1f".into()));
        assert_eq!(back.styles[1].0, "para.");
        assert_eq!(back.style_index, vec![0, 1]);
        assert_eq!(&back.settings[64..88], &new_settings()[64..88]);
        assert_eq!(write(&back), bytes);
        // The older format differs only in its version number.
        let old = write(&DxDocument { version: 0, ..back.clone() });
        assert_eq!(old[9], 0);
        assert_eq!(old[10..], bytes[10..]);
    }

    #[test]
    fn embosser_name_and_other_sections_are_kept() {
        let mut doc = read(&sample(0, b"abc")).unwrap();
        doc.embosser = "Index Everest-D V5".into();
        doc.sections = vec![(98, 3, b"extra".to_vec())];
        let bytes = write(&doc);
        // The name section comes first after the styles, NUL terminated, with its descriptor at settings +88.
        let at = HEADER + 88;
        assert_eq!(u16_at(&bytes, at), 1);
        let (offset, len) = (u32_at(&bytes, at + 2) as usize, u32_at(&bytes, at + 6) as usize);
        assert_eq!(&bytes[offset..offset + len], b"Index Everest-D V5\0");
        let back = read(&bytes).unwrap();
        assert_eq!(back.embosser, "Index Everest-D V5");
        assert_eq!(back.sections, vec![(98, 3, b"extra".to_vec())]);
        assert_eq!(write(&back), bytes);
        // Without a name the descriptor is zero.
        let none = write(&DxDocument { embosser: String::new(), sections: Vec::new(), ..back });
        assert!(none[at..at + 10].iter().all(|&b| b == 0));
    }

    #[test]
    fn the_embosser_layout_is_kept_in_21_bytes_at_settings_108() {
        let mut doc = read(&sample(0, b"abc")).unwrap();
        assert_eq!(doc.layout, None);
        doc.layout = Some(DxLayout {
            lines: 25,
            top_margin: 1,
            cells: 40,
            binding: 2,
            interpoint: true,
            banner: 1,
            interline: 2,
            unknown: 0,
            form: "11_w".into(),
        });
        let bytes = write(&doc);
        let at = HEADER + 108;
        assert_eq!(&bytes[at..at + 21], b"\x00\x19\x00\x01\x00\x28\x00\x02\x01\x01\x02\x0011_w\0\0\0\0\0");
        // Read back as the reference loader takes it: interline 2 comes back as 1.
        let back = read(&bytes).unwrap();
        assert_eq!(back.layout.as_ref().map(|l| (l.lines, l.cells, l.interline, l.form.as_str())), Some((25, 40, 1, "11_w")));
        // A layout with no lines, no cells or no form name is not taken.
        let mut zero_lines = bytes.clone();
        zero_lines[at..at + 2].fill(0);
        assert_eq!(read(&zero_lines).unwrap().layout, None);
        let mut no_form = bytes.clone();
        no_form[at + 12] = 0;
        assert_eq!(read(&no_form).unwrap().layout, None);
    }

    #[test]
    fn the_older_format_uses_older_table_names_when_their_code_is_in_force() {
        let t = |a: &str, b: &str| [a.to_string(), b.to_string(), "AMEBPC".to_string(), "AMEDXB".to_string()];
        assert_eq!(older_table_names(&t("nlaacp", "NLADXP"), |c| c == "g1l"), t("NLAAUP", "NLADXP"));
        assert_eq!(older_table_names(&t("nlaacp", "NLADXP"), |_| false), t("nlaacp", "NLADXP"));
        assert_eq!(older_table_names(&t("BRITIC", "ENGDXP"), |c| c == "caplv1"), t("BRITIC", "ENGDXN"));
        // The scan table must match where the list names one.
        assert_eq!(older_table_names(&t("BRITIC", "OTHER"), |_| true), t("BRITIC", "OTHER"));
    }
}
