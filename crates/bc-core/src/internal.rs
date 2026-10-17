//! Internal text: the 8-bit text the translation engine reads and writes and document files store, and its
//! conversion to and from the Unicode text the program shows.
//!
//! Internal text is code page 437 with two-byte codes: a byte the character model marks as a first byte (0x9C,
//! 0xB0 to 0xF5, 0xFA to 0xFF) and the byte after it are one code (first byte times 256 plus the second), so 0x9C is
//! not the pound sign there, and the pound sign is the code F523. The character table built into the program
//! (tables/characters.txt) gives each code its Unicode character.
//!
//! The conversion keeps every byte: a code with no Unicode character, or one whose character the table gives to an
//! earlier code as well, is shown as the private-use character U+F0000 plus the code and turned back into that
//! code, so opening a file, translating it and saving it never changes a code the program cannot name.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::dxfile::CP437_HIGH;
use crate::engine::chars::flags;
use crate::engine::Chitab;

/// Private-use characters U+F0000 to U+FFFFF carry codes the table cannot name.
const KEPT: u32 = 0xF0000;

struct Codec {
    to_char: HashMap<i64, char>,
    to_code: HashMap<char, i64>,
}

fn codec() -> &'static Codec {
    static CODEC: OnceLock<Codec> = OnceLock::new();
    CODEC.get_or_init(|| {
        let table = Chitab::builtin();
        let mut entries: Vec<(i64, char)> = table
            .unicode_map()
            .iter()
            .filter_map(|(&code, &u)| char::from_u32(u as u32).map(|c| (code, c)))
            .collect();
        // A one-byte code wins over a two-byte code for the same character, then the lower code.
        entries.sort_by_key(|&(code, _)| (code >= 0x100, code));
        // A character below 0x80 is always its own byte, so no other code may come back as one.
        let mut to_code: HashMap<char, i64> = (0u8..0x80).map(|b| (b as char, i64::from(b))).collect();
        for &(code, c) in &entries {
            to_code.entry(c).or_insert(code);
        }
        let to_char = entries.iter().filter(|(code, c)| to_code.get(c) == Some(code)).map(|&(code, c)| (code, c)).collect();
        Codec { to_char, to_code }
    })
}

/// Whether a byte starts a two-byte code.
pub fn is_first_byte(b: u8) -> bool {
    flags(i64::from(b)) & 0x20 != 0
}

fn kept(code: i64) -> char {
    char::from_u32(KEPT + code as u32).unwrap_or('\u{fffd}')
}

/// One code as Unicode: bytes below 0x80 are themselves, apart from what the table names otherwise (0x1E is the
/// hard space); a code the table gives a character to is that character; any other is kept.
fn code_char(code: i64) -> char {
    match codec().to_char.get(&code) {
        Some(&c) => c,
        None if code < 0x80 => char::from_u32(code as u32).unwrap_or('\u{fffd}'),
        None => kept(code),
    }
}

/// Internal text as Unicode.
pub fn decode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if is_first_byte(b) && i + 1 < bytes.len() {
            out.push(code_char(i64::from(b) << 8 | i64::from(bytes[i + 1])));
            i += 2;
        } else {
            out.push(code_char(i64::from(b)));
            i += 1;
        }
    }
    out
}

/// One character's code: itself below 0x80; a kept code; the table's code; else the code page 437 byte when that
/// byte is a one-byte code. None when internal text has no code for it.
pub fn code_of(c: char) -> Option<i64> {
    let u = c as u32;
    if u < 0x80 {
        return Some(i64::from(u));
    }
    if (KEPT..KEPT + 0x10000).contains(&u) {
        return Some(i64::from(u - KEPT));
    }
    if let Some(&code) = codec().to_code.get(&c) {
        return Some(code);
    }
    CP437_HIGH.iter().position(|&h| h == c).map(|p| 0x80 + p as i64).filter(|&b| !is_first_byte(b as u8))
}

fn push_code(out: &mut Vec<u8>, code: i64) {
    if code >= 0x100 {
        out.push((code >> 8) as u8);
    }
    out.push((code & 0xff) as u8);
}

/// Unicode as internal text; Err names the first character internal text has no code for.
pub fn encode(text: &str) -> Result<Vec<u8>, char> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        push_code(&mut out, code_of(c).ok_or(c)?);
    }
    Ok(out)
}

/// Unicode as internal text, with `missing` in place of a character that has no code (for saving files).
pub fn encode_lossy(text: &str, missing: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        match code_of(c) {
            Some(code) => push_code(&mut out, code),
            None => out.push(missing),
        }
    }
    out
}

/// Bytes of a text file that is not UTF-8, read as Windows code page 1252 (curly quotes, dashes, the euro sign and
/// œ are in 0x80 to 0x9F there).
pub fn windows_1252(bytes: &[u8]) -> String {
    encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_byte_codes_and_unicode() {
        // £ is F523, œ B283, the em dash EC44, the euro sign ED5C, Â 9C83; é is the one-byte 82; the hard space 1E.
        assert_eq!(encode("£œ—€Âé\u{a0}"), Ok(vec![0xf5, 0x23, 0xb2, 0x83, 0xec, 0x44, 0xed, 0x5c, 0x9c, 0x83, 0x82, 0x1e]));
        assert_eq!(decode(&[0xf5, 0x23, 0xb2, 0x83, 0x9c, 0x83, 0x82, 0x1e]), "£œÂé\u{a0}");
        // ½ is the fraction code F53D; the code 0xAB (½ in code page 437) has no Unicode form and is kept.
        assert_eq!(encode("½"), Ok(vec![0xf5, 0x3d]));
        assert_eq!(encode(&decode(&[0xab])), Ok(vec![0xab]));
        // A lone first byte and a code the table cannot name come back as the same bytes.
        for bytes in [vec![0x41, 0x9c], vec![0x9c, 0x41, 0x20], vec![0xab, 0x9e, 0xa9]] {
            assert_eq!(encode(&decode(&bytes)), Ok(bytes.clone()));
        }
        assert_eq!(encode("\u{10fffd}"), Err('\u{10fffd}'));
    }

    #[test]
    fn every_byte_pair_survives() {
        for a in 0u8..=255 {
            for b in [0x20u8, 0x41, 0x83, 0xff] {
                let bytes = vec![a, b];
                assert_eq!(encode(&decode(&bytes)), Ok(bytes.clone()), "{a:02x} {b:02x}");
            }
        }
    }

    #[test]
    fn windows_text_files() {
        assert_eq!(windows_1252(&[0x93, b'a', 0x94, 0x9c, 0x80]), "\u{201c}a\u{201d}œ€");
    }
}
