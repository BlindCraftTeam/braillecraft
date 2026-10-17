//! Global, PDF Importer (command 3116): the PDF import settings, kept under the section "PDF" with the reference's
//! keys and defaults (spec 29 section 2), the dialog that edits them, and the options the importer takes from them.

use wxdragon::prelude::*;

use crate::settings_store::{get, set};

const SECTION: &str = "PDF";

/// The stored settings; numbers in thousandths, as the reference stores them.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfSettings {
    pub show_at_import: bool,
    pub pdf_a: bool,
    pub detect_ledgers: bool,
    pub use_header_font_hints: bool,
    pub fix_leader_dots: bool,
    /// Thousandths of an inch.
    pub header_height: i64,
    pub footer_height: i64,
    pub retain_bold: bool,
    pub retain_italic: bool,
    pub retain_underline: bool,
    pub script: String,
    pub advanced: bool,
    pub line_overlap: i64,
    pub char_margin: i64,
    pub word_margin: i64,
    pub line_margin: i64,
    pub boxes_flow: i64,
    pub detect_vertical: bool,
    pub all_texts: bool,
}

impl Default for PdfSettings {
    fn default() -> Self {
        PdfSettings {
            show_at_import: false,
            pdf_a: true,
            detect_ledgers: true,
            use_header_font_hints: true,
            fix_leader_dots: true,
            header_height: 0,
            footer_height: 0,
            retain_bold: true,
            retain_italic: true,
            retain_underline: true,
            script: String::new(),
            advanced: true,
            line_overlap: 500,
            char_margin: 2000,
            word_margin: 100,
            line_margin: 500,
            boxes_flow: 500,
            detect_vertical: false,
            all_texts: false,
        }
    }
}

fn number(key: &str) -> Option<i64> {
    get(SECTION, key)?.trim().parse().ok()
}

fn flag(key: &str) -> Option<bool> {
    number(key).map(|n| n != 0)
}

fn yes_no(key: &str) -> Option<bool> {
    match get(SECTION, key)?.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

impl PdfSettings {
    /// The kept settings over the defaults; a value missing or unreadable keeps its default.
    pub fn load() -> PdfSettings {
        let mut s = PdfSettings::default();
        let flags: [(&str, &mut bool); 8] = [
            ("ShowAtImport", &mut s.show_at_import),
            ("PDF_A", &mut s.pdf_a),
            ("DetectLedgers", &mut s.detect_ledgers),
            ("UseHeaderFontHints", &mut s.use_header_font_hints),
            ("FixLeaderDots", &mut s.fix_leader_dots),
            ("RetainBold", &mut s.retain_bold),
            ("RetainItalic", &mut s.retain_italic),
            ("RetainUnderline", &mut s.retain_underline),
        ];
        for (k, f) in flags {
            if let Some(v) = flag(k) {
                *f = v;
            }
        }
        let numbers: [(&str, &mut i64); 7] = [
            ("HeaderHeight", &mut s.header_height),
            ("FooterHeight", &mut s.footer_height),
            ("PDFMiner.LineOverlap", &mut s.line_overlap),
            ("PDFMiner.CharMargin", &mut s.char_margin),
            ("PDFMiner.WordMargin", &mut s.word_margin),
            ("PDFMiner.LineMargin", &mut s.line_margin),
            ("PDFMiner.BoxesFlow", &mut s.boxes_flow),
        ];
        for (k, f) in numbers {
            if let Some(v) = number(k) {
                *f = v;
            }
        }
        let choices: [(&str, &mut bool); 3] =
            [("PDFMiner.Advanced", &mut s.advanced), ("PDFMiner.DetectVertical", &mut s.detect_vertical), ("PDFMiner.AllTexts", &mut s.all_texts)];
        for (k, f) in choices {
            if let Some(v) = yes_no(k) {
                *f = v;
            }
        }
        if let Some(v) = get(SECTION, "Script") {
            s.script = v;
        }
        s
    }

    pub fn save(&self) {
        let b = |v: bool| if v { "1" } else { "0" };
        let yn = |v: bool| if v { "yes" } else { "no" };
        for (k, v) in [
            ("ShowAtImport", self.show_at_import),
            ("PDF_A", self.pdf_a),
            ("DetectLedgers", self.detect_ledgers),
            ("UseHeaderFontHints", self.use_header_font_hints),
            ("FixLeaderDots", self.fix_leader_dots),
            ("RetainBold", self.retain_bold),
            ("RetainItalic", self.retain_italic),
            ("RetainUnderline", self.retain_underline),
        ] {
            set(SECTION, k, b(v));
        }
        for (k, v) in [
            ("HeaderHeight", self.header_height),
            ("FooterHeight", self.footer_height),
            ("PDFMiner.LineOverlap", self.line_overlap),
            ("PDFMiner.CharMargin", self.char_margin),
            ("PDFMiner.WordMargin", self.word_margin),
            ("PDFMiner.LineMargin", self.line_margin),
            ("PDFMiner.BoxesFlow", self.boxes_flow),
        ] {
            set(SECTION, k, &v.to_string());
        }
        for (k, v) in [("PDFMiner.Advanced", self.advanced), ("PDFMiner.DetectVertical", self.detect_vertical), ("PDFMiner.AllTexts", self.all_texts)] {
            set(SECTION, k, yn(v));
        }
        set(SECTION, "Script", &self.script);
    }

    /// The importer's options, as the reference writes them into its script (spec 29 3b): heights from thousandths
    /// of an inch to points (x 0.072), layout numbers from thousandths; without Advanced, no box flow.
    pub fn options(&self) -> bc_core::pdf_import::PdfOptions {
        let k = |v: i64| v as f64 / 1000.0;
        bc_core::pdf_import::PdfOptions {
            allow_tagged: self.pdf_a,
            header_height: self.header_height as f64 * 0.072,
            footer_height: self.footer_height as f64 * 0.072,
            la: bc_core::pdf_import::layout::LaParams {
                line_overlap: k(self.line_overlap),
                char_margin: k(self.char_margin),
                line_margin: k(self.line_margin),
                word_margin: k(self.word_margin),
                boxes_flow: self.advanced.then(|| k(self.boxes_flow)),
                detect_vertical: self.detect_vertical,
                all_texts: self.all_texts,
            },
        }
    }
}

/// A stored thousandth as the dialog shows it: divided by 1000, trailing zeros and then a trailing point removed.
fn shown(v: i64) -> String {
    let s = format!("{:.6}", v as f64 / 1000.0);
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.').to_string()
}

/// A number field read on OK: its value in thousandths, or the message the reference shows.
fn read_number(text: &str, label: &str, max: f64) -> Result<i64, String> {
    match text.trim().parse::<f64>() {
        Ok(v) if (0.0..=max).contains(&v) => Ok((v * 1000.0 + 0.5) as i64),
        _ => Err(format!("{label} must be a number between {:.6} and {:.6}", 0.0, max)),
    }
}

fn named(d: &crate::dlg::Dlg, name: &str) -> Option<Window> {
    d.dialog.find_window_by_name(name)
}

fn check(d: &crate::dlg::Dlg, name: &str, on: bool) {
    if let Some(c) = named(d, name).and_then(|w| w.as_widget::<CheckBox>()) {
        c.set_value(on);
    }
}

fn checked(d: &crate::dlg::Dlg, name: &str) -> bool {
    named(d, name).and_then(|w| w.as_widget::<CheckBox>()).is_some_and(|c| c.is_checked())
}

fn set_field(d: &crate::dlg::Dlg, name: &str, text: &str) {
    if let Some(t) = named(d, name).and_then(|w| w.as_widget::<TextCtrl>()) {
        t.set_value(text);
    }
}

fn field(d: &crate::dlg::Dlg, name: &str) -> String {
    named(d, name).and_then(|w| w.as_widget::<TextCtrl>()).map(|t| t.get_value()).unwrap_or_default()
}

fn label(d: &crate::dlg::Dlg, name: &str) -> String {
    named(d, name).and_then(|w| w.get_label()).unwrap_or_default()
}

/// Global, PDF Importer: the dialog, its checks on OK, and the settings kept when it closes with OK.
pub fn dialog() {
    let s = PdfSettings::load();
    let d = crate::dlg::Dlg::open_xrc(&crate::app::frame(), 3116);
    check(&d, "SHOW_FOR_EACH_IMPORT", s.show_at_import);
    check(&d, "USE_PDF_A", s.pdf_a);
    check(&d, "DETECT_LEDGER_FORMAT", s.detect_ledgers);
    check(&d, "FIX_LEADER_DOTS", s.fix_leader_dots);
    if let Some(p) = named(&d, "FILEPICKER_PDF_SCRIPT").and_then(|w| w.as_widget::<FilePickerCtrl>()) {
        p.set_path(&s.script);
    }
    set_field(&d, "PDF_HEADER_HEIGHT", &shown(s.header_height));
    set_field(&d, "PDF_FOOTER_HEIGHT", &shown(s.footer_height));
    check(&d, "PDF_USE_HEADER_FONT_HINTS", s.use_header_font_hints);
    check(&d, "PDF_RETAIN_BOLD", s.retain_bold);
    check(&d, "PDF_RETAIN_ITALIC", s.retain_italic);
    check(&d, "PDF_RETAIN_UNDERLINE", s.retain_underline);
    check(&d, "PDFMINER_ADVANCED", s.advanced);
    set_field(&d, "PDFMINER_LINE_OVERLAP", &shown(s.line_overlap));
    set_field(&d, "PDFMINER_CHAR_MARGIN", &shown(s.char_margin));
    set_field(&d, "PDFMINER_WORD_MARGIN", &shown(s.word_margin));
    set_field(&d, "PDFMINER_LINE_MARGIN", &shown(s.line_margin));
    set_field(&d, "PDFMINER_BOXES_FLOW", &shown(s.boxes_flow));
    check(&d, "PDFMINER_DETECT_VERTICAL", s.detect_vertical);
    check(&d, "PDFMINER_ALL_TEXTS", s.all_texts);
    let new = loop {
        if d.show() != 1 {
            d.destroy();
            return;
        }
        let advanced = checked(&d, "PDFMINER_ADVANCED");
        // The number fields in the reference's order of checking; Box Flow only with Advanced on. Line Overlap is
        // shown but not read back, as in the reference (spec 29 2: its edits are dropped).
        let read = || -> Result<PdfSettings, (String, &'static str)> {
            let num = |name: &'static str, label_name: &str, max: f64, old: i64, used: bool| -> Result<i64, (String, &'static str)> {
                if !used {
                    return Ok(old);
                }
                read_number(&field(&d, name), &label(&d, label_name), max).map_err(|e| (e, name))
            };
            Ok(PdfSettings {
                show_at_import: checked(&d, "SHOW_FOR_EACH_IMPORT"),
                pdf_a: checked(&d, "USE_PDF_A"),
                detect_ledgers: checked(&d, "DETECT_LEDGER_FORMAT"),
                fix_leader_dots: checked(&d, "FIX_LEADER_DOTS"),
                script: named(&d, "FILEPICKER_PDF_SCRIPT").and_then(|w| w.as_widget::<FilePickerCtrl>()).map(|p| p.get_path()).unwrap_or_default(),
                header_height: num("PDF_HEADER_HEIGHT", "PDF_HEADER_HEIGHT_LABEL", 10.0, s.header_height, true)?,
                footer_height: num("PDF_FOOTER_HEIGHT", "PDF_FOOTER_HEIGHT_LABEL", 10.0, s.footer_height, true)?,
                use_header_font_hints: checked(&d, "PDF_USE_HEADER_FONT_HINTS"),
                retain_bold: checked(&d, "PDF_RETAIN_BOLD"),
                retain_italic: checked(&d, "PDF_RETAIN_ITALIC"),
                retain_underline: checked(&d, "PDF_RETAIN_UNDERLINE"),
                advanced,
                line_overlap: s.line_overlap,
                char_margin: num("PDFMINER_CHAR_MARGIN", "PDFMINER_CHAR_MARGIN_LABEL", 4.0, s.char_margin, true)?,
                word_margin: num("PDFMINER_WORD_MARGIN", "PDFMINER_WORD_MARGIN_LABEL", 1.0, s.word_margin, true)?,
                line_margin: num("PDFMINER_LINE_MARGIN", "PDFMINER_LINE_MARGIN_LABEL", 1.0, s.line_margin, true)?,
                boxes_flow: num("PDFMINER_BOXES_FLOW", "PDFMINER_BOXES_FLOW_LABEL", 1.0, s.boxes_flow, advanced)?,
                detect_vertical: checked(&d, "PDFMINER_DETECT_VERTICAL"),
                all_texts: checked(&d, "PDFMINER_ALL_TEXTS"),
            })
        };
        match read() {
            Ok(n) => break n,
            Err((msg, bad)) => {
                crate::commands::message(&msg);
                if let Some(w) = named(&d, bad) {
                    w.set_focus();
                }
            }
        }
    };
    d.destroy();
    new.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_the_dialog_shows_and_reads_them() {
        assert_eq!(shown(2000), "2");
        assert_eq!(shown(500), "0.5");
        assert_eq!(shown(0), "0");
        assert_eq!(shown(1250), "1.25");
        assert_eq!(read_number("0.5", "Line Margin", 1.0), Ok(500));
        assert_eq!(read_number("4", "Character Margin", 4.0), Ok(4000));
        assert_eq!(read_number("5", "Character Margin", 4.0), Err("Character Margin must be a number between 0.000000 and 4.000000".into()));
        assert_eq!(read_number("x", "Word Margin", 1.0).is_err(), true);
    }

    #[test]
    fn options_from_the_settings() {
        let s = PdfSettings { header_height: 1000, advanced: false, ..PdfSettings::default() };
        let o = s.options();
        assert!((o.header_height - 72.0).abs() < 1e-9);
        assert_eq!(o.la.boxes_flow, None);
        assert_eq!(PdfSettings::default().options(), bc_core::pdf_import::PdfOptions::default());
    }
}
