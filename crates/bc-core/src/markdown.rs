//! Markdown import (BrailleCraft's own addition; the reference program has no Markdown import): a Markdown file is
//! turned into HTML and then goes through the HTML import chain, so a Markdown document imports exactly as the
//! same content would import from HTML.

use pulldown_cmark::{html, Options, Parser};

/// The HTML of a Markdown text: CommonMark with tables, footnotes, strikethrough and task lists. Headings become
/// h1 to h6, paragraphs p, lists ul and ol with li, emphasis em and strong, code and pre, links a, images their
/// alt text inside img, block quotes blockquote, rules hr, line breaks br, tables table, thead, tbody, tr, th and
/// td, footnotes sup and div; every tag is closed, so the HTML reads as XML.
pub fn to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, options);
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut out, parser);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_paragraphs_lists_and_links_become_closed_tags() {
        let h = to_html("# Title\n\nSome *text* and a [link](http://example.org/).\n\n- a\n- b\n");
        assert_eq!(h, "<h1>Title</h1>\n<p>Some <em>text</em> and a <a href=\"http://example.org/\">link</a>.</p>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n");
    }

    #[test]
    fn the_html_reads_as_xml() {
        let h = to_html("Line one  \nline two\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] task\n\nA note[^1].\n\n[^1]: The note.\n");
        let root = crate::docx::read_xml(&format!("<r>{h}</r>")).unwrap();
        assert!(root.elements("table").next().is_some());
        assert!(h.contains("<br />") && h.contains("<hr />"));
    }
}
