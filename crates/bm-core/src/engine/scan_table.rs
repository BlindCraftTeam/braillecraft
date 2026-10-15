//! Reading a scan table (.sct): the settings that tell the scanner how to read text before the rules see it.
//!
//! The file is text. After the line "-ninit 6" each line is one value: the text before " |" (or the whole line),
//! with blanks at its end removed. "|NNN" stands for the byte NNN (decimal) and "||" for "|". A line that holds only
//! a comment (it starts with "|" and no digit follows) gives an empty value. The values come in a fixed order,
//! read below; the last line is "***".

use super::chars::{latin1, lstrip, rstrip, strip};
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
    /// Emphasis fonts: per font its words (not built; English tables have none).
    pub fonts: Vec<Vec<String>>,
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

/// "|NNN" is the byte NNN (one to three digits), "||" is "|".
fn decode(token: &str) -> Vec<i64> {
    let c: Vec<char> = token.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '|' {
            if c.get(i + 1) == Some(&'|') {
                out.push('|' as i64);
                i += 2;
                continue;
            }
            let digits: String = c[i + 1..].iter().take(3).take_while(|d| d.is_ascii_digit()).collect();
            if !digits.is_empty() {
                out.push(digits.parse::<i64>().unwrap_or(0));
                i += 1 + digits.len();
                continue;
            }
        }
        out.push(c[i] as i64);
        i += 1;
    }
    out
}

fn tokens(text: &str) -> Result<Vec<Vec<i64>>, EngineError> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let start = lines
        .iter()
        .position(|l| strip(l) == "-ninit 6")
        .ok_or_else(|| EngineError::BadTable("not a scan table (no \"-ninit 6\" line)".into()))?;
    Ok(lines[start + 1..]
        .iter()
        .map(|line| {
            let mut tok = match line.find(" |") {
                Some(cut) => &line[..cut],
                None => line,
            };
            let mut after_bar = line.chars().skip(1);
            if line.starts_with('|') && !after_bar.next().is_some_and(|d| d.is_ascii_digit()) {
                tok = "";
            }
            decode(rstrip(tok))
        })
        .collect())
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
            fonts.push(text_of(raw()?).split_whitespace().map(String::from).collect());
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
        assert_eq!(decode("|028x||y|1z"), vec![28, 'x' as i64, '|' as i64, 'y' as i64, 1, 'z' as i64]);
        let t = tokens("head\n-ninit 6\n0 | binmod\n| a comment\n|028 | flag\n").unwrap();
        // The line break at the end leaves one empty value after the last line, as the reader sees it.
        assert_eq!(t, vec![vec!['0' as i64], vec![], vec![28], vec![]]);
    }
}
