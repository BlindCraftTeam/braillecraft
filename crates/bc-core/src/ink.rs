//! The ink pass of an embossing job with interline print (re/spec/25-emboss.md sections 14.3, 14.5, 17 and 17.7):
//! before the braille pass, the print of each braille line goes to the setup's ink printer, each run of cells at its
//! own column, so a TranSend SE system puts the print and the braille on the same sheet. Shared by the desktop
//! program; the drawing itself is the program's.

/// The pass choices of File Emboss for a TranSend SE embosser (dialog 1038 controls 17, 18 and 19, spec 25 17.1),
/// as the job flags the reference gives each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassChoice {
    InkAndBraille,
    InkOnly,
    BrailleOnly,
}

impl PassChoice {
    /// The job flags: bit 0 the ink pass, bit 2 the braille pass, bit 3 a manual second-side pass (0xff, 3, 0x1c).
    pub fn flags(self) -> u32 {
        match self {
            PassChoice::InkAndBraille => 0xff,
            PassChoice::InkOnly => 3,
            PassChoice::BrailleOnly => 0x1c,
        }
    }
}

/// The job flags File Emboss gives a job without the TranSend SE choices (dialog 1039): every pass.
pub const ALL_PASSES: u32 = 0xff;

/// Which passes a job makes (spec 25 14.5 and 17.1): the ink pass when its flag is set, the setup names an ink
/// printer and interline print is on; the braille pass when its flag is set.
pub fn passes(flags: u32, ink_printer: &str, interline: bool) -> (bool, bool) {
    (flags & 1 != 0 && !ink_printer.is_empty() && interline, flags & 4 != 0)
}

/// The ink font and line spacing for TranSend SE font size `size` (0 Small, 1 Large Interline, 2 Large Overlapped;
/// spec 25 14.3): the font's cell height in twips (twice the font size, 120 for Small, 240 for both Large, unless
/// `font_size` gives another) and the step from one braille line to the next in twips (576 for Small and Large
/// Overlapped; for Large Interline 850 with the Index driver, else 706; unless `vshift` gives another). The two
/// overrides are the reference's ink_font_size and ink_vshift settings.
pub fn ink_font(size: usize, index_driver: bool, font_size: Option<i32>, vshift: Option<i32>) -> (i32, i32) {
    let font = font_size.unwrap_or(if size == 0 { 120 } else { 240 });
    let shift = vshift.unwrap_or(match size {
        1 if index_driver => 850,
        1 => 706,
        _ => 576,
    });
    (font * 2, shift)
}

/// The name of TranSend SE font size `size` as the reference's settings name it (spec 25 14.3).
pub fn ink_font_key(size: usize) -> &'static str {
    match size {
        1 => "LargeInterline",
        2 => "LargeOverlapped",
        _ => "Small",
    }
}

/// Where the ink of a line goes on the page, in twips from the printable area's top left (spec 25 14.5 and 17.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// The page's binding margin (cells) and top margin (lines), and the setup's left and top margin settings
    /// (LeftProg and TopProg), which the column and line are counted from.
    pub binding: i32,
    pub left_prog: i32,
    pub top_margin: i32,
    pub top_prog: i32,
    /// The cells per line, and the form's most cells and paper width (tenths of an inch).
    pub cells: i32,
    pub form_cells: i32,
    pub form_width: i32,
    /// The setup's ink adjustments, in hundredths of an inch.
    pub adjust_h: i32,
    pub adjust_v: i32,
    /// The step from one line to the next (ink_font).
    pub vshift: i32,
}

impl Placement {
    /// The column a run starting at cell `cell` of a line is drawn at: the binding margin less the left margin
    /// setting, or on an interpoint back page (`back`) the same from the right (the form's most cells less the
    /// cells per line and the binding margin, plus the left margin setting); never before 0; then the run's cell.
    pub fn column(&self, cell: i32, back: bool) -> i32 {
        let start = if back { self.form_cells - self.cells - self.binding + self.left_prog } else { self.binding - self.left_prog };
        start.max(0) + cell
    }

    /// The x of column `column`: 346 twips a cell, from half an inch plus the horizontal adjustment.
    pub fn x(&self, column: i32) -> i32 {
        column * 346 + ((self.adjust_h + 50) * 1440 + 50) / 100
    }

    /// The width of every piece of ink's box: the form's paper width at 144 twips a tenth of an inch.
    pub fn width(&self) -> i32 {
        self.form_width * 144
    }

    fn step(&self, n: i32) -> i32 {
        n * self.vshift + (self.adjust_v * 1440 + 50) / 100
    }

    /// The top and bottom of the ink of line `line` of a page (from 0): the text is drawn against the bottom.
    pub fn rows(&self, line: i32) -> (i32, i32) {
        let top = self.step(line + self.top_margin - self.top_prog);
        (top, top + self.step(1))
    }
}

/// One run of cells of a braille line: a word, as the reference walks a line for its ink (spec 25 17.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// The cell it starts at, from 0.
    pub start: usize,
    /// Its cells as North American ASCII braille.
    pub cells: String,
}

impl Run {
    /// A fill row: two or more of the same cell, dots 5 (") or dots 4-6 (.), which the ink shows as a row of
    /// full stops of the same length (spec 25 17.4).
    pub fn fill(&self) -> Option<String> {
        let mut chars = self.cells.chars();
        let first = chars.next()?;
        let n = self.cells.chars().count();
        (n >= 2 && (first == '"' || first == '.') && chars.all(|c| c == first)).then(|| ".".repeat(n))
    }
}

/// The runs of a braille line given as North American ASCII braille: each stretch of cells between blank cells.
pub fn runs(line: &str) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    let mut current: Option<Run> = None;
    for (i, c) in line.chars().enumerate() {
        if c == ' ' {
            out.extend(current.take());
        } else {
            current.get_or_insert_with(|| Run { start: i, cells: String::new() }).cells.push(c);
        }
    }
    out.extend(current);
    out
}

/// One piece of ink on a page: its text drawn against the bottom of the box from (x, top) to (x + width, bottom),
/// in twips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub x: i32,
    pub top: i32,
    pub bottom: i32,
    pub width: i32,
    pub text: String,
}

/// The ink of one braille page (`back` for an interpoint back page) whose lines (from 0) are given as North American
/// ASCII braille: each line's runs, each given its print by `print` (the back-translation of the run's cells,
/// without a trailing space; it is given the line number and the run) unless it is a fill row; each run's text ends
/// in a space, as the reference sends it (spec 25 17.4).
pub fn page_items(lines: &[String], place: &Placement, back: bool, mut print: impl FnMut(usize, &Run) -> String) -> Vec<Item> {
    let mut items = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        for run in runs(line) {
            let text = match run.fill() {
                Some(dots) => dots,
                None => format!("{} ", print(n, &run).trim_end()),
            };
            if text.trim().is_empty() {
                continue;
            }
            let (top, bottom) = place.rows(n as i32);
            items.push(Item { x: place.x(place.column(run.start as i32, back)), top, bottom, width: place.width(), text });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_choices_give_the_references_passes() {
        assert_eq!(PassChoice::InkAndBraille.flags(), 0xff);
        assert_eq!(passes(PassChoice::InkAndBraille.flags(), "Epson LQ-590", true), (true, true));
        assert_eq!(passes(PassChoice::InkOnly.flags(), "Epson LQ-590", true), (true, false));
        assert_eq!(passes(PassChoice::BrailleOnly.flags(), "Epson LQ-590", true), (false, true));
        // No ink printer, or interline print off: no ink pass.
        assert_eq!(passes(ALL_PASSES, "", true), (false, true));
        assert_eq!(passes(ALL_PASSES, "Epson LQ-590", false), (false, true));
    }

    #[test]
    fn the_ink_font_and_line_step_follow_the_font_size() {
        assert_eq!(ink_font(0, false, None, None), (240, 576));
        assert_eq!(ink_font(1, false, None, None), (480, 706));
        assert_eq!(ink_font(1, true, None, None), (480, 850));
        assert_eq!(ink_font(2, false, None, None), (480, 576));
        assert_eq!(ink_font(0, false, Some(100), Some(600)), (200, 600));
        assert_eq!(ink_font_key(1), "LargeInterline");
    }

    #[test]
    fn places_follow_the_references_formulas() {
        let p = Placement { binding: 2, left_prog: 1, top_margin: 1, top_prog: 1, cells: 40, form_cells: 42, form_width: 115, vshift: 576, ..Placement::default() };
        // Column 0 is half an inch from the left; each cell 346 twips on.
        assert_eq!(p.column(0, false), 1);
        assert_eq!(p.column(3, false), 4);
        assert_eq!(p.x(0), 720);
        assert_eq!(p.x(3), 3 * 346 + 720);
        // A back page counts from the right: 42 - 40 - 2 + 1.
        assert_eq!(p.column(3, true), 1 + 3);
        // The box is the paper's width.
        assert_eq!(p.width(), 115 * 144);
        assert_eq!(p.rows(0), (0, 576));
        assert_eq!(p.rows(2), (1152, 1728));
        // Adjustments in hundredths of an inch; the start never goes before 0, the run's cell is added after.
        let q = Placement { left_prog: 5, adjust_h: 10, adjust_v: -5, vshift: 576, ..Placement::default() };
        assert_eq!(q.column(2, false), 2);
        assert_eq!(q.x(0), (60 * 1440 + 50) / 100);
        assert_eq!(q.rows(1).0, 576 + (-5 * 1440 + 50) / 100);
    }

    #[test]
    fn a_line_splits_into_runs_and_fill_rows_become_full_stops() {
        let r = runs("  ,! cat \"\"\"\" ..");
        assert_eq!(r.iter().map(|r| (r.start, r.cells.as_str())).collect::<Vec<_>>(), vec![(2, ",!"), (5, "cat"), (9, "\"\"\"\""), (14, "..")]);
        assert_eq!(r[2].fill().as_deref(), Some("...."));
        assert_eq!(r[3].fill().as_deref(), Some(".."));
        assert_eq!(r[1].fill(), None);
        assert_eq!(Run { start: 0, cells: "\"".into() }.fill(), None);
        let p = Placement { vshift: 576, form_width: 85, ..Placement::default() };
        let items = page_items(&["  ,! cat".to_string(), String::new(), "\"\"\"\"".to_string()], &p, false, |_, r| if r.cells == ",!" { "The".into() } else { r.cells.to_uppercase() });
        assert_eq!(items.iter().map(|i| (i.x, i.top, i.text.as_str())).collect::<Vec<_>>(), vec![(720 + 2 * 346, 0, "The "), (720 + 5 * 346, 0, "CAT "), (720, 1152, "....")]);
        assert_eq!(items[0].width, 85 * 144);
    }
}
