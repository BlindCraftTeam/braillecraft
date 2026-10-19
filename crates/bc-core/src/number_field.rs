//! The whole-number rule of dialog number fields: an optional leading "-" and one or more ASCII digits, nothing
//! else (no spaces, no "+"). A field read into a buffer of fixed size is cut to that many characters first.

/// The number in `text`, or None when it is not a whole number by the rule above (empty, "-" alone, spaces, "+"
/// or any other character). A number too large for i64 is None.
pub fn whole_number(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// A field read as at most `width` characters, then by `whole_number` (Page Numbering's first page reads 5).
pub fn read_number(text: &str, width: usize) -> Option<i64> {
    let cut: String = text.chars().take(width).collect();
    whole_number(&cut)
}

/// The number of characters Document, Page Numbering reads of its first-page field.
pub const FIRST_PAGE_WIDTH: usize = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_number_rule() {
        assert_eq!(whole_number("5"), Some(5));
        assert_eq!(whole_number("-3"), Some(-3));
        for bad in ["", "-", " 5", "5 ", "+5", "5a"] {
            assert_eq!(whole_number(bad), None, "{bad:?}");
        }
        assert_eq!(read_number("123456", FIRST_PAGE_WIDTH), Some(12345));
        assert_eq!(read_number("12a", FIRST_PAGE_WIDTH), None);
    }
}
