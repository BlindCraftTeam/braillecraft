//! The bytes an embossing job sends (re/spec/25-emboss.md section 4): the job start, each copy's sides (pages) line
//! by line through a 128-entry character table, the page ends, and the job end. The pages come in as North American
//! ASCII braille (the program's braille text), one string per line, None for a blank side.

/// North American ASCII braille: the character for each cell, by dot pattern (dot 1 = 1, dot 2 = 2, dot 3 = 4,
/// dot 4 = 8, dot 5 = 16, dot 6 = 32). The published standard order.
pub const NA_CELLS: &[u8; 64] = b" A1B'K2L@CIF/MSP\"E3H9O6R^DJG>NTQ,*5<-U8V.%[$+X!&;:4\\0Z7(_?W]#Y)=";

/// How a line ends (End of line indicator; the Puma encoding has its own).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnd {
    #[default]
    CrLf,
    Cr,
    Lf,
    Puma,
}

/// Everything a job needs from the embosser setup and model.
#[derive(Debug, Clone)]
pub struct Options {
    /// Cells per line and lines per page of the braille (without the binding margin and the top margin).
    pub cells: u32,
    pub lines: u32,
    /// Blank lines before each page's first line.
    pub top_margin: u32,
    /// Cells of binding margin (the form's cell offset included).
    pub binding: u32,
    pub interpoint: bool,
    /// The model sets its margins by command, so no left padding is sent.
    pub margins_by_command: bool,
    /// North American output: letters sent upper case; punctuation sent high (` { | } ~ for @ [ \ ] ^).
    pub letters_upper: bool,
    pub high_punctuation: bool,
    /// Another encoding: the byte sent for each cell.
    pub encoding: Option<[u8; 64]>,
    pub line_end: LineEnd,
    /// End of page indicator, 0 to 6 (section 4d).
    pub end_of_page: u8,
    /// Send the line end after a full line; send the page end after a full page.
    pub eol_on_full_line: bool,
    pub eop_on_full_page: bool,
    /// Pad short lines with spaces to the full width.
    pub pad_lines: bool,
    /// Add a blank side to an odd number of interpoint sides.
    pub make_up_sides: bool,
    /// The job start sequence (the driver's, then the custom codes) and the custom end codes.
    pub start: Vec<u8>,
    pub end: Vec<u8>,
    pub control_d: bool,
    pub control_z: bool,
    /// Bytes sent after the job before the output closes.
    pub close_sync: Vec<u8>,
    /// The embosser makes the copies itself: the pages are sent once.
    pub copies_by_embosser: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            cells: 40,
            lines: 25,
            top_margin: 0,
            binding: 0,
            interpoint: false,
            margins_by_command: false,
            letters_upper: false,
            high_punctuation: false,
            encoding: None,
            line_end: LineEnd::CrLf,
            end_of_page: 0,
            eol_on_full_line: true,
            eop_on_full_page: true,
            pad_lines: false,
            make_up_sides: true,
            start: Vec::new(),
            end: Vec::new(),
            control_d: false,
            control_z: false,
            close_sync: Vec::new(),
            copies_by_embosser: false,
        }
    }
}

/// The 128-entry table every byte goes through (section 4b): North American output with its letter case and
/// punctuation settings, or each character's cell sent as the chosen encoding's byte. Control codes pass unchanged.
pub fn char_table(o: &Options) -> [u8; 128] {
    let mut t = [0u8; 128];
    for (i, b) in t.iter_mut().enumerate() {
        *b = i as u8;
    }
    match &o.encoding {
        None => {
            for c in b'a'..=b'z' {
                if o.letters_upper {
                    t[c as usize] = c.to_ascii_uppercase();
                }
            }
            for c in b'A'..=b'Z' {
                if !o.letters_upper {
                    t[c as usize] = c.to_ascii_lowercase();
                }
            }
            let pairs = [(b'@', b'`'), (b'[', b'{'), (b'\\', b'|'), (b']', b'}'), (b'^', b'~')];
            for (low, high) in pairs {
                if o.high_punctuation {
                    t[low as usize] = high;
                } else {
                    t[high as usize] = low;
                }
            }
            if !o.high_punctuation {
                t[0x7f] = b'_';
            }
        }
        Some(codes) => {
            for c in 0x20u8..0x80 {
                // 0x60 to 0x7f fold onto 0x40 to 0x5f, as lower case onto upper case.
                let k = if c >= 0x60 { c - 0x20 } else { c };
                if let Some(cell) = NA_CELLS.iter().position(|&n| n == k) {
                    t[c as usize] = codes[cell];
                }
            }
        }
    }
    t
}

/// Custom job codes as the embosser setup takes them: text with <esc> <lf> <ff> <cr> <ctrl-@> <ctrl-A> to
/// <ctrl-Z> <lt> <gt> standing for those bytes. An unknown <...> is sent as written.
pub fn parse_codes(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.extend_from_slice(rest[..start].as_bytes());
        let after = &rest[start..];
        let Some(end) = after.find('>') else {
            out.extend_from_slice(after.as_bytes());
            return out;
        };
        let name = after[1..end].to_ascii_lowercase();
        let byte = match name.as_str() {
            "esc" => Some(0x1b),
            "lf" => Some(0x0a),
            "ff" => Some(0x0c),
            "cr" => Some(0x0d),
            "lt" => Some(b'<'),
            "gt" => Some(b'>'),
            "ctrl-@" => Some(0),
            n => n.strip_prefix("ctrl-").and_then(|l| {
                let b = l.as_bytes();
                (b.len() == 1 && b[0].is_ascii_lowercase()).then(|| b[0] - b'a' + 1)
            }),
        };
        match byte {
            Some(b) => out.push(b),
            None => out.extend_from_slice(after[..=end].as_bytes()),
        }
        rest = &after[end + 1..];
    }
    out.extend_from_slice(rest.as_bytes());
    out
}

struct Writer<'a> {
    o: &'a Options,
    t: [u8; 128],
    out: Vec<u8>,
    /// End of page mode 1: the line end of the last line is held back until another line follows.
    held: bool,
}

impl Writer<'_> {
    fn byte(&mut self, b: u8) {
        self.out.push(self.t[(b & 0x7f) as usize]);
    }

    fn line_end(&mut self) {
        match self.o.line_end {
            LineEnd::CrLf => {
                self.byte(0x0d);
                self.byte(0x0a);
            }
            LineEnd::Cr => self.byte(0x0d),
            LineEnd::Lf => self.byte(0x0a),
            LineEnd::Puma => self.out.push(0x81),
        }
    }

    fn release(&mut self) {
        if self.held {
            self.held = false;
            self.line_end();
        }
    }

    /// One line of a side; `odd` is an odd (front) side.
    fn line(&mut self, text: &str, odd: bool) {
        self.release();
        let o = self.o;
        // INFERENCE: a blank line gets no margin padding (the reference's padding of an empty line was not traced).
        let pad = if o.margins_by_command || (o.interpoint && !odd) || text.is_empty() { 0 } else { o.binding };
        for _ in 0..pad {
            self.byte(b' ');
        }
        let cells: Vec<u8> = text.chars().map(|c| if c.is_ascii() { c as u8 } else { b' ' }).collect();
        for &c in &cells {
            self.byte(c);
        }
        let len = cells.len() as u32;
        if o.pad_lines {
            for _ in len..o.cells {
                self.byte(b' ');
            }
        }
        if len >= o.cells && !o.eol_on_full_line {
            return;
        }
        if o.end_of_page == 1 {
            self.held = true;
        } else {
            self.line_end();
        }
    }

    /// The end of a side that sent `sent` lines.
    fn page_end(&mut self, sent: u32) {
        let o = self.o;
        let full = sent >= o.lines;
        if full && !o.eop_on_full_page {
            self.release();
            return;
        }
        match o.end_of_page {
            1 => {
                self.held = false;
                self.byte(0x0c);
            }
            2 | 6 => {
                self.release();
                for _ in sent..o.lines {
                    self.line_end();
                }
                if o.end_of_page == 6 {
                    self.byte(0x12);
                }
            }
            3 => {
                self.release();
                self.byte(0x0b);
            }
            4 => self.release(),
            5 => {
                self.release();
                self.out.push(0x83);
            }
            _ => {
                self.release();
                self.byte(0x0c);
            }
        }
    }

    fn side(&mut self, page: Option<&[String]>, odd: bool) {
        let o = self.o;
        // The top margin is blank lines on every model; setting the margins by command takes away only the left
        // padding (spec 25 4c).
        for _ in 0..o.top_margin {
            self.line_end();
        }
        match page {
            Some(lines) => {
                // Trailing blank lines are not sent; a page with no text is a blank side.
                let last = lines.iter().rposition(|l| !l.trim().is_empty());
                match last {
                    Some(last) => {
                        for l in &lines[..=last] {
                            self.line(l.trim_end(), odd);
                        }
                        self.page_end(last as u32 + 1);
                    }
                    None => self.blank_side(),
                }
            }
            None => self.blank_side(),
        }
    }

    /// A blank side: two spaces and a line end, then the page end, so the embosser feeds the sheet (section 4d).
    fn blank_side(&mut self) {
        self.out.extend_from_slice(b"  ");
        self.line_end();
        self.page_end(1);
    }
}

/// The bytes of a job: `pages` (each a list of lines; None for a blank side) embossed `copies` times.
pub fn job(o: &Options, pages: &[Option<Vec<String>>], copies: u32) -> Vec<u8> {
    let mut w = Writer { o, t: char_table(o), out: Vec::new(), held: false };
    w.out.extend_from_slice(&o.start);
    if o.end_of_page == 6 {
        w.byte(0x12);
    }
    let mut sides: Vec<Option<&[String]>> = pages.iter().map(|p| p.as_deref()).collect();
    if o.interpoint && sides.len() % 2 == 1 && o.make_up_sides {
        sides.push(None);
    }
    let copies = if o.copies_by_embosser { 1 } else { copies.max(1) };
    for _ in 0..copies {
        for (i, side) in sides.iter().enumerate() {
            w.side(*side, i % 2 == 0);
        }
    }
    w.release();
    w.out.extend_from_slice(&o.end);
    if o.control_d {
        w.out.push(0x04);
    }
    if o.control_z {
        w.out.push(0x1a);
    }
    w.out.extend_from_slice(&o.close_sync);
    w.out
}

/// What the Index job start is built from (spec 25 section 8.1).
#[derive(Debug, Clone, Default)]
pub struct IndexStart {
    /// The driver level: below 4 no parameter string is built. INFERENCE: 4, 5 and 6 are the Index generations the
    /// model list names V3, V4 and V5.
    pub level: u8,
    /// The form's width and length (tenths of an inch), its largest cells per line, its paper id and whether it is
    /// a folded booklet.
    pub form_width: u32,
    pub form_length: u32,
    pub form_cells: u32,
    pub form_id: String,
    pub booklet: bool,
    /// Width and length are swapped (INFERENCE: landscape or rotated paper).
    pub rotated: bool,
    /// The dimensions go out in millimetres (INFERENCE: the form is in tenths of an inch).
    pub to_mm: bool,
    /// Sideways paper names (_side, _landscape) choose their own double page codes at level 5 and above.
    pub sideways_codes: bool,
    /// The alternative double page codes 3 and 5 (INFERENCE: Z-fold paper).
    pub z_fold: bool,
    /// The 8-dot line spacing (LS100).
    pub eight_dot: bool,
    pub cells: u32,
    pub binding: u32,
    pub lines: u32,
    /// Lines added to LP.
    pub extra_lines: u32,
    pub interpoint: bool,
    pub copies: u32,
    /// The model's field list (InitSequence), empty for none.
    pub sequence: String,
}

/// A form dimension as the Index start sends it.
fn index_dimension(v: u32, to_mm: bool) -> u32 {
    if to_mm {
        match (v * 254 + 50) / 100 {
            147 => 148,
            211 => 210,
            419 => 420,
            mm => mm,
        }
    } else if v % 10 == 5 {
        v - 2
    } else {
        v
    }
}

/// The Index parameter fields, in the driver's own order.
pub fn index_fields(s: &IndexStart) -> Vec<String> {
    if !(4..=6).contains(&s.level) {
        return Vec::new();
    }
    let double = s.interpoint;
    let sideways = s.form_id.contains("_side") || s.form_id.contains("_landscape");
    let dp = if s.booklet {
        if !double && s.level >= 5 { '8' } else { '4' }
    } else if s.sideways_codes && s.level >= 5 && sideways {
        if double { '6' } else { '7' }
    } else if s.z_fold {
        if double { '3' } else { '5' }
    } else if double {
        '2'
    } else {
        '1'
    };
    let (w, l) = if s.rotated { (s.form_length, s.form_width) } else { (s.form_width, s.form_length) };
    let (pw, pl) = (index_dimension(w, s.to_mm), index_dimension(l, s.to_mm));
    let mut f = vec![format!("DP{dp}")];
    if s.level == 4 {
        let (im, om, tm) = if s.booklet {
            let om = (index_dimension(s.form_width, s.to_mm) / 12).saturating_sub(2 + s.binding + s.cells);
            (s.binding + 2, om, s.lines + 1)
        } else {
            (0, s.form_cells.saturating_sub(s.cells + s.binding), 0)
        };
        f.extend([format!("PW{pw}"), format!("PL{pl}"), format!("IM{im}"), format!("OM{om}"), format!("TM{tm}"), "BM0".into()]);
    } else {
        let ls = if s.eight_dot { 100 } else { 50 };
        f.extend([
            format!("LS{ls}"),
            format!("CH{}", s.cells),
            format!("BI{}", s.binding),
            format!("LP{}", s.lines + s.extra_lines),
            "TM0".into(),
        ]);
    }
    if s.copies > 1 {
        f.push(format!("MC{}", s.copies));
    }
    f
}

/// The Index job start: ESC D, the fields separated by commas, and a semicolon. With the model's field list the
/// fields go out in its order: a bare name takes the field built here, a field written with its value is sent as
/// written. INFERENCE: the merge with the model's list was seen, not traced field by field; this is the reading
/// that gives the built order for the lists the models carry. Fields MC and MC1 are never sent.
pub fn index_start(s: &IndexStart) -> Vec<u8> {
    let built = index_fields(s);
    if built.is_empty() {
        return Vec::new();
    }
    let fields: Vec<String> = if s.sequence.trim().is_empty() {
        built
    } else {
        s.sequence
            .split(',')
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .filter_map(|f| {
                if f.len() == 2 && f.bytes().all(|b| b.is_ascii_uppercase()) {
                    built.iter().find(|b| b.starts_with(f)).cloned()
                } else {
                    Some(f.to_string())
                }
            })
            .collect()
    };
    let fields: Vec<&String> = fields.iter().filter(|f| *f != "MC" && *f != "MC1").collect();
    if fields.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0x1b, b'D'];
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        out.extend_from_slice(f.as_bytes());
    }
    out.push(b';');
    out
}

/// What the Enabling job start is built from (spec 25 section 8.2).
#[derive(Debug, Clone, Default)]
pub struct EnablingStart {
    /// The start of a job (the whole set); otherwise only the side is chosen again.
    pub full: bool,
    /// The ESC i argument.
    pub side: u8,
    pub seek_top_of_form: bool,
    /// The model speaks (ET Speaks), and its voice is left on.
    pub voice: bool,
    pub voice_on: bool,
    /// An 8-dot table is in use, and the 8-dot case of it.
    pub dot_table: bool,
    pub eight_dot: bool,
    /// Perforation skip and the interpoint side choice (one setup flag, not traced), and interpoint itself.
    pub perforation_skip: bool,
    pub interpoint: bool,
    pub label_mode: bool,
    /// Form length (tenths of an inch), lines per page, cells and binding margin.
    pub form_length: u32,
    pub lines: u32,
    pub cells: u32,
    pub binding: u32,
    /// The setup's init string; a <Tag> in it takes that code's place.
    pub init: String,
}

/// The Enabling job start: each code in the driver's order, sent when its condition holds, or put in place of its
/// <Tag> where the init string holds that tag; then the init string.
pub fn enabling_start(e: &EnablingStart) -> Vec<u8> {
    const ESC: u8 = 0x1b;
    let pick = |on: bool, a: u8, b: u8| if on { b } else { a };
    let mut init = e.init.clone().into_bytes();
    let mut out = Vec::new();
    let items: [(&str, bool, Vec<u8>); 18] = [
        ("SeekTOF", e.full && e.seek_top_of_form, vec![ESC, b'g']),
        ("VoiceOff", e.full && e.voice, vec![ESC, b'X', b'@']),
        ("BrailleMode", e.full, vec![ESC, b'K', pick(e.eight_dot, b'@', b'A')]),
        ("IgnoreDot7And8", e.dot_table, vec![ESC, pick(e.eight_dot, b'[', b'2')]),
        ("WordWrap", true, vec![ESC, b'W', b'@']),
        ("HotZone", true, vec![ESC, b'H', b'@']),
        ("ASCII", true, vec![ESC, b'&', b'@']),
        ("PerfSkip", e.full && e.perforation_skip, vec![ESC, b'S', pick(e.interpoint, b'@', b'A')]),
        ("LabelMode", e.full && e.label_mode, vec![ESC, b'j', b'A']),
        ("DBSMode", e.dot_table, vec![ESC, b's', b'@']),
        ("FormLength", true, vec![ESC, b'T', (e.form_length / 10 + 0x40) as u8]),
        ("LinesPerPage", true, vec![ESC, b'Q', (e.lines + 0x40) as u8]),
        ("LeftMargin", true, vec![ESC, b'L', b'A']),
        ("RightMargin", true, vec![ESC, b'R', (e.cells + e.binding + 0x40) as u8]),
        ("Interpoint", e.full && e.perforation_skip, vec![ESC, b'i', e.side]),
        ("SideSelection", !e.full, vec![ESC, b'i', e.side]),
        ("VoiceOff", e.full && e.voice && !e.voice_on, vec![ESC, b'X', b'@']),
        ("VoiceOn", e.full && e.voice && e.voice_on, vec![ESC, b'X', b'A']),
    ];
    for (tag, send, code) in items {
        let tag = format!("<{tag}>").into_bytes();
        if let Some(at) = init.windows(tag.len()).position(|w| w == tag.as_slice()) {
            init.splice(at..at + tag.len(), code);
        } else if send {
            out.extend_from_slice(&code);
        }
    }
    out.extend_from_slice(&init);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ESC D and the Index fields after it.
    fn esc_d(fields: &[u8]) -> Vec<u8> {
        let mut v = vec![0x1b, b'D'];
        v.extend_from_slice(fields);
        v
    }

    #[test]
    fn index_start_levels() {
        // V3 (level 4) on 11.5 by 11 inch paper, 40 cells of 42, single sided, with the model's field list.
        let v3 = IndexStart {
            level: 4,
            form_width: 115,
            form_length: 110,
            form_cells: 42,
            form_id: "11_w".into(),
            to_mm: true,
            cells: 40,
            lines: 25,
            copies: 1,
            sequence: "BT0,DP,PW,PL,IM,OM,TM,BM,MC".into(),
            ..IndexStart::default()
        };
        assert_eq!(index_start(&v3), esc_d(b"BT0,DP1,PW292,PL279,IM0,OM2,TM0,BM0;"));
        // Two copies add MC2; interpoint is DP2; without a field list the built order is sent.
        let two = IndexStart { copies: 2, interpoint: true, sequence: String::new(), ..v3.clone() };
        assert_eq!(index_start(&two), esc_d(b"DP2,PW292,PL279,IM0,OM2,TM0,BM0,MC2;"));
        // V5 (level 6): line spacing, cells, binding, lines; LS50 written in the list is sent as written.
        let v5 = IndexStart { level: 6, binding: 2, sequence: "BT0,TD0,PN0,DP,LS50,TM,BI,CH,LP,MC".into(), ..v3.clone() };
        assert_eq!(index_start(&v5), esc_d(b"BT0,TD0,PN0,DP1,LS50,TM0,BI2,CH40,LP25;"));
        // A booklet single sided at level 5 is DP8; V1 and V2 build nothing.
        assert_eq!(index_fields(&IndexStart { booklet: true, ..v5.clone() })[0], "DP8");
        assert!(index_start(&IndexStart { level: 3, ..v3 }).is_empty());
        // Millimetre fixes and the tenths rule.
        assert_eq!((index_dimension(58, true), index_dimension(83, true), index_dimension(165, true)), (148, 210, 420));
        assert_eq!((index_dimension(115, false), index_dimension(110, false)), (113, 110));
    }

    #[test]
    fn enabling_start_codes() {
        let e = EnablingStart { full: true, form_length: 110, lines: 25, cells: 40, binding: 2, ..EnablingStart::default() };
        let out = enabling_start(&e);
        assert_eq!(out, b"\x1bK@\x1bW@\x1bH@\x1b&@\x1bTK\x1bQY\x1bLA\x1bRj".to_vec());
        // A tag in the init string takes its code's place there; a later copy only chooses the side.
        let tagged = EnablingStart { init: "<FormLength>!".into(), ..e.clone() };
        assert_eq!(enabling_start(&tagged), b"\x1bK@\x1bW@\x1bH@\x1b&@\x1bQY\x1bLA\x1bRj\x1bTK!".to_vec());
        let later = EnablingStart { full: false, side: b'A', ..e };
        assert!(enabling_start(&later).ends_with(b"\x1biA"));
    }

    fn page(lines: &[&str]) -> Option<Vec<String>> {
        Some(lines.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn table_cases_and_punctuation() {
        let o = Options::default();
        let t = char_table(&o);
        assert_eq!((t[b'A' as usize], t[b'a' as usize], t[b'`' as usize], t[b'{' as usize], t[0x0c]), (b'a', b'a', b'@', b'[', 0x0c));
        let t = char_table(&Options { letters_upper: true, high_punctuation: true, ..o.clone() });
        assert_eq!((t[b'a' as usize], t[b'@' as usize], t[b']' as usize]), (b'A', b'`', b'}'));
        // Another encoding: each character's cell, as that encoding's byte (here the cell value plus 0x80).
        let mut codes = [0u8; 64];
        for (i, c) in codes.iter_mut().enumerate() {
            *c = 0x80 + i as u8;
        }
        let t = char_table(&Options { encoding: Some(codes), ..o });
        assert_eq!((t[b' ' as usize], t[b'A' as usize], t[b'a' as usize], t[b'=' as usize]), (0x80, 0x81, 0x81, 0x80 + 63));
    }

    #[test]
    fn codes_with_names() {
        assert_eq!(parse_codes("<esc>A<ctrl-z><lt>x<gt><bad>"), vec![0x1b, b'A', 0x1a, b'<', b'x', b'>', b'<', b'b', b'a', b'd', b'>']);
    }

    #[test]
    fn plain_job() {
        let o = Options { cells: 10, lines: 3, binding: 2, ..Options::default() };
        let out = job(&o, &[page(&["AB", "", "C", ""])], 1);
        assert_eq!(out, b"  ab\r\n\r\n  c\r\n\x0c".to_vec());
        // Two copies send the pages twice; Control-Z and closing bytes end the job.
        let o2 = Options { control_z: true, close_sync: vec![0, 0], ..o.clone() };
        assert_eq!(job(&o2, &[page(&["A"])], 2), b"  a\r\n\x0c  a\r\n\x0c\x1a\x00\x00".to_vec());
    }

    #[test]
    fn interpoint_sides() {
        let o = Options { cells: 10, lines: 3, binding: 1, interpoint: true, ..Options::default() };
        // Three pages: the back side has no binding margin padding, and a blank side makes up the last sheet.
        let out = job(&o, &[page(&["A"]), page(&["B"]), page(&["C"])], 1);
        assert_eq!(out, b" a\r\n\x0cb\r\n\x0c c\r\n\x0c  \r\n\x0c".to_vec());
    }

    #[test]
    fn full_lines_and_page_end_modes() {
        let base = Options { cells: 2, lines: 2, ..Options::default() };
        // A full line without the line end; a full page without the page end.
        let o = Options { eol_on_full_line: false, eop_on_full_page: false, ..base.clone() };
        assert_eq!(job(&o, &[page(&["AB", "C"])], 1), b"abc\r\n".to_vec());
        // Mode 1: no line end before the form feed.
        let o = Options { end_of_page: 1, ..base.clone() };
        assert_eq!(job(&o, &[page(&["A", "B"])], 1), b"a\r\nb\x0c".to_vec());
        // Mode 2: line ends to the page length, no form feed.
        let o = Options { end_of_page: 2, lines: 3, ..base.clone() };
        assert_eq!(job(&o, &[page(&["A"])], 1), b"a\r\n\r\n\r\n".to_vec());
        // Mode 3 Control-K; mode 5 the Puma byte with the Puma line end.
        assert_eq!(job(&Options { end_of_page: 3, ..base.clone() }, &[page(&["A"])], 1), b"a\r\n\x0b".to_vec());
        let o = Options { end_of_page: 5, line_end: LineEnd::Puma, ..base.clone() };
        assert_eq!(job(&o, &[page(&["A"])], 1), b"a\x81\x83".to_vec());
        // Mode 6: Control-R at the start and after the padded page.
        let o = Options { end_of_page: 6, ..base };
        assert_eq!(job(&o, &[page(&["A"])], 1), b"\x12a\r\n\r\n\x12".to_vec());
    }
}
