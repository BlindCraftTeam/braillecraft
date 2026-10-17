//! The font lists of Global, Fonts, as the original fills them from the system's fonts: symbol, dingbat, braille and
//! vertical (@) fonts are left out; the print font list puts the typewriter fonts (Courier New, any name with
//! "courier" or "consolas") before the others. Each group is in alphabetical order (INFERENCE: the list the original
//! adds to is taken to be a sorted one).

/// Whether the original lists a font at all (its test of the name).
pub fn listed(name: &str) -> bool {
    if name.starts_with('@') {
        return false;
    }
    if ["RNIB Moon", "dings", "Symbol", "Fences", "Drummer", "MegaDots", "extra"].iter().any(|w| name.contains(w)) {
        return false;
    }
    // A braille font is left out, unless it is an ASCII one.
    !name.contains("Braille") || name.contains("ASCII")
}

/// A typewriter font, listed first for the print font.
pub fn typewriter(name: &str) -> bool {
    let low = name.to_lowercase();
    name.contains("Courier New") || low.contains("courier") || low.contains("consolas")
}

/// The list of `names` (the system's fonts) for the print font (`print`) or the status line font.
pub fn font_list(names: &[String], print: bool) -> Vec<String> {
    let mut first: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    for n in names.iter().filter(|n| listed(n)) {
        let group = if print && typewriter(n) { &mut first } else { &mut rest };
        if !group.contains(n) {
            group.push(n.clone());
        }
    }
    for g in [&mut first, &mut rest] {
        g.sort_by_key(|n| n.to_lowercase());
    }
    first.extend(rest);
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_as_the_original() {
        let names: Vec<String> = ["Verdana", "Wingdings", "@MS Gothic", "Courier New", "Consolas", "Arial", "Braille29", "SimBraille ASCII", "Symbol"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(font_list(&names, true), ["Consolas", "Courier New", "Arial", "SimBraille ASCII", "Verdana"]);
        assert_eq!(font_list(&names, false), ["Arial", "Consolas", "Courier New", "SimBraille ASCII", "Verdana"]);
    }
}
