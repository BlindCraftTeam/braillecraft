//! A document's status line (spec 17 section 6, spec 19 section 4), shared by the desktop program and the web build.

use crate::doctext::Shown;

/// The styles open at a position: each [es~name] code before it without its [ee~name].
pub fn open_styles(doc: &Shown, pos: i64) -> Vec<String> {
    let mut open: Vec<String> = Vec::new();
    for &(start, _) in doc.codes.iter().take_while(|c| (c.1 as i64) <= pos) {
        let Some(code) = doc.code_at(start) else { continue };
        if let Some(name) = code.strip_prefix("es~") {
            open.push(name.to_string());
        } else if let Some(name) = code.strip_prefix("ee~") {
            if let Some(k) = open.iter().rposition(|n| n == name) {
                open.remove(k);
            }
        }
    }
    open
}

/// The style text of the status line: "Current Style: None", the style open at the caret, or the innermost of
/// several with its level.
pub fn style_text(doc: &Shown, pos: i64) -> String {
    let styles = open_styles(doc, pos);
    match styles.len() {
        0 => "Current Style: None".to_string(),
        1 => format!("Current Style: {}", styles[0]),
        n => format!("Current Style: {} (level {n})", styles[n - 1]),
    }
}

/// What a document's status line shows, in the reference program's order: the current style text, then the fields
/// Page:, Line:, Column: (left out in coded view), the character at the caret, and Keys: (each label followed by its
/// value with no space, as screen reader scripts read "Keys:" and the character after it).
pub struct StatusLine {
    pub style: String,
    /// Page, line and column (1-based); None in coded view (View, Codes), where the reference hides those fields.
    pub location: Option<(usize, usize, usize)>,
    pub character: String,
    /// Six-key entry on: "Keys:6", else "Keys:1".
    pub six_key: bool,
    /// Highlighting on: "H" after the number.
    pub highlight: bool,
    /// A braille keyboard connected: "+" after the number (when highlighting is off).
    pub keyboard: bool,
}

impl StatusLine {
    pub fn text(&self) -> String {
        let mut parts = vec![self.style.clone()];
        if let Some((page, line, column)) = self.location {
            parts.push(format!("Page:{page}"));
            parts.push(format!("Line:{line}"));
            parts.push(format!("Column:{column}"));
        }
        parts.push(self.character.clone());
        let mark = if self.highlight { "H" } else if self.keyboard { "+" } else { "" };
        parts.push(format!("Keys:{}{mark}", if self.six_key { 6 } else { 1 }));
        parts.join("  ")
    }
}

/// The status line's character field for the character at `pos`: "(end)" at the end of the document; a code as
/// "[code]", a style start [es~name] as "<name>" and a style end [ee~name] as "</name>"; a hard space as "[']";
/// otherwise the character, followed by " {U+XXXX}" when verbose labels are on.
pub fn char_field(doc: &Shown, pos: i64, verbose: bool) -> String {
    let at = pos.max(0) as usize;
    let Some(c) = doc.text.chars().nth(at) else { return "(end)".to_string() };
    if c == '\n' {
        // A line break is the new-line code, except the editor's own after [p] or [l] (crate::doctext), which is
        // not part of the document: the field shows what follows it.
        return if doc.is_own_break(at) { char_field(doc, pos + 1, verbose) } else { "[<]".to_string() };
    }
    if let Some(code) = doc.code_at(at) {
        // "es~" and "ee~" are matched in either case, as the reference compares them.
        let head = code.get(..3).map(str::to_ascii_lowercase);
        return match head.as_deref() {
            Some("es~") => format!("<{}>", &code[3..]),
            Some("ee~") => format!("</{}>", &code[3..]),
            _ => format!("[{code}]"),
        };
    }
    if c == '\u{a0}' {
        return "[']".to_string();
    }
    if verbose {
        format!("{c} {{U+{:04X}}}", c as u32 & 0xffff)
    } else {
        c.to_string()
    }
}
