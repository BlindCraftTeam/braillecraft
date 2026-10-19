//! Markdown import (BrailleCraft's own addition): a Markdown file is
//! turned into HTML and then goes through the HTML import chain, so a Markdown document imports exactly as the
//! same content would import from HTML.

use pulldown_cmark::{html, Event, LinkType, Options, Parser, Tag, TagEnd};

/// The HTML of a Markdown text: CommonMark with tables, footnotes, strikethrough and task lists, and the dialects of
/// plan line D45. Headings become h1 to h6, paragraphs p, lists ul and ol with li, emphasis em and strong, code and
/// pre, links a, images their alt text inside img, block quotes blockquote, rules hr, line breaks br, tables table,
/// thead, tbody, tr, th and td, footnotes sup and div; every tag is closed, so the HTML reads as XML. D45: YAML
/// (---) and TOML (+++) front matter is left out; $...$ and $$...$$ math is kept as its TeX source in code (shortcut:
/// read as computer braille, until the MathML path of D19 turns it into math codes); a wikilink [[Page]] or
/// [[Page|Shown]] is its shown text; a callout "> [!kind] Title" (GitHub alerts, Obsidian callouts) starts its
/// quote with the title, or the kind, as a bold paragraph.
pub fn to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    options.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
    options.insert(Options::ENABLE_MATH);
    options.insert(Options::ENABLE_WIKILINKS);
    let source = callouts(markdown);
    let mut in_wikilink = false;
    let parser = Parser::new_ext(&source, options).filter_map(|event| match event {
        Event::InlineMath(t) | Event::DisplayMath(t) => Some(Event::Code(t)),
        Event::Start(Tag::Link { link_type: LinkType::WikiLink { .. }, .. }) => {
            in_wikilink = true;
            None
        }
        Event::End(TagEnd::Link) if in_wikilink => {
            in_wikilink = false;
            None
        }
        e => Some(e),
    });
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, parser);
    out
}

/// The text with each callout line "> [!kind]+ Title" made "> **Title**" and a quote line of its own after it
/// (the kind with a capital when there is no title), outside fenced code.
fn callouts(markdown: &str) -> std::borrow::Cow<'_, str> {
    if !markdown.contains("[!") {
        return std::borrow::Cow::Borrowed(markdown);
    }
    let mut out = String::with_capacity(markdown.len() + 64);
    let mut fence: Option<&str> = None;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => fence = Some(m),
            (Some(f), Some(m)) if f == m => fence = None,
            _ => {}
        }
        let quote_end = trimmed.len() - trimmed.trim_start_matches(|c: char| c == '>' || c == ' ').len();
        let body = &trimmed[quote_end..];
        let callout = (fence.is_none() && marker.is_none() && trimmed.starts_with('>'))
            .then(|| body.strip_prefix("[!"))
            .flatten()
            .and_then(|r| r.split_once(']'))
            .filter(|(kind, _)| !kind.is_empty() && kind.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
        match callout {
            Some((kind, rest)) => {
                let prefix = &line[..line.len() - trimmed.len() + quote_end];
                let title = rest.trim_start_matches(['+', '-']).trim();
                let mut chars = kind.chars();
                let named = chars.next().map(|c| c.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()).unwrap_or_default();
                let title = if title.is_empty() { named } else { title.to_string() };
                out.push_str(&format!("{prefix}**{title}**\n{}\n", prefix.trim_end()));
            }
            None => out.push_str(line),
        }
    }
    std::borrow::Cow::Owned(out)
}

/// A braille document as Markdown (BrailleCraft's own addition): each laid-out page's lines, already as Unicode
/// braille patterns, inside a fenced code block, with a blank line between pages, so the cells keep their columns
/// in any Markdown viewer.
pub fn braille_pages(pages: &[Vec<String>]) -> String {
    let mut out = String::new();
    for page in pages {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("```\n");
        for line in page {
            out.push_str(line.trim_end_matches('\u{2800}'));
            out.push('\n');
        }
        out.push_str("```\n");
    }
    out
}

/// Markdown from the HTML the HTML export writes (Save As Markdown, BrailleCraft's own addition, plan line D2): a
/// tolerant tag walker over the export's bytes, which are not always well-formed XML. Headings h1 to h6 become
/// "#" lines, p paragraphs separated by a blank line, it/i/em "*", b/strong "**", a href "[text](url)", br a hard
/// break, ul and ol lists with "-" and "1." items, hr "---", pre and code fenced or backticked code, tables pipe
/// tables; the html and body wrappers, the code instructions "<?$...`>" and any other tag are dropped, their text
/// kept. Numeric and the five named character references are turned into their characters.
pub fn from_html(html: &[u8]) -> String {
    let text = String::from_utf8_lossy(html);
    let mut w = Writer::default();
    let mut rest: &str = &text;
    while let Some(k) = rest.find('<') {
        w.text(&rest[..k]);
        rest = &rest[k..];
        if let Some(stripped) = rest.strip_prefix("<?") {
            // A code instruction: dropped whole.
            match stripped.find("`>").or_else(|| stripped.find("?>")) {
                Some(e) => rest = &stripped[e + 2..],
                None => {
                    rest = "";
                }
            }
            continue;
        }
        if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(e) => rest = &rest[e + 3..],
                None => rest = "",
            }
            continue;
        }
        let Some(end) = rest.find('>') else {
            w.text(rest);
            rest = "";
            break;
        };
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        w.tag(tag);
    }
    w.text(rest);
    w.finish()
}

#[derive(Default)]
struct Writer {
    out: String,
    /// Open list kinds, innermost last: true for an ordered list, with its next item number.
    lists: Vec<(bool, usize)>,
    /// Inside pre: text kept as it is.
    pre: bool,
    /// Inside a table: the current row's cells and whether a header row was written.
    table: Option<Table>,
    /// A link open: its URL, and the text so far.
    link: Option<(String, usize)>,
    /// A link was just closed: the line end the export's link step writes after an anchor is not a break.
    after_link: bool,
}

#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    header: bool,
    cell: Option<String>,
}

impl Writer {
    fn blank_line(&mut self) {
        while self.out.ends_with(' ') {
            self.out.pop();
        }
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            if !self.out.ends_with('\n') {
                self.out.push('\n');
            }
            self.out.push('\n');
        }
    }

    fn line_end(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        let decoded = decode_references(raw);
        if let Some(t) = &mut self.table {
            if let Some(c) = &mut t.cell {
                c.push_str(decoded.trim_matches('\n'));
            }
            return;
        }
        if self.pre {
            self.out.push_str(&decoded);
            return;
        }
        // Inside paragraphs the export's line ends are soft breaks; runs of blanks collapse. The line end the
        // export writes right after a link is dropped, so the text after the link follows it directly.
        let decoded = if std::mem::take(&mut self.after_link) { decoded.trim_start_matches(['\r', '\n']).to_string() } else { decoded };
        let mut last_space = self.out.ends_with(' ') || self.out.ends_with('\n') || self.out.is_empty();
        for c in decoded.chars() {
            if c == '\n' || c == '\r' || c == ' ' || c == '\t' {
                if !last_space {
                    self.out.push(' ');
                    last_space = true;
                }
            } else {
                self.out.push(c);
                last_space = false;
            }
        }
    }

    fn tag(&mut self, tag: &str) {
        let tag = tag.trim();
        let closing = tag.starts_with('/');
        let body = tag.trim_start_matches('/').trim_end_matches('/').trim();
        let name = body.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if closing {
                    self.blank_line();
                } else {
                    self.blank_line();
                    let n = name[1..].parse::<usize>().unwrap_or(1);
                    self.out.push_str(&"#".repeat(n));
                    self.out.push(' ');
                }
            }
            "p" | "div" => {
                // Inside a list the export writes paragraph tags around and between the items; they end the
                // item's line and do not open a blank line, so the list stays tight.
                if self.lists.is_empty() {
                    self.blank_line();
                } else {
                    self.line_end();
                }
            }
            "blockquote" => {
                self.blank_line();
                if !closing {
                    self.out.push_str("> ");
                }
            }
            "it" | "i" | "em" => self.out.push('*'),
            "b" | "strong" => self.out.push_str("**"),
            "br" => self.out.push_str("  \n"),
            "hr" => {
                self.blank_line();
                self.out.push_str("---");
                self.blank_line();
            }
            "ul" | "ol" => {
                if closing {
                    self.lists.pop();
                    if self.lists.is_empty() {
                        self.blank_line();
                    }
                } else {
                    if self.lists.is_empty() {
                        self.blank_line();
                    } else {
                        self.line_end();
                    }
                    self.lists.push((name == "ol", 1));
                }
            }
            "li" => {
                if closing {
                    self.line_end();
                } else {
                    self.line_end();
                    let depth = self.lists.len().saturating_sub(1);
                    self.out.push_str(&"    ".repeat(depth));
                    match self.lists.last_mut() {
                        Some((true, n)) => {
                            self.out.push_str(&format!("{n}. "));
                            *n += 1;
                        }
                        _ => self.out.push_str("- "),
                    }
                }
            }
            "pre" => {
                if closing {
                    self.pre = false;
                    self.line_end();
                    self.out.push_str("```");
                    self.blank_line();
                } else {
                    self.blank_line();
                    self.out.push_str("```\n");
                    self.pre = true;
                }
            }
            "code" | "tt" => {
                if !self.pre {
                    self.out.push('`');
                }
            }
            "a" => {
                if closing {
                    if let Some((url, start)) = self.link.take() {
                        let text = self.out[start..].to_string();
                        self.out.truncate(start);
                        if text.trim().is_empty() {
                            self.out.push_str(&url);
                        } else {
                            self.out.push_str(&format!("[{}]({url})", text.trim()));
                        }
                        self.after_link = true;
                    }
                } else if let Some(url) = attribute(body, "href") {
                    self.link = Some((url, self.out.len()));
                }
            }
            "table" => {
                if closing {
                    if let Some(t) = self.table.take() {
                        self.write_table(t);
                    }
                } else {
                    self.blank_line();
                    self.table = Some(Table::default());
                }
            }
            "tr" => {
                if let Some(t) = &mut self.table {
                    if closing {
                        if let Some(c) = t.cell.take() {
                            t.rows.last_mut().map(|r| r.push(c));
                        }
                    } else {
                        t.rows.push(Vec::new());
                    }
                }
            }
            "td" | "th" => {
                if let Some(t) = &mut self.table {
                    if closing {
                        if let Some(c) = t.cell.take() {
                            if t.rows.is_empty() {
                                t.rows.push(Vec::new());
                            }
                            t.rows.last_mut().map(|r| r.push(c));
                        }
                    } else {
                        if t.rows.is_empty() {
                            t.rows.push(Vec::new());
                        }
                        t.cell = Some(String::new());
                        if name == "th" && t.rows.len() == 1 {
                            t.header = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn write_table(&mut self, t: Table) {
        let width = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if width == 0 {
            return;
        }
        let row = |cells: &[String]| -> String {
            let mut s = String::from("|");
            for k in 0..width {
                let c = cells.get(k).map(|c| c.replace('|', "\\|").replace('\n', " ")).unwrap_or_default();
                s.push_str(&format!(" {} |", c.trim()));
            }
            s
        };
        let mut rows = t.rows.iter();
        if t.header {
            if let Some(first) = rows.next() {
                self.out.push_str(&row(first));
                self.out.push('\n');
            }
        } else {
            // A pipe table needs a header row: an empty one when the table has none.
            self.out.push_str(&row(&vec![String::new(); width]));
            self.out.push('\n');
        }
        self.out.push_str(&format!("|{}", "---|".repeat(width)));
        self.out.push('\n');
        for r in rows {
            self.out.push_str(&row(r));
            self.out.push('\n');
        }
        self.blank_line();
    }

    fn finish(mut self) -> String {
        if let Some(t) = self.table.take() {
            self.write_table(t);
        }
        while self.out.ends_with('\n') || self.out.ends_with(' ') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out
    }
}

/// The value of an attribute in a tag body, quoted with " or ' or bare.
fn attribute(body: &str, name: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let mut from = 0;
    while let Some(k) = lower[from..].find(name) {
        let at = from + k;
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = lower[at + name.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            let value = body[body.len() - after.len() + 1..].trim_start();
            return Some(match value.chars().next() {
                Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or("").to_string(),
                _ => value.split_whitespace().next().unwrap_or("").to_string(),
            });
        }
        from = at + name.len();
    }
    None
}

/// Numeric character references and the five named ones as characters; anything else left as written.
fn decode_references(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_html_becomes_markdown() {
        let html = b"<html>\n<body>\n<h1>Title</h1>\n<p>Some <it>text</it> and <b>strong</b> with a <a href=\"http://x.y/\">Site</a>&#x00e9;.\n</p><p>Second\nline</p>\n<ul><li>one</li><li>two</li></ul>\n<table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>\n<?$p`>gone tag text\n</body>\n</html>\n";
        let md = from_html(html);
        assert_eq!(md, "# Title\n\nSome *text* and **strong** with a [Site](http://x.y/)\u{e9}.\n\nSecond line\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\ngone tag text\n");
    }

    /// A Markdown document imported, exported as Markdown and imported again gives the same document, and the
    /// export of that is the same Markdown (the round trip settles after one pass).
    #[test]
    fn markdown_round_trip_settles() {
        use crate::import_chain::{self, Files, Recipe};
        use crate::rule_script::RuleScript;
        struct Shipped;
        fn tables() -> std::path::PathBuf {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables")
        }
        impl Files for Shipped {
            fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
                std::fs::read(tables().join("scan").join(name)).map_err(|e| e.to_string())
            }
            fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
                RuleScript::open(&tables().join("rule-scripts").join(name)).map_err(|e| e.to_string())
            }
            fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
                import_chain::UNIMAP_TABLES.iter().map(|n| std::fs::read(tables().join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| e.to_string())).collect()
            }
            fn word_map(&self, name: &str) -> Result<String, String> {
                std::fs::read(tables().join("word-styles").join(format!("{name}.map"))).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())
            }
        }
        let export = |md: &str| -> String {
            let got = import_chain::run(md.as_bytes(), &Recipe::Markdown(crate::word::Options::default(), 0), &Shipped);
            assert!(got.errors.is_empty(), "{:?}", got.errors);
            let names = crate::html_export::SCRIPTS;
            let mut scripts = [
                Shipped.rule_script(names[0]).unwrap(),
                Shipped.rule_script(names[1]).unwrap(),
                Shipped.rule_script(names[2]).unwrap(),
                Shipped.rule_script(names[3]).unwrap(),
            ];
            let html = crate::html_export::html(&got.text, &mut scripts).unwrap();
            from_html(&html)
        };
        let md = "# Title\n\nSome *text* and **strong** words, plus a [link](http://example.org/).\n\n- first\n- second\n\n## Second\n\nAnother paragraph.\n";
        let once = export(md);
        eprintln!("ROUND TRIP: {once:?}");
        assert!(once.contains("# Title") && once.contains("## Second") && once.contains("- first"), "{once}");
        let twice = export(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn references_and_attributes() {
        assert_eq!(decode_references("a &amp; b &#x2800; &#65; &unknown; c"), "a & b \u{2800} A &unknown; c");
        assert_eq!(attribute("a href='http://q/' class=\"x\"", "href").as_deref(), Some("http://q/"));
        assert_eq!(attribute("a class=\"x\"", "href"), None);
    }

    #[test]
    fn headings_paragraphs_lists_and_links_become_closed_tags() {
        let h = to_html("# Title\n\nSome *text* and a [link](http://example.org/).\n\n- a\n- b\n");
        assert_eq!(h, "<h1>Title</h1>\n<p>Some <em>text</em> and a <a href=\"http://example.org/\">link</a>.</p>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n");
    }

    #[test]
    fn the_dialects_of_d45() {
        let md = "---\ntitle: Notes\ntags: [a]\n---\n# Head\n\nArea $\\pi r^2$ here.\n\nSee [[Other note]] and [[Page|that page]].\n\n> [!note]\n> Body text.\n\n> [!tip]- Read this\n> More.\n\n```\n> [!warning] kept\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nText[^1].\n\n[^1]: The note.\n";
        let html = to_html(md);
        assert!(!html.contains("title: Notes"), "{html}");
        assert!(html.contains("<code>\\pi r^2</code>"), "{html}");
        assert!(html.contains("See Other note and that page."), "{html}");
        assert!(!html.contains("href=\"Other") && !html.contains("href=\"Page"), "{html}");
        assert!(html.contains("<blockquote>\n<p><strong>Note</strong></p>\n<p>Body text.</p>"), "{html}");
        assert!(html.contains("<p><strong>Read this</strong></p>\n<p>More.</p>"), "{html}");
        assert!(html.contains("&gt; [!warning] kept"), "{html}");
        assert!(html.contains("<td>1</td>") && html.contains("footnote-definition"), "{html}");
    }

    #[test]
    fn the_html_reads_as_xml() {
        let h = to_html("Line one  \nline two\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] task\n\nA note[^1].\n\n[^1]: The note.\n");
        let root = crate::docx::read_xml(&format!("<r>{h}</r>")).unwrap();
        assert!(root.elements("table").next().is_some());
        assert!(h.contains("<br />") && h.contains("<hr />"));
    }
}
