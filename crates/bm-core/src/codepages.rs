//! The code pages Save As offers for formatted text (.prf), in the reference program's order, and how each writes a
//! character (spec 04d-E, 04d-F).
//!
//! The reference makes a converter for the chosen name: a Windows code page where one exists, its own converters for
//! UTF-16 and UTF-32, and plain Latin-1 for a name it has no converter for. A character the converter cannot write is
//! written '#', and nothing is written before the text (no byte-order mark). Text is converted one character at a
//! time, as the reference's page writer does.
//!
//! Here the single-byte code pages come from data/codepages.txt, the multi-byte ones from encoding_rs.
//! INTERIM: the reference uses the Windows tables; encoding_rs follows the WHATWG Encoding Standard, which differs
//! from them in a few rare characters, and the four Mac multi-byte code pages are written with the nearest encoding
//! (Shift_JIS, Big5, EUC-KR, GBK).

use std::collections::HashMap;
use std::sync::OnceLock;

/// The names in the order Save As lists them; the first is chosen every time.
pub const NAMES: [&str; 82] = [
    "ISO-8859-1", "ISO-8859-2", "ISO-8859-3", "ISO-8859-4", "ISO-8859-5", "ISO-8859-6", "ISO-8859-7", "ISO-8859-8",
    "ISO-8859-9", "ISO-8859-10", "ISO-8859-11", "ISO-8859-12", "ISO-8859-13", "ISO-8859-14", "ISO-8859-15", "KOI8-R",
    "KOI8-U", "WINDOWS-437", "WINDOWS-866", "WINDOWS-874", "WINDOWS-932", "WINDOWS-936", "WINDOWS-949", "WINDOWS-950",
    "WINDOWS-1250", "WINDOWS-1251", "WINDOWS-1252", "WINDOWS-1253", "WINDOWS-1254", "WINDOWS-1255", "WINDOWS-1256",
    "WINDOWS-1257", "WINDOWS-1258", "WINDOWS-1361", "UTF-7", "UTF-8", "EUC-JP", "UTF-16BE", "UTF-16LE", "UTF-32BE",
    "UTF-32LE", "MacRoman", "MacJapanese", "MacChineseTrad", "MacKorean", "MacArabic", "MacHebrew", "MacGreek",
    "MacCyrillic", "MacDevanagari", "MacGurmukhi", "MacGujarati", "MacOriya", "MacBengali", "MacTamil", "MacTelugu",
    "MacKannada", "MacMalayalam", "MacSinhalese", "MacBurmese", "MacKhmer", "MacThai", "MacLaotian", "MacGeorgian",
    "MacArmenian", "MacChineseSimp", "MacTibetan", "MacMongolian", "MacEthiopic", "MacCentralEurRoman",
    "MacVietnamese", "MacExtArabic", "MacSymbol", "MacDingbats", "MacTurkish", "MacCroatian", "MacIcelandic",
    "MacRomanian", "MacCeltic", "MacGaelic", "MacKeyboardGlyphs", "ISO-2022-JP",
];

/// The single-byte code pages: per name, the byte for each character it defines.
fn single_byte() -> &'static HashMap<String, HashMap<char, u8>> {
    static TABLES: OnceLock<HashMap<String, HashMap<char, u8>>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut out = HashMap::new();
        for line in include_str!("../data/codepages.txt").lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let Some((name, row)) = line.split_once('\t') else { continue };
            let mut map = HashMap::new();
            for (b, cell) in row.split(' ').enumerate() {
                if let Some(c) = u32::from_str_radix(cell, 16).ok().and_then(char::from_u32) {
                    map.entry(c).or_insert(b as u8);
                }
            }
            out.insert(name.to_string(), map);
        }
        out
    })
}

/// The multi-byte code pages and the encoding each is written with.
fn multi_byte(name: &str) -> Option<&'static encoding_rs::Encoding> {
    Some(match name {
        "WINDOWS-932" | "MacJapanese" => encoding_rs::SHIFT_JIS,
        "WINDOWS-936" | "MacChineseSimp" => encoding_rs::GBK,
        "WINDOWS-949" | "MacKorean" => encoding_rs::EUC_KR,
        "WINDOWS-950" | "MacChineseTrad" => encoding_rs::BIG5,
        "EUC-JP" => encoding_rs::EUC_JP,
        "ISO-2022-JP" => encoding_rs::ISO_2022_JP,
        _ => return None,
    })
}

/// UTF-7 for one character (RFC 2152): letters, digits, space, line ends and the characters '(),-./:?' as
/// themselves, '+' as "+-", every other character as "+", the base64 of its UTF-16, "-".
fn utf7(c: char) -> Vec<u8> {
    const DIRECT: &str = "'(),-./:? \t\r\n";
    if c.is_ascii_alphanumeric() || DIRECT.contains(c) {
        return vec![c as u8];
    }
    if c == '+' {
        return b"+-".to_vec();
    }
    const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut units = [0u16; 2];
    let bytes: Vec<u8> = c.encode_utf16(&mut units).iter().flat_map(|u| u.to_be_bytes()).collect();
    let mut out = vec![b'+'];
    let mut bits = 0u32;
    let mut n = 0;
    for b in bytes {
        bits = (bits << 8) | u32::from(b);
        n += 8;
        while n >= 6 {
            n -= 6;
            out.push(B64[((bits >> n) & 63) as usize]);
        }
    }
    if n > 0 {
        out.push(B64[((bits << (6 - n)) & 63) as usize]);
    }
    out.push(b'-');
    out
}

/// One character in the named code page; None when the code page cannot write it.
pub fn encode_char(name: &str, c: char) -> Option<Vec<u8>> {
    let mut units = [0u16; 2];
    match name {
        "UTF-7" => return Some(utf7(c)),
        "UTF-8" => return Some(c.to_string().into_bytes()),
        "UTF-16BE" => return Some(c.encode_utf16(&mut units).iter().flat_map(|u| u.to_be_bytes()).collect()),
        "UTF-16LE" => return Some(c.encode_utf16(&mut units).iter().flat_map(|u| u.to_le_bytes()).collect()),
        "UTF-32BE" => return Some((c as u32).to_be_bytes().to_vec()),
        "UTF-32LE" => return Some((c as u32).to_le_bytes().to_vec()),
        _ => {}
    }
    if let Some(table) = single_byte().get(name) {
        return table.get(&c).map(|b| vec![*b]);
    }
    if let Some(enc) = multi_byte(name) {
        let one = c.to_string();
        let (bytes, _, unmappable) = enc.encode(&one);
        return (!unmappable).then(|| bytes.into_owned());
    }
    // ISO-8859-1, and a name the reference has no converter for: plain Latin-1.
    ((c as u32) <= 0xFF).then(|| vec![c as u32 as u8])
}

/// Text in the named code page, one character at a time; a character it cannot write is written '#'.
pub fn encode(name: &str, text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        match encode_char(name, c) {
            Some(b) => out.extend(b),
            None => out.extend(encode_char(name, '#').unwrap_or_else(|| vec![b'#'])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_single_byte_code_page_has_its_table() {
        for name in NAMES {
            let latin1 = name == "ISO-8859-1" || name == "ISO-8859-12" || name == "WINDOWS-1361";
            if !name.starts_with("UTF") && multi_byte(name).is_none() && !name.starts_with("Mac") && !latin1 {
                assert!(single_byte().contains_key(name), "{name}");
            }
        }
        assert_eq!(single_byte().len(), 37);
    }

    #[test]
    fn characters_are_written_in_the_chosen_code_page() {
        assert_eq!(encode("ISO-8859-1", "Caf\u{e9} \u{3b1}"), b"Caf\xe9 #".to_vec());
        assert_eq!(encode("WINDOWS-1252", "\u{20ac}5"), b"\x805".to_vec());
        assert_eq!(encode("WINDOWS-437", "\u{e9}"), b"\x82".to_vec());
        assert_eq!(encode("KOI8-R", "\u{430}"), b"\xc1".to_vec());
        assert_eq!(encode("UTF-16LE", "A"), b"A\0".to_vec());
        assert_eq!(encode("UTF-7", "A\u{e9}"), b"A+AOk-".to_vec());
        assert_eq!(encode("WINDOWS-932", "\u{3042}"), b"\x82\xa0".to_vec());
        // A Mac code page the reference has no converter for writes plain Latin-1.
        assert_eq!(encode("MacTibetan", "\u{e9}\u{f40}"), b"\xe9#".to_vec());
        assert_eq!(encode("MacRoman", "\u{e9}"), b"\x8e".to_vec());
    }
}
