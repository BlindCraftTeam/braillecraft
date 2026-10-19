//! Document, Page Numbering (dialog 1031): which corners each group may choose. The four groups are braille page
//! numbers on even pages, on odd pages, print page numbers on even pages, on odd pages; each corner is 0 None,
//! 1 upper left, 2 upper right, 3 lower left, 4 lower right.

/// The group paired with group `g`: even braille with even print, odd braille with odd print.
pub fn partner(g: usize) -> usize {
    (g + 2) % 4
}

/// The corners greyed in each group for the chosen corners `chosen` (in group order): the corner one group has
/// chosen is greyed in its partner group, so braille and print page numbers never share a corner of the same pages.
/// None is never greyed. The dialog redoes this on every radio click.
pub fn disabled_corners(chosen: [u32; 4]) -> [[bool; 5]; 4] {
    let mut out = [[false; 5]; 4];
    for (g, &v) in chosen.iter().enumerate() {
        if (1..5).contains(&v) {
            out[partner(g)][v as usize] = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_number_corner_excludes_partner() {
        let d = disabled_corners([2, 0, 0, 0]);
        assert!(d[2][2]);
        assert_eq!(d[2].iter().filter(|x| **x).count(), 1);
        assert!(d[0].iter().chain(&d[1]).chain(&d[3]).all(|x| !x));
        // None is never greyed.
        assert!(disabled_corners([1, 2, 3, 4]).iter().all(|g| !g[0]));
    }

    #[test]
    fn page_number_corner_both_directions() {
        let d = disabled_corners([0, 0, 4, 0]);
        assert!(d[0][4]);
        assert_eq!(d.iter().flatten().filter(|x| **x).count(), 1);
        let d = disabled_corners([1, 3, 2, 4]);
        assert!(d[2][1] && d[3][3] && d[0][2] && d[1][4]);
    }
}
