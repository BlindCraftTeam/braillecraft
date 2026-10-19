//! File, Print's page fonts and page geometry (spec 24, A16.1, A16.4 and A16.5), shared by the desktop and web
//! builds.
//!
//! Every page line of either kind of document prints in the document's view font, in its printing face: the Print
//! font (Global, Fonts), "Braille" or "SimBraille", or "RNIB Moon" for a braille view font when the document's
//! print-to-braille table name starts with "ENMOCP" and that font is installed. Bold, italic and underline apply
//! only to the Print font. With interline print, each line's print is drawn above its braille, in 9-point Times New
//! Roman, inside the extra space that font adds to the line.

/// The face and point size of the interline print.
pub const INTERLINE_FACE: &str = "Times New Roman";
pub const INTERLINE_POINTS: i32 = 9;
/// The face a braille view font prints and shows in for a Moon table.
pub const MOON_FACE: &str = "RNIB Moon";

/// The font a page prints in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFont {
    /// The Print font of Global, Fonts.
    Print,
    Braille,
    SimBraille,
    Moon,
}

/// Whether a braille view font becomes the Moon font: the document's print-to-braille table name starts with
/// "ENMOCP" (case ignored) and a font named "RNIB Moon" is installed. The same switch applies on the screen.
pub fn uses_moon(forward_table: &str, moon_installed: bool) -> bool {
    moon_installed && forward_table.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("ENMOCP"))
}

impl PageFont {
    /// The page font of a view font (0 print, 1 braille, 2 SimBraille) for a document with this print-to-braille
    /// table, `moon_installed` saying whether the Moon font is there.
    pub fn of(view: u8, forward_table: &str, moon_installed: bool) -> PageFont {
        match view {
            0 => PageFont::Print,
            _ if uses_moon(forward_table, moon_installed) => PageFont::Moon,
            1 => PageFont::Braille,
            _ => PageFont::SimBraille,
        }
    }

    /// The face the page prints in; None for the Print font, whose face is the Global, Fonts print face.
    pub fn printing_face(self) -> Option<&'static str> {
        match self {
            PageFont::Print => None,
            PageFont::Braille => Some("Braille"),
            PageFont::SimBraille => Some("SimBraille"),
            PageFont::Moon => Some(MOON_FACE),
        }
    }

    /// A run's font attribute (1 bold, 2 italic, 4 underline) as printed: kept for the Print font only.
    pub fn run_attribute(self, attr: u8) -> u8 {
        if self == PageFont::Print {
            attr
        } else {
            0
        }
    }

    /// Whether the line step is cut to nine tenths: the Braille and SimBraille printing faces.
    pub fn tight_lines(self) -> bool {
        matches!(self, PageFont::Braille | PageFont::SimBraille)
    }
}

/// A font's metrics on the printer: ascent, descent and external leading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metrics {
    pub ascent: i32,
    pub descent: i32,
    pub leading: i32,
}

impl Metrics {
    /// Ascent, descent and external leading together.
    pub fn height(self) -> i32 {
        self.ascent + self.descent + self.leading
    }
}

/// The interline extra space: the height of the 9-point interline font with interline print on, else 0.
pub fn interline_extra(interline: Option<Metrics>) -> i32 {
    interline.map_or(0, Metrics::height)
}

/// The distance from one line to the next: the page font's height, the extra space and a tenth of it; nine tenths
/// of that (at least 1) for the Braille and SimBraille printing faces.
pub fn line_step(page: Metrics, extra: i32, font: PageFont) -> i32 {
    let step = page.height() + extra + extra / 10;
    if font.tight_lines() {
        (step * 9 / 10).max(1)
    } else {
        step
    }
}

/// The baseline of line `n` (from 0) of a page whose text starts at `top`.
pub fn baseline(top: i32, page: Metrics, extra: i32, step: i32, n: i32) -> i32 {
    top + page.ascent + extra + extra / 10 + n * step
}

/// The baseline of the interline print of a line whose braille stands on `baseline`: the extra space below the
/// top of the line, above the braille.
pub fn interline_baseline(baseline: i32, page: Metrics, extra: i32) -> i32 {
    baseline - (page.ascent + extra / 10)
}

/// Whether File, Print's Interline Print box starts checked: the document's embosser layout has interline (byte
/// 10) or DoubleSizePrint (byte 11) set.
pub fn interline_starts(interline: u8, double_size: u8) -> bool {
    interline != 0 || double_size != 0
}

/// The point size Auto-determine finds: sizes 1, 2, ... up to 48 are tried while a line of `chars` characters and
/// a page of `lines` lines fit, and the last that fits is taken (at least 1). `measure` gives the width of "a" and
/// the line step at a size, None when the font cannot be made (then 6). `numbers` is the width of the line-number
/// columns, measured once in the window font, the same at every size.
pub fn auto_size(measure: impl Fn(i32) -> Option<(i32, i32)>, width: i32, height: i32, numbers: i32, chars: i32, lines: i32) -> i32 {
    let mut best = 1;
    for size in 1..=48 {
        let Some((a, step)) = measure(size) else { return 6 };
        let cols = (width - numbers) / a.max(1);
        let rows = height / step.max(1);
        if cols >= chars && rows >= lines {
            best = size;
        } else {
            break;
        }
    }
    best
}

/// What a printer reports about its page, in printer pixels: the paper's size, the printable area's offset on it
/// and size, and the resolution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Device {
    pub paper: (i32, i32),
    pub offset: (i32, i32),
    pub printable: (i32, i32),
    pub ppi: (i32, i32),
}

/// The margin rectangle (left, top, right, bottom) in printable-area pixels, as the printer object works it out
/// from its margins in thousandths of an inch (spec A16.4 unknown 3). `margins` is None for a setup that never
/// stored margins: then each is one inch, the top worked out with the paper's width over the page's height, and
/// the right and bottom one inch in from the far edge less the left and top. The left and top lose the printable
/// area's offset (not below 0); the right and bottom gain the unprintable strip past the printable area (not past
/// the paper), so the area Auto-determine fills is wider and taller than the margins say. The page text starts at
/// the left and top.
pub fn margin_area(d: &Device, margins: Option<[i32; 4]>) -> [i32; 4] {
    let (w, h) = d.paper;
    let inch = |t: i32, ppi: i32| (i64::from(t) * i64::from(ppi) / 1000) as i32;
    let (left, top, right, bottom) = match margins {
        Some(m) => (inch(m[0], d.ppi.0), inch(m[1], d.ppi.1), w - inch(m[2], d.ppi.0), h - inch(m[3], d.ppi.1)),
        None => {
            let left = d.ppi.0;
            let top = (i64::from(d.ppi.1) * i64::from(w) / i64::from(h.max(1))) as i32;
            (left, top, (w - d.ppi.0) - left, (h - d.ppi.1) - top)
        }
    };
    let (ox, oy) = d.offset;
    [
        (left - ox).max(0),
        (top - oy).max(0),
        (right + (w - ox - d.printable.0)).min(w),
        (bottom + (h - oy - d.printable.1)).min(h),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_page_font_attributes_only_for_the_print_font() {
        assert_eq!(PageFont::Print.run_attribute(5), 5);
        assert_eq!(PageFont::Braille.run_attribute(5), 0);
        assert_eq!(PageFont::of(1, "amerc", true).printing_face(), Some("Braille"));
        assert_eq!(PageFont::of(2, "amerc", false).printing_face(), Some("SimBraille"));
        assert_eq!(PageFont::of(0, "ENMOCP", true).printing_face(), None);
    }

    #[test]
    fn print_moon_face_for_a_moon_table_with_the_font_installed() {
        assert_eq!(PageFont::of(1, "ENMOCP", true), PageFont::Moon);
        assert_eq!(PageFont::of(2, "enmocpx", true), PageFont::Moon);
        assert_eq!(PageFont::of(1, "XENMOCP", true), PageFont::Braille);
        assert_eq!(PageFont::of(1, "ENMOCP", false), PageFont::Braille);
        assert_eq!(PageFont::Moon.printing_face(), Some("RNIB Moon"));
    }

    #[test]
    fn print_line_step_and_baselines() {
        let m = Metrics { ascent: 10, descent: 3, leading: 2 };
        assert_eq!(line_step(m, 0, PageFont::Print), 15);
        assert_eq!(line_step(m, 0, PageFont::Braille), 13);
        assert_eq!(line_step(m, 0, PageFont::Moon), 15);
        assert_eq!(baseline(100, m, 14, 30, 0), 100 + 10 + 14 + 1);
        assert_eq!(interline_baseline(baseline(100, m, 14, 30, 0), m, 14), 100 + 14);
        assert_eq!(baseline(100, m, 14, 30, 2), 100 + 25 + 60);
        assert_eq!(interline_extra(None), 0);
    }

    #[test]
    fn print_interline_starts_from_interline_or_double_size() {
        assert!(interline_starts(0, 1));
        assert!(interline_starts(1, 0));
        assert!(!interline_starts(0, 0));
    }

    #[test]
    fn print_auto_size_keeps_the_last_size_that_fits() {
        // "a" is the size wide and a line is twice the size: 40 characters and 25 lines in 400 by 1000 pixels with
        // 40 pixels of number columns fit up to size 9.
        assert_eq!(auto_size(|s| Some((s, 2 * s)), 400, 1000, 40, 40, 25), 9);
        assert_eq!(auto_size(|_| None, 400, 1000, 0, 40, 25), 6);
        assert_eq!(auto_size(|s| Some((s * 100, s)), 10, 10, 0, 40, 25), 1);
    }

    #[test]
    fn print_margin_area_as_the_printer_object_gives_it() {
        // Letter at 100 pixels per inch, the printable area 25 pixels in on each side.
        let d = Device { paper: (850, 1100), offset: (25, 25), printable: (800, 1050), ppi: (100, 100) };
        assert_eq!(margin_area(&d, Some([1000; 4])), [75, 75, 775, 1025]);
        // No margins stored: one inch, the top with the paper's width over its height, the far sides less the near.
        assert_eq!(margin_area(&d, None), [75, 52, 675, 948]);
    }
}
