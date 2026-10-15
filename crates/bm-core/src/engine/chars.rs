//! Character helpers shared by the parts of the engine: code page 437, the kinds of the internal character codes,
//! and the text rules the table files are written for (which characters count as blank, where lines end, how hex
//! numbers are written).

use crate::dxfile::CP437_HIGH;

/// End of input, after the 256 byte codes.
pub(crate) const EOF: i64 = 0x100;

/// A blank character, as the table texts treat them: tab, line feed, vertical tab, form feed, carriage return, the
/// four separator controls 0x1C to 0x1F, space, next line (0x85) and no-break space.
pub(crate) fn is_blank(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' | ' ' | '\u{85}' | '\u{a0}')
}

pub(crate) fn strip(s: &str) -> &str {
    s.trim_matches(is_blank)
}

pub(crate) fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_blank)
}

pub(crate) fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_blank)
}

/// Splits a text into lines at line feed, carriage return (with or without a following line feed), vertical tab,
/// form feed, the separators 0x1C to 0x1E, next line (0x85), and the Unicode line and paragraph separators. A line
/// end at the very end gives no empty last line.
pub(crate) fn split_lines(s: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let end = match c {
            '\r' => {
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                    Some((i, i + 2))
                } else {
                    Some((i, i + 1))
                }
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                Some((i, i + c.len_utf8()))
            }
            _ => None,
        };
        if let Some((cut, next)) = end {
            lines.push(&s[start..cut]);
            start = next;
        }
    }
    if start < s.len() {
        lines.push(&s[start..]);
    }
    lines
}

/// A hex number as the character table writes them: blanks around it, an optional sign and "0x", digits, with
/// single underscores allowed between digits. None when the text is not such a number.
pub(crate) fn parse_hex(s: &str) -> Option<i64> {
    let s = strip(s);
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).map_or(s, |r| r.strip_prefix('_').unwrap_or(r));
    if s.is_empty() || s.starts_with('_') || s.ends_with('_') || s.contains("__") {
        return None;
    }
    let mut v: i64 = 0;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        v = v.checked_mul(16)?.checked_add(c.to_digit(16)? as i64)?;
    }
    Some(if negative { -v } else { v })
}

/// Item `i` of a list, counting from the end when `i` is negative (the tables use 1-based numbers, and a 0 there
/// names the last item).
pub(crate) fn item<T>(v: &[T], i: i64) -> Option<&T> {
    let len = v.len() as i64;
    let i = if i < 0 { len + i } else { i };
    if (0..len).contains(&i) {
        v.get(i as usize)
    } else {
        None
    }
}

/// A text in code page 437, one code per character; Err names the first character that has no code there.
pub(crate) fn encode_cp437(text: &str) -> Result<Vec<i64>, char> {
    text.chars()
        .map(|c| {
            if (c as u32) < 0x80 {
                Ok(c as i64)
            } else {
                CP437_HIGH.iter().position(|&h| h == c).map(|p| 0x80 + p as i64).ok_or(c)
            }
        })
        .collect()
}

/// Bytes read as Latin-1: each byte is the character with that number.
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// The kind of an internal character code: flags (0x01 digit, 0x02 letter, 0x04 lower case, 0x08 upper case,
/// 0x10 printable, 0x20 first byte of a two-byte code), its lower-case form and its capital form. Codes 0x80 to
/// 0xA5 are the accented letters of code page 437; 0x9C as a capital means "no one-byte capital".
pub(crate) fn kind(c: i64) -> (u8, i64, i64) {
    const HIGH: [(u8, i64, i64); 38] = [
        (0x1a, 0x87, 0x80), (0x16, 0x81, 0x9a), (0x16, 0x82, 0x90), (0x16, 0x83, 0x9c), (0x16, 0x84, 0x8e),
        (0x16, 0x85, 0x9c), (0x16, 0x86, 0x8f), (0x16, 0x87, 0x80), (0x16, 0x88, 0x9c), (0x16, 0x89, 0x9c),
        (0x16, 0x8a, 0x9c), (0x16, 0x8b, 0x9c), (0x16, 0x8c, 0x9c), (0x16, 0x8d, 0x9c), (0x1a, 0x84, 0x8e),
        (0x1a, 0x86, 0x8f), (0x1a, 0x82, 0x90), (0x16, 0x91, 0x92), (0x1a, 0x91, 0x92), (0x16, 0x93, 0x9c),
        (0x16, 0x94, 0x99), (0x16, 0x95, 0x9c), (0x16, 0x96, 0x9c), (0x16, 0x97, 0x9c), (0x16, 0x98, 0x9c),
        (0x1a, 0x94, 0x99), (0x1a, 0x81, 0x9a), (0x10, 0x9b, 0x9b), (0x20, 0x9c, 0x9c), (0x10, 0x9d, 0x9d),
        (0x10, 0x9e, 0x9e), (0x16, 0x9f, 0xb341), (0x16, 0xa0, 0x9c), (0x16, 0xa1, 0x9c), (0x16, 0xa2, 0x9c),
        (0x16, 0xa3, 0x9c), (0x16, 0xa4, 0xa5), (0x1a, 0xa4, 0xa5),
    ];
    match c {
        0x80..=0xa5 => HIGH[(c - 0x80) as usize],
        0x00..=0x1f | 0x7f => (0x00, c, c),
        0x30..=0x39 => (0x11, c, c),
        0x41..=0x5a => (0x1a, c + 0x20, c),
        0x61..=0x7a => (0x16, c, c - 0x20),
        0x20..=0x7e | 0xa6..=0xaf | 0xf6..=0xf9 => (0x10, c, c),
        0xb0..=0xff => (0x20, c, c),
        _ => (0, c, c),
    }
}

/// The flags of a code (0 for codes past the byte range).
pub(crate) fn flags(c: i64) -> u8 {
    if (0..256).contains(&c) {
        kind(c).0
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_blanks_and_numbers() {
        assert_eq!(split_lines("a\r\nb\rc\nd\u{c}e\n"), vec!["a", "b", "c", "d", "e"]);
        assert_eq!(strip("\u{1c} x \u{a0}"), "x");
        assert_eq!(parse_hex(" 0041 "), Some(0x41));
        assert_eq!(parse_hex("1a"), Some(26));
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("\"A\""), None);
        assert_eq!(item(&[1, 2, 3], -1), Some(&3));
        assert_eq!(item(&[1, 2, 3], 3), None);
    }

    #[test]
    fn code_page_437_and_kinds() {
        assert_eq!(encode_cp437("Café½\u{a0}"), Ok(vec![0x43, 0x61, 0x66, 0x82, 0xab, 0xff]));
        assert_eq!(encode_cp437("€"), Err('€'));
        assert_eq!(kind(b'A' as i64), (0x1a, 0x61, 0x41));
        assert_eq!(kind(0x82), (0x16, 0x82, 0x90));
        assert_eq!(flags(0xb0), 0x20);
        assert_eq!(flags(0x100), 0);
    }
}
