//! Page ranges as the Print and Emboss dialogs take them (spec 24 section 2a, spec 25 section 2a).

/// What a page range asks for: the pages (indexes into the laid-out pages; None for a blank page, "b"), and what
/// was wrong with it.
#[derive(Debug, PartialEq, Eq)]
pub enum Range {
    Pages(Vec<Option<usize>>),
    /// Not written properly (message 2674).
    Malformed,
    /// No page number in it (message 2666).
    Empty,
    /// None of its pages are in the document (message 2672).
    NoneFound,
}

/// A page range as the Print dialog takes it, e.g. "1-7,b,9" (pages 1 through 7, a blank page, page 9): numbers,
/// ranges a-b, a range to the end a-, and b for a blank page, separated by commas. Page n is the n-th laid-out page.
/// Also returns whether pages asked for were missing (message 2664) and whether a page comes more than once (2668).
pub fn parse_range(text: &str, page_count: usize) -> (Range, bool, bool) {
    let mut out = Vec::new();
    let (mut missing, mut asked) = (false, false);
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if item.eq_ignore_ascii_case("b") {
            out.push(None);
            continue;
        }
        let (from, to) = match item.split_once('-') {
            Some((a, b)) => {
                let Ok(a) = a.trim().parse::<usize>() else { return (Range::Malformed, false, false) };
                let b = b.trim();
                let b = if b.is_empty() {
                    page_count.max(a)
                } else {
                    let Ok(b) = b.parse::<usize>() else { return (Range::Malformed, false, false) };
                    b
                };
                (a, b)
            }
            None => {
                let Ok(n) = item.parse::<usize>() else { return (Range::Malformed, false, false) };
                (n, n)
            }
        };
        if from == 0 || to < from {
            return (Range::Malformed, false, false);
        }
        asked = true;
        for n in from..=to {
            if n <= page_count {
                out.push(Some(n - 1));
            } else {
                missing = true;
            }
        }
    }
    if !asked && out.is_empty() {
        return (Range::Empty, false, false);
    }
    if !out.iter().any(Option::is_some) && asked {
        return (Range::NoneFound, false, false);
    }
    let mut seen = std::collections::HashSet::new();
    let repeated = out.iter().flatten().any(|p| !seen.insert(*p));
    (Range::Pages(out), missing, repeated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges() {
        assert_eq!(parse_range("1-3,b,5", 6), (Range::Pages(vec![Some(0), Some(1), Some(2), None, Some(4)]), false, false));
        assert_eq!(parse_range("4-", 6), (Range::Pages(vec![Some(3), Some(4), Some(5)]), false, false));
        assert_eq!(parse_range("5-8", 6), (Range::Pages(vec![Some(4), Some(5)]), true, false));
        assert_eq!(parse_range("2,2", 6), (Range::Pages(vec![Some(1), Some(1)]), false, true));
        assert_eq!(parse_range("9", 6).0, Range::NoneFound);
        assert_eq!(parse_range("x", 6).0, Range::Malformed);
        assert_eq!(parse_range("3-1", 6).0, Range::Malformed);
        assert_eq!(parse_range(" ", 6).0, Range::Empty);
    }
}
