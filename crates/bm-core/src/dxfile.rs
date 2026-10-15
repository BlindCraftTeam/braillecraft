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
//! In BrailleMaker's editor a code is written between square brackets, and a code that starts a new line ([<], [p],
//! [l]) is followed by a line break so the text reads line by line.

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
    /// The text with codes in square brackets.
    pub text: String,
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
                "The document was saved in format version {v}; this version of BrailleMaker opens versions 0 and 1."
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

/// Codes after which the editor starts a new line.
fn breaks_line(code: &str) -> bool {
    matches!(code, "<" | "p" | "l")
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

    let mut text = String::with_capacity(body.len() + body.len() / 8);
    // Codes nest (a table-of-contents entry code can hold a style code: 0x1C hl4 0x1C es~para. 0x1F 0x1F), so the
    // two bytes pair like brackets.
    let mut open: Vec<usize> = Vec::new();
    for &b in body {
        match b {
            0x1C => {
                text.push('[');
                open.push(text.len());
            }
            0x1F => {
                if let Some(start) = open.pop() {
                    let line_break = open.is_empty() && breaks_line(&text[start..]);
                    text.push(']');
                    if line_break {
                        text.push('\n');
                    }
                }
            }
            0x1E => text.push('\u{a0}'),
            b => text.push(cp437(b)),
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
    })
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
        let doc = read(&sample(1, b"\x1ces~h1.\x1fCaf\x82\x1cee~h1.\x1f\x1cp\x1fPage\x1eUp\x1chl4\x1ces~para.\x1f\x1f6")).unwrap();
        assert_eq!(doc.kind, DocumentKind::Print);
        assert_eq!(doc.text, "[es~h1.]Café[ee~h1.][p]\nPage\u{a0}Up[hl4[es~para.]]6");
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

    /// The shipped sample documents, when a folder of them is named in BM_DX_SAMPLES.
    #[test]
    fn reads_every_sample_document() {
        let Ok(dir) = std::env::var("BM_DX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext != "dxp" && ext != "dxb" {
                continue;
            }
            let doc = read(&std::fs::read(&p).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(!doc.text.is_empty(), "{}", p.display());
            assert!(!doc.text.contains('\u{1c}') && !doc.text.contains('\u{1f}'), "{}", p.display());
            count += 1;
        }
        assert!(count > 0);
    }
}
