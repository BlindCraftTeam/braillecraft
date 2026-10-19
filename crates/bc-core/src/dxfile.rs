//! Reading and writing .dxp (print) and .dxb (braille) document files, format versions 0, 1 and 2.
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
//! - The version 2 section (descriptor +153, kind 0) is a zip archive: the embedded objects, then the folder entry
//!   "ebraille" and the entry "/ebraille/package.opf" holding the document's eBraille metadata as package.opf text
//!   (bc_core::ebraille::opf_text). A file that has this section is format version 2.
//! - Settings +108 to +128 hold the document's embosser page layout (DxLayout, spec 25 section 10); +129 and +131 a
//!   print document's page size from Document, Printer Setup (lines, characters; spec 05 section A10).
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
    /// Braille page numbers on even and odd pages, print page numbers on even and odd pages, first page number. Held
    /// as 32-bit numbers in a document (+0x478 ... +0x488); the file keeps 16 bits of each.
    pub page_numbering: [u32; 5],
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
    /// The view the document was saved in: the coded view (settings +14), its hyphenation (+16: 0 none, 1 dashes, 2
    /// dashes and hyphens, 3 French, 4 French 1103) and the view font (+30: 1201 print, 1202 braille, 1203
    /// SimBraille).
    pub coded_view: bool,
    pub hyphenation: u16,
    pub view_font: u16,
    /// The 224-byte settings block as read, for writing back what BrailleCraft does not use itself.
    pub settings: Vec<u8>,
    /// The name of the document's embosser setup (settings +88); empty for none.
    pub embosser: String,
    /// The other optional sections as read: their descriptor's place in the settings block, kind and bytes.
    pub sections: Vec<(usize, u16, Vec<u8>)>,
    /// The document's embosser page layout (settings +108); None when the file holds none.
    pub layout: Option<DxLayout>,
    /// The document's page size from Document, Printer Setup: lines per page and characters per line (settings
    /// +129 and +131, the document's +0x58c and +0x590; spec 05 section A10, A16.2), kept for both kinds of
    /// document. 0 in either is read as no size, and a document without one takes the Global, Printer Setup page
    /// size (print_page_or).
    pub print_page: (u16, u16),
    /// The caret's place when the document was saved (settings +22, u32): a place in the body, one for each of its
    /// bytes; the document opens with the caret there (spec 05 section A10). 0 for a new document.
    pub caret: u32,
    /// The document's eBraille metadata as package.opf text (Document, eBraille Metadata), kept in the version 2
    /// section; None when the document has none.
    pub ebraille: Option<String>,
}

/// The descriptor place and kind of the version 2 section.
const VERSION2_SLOT: usize = 153;
const VERSION2_KIND: u16 = 0;
/// The version 2 section's eBraille entries: the folder, and the metadata entry as it is written (a name read
/// without the leading "/" is the same entry).
const EBRAILLE_FOLDER: &str = "ebraille/";
const EBRAILLE_OPF: &str = "/ebraille/package.opf";

fn is_ebraille_entry(name: &str) -> bool {
    let name = name.trim_start_matches('/');
    name == "ebraille" || name == "ebraille/" || name == "ebraille/package.opf"
}

/// The package.opf text in a version 2 section's archive (the entry "ebraille/package.opf", with or without a
/// leading "/"); None when the section is not an archive or has no such entry.
pub fn section_opf(section: &[u8]) -> Option<String> {
    let parts = crate::docx::parts(section).ok()?;
    let part = parts.iter().find(|p| p.name.trim_start_matches('/') == "ebraille/package.opf")?;
    Some(String::from_utf8_lossy(&part.bytes).into_owned())
}

/// The version 2 section with `opf` as its eBraille metadata: the section's other entries (the embedded objects)
/// first, as read, then the folder "ebraille" and "/ebraille/package.opf"; with no metadata only the other entries.
/// None when nothing is left to keep. A section that is not an archive is kept as it is (its metadata cannot be
/// put in).
pub fn section_with_opf(section: Option<&[u8]>, opf: Option<&str>) -> Option<Vec<u8>> {
    let kept = match section {
        Some(bytes) => match crate::docx::parts(bytes) {
            Ok(parts) => parts.into_iter().filter(|p| !is_ebraille_entry(&p.name)).collect(),
            Err(_) => return Some(bytes.to_vec()),
        },
        None => Vec::new(),
    };
    if kept.is_empty() && opf.is_none() {
        return None;
    }
    let mut entries: Vec<(&str, &[u8])> = kept.iter().map(|p| (p.name.as_str(), p.bytes.as_slice())).collect();
    if let Some(opf) = opf {
        entries.push((EBRAILLE_FOLDER, b""));
        entries.push((EBRAILLE_OPF, opf.as_bytes()));
    }
    Some(crate::docx::deflated_zip(&entries))
}

/// The Global, Printer Setup page size before it is changed (lines, characters: the settings DesiredLinesPerPage
/// and DesiredCharsPerLine, defaults 54 and 65; spec A16.4 unknown 6).
pub const PRINT_PAGE_DEFAULT: (u16, u16) = (54, 65);

/// A document's print page size: the file's own, or the Global, Printer Setup page size `global` (lines,
/// characters) when the file holds none (spec A16.4 unknown 6).
pub fn print_page_or(own: (u16, u16), global: (u32, u32)) -> (u32, u32) {
    if own.0 == 0 || own.1 == 0 {
        global
    } else {
        (u32::from(own.0), u32::from(own.1))
    }
}

/// Save As's dialog title when File, Save As asks.
pub const SAVE_AS_TITLE: &str = "Save as:";
/// Save As's dialog title when File, Save falls back to it.
pub const SAVE_TITLE: &str = "Save:";

/// Whether File, Save writes the document in place, without asking (spec A15.10): it was read from or saved to a
/// document file in the current format (`document_file`) and has a file name. Otherwise Save asks with Save As,
/// titled SAVE_TITLE.
pub fn saves_in_place(document_file: bool, path: &str) -> bool {
    document_file && !path.is_empty()
}
/// Where the print page size is in the settings block.
const PRINT_PAGE_AT: usize = 129;

/// The embosser page layout a document keeps in 21 bytes at settings +108 (spec 25 section 10): u16 lines per page,
/// top margin, cells per line, binding margin; u8 interpoint, banner page, interline, double-size print; the
/// paper form name in 9 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DxLayout {
    pub lines: u16,
    pub top_margin: u16,
    pub cells: u16,
    pub binding: u16,
    pub interpoint: bool,
    pub banner: u8,
    /// 0 off, 1 ink and braille, 2 braille-only pass (read back as 1).
    pub interline: u8,
    /// Byte +11: the setup's DoubleSizePrint (formatting block +0x24, the settings key of that name; spec 05
    /// section A10). The setup dialog and the model check set it to 0; kept as read.
    pub double_size: u8,
    /// The form name, at most 9 characters.
    pub form: String,
}

const LAYOUT_AT: usize = 108;
const LAYOUT_LEN: usize = 21;

/// The layout in a settings block, read (spec 25 10.1) only when lines, cells and the
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
        double_size: b[11],
        form: b[12..21].iter().take_while(|&&c| c != 0).map(|&c| cp437(c)).collect(),
    })
}

/// The 21 bytes stored for a layout (spec 25 10.1: big-endian, the form name's first 9 bytes,
/// no terminator).
fn layout_bytes(l: &DxLayout) -> [u8; LAYOUT_LEN] {
    let mut b = [0u8; LAYOUT_LEN];
    for (i, v) in [l.lines, l.top_margin, l.cells, l.binding].iter().enumerate() {
        b[2 * i..2 * i + 2].copy_from_slice(&v.to_be_bytes());
    }
    b[8] = u8::from(l.interpoint);
    b[9] = l.banner;
    b[10] = l.interline;
    b[11] = l.double_size;
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
/// - +10 1 and +12 8: constants written (spec 05).
/// - +16 2 and +30 1201: placeholders the document overwrites: a new document takes +16 from its template's
///   Hyphenation and +30 from Global Default Views (a print document 1201; spec 05 section
///   A10).
/// - +64 to +87: the Table of Contents settings a document that has none is given on loading (spec 05-B
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
                "The document was saved in format version {v}; this version of BrailleCraft opens versions 0, 1 and 2."
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
    if version > 2 {
        return Err(DxError::NewerVersion(version));
    }
    let s = HEADER;
    let page_numbering = std::array::from_fn(|i| u16_at(d, s + 2 * i) as u32);
    let length = u32_at(d, s + 18) as usize;
    let tables = std::array::from_fn(|i| {
        let name = &d[s + 32 + i * 8..s + 40 + i * 8];
        let end = name.iter().position(|&b| b == 0).unwrap_or(8);
        name[..end].iter().map(|&b| cp437(b)).collect::<String>()
    });
    let body = d.get(BODY..BODY + length).ok_or(DxError::BodyOutOfRange)?;

    // The running text: internal text through the character table, codes kept as the file marks them
    // (doctext::OPEN and CLOSE are 0x1C and 0x1F), 0x1E the hard space.
    let text = crate::internal::decode(body);
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
    let ebraille = sections.iter().find(|s| s.0 == VERSION2_SLOT).and_then(|s| section_opf(&s.2));
    Ok(DxDocument {
        ebraille,
        kind,
        version,
        page_numbering,
        translated_line: u16_at(d, s + 26) != 0,
        coded_view: u16_at(d, s + 14) != 0,
        hyphenation: u16_at(d, s + 16),
        view_font: u16_at(d, s + 30),
        six_key: u16_at(d, s + 28) != 0,
        tables,
        text,
        styles,
        style_index,
        settings: d[HEADER..BODY].to_vec(),
        embosser,
        sections,
        layout: read_layout(&d[HEADER..BODY]),
        caret: u32_at(d, s + 22),
        print_page: {
            let (l, c) = (u16_at(d, HEADER + PRINT_PAGE_AT), u16_at(d, HEADER + PRINT_PAGE_AT + 2));
            if l == 0 || c == 0 { (0, 0) } else { (l, c) }
        },
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
    crate::internal::decode(b)
}

/// The two style blocks after the body.
fn read_styles(d: &[u8], at: usize) -> Option<(Vec<(String, String, String)>, Vec<u16>)> {
    let mut at = at;
    let (index, names) = read_block(d, &mut at, 2)?;
    let (_, codes) = read_block(d, &mut at, 0)?;
    if codes.len() != 2 * names.len() {
        return None;
    }
    // The codes of the name at place i are the pair at place index[i] (checked on the samples: dochow's h1. is
    // then its heading codes; taken in the names' own order it was another style's).
    if index.iter().any(|&k| k as usize >= names.len()) {
        return None;
    }
    let styles = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let k = index[i] as usize;
            (codes_text(n), codes_text(&codes[2 * k]), codes_text(&codes[2 * k + 1]))
        })
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
    crate::internal::encode_lossy(t, b'?')
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

/// Running text (codes marked 0x1C ... 0x1F) as a document file stores its body: internal text (code page 437 with
/// two-byte codes, crate::internal), codes as the bytes 0x1C and 0x1F, a hard space as 0x1E; "?" for a character
/// internal text has no code for.
pub fn body_bytes(running: &str) -> Vec<u8> {
    crate::internal::encode_lossy(running, b'?')
}

/// The table names a document file in the older format (version 0) gives in place of newer ones (spec 04d-C, the
/// compatibility list): older print-to-braille table, its older scan table (empty: kept),
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
/// whether a code word, such as "g1l", is in the document's initial codes: codes_hold; spec 04d-C); the
/// braille-to-print pair is written as it is.
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

/// Whether `codes` (the document's initial codes) hold `word` anywhere, A to Z matched without case (spec 04d-C):
/// "g1l" is found in "[G1L]" and in "[g1lx]" alike.
pub fn codes_hold(codes: &str, word: &str) -> bool {
    word.is_empty() || codes.to_ascii_lowercase().contains(&word.to_ascii_lowercase())
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
    // The version 2 section with the document's eBraille metadata: kept as read while the metadata is the one it
    // holds, else made again with it.
    let mut sections: Vec<(usize, u16, Vec<u8>)> = doc.sections.iter().filter(|s| s.0 != EMBOSSER_SLOT).cloned().collect();
    let old = sections.iter().position(|s| s.0 == VERSION2_SLOT);
    let same = old.is_some_and(|i| section_opf(&sections[i].2) == doc.ebraille) || (old.is_none() && doc.ebraille.is_none());
    if !same {
        let made = section_with_opf(old.map(|i| sections[i].2.as_slice()), doc.ebraille.as_deref());
        let kind = old.map_or(VERSION2_KIND, |i| sections[i].1);
        if let Some(i) = old {
            sections.remove(i);
        }
        if let Some(bytes) = made {
            sections.push((VERSION2_SLOT, kind, bytes));
        }
    }
    // Version 2 when the document carries the version-2 section (its descriptor at +153: embedded objects or
    // eBraille data), else the version asked for (0 or 1).
    let version = if sections.iter().any(|s| s.0 == VERSION2_SLOT) && doc.version >= 1 { 2 } else { doc.version.min(1) };
    out.extend_from_slice(&version.to_be_bytes());
    let mut s = if doc.settings.len() == SETTINGS { doc.settings.clone() } else { new_settings() };
    for (i, v) in doc.page_numbering.iter().enumerate() {
        s[2 * i..2 * i + 2].copy_from_slice(&(*v as u16).to_be_bytes());
    }
    s[18..22].copy_from_slice(&(body.len() as u32).to_be_bytes());
    s[22..26].copy_from_slice(&doc.caret.min(body.len() as u32).to_be_bytes());
    s[14..16].copy_from_slice(&u16::from(doc.coded_view).to_be_bytes());
    s[16..18].copy_from_slice(&doc.hyphenation.to_be_bytes());
    s[26..28].copy_from_slice(&u16::from(doc.translated_line).to_be_bytes());
    if (1201..=1203).contains(&doc.view_font) {
        s[30..32].copy_from_slice(&doc.view_font.to_be_bytes());
    }
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
    s[PRINT_PAGE_AT..PRINT_PAGE_AT + 2].copy_from_slice(&doc.print_page.0.to_be_bytes());
    s[PRINT_PAGE_AT + 2..PRINT_PAGE_AT + 4].copy_from_slice(&doc.print_page.1.to_be_bytes());
    out.extend_from_slice(&s);
    out.extend_from_slice(&body);
    // Style names are kept in a list sorted as they are inserted (case is ignored), each with the
    // number of its style, the order styles were added; the codes are kept in that order (spec 05 section A10). A
    // document read from a file keeps its order and numbers; for others the number is the style's place in the
    // document's own list, so a style added later comes after the template's.
    let mut styles: Vec<&(String, String, String)> = doc.styles.iter().collect();
    let kept = doc.style_index.len() == styles.len();
    let index: Vec<u16> = if kept {
        doc.style_index.clone()
    } else {
        let mut order: Vec<usize> = (0..styles.len()).collect();
        order.sort_by(|&a, &b| name_order(&codes_bytes(&styles[a].0), &codes_bytes(&styles[b].0)));
        styles = order.iter().map(|&i| styles[i]).collect();
        order.iter().map(|&i| i as u16).collect()
    };
    write_block(&mut out, Some(&index), &styles.iter().map(|s| codes_bytes(&s.0)).collect::<Vec<_>>());
    // Each style's codes go at the place its index value names, as they are read.
    let mut codes: Vec<Vec<u8>> = vec![Vec::new(); 2 * styles.len()];
    for (s, &k) in styles.iter().zip(&index) {
        let k = (k as usize).min(styles.len().saturating_sub(1));
        codes[2 * k] = codes_bytes(&s.1);
        codes[2 * k + 1] = codes_bytes(&s.2);
    }
    write_block(&mut out, None, &codes);
    // The optional sections after the styles, each with its descriptor pointing at where it now lies.
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

/// The styles in the order of a document file's style-name list: as given when `kept` (the order a file was read
/// with), else sorted as `write` sorts the names.
pub fn name_list_order(styles: &[(String, String, String)], kept: bool) -> Vec<(String, String, String)> {
    let mut out = styles.to_vec();
    if !kept {
        out.sort_by(|a, b| name_order(&codes_bytes(&a.0), &codes_bytes(&b.0)));
    }
    out
}

/// What the command line's /RemoveEmbosserInfo, /RemovePrinterInfo and /RemovePageInfo take out of a document file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remove {
    /// The embosser part: the embosser setup name (descriptor +88) and the embosser layout (settings +108, 21 bytes).
    Embosser,
    /// The printer part: the extra block (descriptor +98), the page size of Document, Printer Setup (+129 and +131)
    /// and the section at +143.
    Printer,
    /// Both parts: "page info" is the embosser part and the printer part together.
    Page,
}

/// The descriptor places of the printer part.
const PRINTER_SLOTS: [usize; 2] = [98, 143];

/// Takes the embosser part, the printer part or both out of a document read from a file, so that `write` gives the
/// file again with those parts zero and everything else (body, styles, table names, the other sections) as read.
/// The body's table names are not checked against the tables installed: a remove run changes only what it removes.
pub fn remove_info(doc: &mut DxDocument, what: Remove) {
    if matches!(what, Remove::Embosser | Remove::Page) {
        doc.embosser.clear();
        doc.layout = None;
        // `write` keeps the settings bytes when there is no layout, so they are zeroed here.
        if doc.settings.len() == SETTINGS {
            doc.settings[LAYOUT_AT..LAYOUT_AT + LAYOUT_LEN].fill(0);
        }
    }
    if matches!(what, Remove::Printer | Remove::Page) {
        doc.sections.retain(|s| !PRINTER_SLOTS.contains(&s.0));
        doc.print_page = (0, 0);
    }
}

/// The order of two style names: byte by byte, each lowered from A to Z and taken
/// as a signed byte, the end of a name counting as 0 (so a byte from 0x80 sorts before the end and before ASCII).
fn name_order(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    let at = |n: &[u8], i: usize| n.get(i).map_or(0i8, |&c| c.to_ascii_lowercase() as i8);
    let mut i = 0;
    loop {
        let (x, y) = (at(a, i), at(b, i));
        if x != y || x == 0 {
            return x.cmp(&y);
        }
        i += 1;
    }
}

/// The view font a file's +30 names, as 1201 print, 1202 braille or 1203 SimBraille: the ids 1093 to 1095 stand
/// for the same three fonts. None for any other value.
pub fn view_font_id(raw: u16) -> Option<u16> {
    match raw {
        1201..=1203 => Some(raw),
        1093..=1095 => Some(raw - 1093 + 1201),
        _ => None,
    }
}

/// Whether an opened file's saved view (view font, coded view, translated line, six-key entry) and caret are used
/// (spec 27 section 9, unknown 6): only when the file is reopened from the recent documents list and its +30 holds
/// a valid view font. Any other file takes the Global Default Views settings with the caret at the start; Full-Page
/// always comes from those settings.
pub fn restores_view(in_recent: bool, view_font: u16) -> bool {
    in_recent && view_font_id(view_font).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_view_only_from_recent_list() {
        assert!(restores_view(true, 1202));
        assert!(restores_view(true, 0x446));
        assert!(!restores_view(false, 1202));
        assert!(!restores_view(true, 0));
        assert_eq!(view_font_id(1093), Some(1201));
        assert_eq!(view_font_id(1095), Some(1203));
        assert_eq!(view_font_id(1204), None);
    }

    /// Names sort as they are inserted, case ignored; each keeps the number of its place in the
    /// document's list and its codes go there.
    #[test]
    fn a_code_word_is_found_anywhere_in_the_initial_codes_without_case() {
        assert!(codes_hold("[G1L]", "g1l"));
        assert!(codes_hold("[ab][g1lx]", "g1l"));
        assert!(!codes_hold("[g2]", "g1l"));
        assert!(codes_hold("", ""));
    }

    #[test]
    fn style_names_sort_without_case_and_keep_their_document_numbers() {
        assert_eq!(name_order(b"Box", b"acronym"), std::cmp::Ordering::Greater);
        assert_eq!(name_order(b"box", b"box-h1."), std::cmp::Ordering::Less);
        assert_eq!(name_order(b"ab", b"ab\x80"), std::cmp::Ordering::Greater);
        let doc = DxDocument {
            kind: DocumentKind::Print,
            version: 1,
            page_numbering: [0; 5],
            translated_line: false,
            six_key: false,
            tables: ["amerc".into(), "AMEDXP".into(), "AMEBPC".into(), "AMEDXB".into()],
            text: "a".into(),
            styles: vec![("para.".into(), "\x1cp\x1f".into(), String::new()), ("Box".into(), "\x1cbx\x1f".into(), String::new()), ("added".into(), "\x1cx\x1f".into(), String::new())],
            style_index: Vec::new(),
            coded_view: false,
            hyphenation: 2,
            view_font: 1201,
            settings: Vec::new(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
            print_page: (0, 0),
            caret: 0,
            ebraille: None,
        };
        let back = read(&write(&doc)).unwrap();
        let names: Vec<&str> = back.styles.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(names, ["added", "Box", "para."]);
        assert_eq!(back.style_index, [2, 1, 0]);
        assert_eq!(back.styles[1].1, "\x1cbx\x1f");
    }

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
        d[9] = 3;
        assert_eq!(read(&d), Err(DxError::NewerVersion(3)));
        // Version 2 (embedded objects, eBraille data) opens.
        let mut d = sample(0, b"x");
        d[9] = 2;
        assert_eq!(read(&d).map(|r| r.version), Ok(2));
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
            // Each style has its own codes: the heading style h1. is a centred heading wherever a sample has it.
            if let Some(h1) = doc.styles.iter().find(|s| s.0 == "h1.") {
                assert!(h1.1.contains("\x1chds\x1f"), "{}: h1. begins {:?}", p.display(), h1.1);
            }
            // Written back, the styles are read again as they were.
            assert_eq!(read(&write(&doc)).unwrap().styles, doc.styles, "{}", p.display());
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
    fn a_print_document_keeps_six_key_through_save_and_open() {
        let doc = DxDocument {
            kind: DocumentKind::Print,
            version: 1,
            page_numbering: [0, 0, 0, 0, 1],
            translated_line: false,
            six_key: true,
            tables: ["amerc".into(), "AMEDXP".into(), "AMEBPC".into(), "AMEDXB".into()],
            text: "words".into(),
            styles: Vec::new(),
            style_index: Vec::new(),
            coded_view: false,
            hyphenation: 1,
            view_font: 1201,
            settings: Vec::new(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
            print_page: (0, 0),
            caret: 0,
            ebraille: None,
        };
        let back = read(&write(&doc)).unwrap();
        assert_eq!(back.kind, DocumentKind::Print);
        assert!(back.six_key && !back.translated_line);
    }

    #[test]
    fn print_page_falls_back_to_global_and_save_writes_in_place_only_for_a_document_file() {
        assert_eq!(print_page_or((0, 0), (50, 70)), (50, 70));
        assert_eq!(print_page_or((40, 0), (50, 70)), (50, 70));
        assert_eq!(print_page_or((40, 72), (50, 70)), (40, 72));
        // An imported document asks; one read from "x.DXP" is written in place; a document file needs a name.
        assert!(!saves_in_place(false, "/home/a/import.docx"));
        assert!(saves_in_place(true, "/home/a/x.DXP"));
        assert!(!saves_in_place(true, ""));
        assert_eq!((SAVE_AS_TITLE, SAVE_TITLE), ("Save as:", "Save:"));
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
            coded_view: true,
            hyphenation: 1,
            view_font: 1203,
            settings: Vec::new(),
            embosser: String::new(),
            sections: Vec::new(),
            layout: None,
            print_page: (0, 0),
            caret: 0,
            ebraille: None,
        };
        let bytes = write(&doc);
        assert_eq!(&bytes[..10], b"\xFFDSI\x00\x02\x00\x00\x00\x01");
        let back = read(&bytes).unwrap();
        assert_eq!(back.text, doc.text);
        assert_eq!(back.tables, doc.tables);
        assert_eq!(back.page_numbering, doc.page_numbering);
        assert!(back.translated_line && !back.six_key);
        assert!(back.coded_view);
        assert_eq!((back.hyphenation, back.view_font), (1, 1203));
        assert_eq!(back.print_page, (0, 0));
        // A document's own print page size goes to settings +129 and +131 and comes back (a braille one here).
        let sized = read(&write(&DxDocument { print_page: (40, 72), ..back.clone() })).unwrap();
        assert_eq!(sized.print_page, (40, 72));
        assert_eq!(&write(&DxDocument { print_page: (40, 72), ..back.clone() })[10 + 129..10 + 133], &[0, 40, 0, 72]);
        // Styles are stored sorted by name, each with the number of its place in the document's list (para. first).
        assert_eq!(back.styles[0], ("bold".into(), "\x1ccs~b\x1f".into(), "\x1cce~b\x1f".into()));
        assert_eq!(back.styles[1].0, "para.");
        assert_eq!(back.style_index, vec![1, 0]);
        assert_eq!(&back.settings[64..88], &new_settings()[64..88]);
        assert_eq!(write(&back), bytes);
        // Other index values: each name keeps its own codes through a write and a read.
        let shuffled = DxDocument { style_index: vec![0, 1], ..back.clone() };
        let again = read(&write(&shuffled)).unwrap();
        assert_eq!(again.styles, back.styles);
        assert_eq!(again.style_index, vec![0, 1]);
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

    /// A document with an embosser name and layout, a printer page size and sections at 98, 133 and 143, written and
    /// read back.
    fn with_page_info() -> DxDocument {
        let mut doc = read(&sample(0, b"abc\x1cp\x1fdef")).unwrap();
        doc.embosser = "Index Everest-D V5".into();
        doc.layout = Some(DxLayout {
            lines: 25,
            top_margin: 1,
            cells: 40,
            binding: 2,
            interpoint: true,
            banner: 1,
            interline: 1,
            double_size: 0,
            form: "11_w".into(),
        });
        doc.print_page = (54, 65);
        doc.sections = vec![(98, 3, b"printer".to_vec()), (133, 2, b"objects".to_vec()), (143, 4, b"pdesc".to_vec())];
        doc.styles = vec![("h1.".into(), "\x1cfr\x1f".into(), "\x1cfx\x1f".into()), ("para.".into(), "\x1cp\x1f".into(), String::new())];
        doc.style_index = vec![1, 0];
        read(&write(&doc)).unwrap()
    }

    fn removed(what: Remove) -> (Vec<u8>, DxDocument) {
        let mut doc = with_page_info();
        remove_info(&mut doc, what);
        let bytes = write(&doc);
        let back = read(&bytes).unwrap();
        (bytes, back)
    }

    #[test]
    fn remove_info_embosser_clears_name_and_layout() {
        let (bytes, back) = removed(Remove::Embosser);
        assert_eq!(back.embosser, "");
        assert_eq!(back.layout, None);
        assert!(bytes[HEADER + 108..HEADER + 129].iter().all(|&b| b == 0));
        assert!(bytes[HEADER + 88..HEADER + 98].iter().all(|&b| b == 0));
        assert!(back.sections.iter().any(|s| s.0 == 98));
        assert_eq!(back.print_page, (54, 65));
    }

    #[test]
    fn remove_info_printer_clears_extra_block_and_print_page() {
        let (bytes, back) = removed(Remove::Printer);
        assert!(!back.sections.iter().any(|s| s.0 == 98 || s.0 == 143));
        assert!(back.sections.iter().any(|s| s.0 == 133));
        assert!(bytes[HEADER + 129..HEADER + 133].iter().all(|&b| b == 0));
        assert!(bytes[HEADER + 98..HEADER + 108].iter().all(|&b| b == 0));
        assert!(bytes[HEADER + 143..HEADER + 153].iter().all(|&b| b == 0));
        assert_eq!(back.embosser, "Index Everest-D V5");
        assert!(back.layout.is_some());
    }

    #[test]
    fn remove_info_page_clears_both() {
        let (bytes, back) = removed(Remove::Page);
        assert_eq!((back.embosser.as_str(), back.layout.is_none(), back.print_page), ("", true, (0, 0)));
        assert_eq!(back.sections.iter().map(|s| s.0).collect::<Vec<_>>(), vec![133]);
        assert!(bytes[HEADER + 88..HEADER + 108].iter().all(|&b| b == 0));
        assert!(bytes[HEADER + 108..HEADER + 133].iter().all(|&b| b == 0));
    }

    #[test]
    fn remove_info_keeps_body_styles_and_table_names() {
        let before = with_page_info();
        for what in [Remove::Embosser, Remove::Printer, Remove::Page] {
            let (bytes, back) = removed(what);
            assert_eq!(back.text, before.text);
            assert_eq!(back.styles, before.styles);
            assert_eq!(back.style_index, before.style_index);
            assert_eq!(back.tables, before.tables);
            // The body and the style blocks are the same bytes.
            let full = write(&before);
            let body_at = BODY;
            let body_len = u32_at(&full, HEADER + 18) as usize;
            assert_eq!(bytes[body_at..body_at + body_len], full[body_at..body_at + body_len]);
            // The settings outside the removed parts are the same bytes.
            assert_eq!(bytes[HEADER..HEADER + 88], full[HEADER..HEADER + 88]);
        }
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
            double_size: 0,
            form: "11_w".into(),
        });
        let bytes = write(&doc);
        let at = HEADER + 108;
        assert_eq!(&bytes[at..at + 21], b"\x00\x19\x00\x01\x00\x28\x00\x02\x01\x01\x02\x0011_w\0\0\0\0\0");
        // Read back: interline 2 comes back as 1.
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

    #[test]
    fn ebraille_metadata_survives_save_and_open() {
        let meta = crate::ebraille::Metadata { titles: vec!["Book".into()], date: "2026-01-02".into(), date_copyrighted: "2026-01-02".into(), ..Default::default() };
        let mut doc = read(&sample(0, b"abc")).unwrap();
        doc.ebraille = Some(crate::ebraille::opf_text(&meta));
        let bytes = write(&doc);
        assert_eq!(u16_at(&bytes, 8), 2);
        let back = read(&bytes).unwrap();
        assert_eq!(back.version, 2);
        assert_eq!(crate::ebraille::metadata_from_opf(back.ebraille.as_deref().unwrap()), meta);
        let section = &back.sections.iter().find(|s| s.0 == VERSION2_SLOT).unwrap().2;
        let parts = crate::docx::parts(section).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["ebraille/", "/ebraille/package.opf"]);
        // Written again unchanged, the file is the same.
        assert_eq!(write(&back), bytes);
        // Edited metadata replaces the old entry and keeps another entry of the section.
        let other = crate::docx::stored_zip(&[("ObjectCollection.xml", b"<objects/>"), ("ebraille/package.opf", b"<package/>")]);
        let mut with_objects = back.clone();
        with_objects.sections = vec![(VERSION2_SLOT, 0, other)];
        with_objects.ebraille = Some(crate::ebraille::opf_text(&crate::ebraille::Metadata { titles: vec!["New".into()], ..meta.clone() }));
        let again = read(&write(&with_objects)).unwrap();
        let parts = crate::docx::parts(&again.sections.iter().find(|s| s.0 == VERSION2_SLOT).unwrap().2).unwrap();
        let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["ObjectCollection.xml", "ebraille/", "/ebraille/package.opf"]);
        assert_eq!(crate::docx::part(&parts, "ObjectCollection.xml"), Some(&b"<objects/>"[..]));
        assert_eq!(crate::ebraille::metadata_from_opf(again.ebraille.as_deref().unwrap()).titles, ["New"]);
        // Metadata removed: only the other entries stay; with none left the section goes and the version is 1.
        let gone = read(&write(&DxDocument { ebraille: None, ..again.clone() })).unwrap();
        assert!(gone.ebraille.is_none() && gone.version == 2);
        let none = write(&DxDocument { ebraille: None, ..back });
        assert_eq!(u16_at(&none, 8), 1);
        assert!(read(&none).unwrap().sections.is_empty());
    }
}
