//! Reading a scan table (.sct): the settings that tell the scanner how to read text before the rules see it.
//!
//! The file is text. After the line "-ninit 6" each line is one value: the text before a comment ("|" and a
//! space), with blanks at its end removed. "|NNN" stands for the byte NNN (three decimal digits) and "||" for "|". A
//! line that holds only a comment gives no value. The values come in a fixed order, read below; the last is "***".

use super::chars::{latin1, lstrip};
use super::EngineError;

#[derive(Debug, Clone, PartialEq)]
pub struct ScanTable {
    /// How bytes are read (0: text; the other modes are not built).
    pub binmod: i64,
    /// 1: blanks at the end of a line are dropped.
    pub trailing_blanks: i64,
    /// The flag character that starts a code (0x1C when codes are written between 0x1C and 0x1F).
    pub flag: i64,
    pub fullstop: i64,
    /// Line ends: 0 joins lines, 1 a line end is a space, 2 a line end becomes a new-line code.
    pub newline: i64,
    /// What a form feed becomes.
    pub ff_text: Vec<i64>,
    /// The paragraph code put in for a new paragraph.
    pub par_code: Vec<i64>,
    /// New paragraphs: 0 none, 1 at a blank line, 2 at an indented line, 3 both.
    pub no_par: i64,
    /// Hyphens at a line end: 0 kept, 1 the word is joined again, 2 also a doubled hyphen.
    pub rejoin: i64,
    /// Runs of spaces: 0 one space, 1 two spaces after a full stop, 2 kept, 3 a run becomes a tab code.
    pub spaces: i64,
    /// Spaces per tab (-1: a tab is an error).
    pub tabs: i64,
    pub ninmod: i64,
    /// Case of format words: 0 upper, 1 lower, 2 as written.
    pub fold_controls: i64,
    /// Case of words: 0 upper, 1 lower with capital signs, 2 as written.
    pub fold_words: i64,
    /// 1: a capital passage ends at a word with lower case.
    pub cessation: i64,
    pub map_braces: i64,
    /// Emphasis fonts (italic, bold, ...): see `Font`.
    pub fonts: Vec<Font>,
    pub spare: i64,
    pub cpe: i64,
    pub nxsls: i64,
    /// Output characters: format word, two-character format word, capital sign, capital termination.
    pub out_format: i64,
    pub out_format2: i64,
    pub out_cap: i64,
    pub out_capterm: i64,
    /// Computer-braille switching for web and mail addresses (not built; English tables have it off).
    pub icb: i64,
    /// The eleven words that change the line, space and paragraph modes while reading.
    pub modes: Vec<String>,
    /// More than this many all-capital words in a row make a capital passage (0: no passages).
    pub triple_caps: i64,
    /// Doubled adjectives (not built; English tables have it off).
    pub dbl_adj: i64,
    pub cpe_fts: i64,
    pub cpe_l: i64,
    pub dbl_adj_after: i64,
    /// The range of characters made upper case when words are folded to upper case.
    pub fold_lo: i64,
    pub fold_hi: i64,
}

/// An emphasis font, one line of the scan table: "start end words code", for example "fts~i fte~i 4 3". Between the
/// format words [start] and [end] the scanner writes the font's code in front of each word when fewer than `words`
/// words lie between them (0: always), else twice in front of the first word and once in front of the last.
#[derive(Debug, Clone, PartialEq)]
pub struct Font {
    pub start: String,
    pub end: String,
    pub words: i64,
    pub code: i64,
}

impl Font {
    /// Reads a font line the way the reference reads it: two words, then two numbers (each a sign and digits at
    /// its start); None when the line does not give all four.
    pub fn parse(text: &str) -> Option<Font> {
        let mut it = text.split_whitespace();
        let start = it.next()?.to_string();
        let end = it.next()?.to_string();
        let mut number = || -> Option<i64> {
            let w = it.next()?;
            let (sign, rest) = match w.as_bytes().first() {
                Some(b'-') => (-1, &w[1..]),
                Some(b'+') => (1, &w[1..]),
                _ => (1, w),
            };
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                return None;
            }
            Some(sign * digits.parse::<i64>().unwrap_or(i64::MAX))
        };
        let words = number()?;
        let code = number()?;
        Some(Font { start, end, words, code })
    }
}

/// The values after the "-ninit 6" line, read as the reference reads them: "|" and a space (or a line end) starts a
/// comment to the end of the line; "||" is "|"; "|" and exactly three digits is that byte (another "|" is kept, the
/// three characters used up); a line that is only a comment gives no value; blanks at the end of a value are taken
/// off. The file's line ends may be CR LF.
fn tokens(text: &str) -> Result<Vec<Vec<i64>>, EngineError> {
    let text: Vec<char> = text.replace("\r\n", "\n").chars().collect();
    const END: i64 = 0x100;
    const COMMENT: i64 = 0x101;
    let mut pos = 0usize;
    let ch = |pos: &mut usize| -> i64 {
        let Some(&c) = text.get(*pos) else { return END };
        *pos += 1;
        if c != '|' {
            return c as i64;
        }
        match text.get(*pos) {
            Some(' ') => {
                *pos = text[*pos..].iter().position(|&x| x == '\n').map_or(text.len(), |k| *pos + k);
                COMMENT
            }
            Some('\n') => {
                *pos += 1;
                COMMENT
            }
            Some('|') => {
                *pos += 1;
                '|' as i64
            }
            _ => {
                let digits: String = text[*pos..].iter().take(3).collect();
                *pos = (*pos + 3).min(text.len());
                match digits.parse::<i64>() {
                    Ok(v) if digits.len() == 3 && digits.chars().all(|d| d.is_ascii_digit()) && v <= 0xff => v,
                    _ => '|' as i64,
                }
            }
        }
    };
    let token = |pos: &mut usize| -> Option<Vec<i64>> {
        loop {
            let mut buf = Vec::new();
            let c = loop {
                let c = ch(pos);
                if c == 10 || c == END || c == COMMENT {
                    break c;
                }
                buf.push(c);
            };
            if c == 10 {
                return Some(buf);
            }
            if c == END {
                return if buf.is_empty() { None } else { Some(buf) };
            }
            // A comment: the rest of its line; a line that was only a comment gives no value.
            let end = loop {
                let c = ch(pos);
                if c == 10 || c == END {
                    break c == END;
                }
            };
            if !buf.is_empty() {
                return Some(buf);
            }
            if end {
                return None;
            }
        }
    };
    let strip_end = |mut b: Vec<i64>| {
        while b.last() == Some(&0x20) {
            b.pop();
        }
        b
    };
    // The first value is "-ninit 6"; the settings follow it.
    let head = strip_end(token(&mut pos).unwrap_or_default());
    if head != "-ninit 6".chars().map(|c| c as i64).collect::<Vec<i64>>() {
        return Err(EngineError::BadTable("not a scan table (no \"-ninit 6\" line)".into()));
    }
    let mut out = Vec::new();
    while let Some(t) = token(&mut pos) {
        out.push(strip_end(t));
    }
    Ok(out)
}

impl ScanTable {
    pub fn read(data: &[u8]) -> Result<ScanTable, EngineError> {
        let toks = tokens(&latin1(data))?;
        let mut it = toks.into_iter();
        let mut raw = || it.next().ok_or_else(|| EngineError::BadTable("the scan table ends early".into()));
        // A number: leading digits after blanks (with a sign), else 0, then a range check.
        let num = |t: Vec<i64>, lo: i64, hi: i64| -> Result<i64, EngineError> {
            let s: String = t.iter().map(|&b| char::from_u32((b & 0xff) as u32).unwrap_or('\0')).collect();
            let s = lstrip(&s);
            let (sign, rest) = match s.as_bytes().first() {
                Some(b'-') => (-1, &s[1..]),
                Some(b'+') => (1, &s[1..]),
                _ => (1, s),
            };
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let v = if digits.is_empty() { 0 } else { sign * digits.parse::<i64>().unwrap_or(i64::MAX) };
            if !(lo..=hi).contains(&v) {
                return Err(EngineError::BadTable(format!("scan table value {v} is outside {lo}..{hi}")));
            }
            Ok(v)
        };
        let char_of = |t: Vec<i64>| t.first().copied().unwrap_or(0);
        let string = |t: Vec<i64>, n: usize| t.into_iter().take(n).collect::<Vec<i64>>();
        let text_of = |t: Vec<i64>| t.iter().map(|&b| char::from_u32((b & 0xff) as u32).unwrap_or('\0')).collect::<String>();
        const BIG: i64 = 0x7fff_ffff;

        let binmod = num(raw()?, 0, BIG)?;
        let trailing_blanks = num(raw()?, 0, 1)?;
        let flag = char_of(raw()?);
        let fullstop = char_of(raw()?);
        let newline = num(raw()?, 0, BIG)?;
        let ff_text = string(raw()?, 15);
        let par_code = string(raw()?, 15);
        let no_par = num(raw()?, 0, BIG)?;
        let rejoin = num(raw()?, 0, BIG)?;
        let spaces = num(raw()?, 0, BIG)?;
        let tabs = num(raw()?, -1, BIG)?;
        let ninmod = num(raw()?, 0, 1)?;
        let fold_controls = num(raw()?, 0, 2)?;
        let fold_words = num(raw()?, 0, 2)?;
        let cessation = num(raw()?, 0, 1)?;
        let map_braces = num(raw()?, 0, 1)?;
        let nfonts = num(raw()?, 0, BIG)?;
        let mut fonts = Vec::new();
        for _ in 0..nfonts {
            let line: Vec<i64> = raw()?.into_iter().take(39).collect();
            fonts.push(
                Font::parse(&text_of(line))
                    .ok_or_else(|| EngineError::BadTable("a font line needs start, end, words and code".into()))?,
            );
        }
        let spare = num(raw()?, 0, 3)?;
        let cpe = num(raw()?, 0, 1)?;
        let nxsls = num(raw()?, 0, 1)?;
        let out_format = char_of(raw()?);
        let out_format2 = char_of(raw()?);
        let out_cap = char_of(raw()?);
        let out_capterm = char_of(raw()?);
        let icb = num(raw()?, 0, 1)?;
        let mut modes = Vec::new();
        for _ in 0..11 {
            modes.push(text_of(string(raw()?, 15)));
        }
        let triple_caps = num(raw()?, 0, BIG)?;
        let dbl_adj = num(raw()?, 0, BIG)?;
        let end = raw()?;
        if end != vec![42, 42, 42] {
            return Err(EngineError::BadTable("the scan table has no \"***\" end line".into()));
        }
        let (cpe_fts, cpe_l) = match cpe {
            1 => (1, 0),
            2 => (0, 1),
            3 => (1, 1),
            _ => (0, 0),
        };
        let (fold_lo, fold_hi) = if map_braces == 1 { (0x60, 0x7e) } else { (0x61, 0x7a) };
        Ok(ScanTable {
            binmod,
            trailing_blanks,
            flag,
            fullstop,
            newline,
            ff_text,
            par_code,
            no_par,
            rejoin,
            spaces,
            tabs,
            ninmod,
            fold_controls,
            fold_words,
            cessation,
            map_braces,
            fonts,
            spare,
            cpe,
            nxsls,
            out_format,
            out_format2,
            out_cap,
            out_capterm,
            icb,
            modes,
            triple_caps,
            dbl_adj,
            cpe_fts,
            cpe_l,
            dbl_adj_after: if dbl_adj == 0 { 0 } else { dbl_adj + 1 },
            fold_lo,
            fold_hi,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_escapes() {
        let t = tokens("| head\r\n-ninit 6\r\n0 | binmod\r\n| a comment\r\n|028 | flag\r\n|| | the flag |\r\nx||y|1z\r\n").unwrap();
        // A line that is only a comment gives no value; "||" at a line start is "|"; "|1z" keeps the bar.
        assert_eq!(t, vec![vec!['0' as i64], vec![28], vec!['|' as i64], vec!['x' as i64, '|' as i64, 'y' as i64, '|' as i64]]);
    }
}
