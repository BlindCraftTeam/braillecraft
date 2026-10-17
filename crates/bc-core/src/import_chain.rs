//! The import chains for braille and text files, as the reference program's import recipes run them: each recipe is
//! a list of steps (built-in steps and rule scripts) run one after another over the file's bytes, and the result is
//! internal text with codes marked 0x1C ... 0x1F.
//!
//! - Formatted braille (.brf): page-end blanks taken off (only when formatting is preserved), the formatted braille
//!   reader (`braide`), the import scan with the braille scan table, the ninit codes, tables to outlines, spaces
//!   normalised three times, then the Nemeth-in-UEB scripts with the text reversed between them and reversed back.
//! - Coded braille (.bru): the import scan with the braille scan table, the ninit codes, tables to outlines, spaces
//!   normalised three times.
//! - Coded text: the file decoded in the chosen code page and written as internal text, then the same steps as coded
//!   braille with the print scan table.
//! - Orbit Reader braille (.obf): line ends to line codes and capitals to small letters, the import scan with the
//!   braille scan table, then the Nemeth-in-UEB scripts as for formatted braille.
//!
//! The braille encoding step that comes first for formatted braille (the file's encoding to North American braille
//! ASCII) is the caller's, which holds the encodings. A step that fails is reported and the chain goes on with the
//! text as it was, as the reference's recipe runner does.

use crate::braide;
use crate::rule_script::RuleScript;

/// What the chain reads by name: the import scan tables (tables/scan) and the rule scripts (tables/rule-scripts).
pub trait Files {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String>;
    fn rule_script(&self, name: &str) -> Result<RuleScript, String>;
}

/// Which recipe a file is imported with.
#[derive(Debug, Clone, PartialEq)]
pub enum Recipe {
    /// Formatted braille in North American braille ASCII, with the formatted braille import settings.
    FormattedBraille(braide::Settings),
    CodedBraille,
    /// Coded text in the named code page (as `codepages::decode` names them).
    CodedText(String),
    Orbit,
}

/// The chain's result: the internal text, the steps that failed (each with its reason) and what the formatted
/// braille reader warns about.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    pub text: Vec<u8>,
    pub errors: Vec<String>,
    pub warnings: braide::Warnings,
}

const BRAILLE_SCAN: &str = "import-braille.sct";
const PRINT_SCAN: &str = "import-print.sct";

/// Runs the recipe over the file's bytes.
pub fn run(bytes: &[u8], recipe: &Recipe, files: &dyn Files) -> Imported {
    let mut out = Imported::default();
    let mut text = bytes.to_vec();
    let script = |name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>| match files.rule_script(name) {
        Ok(mut s) => match s.run(text) {
            Ok(t) => *text = t,
            Err(e) => errors.push(format!("{name}: {e}")),
        },
        Err(e) => errors.push(format!("{name}: {e}")),
    };
    let scan = |name: &str, text: &mut Vec<u8>, errors: &mut Vec<String>| {
        match files.scan_table(name).and_then(|t| crate::engine::import_scan(&t, text).map_err(|e| format!("{e:?}"))) {
            Ok(t) => *text = t,
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    };
    match recipe {
        Recipe::FormattedBraille(settings) => {
            if settings.preserve {
                script("page-end-blanks.rules", &mut text, &mut out.errors);
            }
            let (t, w) = braide::braide(&text, *settings);
            text = t;
            out.warnings = w;
            scan(BRAILLE_SCAN, &mut text, &mut out.errors);
        }
        Recipe::CodedBraille => scan(BRAILLE_SCAN, &mut text, &mut out.errors),
        Recipe::CodedText(page) => {
            text = coded_text_bytes(&text, page);
            scan(PRINT_SCAN, &mut text, &mut out.errors);
        }
        Recipe::Orbit => {
            script("orbit-lines.rules", &mut text, &mut out.errors);
            scan(BRAILLE_SCAN, &mut text, &mut out.errors);
            nemeth(files, &mut text, &mut out.errors, true);
            out.text = text;
            return out;
        }
    }
    script("ninit-codes.rules", &mut text, &mut out.errors);
    // Tables to outlines is optional in the recipes: it runs when it is there. The optional maths step that follows
    // it in the recipes is not part of the reference program's installation, so there is nothing to run.
    if let Ok(mut s) = files.rule_script("table-outline.rules") {
        match s.run(&text) {
            Ok(t) => text = t,
            Err(e) => out.errors.push(format!("table-outline.rules: {e}")),
        }
    }
    for _ in 0..3 {
        script("normalize-spaces.rules", &mut text, &mut out.errors);
    }
    if matches!(recipe, Recipe::FormattedBraille(_)) {
        nemeth(files, &mut text, &mut out.errors, false);
    }
    out.text = text;
    out
}

/// The Nemeth-in-UEB scripts: the first, the text reversed, the second on the reversed text, reversed back. In the
/// Orbit recipe the two scripts are optional (run when they are there).
fn nemeth(files: &dyn Files, text: &mut Vec<u8>, errors: &mut Vec<String>, optional: bool) {
    for (k, name) in ["nemeth-in-ueb-1.rules", "nemeth-in-ueb-2.rules"].into_iter().enumerate() {
        if k == 1 {
            *text = reversed(text);
        }
        match files.rule_script(name) {
            Ok(mut s) => match s.run(text) {
                Ok(t) => *text = t,
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Err(_) if optional => {}
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    *text = reversed(text);
}

/// Coded text as internal text: decoded in its code page (a byte order mark at the start dropped), each zero-width
/// joiner taken out unless it follows the Malayalam virama, then encoded.
fn coded_text_bytes(bytes: &[u8], page: &str) -> Vec<u8> {
    let text = crate::codepages::decode(page, bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut kept = String::with_capacity(text.len());
    let mut prev = '\0';
    for c in text.chars() {
        if c != '\u{200d}' || prev == '\u{0d4d}' {
            kept.push(c);
        }
        prev = c;
    }
    crate::internal::encode_lossy(&kept, b'?')
}

/// Internal text with its codes in reverse order: each two-byte code stays one code, and a format code (0x1C to the
/// next 0x1F) stays whole and in its own order.
pub fn reversed(bytes: &[u8]) -> Vec<u8> {
    let mut units: Vec<&[u8]> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let n = if crate::internal::is_first_byte(bytes[i]) && i + 1 < bytes.len() { 2 } else { 1 };
        units.push(&bytes[i..i + n]);
        i += n;
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut k = units.len();
    while k > 0 {
        k -= 1;
        if units[k] == [0x1f] {
            // Back to the code's start, written forward.
            let end = k;
            while k > 0 && units[k] != [0x1c] {
                k -= 1;
            }
            for u in &units[k..=end] {
                out.extend_from_slice(u);
            }
        } else {
            out.extend_from_slice(units[k]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Shipped;

    fn tables() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables")
    }

    impl Files for Shipped {
        fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
            std::fs::read(tables().join("scan").join(name)).map_err(|e| e.to_string())
        }
        fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
            RuleScript::open(&tables().join("rule-scripts").join(name)).map_err(|e| e.to_string())
        }
    }

    #[test]
    fn reversal_keeps_codes_and_two_byte_codes_whole() {
        assert_eq!(reversed(b"ab\x1cp\x1fcd"), b"dc\x1cp\x1fba".to_vec());
        assert_eq!(reversed(&[0x61, 0xf5, 0x23, 0x62]), vec![0x62, 0xf5, 0x23, 0x61]);
        assert_eq!(reversed(&reversed(b"x \x1cind5\x1fy")), b"x \x1cind5\x1fy".to_vec());
    }

    #[test]
    fn coded_braille_gets_codes() {
        let got = run(b"|P This is |IND5 a test.", &Recipe::CodedBraille, &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        assert_eq!(got.text, b"\x1cp\x1fthis is \x1cind5\x1fa test.".to_vec());
    }

    #[test]
    fn coded_text_gets_codes_and_joins_hyphens() {
        let got = run(b"$p Hello hyphen-\r\nated.", &Recipe::CodedText("WINDOWS-1252".into()), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        assert_eq!(got.text, b"\x1cp\x1fHello hyphenated.".to_vec());
    }

    #[test]
    fn orbit_lines_become_line_codes() {
        let got = run(b"ABC\r\nDEF", &Recipe::Orbit, &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        // The script writes the line code in its internal form, which the scan passes on.
        assert_eq!(got.text, b"abc\x1cl\x1fdef".to_vec());
    }

    #[test]
    fn formatted_braille_runs_every_step() {
        let page = b",! QUICK BR[N FOX JUMPS OV] ! LAZY DOG4\nCONT9UES ON ! NEXT L9E\n\x0c";
        let got = run(page, &Recipe::FormattedBraille(braide::Settings::default()), &Shipped);
        assert!(got.errors.is_empty(), "{:?}", got.errors);
        let text = String::from_utf8_lossy(&got.text);
        assert!(text.contains(",! quick br[n fox jumps ov] ! lazy dog4 cont9ues on ! next l9e"), "{text}");
    }
}
