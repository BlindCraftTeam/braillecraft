//! What Printer Setup keeps (dialog 1071, for a document or for the program), shared by the desktop and web builds:
//! the papers the dialog lists, the setup with its settings text, and how margins are shown and read.

/// A paper the Printer Setup dialog lists: its name and its size in tenths of a millimetre.
pub const PAPERS: &[(&str, i32, i32)] = &[
    ("Letter (8.5 x 11 inches)", 2159, 2794),
    ("Legal (8.5 x 14 inches)", 2159, 3556),
    ("Tabloid (11 x 17 inches)", 2794, 4318),
    ("Executive (7.25 x 10.5 inches)", 1841, 2667),
    ("A3 (297 x 420 mm)", 2970, 4200),
    ("A4 (210 x 297 mm)", 2100, 2970),
    ("A5 (148 x 210 mm)", 1480, 2100),
    ("B5 (182 x 257 mm)", 1820, 2570),
];

/// What Printer Setup keeps: for a document (Document, Printer Setup) or for the program (Global, Printer Setup).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrinterSetup {
    /// The printer's name; empty for the system's default printer.
    pub printer: String,
    /// The paper's width and height, in tenths of a millimetre.
    pub paper: (i32, i32),
    pub landscape: bool,
    /// The margins in thousandths of an inch: left, top, right, bottom.
    pub margins: [i32; 4],
    /// The margins are shown and typed in millimetres (else inches); the reference program keeps the unit with them.
    pub metric: bool,
    /// The formatted page size Global, Printer Setup keeps (lines per page, characters per line).
    pub lines: u32,
    pub chars: u32,
}

impl Default for PrinterSetup {
    fn default() -> Self {
        // The reference program's defaults: Letter paper (8.5 x 11 inches), and for a print document 54 lines of 65
        // characters (spec 24, 2b and 3c). One-inch margins are the system page setup's default.
        PrinterSetup {
            printer: String::new(),
            paper: (2159, 2794),
            landscape: false,
            margins: [1000; 4],
            metric: false,
            lines: 54,
            chars: 65,
        }
    }
}

impl PrinterSetup {
    /// The settings section Global, Printer Setup is kept in.
    pub const SECTION: &'static str = "Printer";

    /// The setup as one settings value: paper, orientation, margins, unit, page size, then the printer's name.
    pub fn to_text(&self) -> String {
        let m = self.margins;
        format!(
            "{}x{}\t{}\t{},{},{},{}\t{}\t{}\t{}\t{}",
            self.paper.0,
            self.paper.1,
            if self.landscape { "landscape" } else { "portrait" },
            m[0],
            m[1],
            m[2],
            m[3],
            if self.metric { "mm" } else { "in" },
            self.lines,
            self.chars,
            self.printer
        )
    }

    pub fn from_text(text: &str) -> Option<PrinterSetup> {
        let f: Vec<&str> = text.splitn(7, '\t').collect();
        if f.len() != 7 {
            return None;
        }
        let (w, h) = f[0].split_once('x')?;
        let m: Vec<i32> = f[2].split(',').map(|v| v.trim().parse().ok()).collect::<Option<_>>()?;
        if m.len() != 4 {
            return None;
        }
        Some(PrinterSetup {
            paper: (w.parse().ok()?, h.parse().ok()?),
            landscape: f[1] == "landscape",
            margins: [m[0], m[1], m[2], m[3]],
            metric: f[3] == "mm",
            lines: f[4].parse().ok()?,
            chars: f[5].parse().ok()?,
            printer: f[6].to_string(),
        })
    }

    /// A margin as the dialog shows it: 1" or 0.75", or 25.4 mm.
    pub fn margin_text(&self, thousandths: i32) -> String {
        if self.metric {
            let mm = thousandths as f64 * 0.0254;
            format!("{} mm", trim_number(&format!("{mm:.1}")))
        } else {
            format!("{}\"", trim_number(&format!("{:.3}", thousandths as f64 / 1000.0)))
        }
    }

    /// The paper as the dialog lists it.
    pub fn paper_name(&self) -> String {
        match PAPERS.iter().find(|p| (p.1, p.2) == self.paper) {
            Some(p) => p.0.to_string(),
            None => format!("Custom ({} x {} mm)", trim_number(&format!("{:.1}", self.paper.0 as f64 / 10.0)), trim_number(&format!("{:.1}", self.paper.1 as f64 / 10.0))),
        }
    }
}

fn trim_number(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// The least characters per line and lines per page Printer Setup allows: the original's MinPageWidth and
/// MinPageDepth settings for print, which nothing sets, so their defaults.
pub const MIN_CHARS: u32 = 7;
pub const MIN_LINES: u32 = 1;

/// The page size typed in Printer Setup, checked as the original checks it on OK: a field's digits make its number
/// (other characters are skipped); with no digits or below the least it becomes the least, above 160 characters or
/// 100 lines the most; a field holding other characters counts as corrected too. Returns (lines, chars, corrected
/// field: 0 none, 1 lines, 2 chars; lines when both), the field for question 1000's Cancel to go to.
pub fn page_size_check(lines_text: &str, chars_text: &str) -> (u32, u32, u8) {
    fn read(text: &str, least: u32, most: u32) -> (u32, bool) {
        let (mut n, mut digits, mut others) = (0u32, 0, 0);
        for c in text.chars() {
            match c.to_digit(10).filter(|_| c.is_ascii()) {
                Some(d) => {
                    digits += 1;
                    n = n.saturating_mul(10).saturating_add(d);
                }
                None => others += 1,
            }
        }
        if digits == 0 || n < least {
            (least, true)
        } else if n <= most {
            (n, others > 0)
        } else {
            (most, true)
        }
    }
    let (chars, c_flag) = read(chars_text, MIN_CHARS, 160);
    let (lines, l_flag) = read(lines_text, MIN_LINES, 100);
    (lines, chars, if l_flag { 1 } else if c_flag { 2 } else { 0 })
}

/// Question 1000 (OK accepts the corrected size; Cancel keeps the dialog open on the corrected field).
pub fn page_size_question(lines: u32, chars: u32) -> String {
    format!("BrailleCraft cannot support the page dimensions you requested.  It can support a page size of {lines} lines by {chars} characters.  Press OK to accept these dimensions or Cancel to alter them.")
}

/// The notices (1054, with a don't-show-again box kept under Mutable by these names) shown before Global, Printer
/// Setup and Global, Embosser Setup when a document is open; the dialog opens only on OK.
pub const GLOBAL_PRINTER_WARNING: (&str, &str) = ("GlobalPrinterWarning", "Changes that you make through Global, Printer Setup will not affect this document.  Use Document, Printer Setup instead to change document settings.");
pub const GLOBAL_EMBOSSER_WARNING: (&str, &str) = ("GlobalEmbosserWarning", "Changes that you make through Global, Embosser Setup will not affect this document.  Use Document, Embosser Setup instead to change document settings.");

/// A margin typed in the dialog, in thousandths of an inch: a number, then " or in for inches, mm or cm (without a
/// unit, the setup's own). None when it is not a number of 0 or more.
pub fn parse_margin(text: &str, metric: bool) -> Option<i32> {
    let t = text.trim().to_lowercase();
    let (number, unit) = if let Some(n) = t.strip_suffix("mm") {
        (n, Some(25.4))
    } else if let Some(n) = t.strip_suffix("cm") {
        (n, Some(2.54))
    } else if let Some(n) = t.strip_suffix('"').or_else(|| t.strip_suffix("in")) {
        (n, Some(1.0))
    } else {
        (t.as_str(), None)
    };
    let per_inch = unit.unwrap_or(if metric { 25.4 } else { 1.0 });
    let v: f64 = number.trim().parse().ok()?;
    (v >= 0.0 && v.is_finite()).then(|| (v / per_inch * 1000.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_text_round_trip() {
        let s = PrinterSetup { printer: "Office Laser".into(), paper: (2100, 2970), landscape: true, margins: [500, 750, 1000, 1250], metric: true, lines: 50, chars: 70 };
        assert_eq!(PrinterSetup::from_text(&s.to_text()), Some(s));
    }

    #[test]
    fn margins_read_and_shown() {
        assert_eq!(parse_margin("1\"", false), Some(1000));
        assert_eq!(parse_margin("0.75", false), Some(750));
        assert_eq!(parse_margin("25.4 mm", false), Some(1000));
        assert_eq!(parse_margin("2.54", true), Some(100));
        assert_eq!(parse_margin("-1", false), None);
        let s = PrinterSetup::default();
        assert_eq!(s.margin_text(1000), "1\"");
        assert_eq!(s.margin_text(750), "0.75\"");
        assert_eq!(PrinterSetup { metric: true, ..s }.margin_text(1000), "25.4 mm");
    }
}
