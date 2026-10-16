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
