//! Word import (.docx), as the original's Word recipe runs it (spec 28 sections 3, 6 to 8, 10, 12 and 14): the
//! package's parts, the stylesheet's rules ported element by element, the run combining, the style map, the tag
//! processor with its scrub rules, then the recipe's rule scripts and the shared steps. This file holds the package
//! reader (a zip whose parts are inflated) and the XML reader the parts go through.

use std::io::Read;

// ------------------------------------------------------------------ the package (zip)

/// A part of the package: its name as the zip names it and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub bytes: Vec<u8>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]))
}

/// The parts of a zip file (a .docx), read from its central directory: stored and deflated entries; any other
/// method fails. A file with no central directory is not a zip.
pub fn parts(bytes: &[u8]) -> Result<Vec<Part>, String> {
    // The end of central directory record, searched backward (a comment may follow it).
    let eocd = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|&k| bytes[k..k + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or("This is not a Word document (no zip directory).")?;
    let count = u16_at(bytes, eocd + 10).ok_or("zip directory cut short")? as usize;
    let mut at = u32_at(bytes, eocd + 16).ok_or("zip directory cut short")? as usize;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, at) != Some(0x0201_4b50) {
            return Err("zip directory entry expected".into());
        }
        let method = u16_at(bytes, at + 10).ok_or("zip entry cut short")?;
        let compressed = u32_at(bytes, at + 20).ok_or("zip entry cut short")? as usize;
        let size = u32_at(bytes, at + 24).ok_or("zip entry cut short")? as usize;
        let name_len = u16_at(bytes, at + 28).ok_or("zip entry cut short")? as usize;
        let extra_len = u16_at(bytes, at + 30).ok_or("zip entry cut short")? as usize;
        let comment_len = u16_at(bytes, at + 32).ok_or("zip entry cut short")? as usize;
        let local = u32_at(bytes, at + 42).ok_or("zip entry cut short")? as usize;
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + name_len).ok_or("zip entry cut short")?).into_owned();
        at += 46 + name_len + extra_len + comment_len;
        // The local header before the data has its own name and extra lengths.
        if u32_at(bytes, local) != Some(0x0403_4b50) {
            return Err(format!("zip local header of {name} expected"));
        }
        let ln = u16_at(bytes, local + 26).ok_or("zip header cut short")? as usize;
        let le = u16_at(bytes, local + 28).ok_or("zip header cut short")? as usize;
        let data = bytes.get(local + 30 + ln + le..local + 30 + ln + le + compressed).ok_or_else(|| format!("zip data of {name} cut short"))?;
        let bytes = match method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(size);
                flate2::read::DeflateDecoder::new(data).read_to_end(&mut out).map_err(|e| format!("{name}: {e}"))?;
                out
            }
            m => return Err(format!("{name}: zip method {m} is not supported")),
        };
        out.push(Part { name, bytes });
    }
    Ok(out)
}

/// The bytes of a part by name (the zip's names, forward slashes), or None.
pub fn part<'a>(parts: &'a [Part], name: &str) -> Option<&'a [u8]> {
    parts.iter().find(|p| p.name == name).map(|p| p.bytes.as_slice())
}

/// A zip with stored (uncompressed) entries, for tests and for files written here.
pub fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        for (buf, sig) in [(&mut out, 0x0403_4b50u32), (&mut directory, 0x0201_4b50u32)] {
            buf.extend_from_slice(&sig.to_le_bytes());
            if sig == 0x0201_4b50 {
                buf.extend_from_slice(&20u16.to_le_bytes());
            }
            buf.extend_from_slice(&20u16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&crc.to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            if sig == 0x0201_4b50 {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&0u32.to_le_bytes());
                buf.extend_from_slice(&offset.to_le_bytes());
            }
            buf.extend_from_slice(name_bytes);
        }
        out.extend_from_slice(data);
    }
    let dir_offset = out.len() as u32;
    let dir_len = directory.len() as u32;
    out.extend_from_slice(&directory);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&dir_len.to_le_bytes());
    out.extend_from_slice(&dir_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

// ------------------------------------------------------------------ XML

/// An element of an XML part: its name as written (prefix included), its attributes in order and its children.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
    /// A code instruction the run combining writes (<?bc ...?>); the reader makes none.
    Pi(String),
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    /// The child elements named `name`.
    pub fn elements<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a Element> + 'a {
        let name = name.to_string();
        self.children.iter().filter_map(move |c| match c {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements(name).next()
    }

    /// The element at a path of child names.
    pub fn at(&self, path: &[&str]) -> Option<&Element> {
        path.iter().try_fold(self, |e, n| e.element(n))
    }

    /// Every element child.
    pub fn element_children(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// The text of the element and everything inside it.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for c in &self.children {
            match c {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => e.collect_text(out),
                Node::Pi(_) => {}
            }
        }
    }

    /// Every element in the tree, depth first, that `keep` accepts.
    pub fn find_all<'a>(&'a self, keep: &dyn Fn(&Element) -> bool, out: &mut Vec<&'a Element>) {
        for e in self.element_children() {
            if keep(e) {
                out.push(e);
            }
            e.find_all(keep, out);
        }
    }
}

/// The root element of an XML text. The reader takes the declaration, comments, processing instructions and CDATA,
/// the five named entities and numeric references; it keeps names as written and does not check the document
/// beyond matching end tags.
pub fn read_xml(text: &str) -> Result<Element, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut p = 0;
    let mut stack: Vec<Element> = vec![Element::default()];
    while p < chars.len() {
        if chars[p] == '<' {
            if chars[p..].starts_with(&['<', '!', '-', '-']) {
                p = find(&chars, p + 4, "-->").map(|q| q + 3).ok_or("comment not closed")?;
            } else if chars[p..].starts_with(&['<', '!', '[', 'C', 'D', 'A', 'T', 'A', '[']) {
                let end = find(&chars, p + 9, "]]>").ok_or("CDATA not closed")?;
                push_text(&mut stack, chars[p + 9..end].iter().collect());
                p = end + 3;
            } else if chars[p..].starts_with(&['<', '?']) {
                p = find(&chars, p + 2, "?>").map(|q| q + 2).ok_or("processing instruction not closed")?;
            } else if chars[p..].starts_with(&['<', '!']) {
                p = chars[p..].iter().position(|&c| c == '>').map(|q| p + q + 1).ok_or("declaration not closed")?;
            } else if chars[p..].starts_with(&['<', '/']) {
                let end = chars[p..].iter().position(|&c| c == '>').map(|q| p + q).ok_or("end tag not closed")?;
                let name: String = chars[p + 2..end].iter().collect::<String>().trim().to_string();
                let done = stack.pop().ok_or("end tag with nothing open")?;
                if done.name != name {
                    return Err(format!("end tag {name} closes {}", done.name));
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(done)),
                    None => return Err("end tag with nothing open".into()),
                }
                p = end + 1;
            } else {
                let (element, close, next) = start_tag(&chars, p)?;
                if close {
                    stack.last_mut().ok_or("element with nothing open")?.children.push(Node::Element(element));
                } else {
                    stack.push(element);
                }
                p = next;
            }
        } else {
            let end = chars[p..].iter().position(|&c| c == '<').map_or(chars.len(), |q| p + q);
            let raw: String = chars[p..end].iter().collect();
            if stack.len() > 1 || !raw.trim().is_empty() {
                push_text(&mut stack, decode_entities(&raw));
            }
            p = end;
        }
    }
    let mut root = stack.pop().ok_or("no root element")?;
    if !stack.is_empty() {
        return Err(format!("element {} not closed", root.name));
    }
    root.children.retain(|c| matches!(c, Node::Element(_)));
    match root.children.pop() {
        Some(Node::Element(e)) => Ok(e),
        _ => Err("no root element".into()),
    }
}

fn find(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let pat: Vec<char> = pat.chars().collect();
    (from..chars.len().saturating_sub(pat.len() - 1)).find(|&k| chars[k..k + pat.len()] == pat[..])
}

fn push_text(stack: &mut [Element], text: String) {
    if let Some(top) = stack.last_mut() {
        if let Some(Node::Text(t)) = top.children.last_mut() {
            t.push_str(&text);
        } else {
            top.children.push(Node::Text(text));
        }
    }
}

/// A start tag at `p`: the element with its attributes, whether it closes itself, and the place after it.
fn start_tag(chars: &[char], p: usize) -> Result<(Element, bool, usize), String> {
    let mut q = p + 1;
    let name_end = (q..chars.len()).find(|&k| chars[k].is_whitespace() || chars[k] == '>' || chars[k] == '/').ok_or("start tag not closed")?;
    let mut element = Element { name: chars[q..name_end].iter().collect(), ..Element::default() };
    q = name_end;
    loop {
        while q < chars.len() && chars[q].is_whitespace() {
            q += 1;
        }
        match chars.get(q) {
            None => return Err("start tag not closed".into()),
            Some('>') => return Ok((element, false, q + 1)),
            Some('/') => {
                if chars.get(q + 1) == Some(&'>') {
                    return Ok((element, true, q + 2));
                }
                return Err("slash in a start tag".into());
            }
            Some(_) => {
                let eq = (q..chars.len()).find(|&k| chars[k] == '=').ok_or("attribute without a value")?;
                let name: String = chars[q..eq].iter().collect::<String>().trim().to_string();
                let mut v = eq + 1;
                while v < chars.len() && chars[v].is_whitespace() {
                    v += 1;
                }
                let quote = *chars.get(v).ok_or("attribute value cut short")?;
                if quote != '"' && quote != '\'' {
                    return Err(format!("attribute {name} without quotes"));
                }
                let end = (v + 1..chars.len()).find(|&k| chars[k] == quote).ok_or("attribute value not closed")?;
                let raw: String = chars[v + 1..end].iter().collect();
                element.attrs.push((name, decode_entities(&raw)));
                q = end + 1;
            }
        }
    }
}

/// The five named entities and numeric references decoded; anything else stays as written.
pub fn decode_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest.find(';').filter(|&e| e <= 12) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16).ok().and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
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

/// A part of the package read as XML.
pub fn xml_part(parts: &[Part], name: &str) -> Result<Option<Element>, String> {
    match part(parts, name) {
        None => Ok(None),
        Some(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
            read_xml(text).map(Some).map_err(|e| format!("{name}: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_zip_is_read_back_and_a_deflated_part_inflated() {
        let doc = b"<w:document><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>";
        let zip = stored_zip(&[("word/document.xml", doc), ("[Content_Types].xml", b"<Types/>")]);
        let ps = parts(&zip).unwrap();
        assert_eq!(ps.len(), 2);
        assert_eq!(part(&ps, "word/document.xml"), Some(&doc[..]));
        // A deflated entry, made with the same crate.
        let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, doc).unwrap();
        let deflated = encoder.finish().unwrap();
        let mut zip = stored_zip(&[("word/document.xml", &deflated)]);
        // Mark the entry deflated in both headers (method 8).
        zip[8] = 8;
        let dir = zip.len() - 22 - 46 - "word/document.xml".len();
        zip[dir + 10] = 8;
        zip[dir + 24..dir + 28].copy_from_slice(&(doc.len() as u32).to_le_bytes());
        let ps = parts(&zip).unwrap();
        assert_eq!(part(&ps, "word/document.xml"), Some(&doc[..]));
        assert!(parts(b"not a zip at all").is_err());
    }

    #[test]
    fn xml_elements_attributes_text_and_entities_are_read() {
        let e = read_xml("<?xml version=\"1.0\"?><!-- c --><w:p w:a='1' b=\"x&amp;y\"><w:t xml:space=\"preserve\"> a&lt;b &#65;&#x42; </w:t><w:br/><![CDATA[<raw>]]></w:p>").unwrap();
        assert_eq!(e.name, "w:p");
        assert_eq!(e.attr("w:a"), Some("1"));
        assert_eq!(e.attr("b"), Some("x&y"));
        assert_eq!(e.element("w:t").unwrap().text(), " a<b AB ");
        assert!(e.element("w:br").is_some());
        assert_eq!(e.text(), " a<b AB <raw>");
        assert!(read_xml("<a><b></a>").is_err());
    }
}
