//! Page ranges as the Print and Emboss dialogs take them (spec 24 section 2a, spec 25 section 2a), read as the
//! original reads them.

/// What a page range asks for: the pages (indexes into the laid-out pages; None for a blank page, "b"), and what
/// was wrong with it.
#[derive(Debug, PartialEq, Eq)]
pub enum Range {
    Pages(Vec<Option<usize>>),
    /// Not written properly (message 2674).
    Malformed,
    /// Nothing typed (message 2666).
    Empty,
    /// An item matches no page of the document (message 2672).
    NoneFound,
}

/// A page's number as the page shows it: the number, its prefix letter, and whether it is a roman numeral.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageNumber {
    pub number: u32,
    pub letter: Option<char>,
    pub roman: bool,
}

/// An end of a range left open ("-5", "5-").
const OPEN: i64 = i64::MAX;

#[derive(Clone, Copy, Debug)]
struct Num {
    n: i64,
    roman: bool,
    letter: Option<char>,
}

const ROMAN: [(char, i64); 7] = [('I', 1), ('V', 5), ('X', 10), ('L', 50), ('C', 100), ('D', 500), ('M', 1000)];

fn roman_digit(c: char) -> Option<i64> {
    ROMAN.iter().find(|(r, _)| *r == c.to_ascii_uppercase()).map(|(_, v)| *v)
}

/// A roman numeral's value, read right to left: a letter below the largest met so far is taken off.
fn roman_value(s: &[char]) -> i64 {
    let (mut total, mut most) = (0i64, 0i64);
    for v in s.iter().rev().filter_map(|&c| roman_digit(c)) {
        if v < most {
            total -= v;
        } else {
            most = v;
            total = total.saturating_add(v);
        }
    }
    total
}

/// One number of a range at `i`: with `any`, a letter then digits (p5); digits; with `any`, a roman numeral.
fn number(s: &[char], i: &mut usize, any: bool) -> Option<Num> {
    if any && s.get(*i).is_some_and(|c| c.is_ascii_alphabetic()) {
        let (letter, back) = (s[*i], *i);
        *i += 1;
        if let Some(n) = number(s, i, false) {
            return Some(Num { letter: Some(letter), ..n });
        }
        *i = back;
    }
    if s.get(*i).is_some_and(|c| c.is_ascii_digit()) {
        let mut n = 0i64;
        while let Some(d) = s.get(*i).and_then(|c| c.to_digit(10)) {
            n = n.saturating_mul(10).saturating_add(i64::from(d));
            *i += 1;
        }
        return Some(Num { n, roman: false, letter: None });
    }
    if any {
        let len = s[*i..].iter().take_while(|&&c| roman_digit(c).is_some()).count();
        if len > 0 {
            let n = roman_value(&s[*i..*i + len]);
            *i += len;
            return Some(Num { n, roman: true, letter: None });
        }
    }
    None
}

enum Item {
    Blank,
    Span(Option<Num>, Option<Num>),
}

/// The next item from `i` (commas before it skipped): Ok(None) at the end, Err when it is not written properly. No
/// spaces are skipped.
fn item(s: &[char], i: &mut usize) -> Result<Option<Item>, ()> {
    while s.get(*i) == Some(&',') {
        *i += 1;
    }
    if *i >= s.len() {
        return Ok(None);
    }
    let item = if s[*i] == '-' {
        *i += 1;
        Item::Span(None, number(s, i, true))
    } else {
        match number(s, i, true) {
            None if matches!(s[*i], 'b' | 'B') => {
                *i += 1;
                Item::Blank
            }
            None => return Err(()),
            Some(start) if s.get(*i) == Some(&'-') => {
                *i += 1;
                Item::Span(Some(start), number(s, i, true))
            }
            Some(start) => Item::Span(Some(start), Some(start)),
        }
    };
    match s.get(*i) {
        None | Some(',') => Ok(Some(item)),
        _ => Err(()),
    }
}

/// How far a typed number is from a page's number, as the original weighs it: only a page with the same letter and
/// the same kind of number counts; each page of difference counts two, less one on the side `up` prefers (above the
/// typed number when `up`, below it otherwise).
fn distance(typed: &Num, page: &PageNumber, up: bool) -> Option<i64> {
    if page.letter.filter(|&c| c != ' ') != typed.letter || page.roman != typed.roman {
        return None;
    }
    let page_n = i64::from(page.number);
    Some(if typed.n < page_n {
        (page_n - typed.n) * 2 - i64::from(up)
    } else if page_n < typed.n {
        (typed.n - page_n) * 2 - i64::from(!up)
    } else {
        0
    })
}

/// The page (counted from 1) a typed number means: the nearest page number, the first of equals; an open end is the
/// last page. Err when no page has its letter and kind of number; `adjusted` when the nearest is not the same.
fn matched(typed: Option<Num>, table: &[PageNumber], up: bool, adjusted: &mut bool) -> Result<i64, ()> {
    let Some(typed) = typed else { return Ok(table.len() as i64) };
    let mut best: Option<(i64, usize)> = None;
    for (k, page) in table.iter().enumerate() {
        let Some(d) = distance(&typed, page, up) else { continue };
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, k));
        }
        if d == 0 {
            break;
        }
    }
    let (d, k) = best.ok_or(())?;
    if d != 0 {
        *adjusted = true;
    }
    Ok(k as i64 + 1)
}

/// A page range as the Print and Emboss dialogs take it, e.g. "1-7,b,9" (pages 1 through 7, a blank page, page 9),
/// read as the original reads it: items separated by commas, each a page, a range from-to (to below from runs
/// backward), a range to the end "5-", from the last page back "-5", the whole document backward "-", or b for a
/// blank page; a page is a number, a letter then a number (p5) or a roman numeral. With `numbers` (the pages' own
/// numbers, Print or Braille page numbers) each typed number is the nearest page number with the same letter and
/// kind, of the two roundings the one giving the longer range; without, page n is the n-th page. A range reaching
/// past the document is cut to it. Also returns whether a range had to be adjusted (question 2664) and, when not,
/// whether a page comes more than once (question 2668).
pub fn parse_range(text: &str, page_count: usize, numbers: Option<&[PageNumber]>) -> (Range, bool, bool) {
    if text.is_empty() {
        return (Range::Empty, false, false);
    }
    let s: Vec<char> = text.chars().collect();
    let count = numbers.map_or(page_count, <[PageNumber]>::len) as i64;
    let mut i = 0;
    let mut out = Vec::new();
    let (mut adjusted, mut repeated) = (false, false);
    let mut used = vec![false; count.max(0) as usize];
    loop {
        let (start, end) = match item(&s, &mut i) {
            Err(()) => return (Range::Malformed, false, false),
            Ok(None) => break,
            Ok(Some(Item::Blank)) => {
                out.push(None);
                continue;
            }
            Ok(Some(Item::Span(start, end))) => (start, end),
        };
        let (mut a, mut b) = match (start, end, numbers) {
            (None, None, _) => (count, 1),
            (_, _, Some(table)) => {
                let mut wide = || -> Result<(i64, i64), ()> {
                    Ok((matched(start, table, false, &mut adjusted)?, matched(end, table, true, &mut adjusted)?))
                };
                let Ok(w) = wide() else { return (Range::NoneFound, false, false) };
                let Ok(n) = matched(start, table, true, &mut adjusted).and_then(|a| Ok((a, matched(end, table, false, &mut adjusted)?)))
                else {
                    return (Range::NoneFound, false, false);
                };
                if (w.1 - w.0).abs() < (n.1 - n.0).abs() {
                    n
                } else {
                    w
                }
            }
            _ => (start.map_or(OPEN, |n| n.n), end.map_or(OPEN, |n| n.n)),
        };
        if (count < a && count < b) || (a < 1 && b < 1) {
            return (Range::NoneFound, false, false);
        }
        for x in [&mut a, &mut b] {
            if *x < 1 {
                adjusted = true;
                *x = 1;
            }
            if *x > count {
                if *x != OPEN {
                    adjusted = true;
                }
                *x = count;
            }
        }
        let step = if b < a { -1 } else { 1 };
        let mut p = a;
        loop {
            let k = (p - 1) as usize;
            if !adjusted && !repeated && used[k] {
                repeated = true;
            }
            used[k] = true;
            out.push(Some(k));
            if p == b {
                break;
            }
            p += step;
        }
    }
    (Range::Pages(out), adjusted, repeated && !adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges_read_as_the_original() {
        assert_eq!(parse_range("1-3,b,5", 6, None), (Range::Pages(vec![Some(0), Some(1), Some(2), None, Some(4)]), false, false));
        assert_eq!(parse_range("4-", 6, None), (Range::Pages(vec![Some(3), Some(4), Some(5)]), false, false));
        // Backward ranges, from the last page back, and the whole document backward.
        assert_eq!(parse_range("3-1", 6, None).0, Range::Pages(vec![Some(2), Some(1), Some(0)]));
        assert_eq!(parse_range("-5", 6, None).0, Range::Pages(vec![Some(5), Some(4)]));
        assert_eq!(parse_range("-", 3, None).0, Range::Pages(vec![Some(2), Some(1), Some(0)]));
        // Cut to the document, with the adjusted question; repeats only asked when nothing was adjusted.
        assert_eq!(parse_range("5-8", 6, None), (Range::Pages(vec![Some(4), Some(5)]), true, false));
        assert_eq!(parse_range("2,2", 6, None), (Range::Pages(vec![Some(1), Some(1)]), false, true));
        assert_eq!(parse_range("2,2,5-9", 6, None).1, true);
        assert_eq!(parse_range("2,2,5-9", 6, None).2, false);
        assert_eq!(parse_range("9", 6, None).0, Range::NoneFound);
        // A lone x is the roman numeral 10, so not found; z is no number at all.
        assert_eq!(parse_range("x", 6, None).0, Range::NoneFound);
        assert_eq!(parse_range("z", 6, None).0, Range::Malformed);
        // No spaces are skipped.
        assert_eq!(parse_range(" 1", 6, None).0, Range::Malformed);
        assert_eq!(parse_range("", 6, None).0, Range::Empty);
        assert_eq!(parse_range(",", 6, None).0, Range::Pages(vec![]));
    }

    #[test]
    fn page_numbers_match_the_nearest_page_number() {
        let n = |number, letter| PageNumber { number, letter, roman: false };
        // Two p pages, then pages 1 to 4 numbered 1, 2, 5, 6 (a page number set on the third).
        let table = [n(1, Some('p')), n(2, Some('p')), n(1, None), n(2, None), n(5, None), n(6, None)];
        assert_eq!(parse_range("p2", 0, Some(&table)), (Range::Pages(vec![Some(1)]), false, false));
        assert_eq!(parse_range("1-2", 0, Some(&table)).0, Range::Pages(vec![Some(2), Some(3)]));
        // 3 and 4 are no page's number: of the two roundings, the longer range, and the adjusted question.
        assert_eq!(parse_range("3-4", 0, Some(&table)), (Range::Pages(vec![Some(3), Some(4)]), true, false));
        // No page has the letter q.
        assert_eq!(parse_range("q1", 0, Some(&table)).0, Range::NoneFound);
        // Roman numerals match only roman page numbers.
        let roman = [PageNumber { number: 4, letter: None, roman: true }];
        assert_eq!(parse_range("iv", 0, Some(&roman)).0, Range::Pages(vec![Some(0)]));
        assert_eq!(parse_range("4", 0, Some(&roman)).0, Range::NoneFound);
    }
}
