//! PEF, the Portable Embosser Format (PEF 1.0, version 2008-1, DAISY Consortium): braille pages as Unicode braille
//! in XML. The reference program has no PEF; BrailleCraft writes and reads it from the published standard, as the
//! version 1 scope asks (open and save PEF).
//!
//! Written: one volume holding one section, its page size (columns and rows) and whether it is embossed on both
//! sides (duplex); one page element per laid-out page, one row element per line, the cells as U+2800 to U+283F with
//! the trailing blank cells left off; the required metadata dc:format and dc:identifier, with dc:title and dc:date.
//! Read: the rows of every page of every section of every volume, in order, as Unicode braille.

use crate::format::Page;

/// The cell of each character: its dots as a number 0 to 63 (dot 1 = 1 ... dot 6 = 32).
pub fn write(pages: &[Page], cols: usize, rows: usize, duplex: bool, title: &str, identifier: &str, date: &str, cell: impl Fn(char) -> u8) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<pef version=\"2008-1\" xmlns=\"http://www.daisy.org/ns/2008/pef\">\n");
    out.push_str("  <head>\n    <meta xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");
    out.push_str("      <dc:format>application/x-pef+xml</dc:format>\n");
    out.push_str(&format!("      <dc:identifier>{}</dc:identifier>\n", escape(identifier)));
    if !title.is_empty() {
        out.push_str(&format!("      <dc:title>{}</dc:title>\n", escape(title)));
    }
    if !date.is_empty() {
        out.push_str(&format!("      <dc:date>{}</dc:date>\n", escape(date)));
    }
    out.push_str("    </meta>\n  </head>\n  <body>\n");
    out.push_str(&format!("    <volume cols=\"{cols}\" rows=\"{rows}\" rowgap=\"0\" duplex=\"{duplex}\">\n      <section>\n"));
    for page in pages {
        out.push_str("        <page>\n");
        for line in &page.lines {
            let cells: String = line.text.chars().map(|c| char::from_u32(0x2800 + u32::from(cell(c) & 0x3f)).unwrap_or('\u{2800}')).collect();
            let cells = cells.trim_end_matches('\u{2800}');
            if cells.is_empty() {
                out.push_str("          <row/>\n");
            } else {
                out.push_str(&format!("          <row>{cells}</row>\n"));
            }
        }
        out.push_str("        </page>\n");
    }
    out.push_str("      </section>\n    </volume>\n  </body>\n</pef>\n");
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The pages of a PEF file, each a list of its rows in Unicode braille (8-dot cells kept as they are). An error
/// when the text is not a PEF file.
pub fn read(xml: &str) -> Result<Vec<Vec<String>>, String> {
    if !has_element(xml, "pef") {
        return Err("This is not a PEF file (no pef element).".into());
    }
    let mut pages = Vec::new();
    let mut rest = xml;
    while let Some((page, after)) = element(rest, "page") {
        let mut rows = Vec::new();
        let mut inner = page;
        while let Some((row, after_row)) = element(inner, "row") {
            rows.push(row.chars().filter(|c| ('\u{2800}'..='\u{28ff}').contains(c)).collect());
            inner = after_row;
        }
        pages.push(rows);
        rest = after;
    }
    Ok(pages)
}

/// Whether the text has an element named `name` (with or without a namespace prefix).
fn has_element(xml: &str, name: &str) -> bool {
    open_tag(xml, name).is_some()
}

/// The start of the next opening tag of `name` (`<name`, `<prefix:name`), and the index after the tag name.
fn open_tag(xml: &str, name: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(lt) = xml[from..].find('<').map(|i| from + i) {
        let tag = &xml[lt + 1..];
        let local_start = tag.find(|c: char| !(c.is_alphanumeric() || c == ':' || c == '-' || c == '_' || c == '.')).unwrap_or(tag.len());
        let full = &tag[..local_start];
        let local = full.rsplit(':').next().unwrap_or(full);
        if local == name && !full.is_empty() {
            return Some((lt, lt + 1 + local_start));
        }
        from = lt + 1;
    }
    None
}

/// The content of the next `name` element and the text after it; an empty element (`<row/>`) has empty content.
fn element<'a>(xml: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let (_, after_name) = open_tag(xml, name)?;
    let gt = xml[after_name..].find('>')? + after_name;
    if xml[..gt].ends_with('/') {
        return Some(("", &xml[gt + 1..]));
    }
    let body = &xml[gt + 1..];
    // The matching end tag: rows and pages are not nested in themselves.
    let mut from = 0;
    while let Some(lt) = body[from..].find("</").map(|i| from + i) {
        let tag = &body[lt + 2..];
        let end = tag.find('>')?;
        let full = tag[..end].trim();
        if full.rsplit(':').next() == Some(name) {
            return Some((&body[..lt], &body[lt + 2 + end + 1..]));
        }
        from = lt + 2;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Line;

    fn page(lines: &[&str]) -> Page {
        Page { lines: lines.iter().map(|l| Line { text: l.to_string(), sources: vec![None; l.chars().count()] }).collect(), start: 0 }
    }

    #[test]
    fn written_and_read_back() {
        // Cells by a simple map: 'a' is dot 1, 'b' dots 1 2, a space no dots.
        let cell = |c: char| match c {
            'a' => 1,
            'b' => 3,
            _ => 0,
        };
        let xml = write(&[page(&["ab a", "", "b  "]), page(&["a"])], 40, 25, true, "T & U", "id-1", "2026-10-16", cell);
        assert!(xml.contains("<dc:format>application/x-pef+xml</dc:format>"));
        assert!(xml.contains("<dc:title>T &amp; U</dc:title>"));
        assert!(xml.contains("cols=\"40\" rows=\"25\" rowgap=\"0\" duplex=\"true\""));
        assert!(xml.contains("<row>\u{2801}\u{2803}\u{2800}\u{2801}</row>") && xml.contains("<row/>"));
        let back = read(&xml).unwrap();
        assert_eq!(back, vec![vec!["\u{2801}\u{2803}\u{2800}\u{2801}".to_string(), String::new(), "\u{2803}".into()], vec!["\u{2801}".into()]]);
    }

    #[test]
    fn reads_namespace_prefixes_and_refuses_other_files() {
        let xml = "<p:pef xmlns:p=\"http://www.daisy.org/ns/2008/pef\"><p:body><p:volume><p:section><p:page><p:row>\u{2807}</p:row><p:row/></p:page></p:section></p:volume></p:body></p:pef>";
        assert_eq!(read(xml).unwrap(), vec![vec!["\u{2807}".to_string(), String::new()]]);
        assert!(read("<html></html>").is_err());
    }
}
