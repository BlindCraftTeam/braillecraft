//! The font lists of Global, Fonts: symbol, dingbat, braille and vertical (@) fonts are left out; the print font
//! list puts the typewriter fonts (a name holding "Courier New", or named "Courier" or "Consolas") before the others.
//! Every test of a name ignores case. Each group is a sorted list: a name goes in before the first name that sorts
//! after it, case ignored, and a name equal to one already in its group, case ignored, is not added again. Not
//! known: the order of names with letters outside ASCII (the comparison follows the C runtime's locale); here they
//! sort by their lower-case form.

/// Whether a font is listed at all (the test of its name, case ignored): out when it holds "RNIB Moon" or "dings",
/// or "Braille" unless it also holds "ASCII", or "Symbol", "Fences", "Drummer", "MegaDots" or "extra", or when it
/// starts with "@".
pub fn listed(name: &str) -> bool {
    let low = name.to_lowercase();
    if ["rnib moon", "dings"].iter().any(|w| low.contains(w)) {
        return false;
    }
    if low.contains("braille") && !low.contains("ascii") {
        return false;
    }
    if ["symbol", "fences", "drummer", "megadots", "extra"].iter().any(|w| low.contains(w)) {
        return false;
    }
    !name.starts_with('@')
}

/// A typewriter font, listed first for the print font: a name holding "Courier New", or the whole name
/// "Courier" or "Consolas", case ignored.
pub fn typewriter(name: &str) -> bool {
    let low = name.to_lowercase();
    low.contains("courier new") || low == "courier" || low == "consolas"
}

/// The list of `names` (the system's fonts) for the print font (`print`) or the status line font.
pub fn font_list(names: &[String], print: bool) -> Vec<String> {
    let mut first: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    for n in names.iter().filter(|n| listed(n)) {
        let group = if print && typewriter(n) { &mut first } else { &mut rest };
        let key = n.to_lowercase();
        if group.iter().any(|g| g.to_lowercase() == key) {
            continue;
        }
        let at = group.iter().position(|g| g.to_lowercase() > key).unwrap_or(group.len());
        group.insert(at, n.clone());
    }
    first.extend(rest);
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_as_specified() {
        let names: Vec<String> = ["Verdana", "Wingdings", "@MS Gothic", "Courier New", "Consolas", "Arial", "Braille29", "SimBraille ASCII", "Symbol"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(font_list(&names, true), ["Consolas", "Courier New", "Arial", "SimBraille ASCII", "Verdana"]);
        assert_eq!(font_list(&names, false), ["Arial", "Consolas", "Courier New", "SimBraille ASCII", "Verdana"]);
    }

    #[test]
    fn the_tests_ignore_case() {
        assert!(!listed("Arial Nova ExtraBold"));
        assert!(!listed("Segoe UI Extralight"));
        assert!(!listed("SYMBOL Pro"));
        assert!(!listed("Webdings"));
        assert!(!listed("my braille"));
        assert!(listed("SimBraille ascii"));
        assert!(!listed("@Arial"));
    }

    #[test]
    fn typewriter_is_courier_new_or_exact_names() {
        assert!(!typewriter("Courier Prime"));
        assert!(!typewriter("Consolas Nerd Font"));
        assert!(typewriter("COURIER"));
        assert!(typewriter("consolas"));
        assert!(typewriter("Courier New Baltic"));
        let names: Vec<String> = ["Courier Prime", "Arial", "COURIER"].iter().map(|s| s.to_string()).collect();
        assert_eq!(font_list(&names, true), ["COURIER", "Arial", "Courier Prime"]);
    }

    #[test]
    fn a_name_differing_only_in_case_is_listed_once() {
        let names: Vec<String> = ["Arial", "ARIAL", "arial", "Verdana"].iter().map(|s| s.to_string()).collect();
        assert_eq!(font_list(&names, false), ["Arial", "Verdana"]);
    }
}
