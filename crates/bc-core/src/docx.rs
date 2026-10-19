//! Word import (.docx), as the Word recipe runs it (spec 28 sections 3, 6 to 8, 10, 12 and 14): the
//! package's parts, the stylesheet's rules element by element, the run combining, the style map, the tag
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
    zip(entries, false)
}

/// A zip whose entries are deflated at the default level, each written as it is compressed; an entry whose name
/// ends with "/" is a folder and is stored empty.
pub fn deflated_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zip(entries, true)
}

/// The save time in UTC as a zip entry's DOS time and date: hours, minutes and seconds / 2; years from 1980, month
/// 1-12, day 1-31. A clock before 1980 gives 1980-01-01 00:00 (the earliest date a zip entry can hold).
pub fn dos_time_now() -> (u16, u16) {
    let (y, m, d, h, mi, s) = crate::ebraille::utc_now();
    if y < 1980 {
        return (0, (1 << 5) | 1);
    }
    let time = ((h as u16) << 11) | ((mi as u16) << 5) | (s as u16 / 2);
    let date = (((y - 1980).min(127) as u16) << 9) | ((m as u16) << 5) | d as u16;
    (time, date)
}

fn zip(entries: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
    // Every entry carries the save time as a valid DOS date (month 1-12, day 1-31); a zero date (1980-00-00) is not
    // a date and some readers refuse it.
    let (time, date) = dos_time_now();
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        let packed = (deflate && !name.ends_with('/')).then(|| {
            use std::io::Write;
            let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).and_then(|_| e.finish()).unwrap_or_default()
        });
        let (method, stored): (u16, &[u8]) = match &packed {
            Some(p) => (8, p.as_slice()),
            None => (0, data),
        };
        for (buf, sig) in [(&mut out, 0x0403_4b50u32), (&mut directory, 0x0201_4b50u32)] {
            buf.extend_from_slice(&sig.to_le_bytes());
            if sig == 0x0201_4b50 {
                buf.extend_from_slice(&20u16.to_le_bytes());
            }
            buf.extend_from_slice(&20u16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&method.to_le_bytes());
            buf.extend_from_slice(&time.to_le_bytes());
            buf.extend_from_slice(&date.to_le_bytes());
            buf.extend_from_slice(&crc.to_le_bytes());
            buf.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            if sig == 0x0201_4b50 {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes());
                // A folder entry carries the folder attribute.
                buf.extend_from_slice(&(if name.ends_with('/') { 0x10u32 } else { 0 }).to_le_bytes());
                buf.extend_from_slice(&offset.to_le_bytes());
            }
            buf.extend_from_slice(name_bytes);
        }
        out.extend_from_slice(stored);
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

/// One event of an XML text as the reader walks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A start tag: the element's name as written, its attributes in order, and whether the tag closes itself.
    Start { name: String, attrs: Vec<(String, String)>, self_closing: bool },
    End(String),
    /// Text with its entities decoded.
    Text(String),
    /// A CDATA section, as written.
    Cdata(String),
}

/// A reader of XML events over a text, one event at a time: it holds no copy of the text and builds no tree, so
/// a chain that reads element by element (the tag processor of the HTML chain) keeps only the event in hand.
/// It passes over the declaration, comments and processing instructions, decodes the five named entities and
/// numeric references, keeps names as written and does not check the document beyond what `read_xml` checks.
pub struct XmlReader<'a> {
    text: &'a str,
    pos: usize,
    peeked: Option<Event>,
}

impl<'a> XmlReader<'a> {
    pub fn new(text: &'a str) -> XmlReader<'a> {
        XmlReader { text, pos: 0, peeked: None }
    }

    /// The next event; None at the end of the text.
    pub fn next_event(&mut self) -> Result<Option<Event>, String> {
        if let Some(e) = self.peeked.take() {
            return Ok(Some(e));
        }
        loop {
            let rest = &self.text[self.pos..];
            if rest.is_empty() {
                return Ok(None);
            }
            let Some(after) = rest.strip_prefix('<') else {
                let end = rest.find('<').unwrap_or(rest.len());
                self.pos += end;
                return Ok(Some(Event::Text(decode_entities(&rest[..end]))));
            };
            if after.starts_with("!--") {
                self.pos += rest[4..].find("-->").map(|q| q + 4 + 3).ok_or("comment not closed")?;
            } else if after.starts_with("![CDATA[") {
                let end = rest[9..].find("]]>").map(|q| q + 9).ok_or("CDATA not closed")?;
                self.pos += end + 3;
                return Ok(Some(Event::Cdata(rest[9..end].to_string())));
            } else if after.starts_with('?') {
                self.pos += rest[2..].find("?>").map(|q| q + 2 + 2).ok_or("processing instruction not closed")?;
            } else if after.starts_with('!') {
                self.pos += rest.find('>').map(|q| q + 1).ok_or("declaration not closed")?;
            } else if let Some(end_tag) = after.strip_prefix('/') {
                let end = end_tag.find('>').ok_or("end tag not closed")?;
                let name = end_tag[..end].trim().to_string();
                self.pos += 2 + end + 1;
                return Ok(Some(Event::End(name)));
            } else {
                let (name, attrs, self_closing, taken) = start_tag(rest)?;
                self.pos += taken;
                return Ok(Some(Event::Start { name, attrs, self_closing }));
            }
        }
    }

    /// The next event without taking it.
    pub fn peek(&mut self) -> Result<Option<&Event>, String> {
        if self.peeked.is_none() {
            self.peeked = self.next_event()?;
        }
        Ok(self.peeked.as_ref())
    }
}

/// The root element of an XML text, built from the reader's events: text at the root that is only whitespace is
/// left out, and the last element at the root is the one given.
pub fn read_xml(text: &str) -> Result<Element, String> {
    let mut reader = XmlReader::new(text);
    let mut stack: Vec<Element> = vec![Element::default()];
    while let Some(event) = reader.next_event()? {
        match event {
            Event::Start { name, attrs, self_closing } => {
                let element = Element { name, attrs, children: Vec::new() };
                if self_closing {
                    stack.last_mut().ok_or("element with nothing open")?.children.push(Node::Element(element));
                } else {
                    stack.push(element);
                }
            }
            Event::End(name) => {
                let done = stack.pop().ok_or("end tag with nothing open")?;
                if done.name != name {
                    return Err(format!("end tag {name} closes {}", done.name));
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(done)),
                    None => return Err("end tag with nothing open".into()),
                }
            }
            Event::Text(t) => {
                if stack.len() > 1 || !t.trim().is_empty() {
                    push_text(&mut stack, t);
                }
            }
            Event::Cdata(t) => push_text(&mut stack, t),
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

fn push_text(stack: &mut [Element], text: String) {
    if let Some(top) = stack.last_mut() {
        if let Some(Node::Text(t)) = top.children.last_mut() {
            t.push_str(&text);
        } else {
            top.children.push(Node::Text(text));
        }
    }
}

/// The start tag at the start of `s`: the name, the attributes, whether the tag closes itself, and the length of
/// text it takes.
fn start_tag(s: &str) -> Result<(String, Vec<(String, String)>, bool, usize), String> {
    let name_end = s[1..].char_indices().find(|&(_, c)| c.is_whitespace() || c == '>' || c == '/').map(|(k, _)| k + 1).ok_or("start tag not closed")?;
    let name = s[1..name_end].to_string();
    let mut attrs = Vec::new();
    let mut q = name_end;
    loop {
        q += s[q..].len() - s[q..].trim_start().len();
        match s[q..].chars().next() {
            None => return Err("start tag not closed".into()),
            Some('>') => return Ok((name, attrs, false, q + 1)),
            Some('/') => {
                if s[q..].starts_with("/>") {
                    return Ok((name, attrs, true, q + 2));
                }
                return Err("slash in a start tag".into());
            }
            Some(_) => {
                let eq = s[q..].find('=').map(|k| q + k).ok_or("attribute without a value")?;
                let attr = s[q..eq].trim().to_string();
                let mut v = eq + 1;
                v += s[v..].len() - s[v..].trim_start().len();
                let quote = s[v..].chars().next().ok_or("attribute value cut short")?;
                if quote != '"' && quote != '\'' {
                    return Err(format!("attribute {attr} without quotes"));
                }
                let end = s[v + 1..].find(quote).map(|k| v + 1 + k).ok_or("attribute value not closed")?;
                attrs.push((attr, decode_entities(&s[v + 1..end])));
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

    /// Every entry of a written zip, in its local header and its directory entry, carries a valid DOS date and
    /// time (month 1-12, day 1-31, hour 0-23, minute 0-59, seconds 0-58).
    #[test]
    fn zip_entry_dates_are_valid_dos_dates() {
        let valid = |time: u16, date: u16| {
            let (month, day) = ((date >> 5) & 0x0f, date & 0x1f);
            let (hour, minute, half) = (time >> 11, (time >> 5) & 0x3f, time & 0x1f);
            (1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60 && half < 30
        };
        for zip in [stored_zip(&[("mimetype", b"application/epub+zip"), ("a/b.xml", b"<a/>")]), deflated_zip(&[("f/", b""), ("f/c.xml", b"<c/>")])] {
            let eocd = zip.len() - 22;
            let count = u16_at(&zip, eocd + 10).unwrap() as usize;
            let mut at = u32_at(&zip, eocd + 16).unwrap() as usize;
            assert_eq!(count, 2);
            for _ in 0..count {
                let (time, date) = (u16_at(&zip, at + 12).unwrap(), u16_at(&zip, at + 14).unwrap());
                assert!(valid(time, date), "directory entry: time {time:#06x} date {date:#06x}");
                let local = u32_at(&zip, at + 42).unwrap() as usize;
                let (time, date) = (u16_at(&zip, local + 10).unwrap(), u16_at(&zip, local + 12).unwrap());
                assert!(valid(time, date), "local header: time {time:#06x} date {date:#06x}");
                at += 46 + u16_at(&zip, at + 28).unwrap() as usize + u16_at(&zip, at + 30).unwrap() as usize + u16_at(&zip, at + 32).unwrap() as usize;
            }
        }
        let (time, date) = dos_time_now();
        assert!(valid(time, date) && date >> 9 >= 46, "{time:#06x} {date:#06x}");
    }

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
