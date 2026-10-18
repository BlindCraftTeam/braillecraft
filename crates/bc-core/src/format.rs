//! The page formatter: a document laid out into pages of lines of cells, from its codes (spec 19).
//!
//! The reference program keeps a document as running text with codes and lays it out page by page when the view,
//! Print or Emboss asks for a page. This formatter reads the document in the editor's form (bc_core::doctext: codes
//! in brackets, [<] shown as a line break) and records, for every cell it places, the character position in that
//! text it came from, so a caret position in the editor can be found on its page, line and column.
//!
//! Rules (spec 19, from the reference program's documentation unless marked):
//! - margins: left (where a line starts after a forced new line), runover (where it starts after a word wrap) and a
//!   right margin counted in from the full width; at first left = runover = the first cell, right margin 0.
//!   [indP:F] new line if not at the start of one, left margin at cell P counted from 1 (1 when not given; from the
//!   current margin when F is 1 or 3), runover keeping its offset; [inmP:F] the same without a new line; [ptysN]
//!   runover = left + N (2 when not given); [ptye] runover = left; [rmN] right margin.
//! - [<] new line always (a blank line when already at the start of one); [l] new line unless at the start of one;
//!   [p] new paragraph: new line, then (print documents) one blank line, the first line starting at the third cell of
//!   a braille document and the first of a print document, counted from the edge of the page (F0b, from the braille
//!   and print formatting files); [skN] [l] then N blank lines (0 when not given; a negative N to the line -N from
//!   the page end); [svsblN] N blank lines after every line (1 when not given).
//! - Words that do not fit go to the next line at the runover margin. Extra spaces are left out.
//! - [pg] new page always (two in a row give a blank page); [top] new page once the page is started; [cpN:L] new
//!   page if fewer than N + 1 lines remain, else a new line and L blank lines (braille 1 and 0, print 2 and 1, F0b).
//! - [hdsN:L] ... [hde] centred heading: on a new page if fewer than N + 1 lines remain (braille 1, print 2), else
//!   after L blank lines (braille 0, print 1); each heading line is centred on the whole line, (cells - length) / 2,
//!   its words wrapping inside the right margin.
//! - [fr;F~X] the next word flush right, [tabP:A:F~X] at cell P justified A, [run] at the runover margin, with a
//!   fill from the pen; a tab or flush right behind the pen goes to the next line.
//! - [es~name] ... [ee~name]: the style's begin and end codes from the document's styles, in place.
//! - Braille page numbers: in the corner the document's page numbering names for even and odd pages, on the first
//!   or last line beside the text, which keeps three cells clear of them; [pgN~X] numbers the next page.
//! - A word that does not fit breaks after its last hyphen that does; one longer than a line starts on the line.
//! - [tls] ... [tle] a running head on the first lines of later pages, [rfs] ... [rfe] a running foot on the last
//!   line (spec 19 7.1, 7.2).
//! - [kps] ... [kpeL:M] a block kept on one page (with L lines after it): a page that fills inside a block that
//!   started on it is laid out again, ending before the block (spec 19 7.4); [hi] and [hlN] level margins (7.3).
//! - [lea] the next word is the print page number: in its corner (with a, b, ... on later braille pages of the same
//!   print page), and a print page change line (dots 3-6, then the number) where it begins in the middle of a page.
//!   Within [svlesN:M:L:P]'s bottom tolerance L of the page end, with the next page's corner at the top, the page is
//!   filled out and the print page begins on the next page; N 1 the British line (centred, after dots 5 and 2-5), M 1
//!   no continuation letters, P where the text after the line starts (spec 19 section 8).
//! - [pntr] and [pnta] roman and arabic page numbers, [svgan~X...] the number sign and digits, [svgrnN~X] the roman
//!   indicator (doubled before a numeral of more than one letter when N is not 0), [pvN~X] the page number without a
//!   new page; a page's number is taken when the page opens (spec 19 section 8).
//! Codes this formatter does not lay out take no room.

use crate::doctext::Shown;
use crate::engine::StyleTable;

mod table;
pub use table::TableRegion;

/// Where a page number goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Corner {
    #[default]
    None,
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
}

impl Corner {
    /// The value a document file and the Page Numbering dialog use: 0 none, 1 upper left, 2 upper right, 3 lower
    /// left, 4 lower right.
    pub fn from_code(n: u32) -> Corner {
        match n {
            1 => Corner::UpperLeft,
            2 => Corner::UpperRight,
            3 => Corner::LowerLeft,
            4 => Corner::LowerRight,
            _ => Corner::None,
        }
    }

    fn top(self) -> bool {
        matches!(self, Corner::UpperLeft | Corner::UpperRight)
    }

    fn bottom(self) -> bool {
        matches!(self, Corner::LowerLeft | Corner::LowerRight)
    }

    fn right(self) -> bool {
        matches!(self, Corner::UpperRight | Corner::LowerRight)
    }
}

/// The page a document is laid out on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub cells: usize,
    pub lines: usize,
    /// A braille document (paragraphs start at the third cell, page numbers in braille).
    pub braille: bool,
    /// Braille page numbers on even pages and on odd pages.
    pub numbers_even: Corner,
    pub numbers_odd: Corner,
    /// The first page whose number is shown.
    pub first_number_shown: usize,
    /// Print page numbers (from [lea]) on even pages and on odd pages.
    pub print_numbers_even: Corner,
    pub print_numbers_odd: Corner,
    /// The document's hyphenation table (Document, Hyphen Table; spec 36); none breaks words only where the text
    /// marks a break, as the None table does.
    pub hyphen: Option<crate::hyphen::HyphenTable>,
}

impl Layout {
    pub fn braille(cells: usize, lines: usize) -> Layout {
        Layout {
            cells,
            lines,
            braille: true,
            numbers_even: Corner::None,
            numbers_odd: Corner::None,
            first_number_shown: 1,
            print_numbers_even: Corner::None,
            print_numbers_odd: Corner::None,
            hyphen: None,
        }
    }

    pub fn print(cells: usize, lines: usize) -> Layout {
        Layout { braille: false, ..Layout::braille(cells, lines) }
    }
}

/// The document place each cell of a line came from, four bytes a cell: the pages of a large document are mostly
/// these, and a place fits in 32 bits (a document under four thousand million characters). A cell the formatter
/// added (indentation, the space between words, page numbers) has no place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources(Vec<u32>);

const NO_SOURCE: u32 = u32::MAX;

fn pack_place(s: Option<usize>) -> u32 {
    s.map_or(NO_SOURCE, |s| s as u32)
}

fn unpack_place(s: u32) -> Option<usize> {
    (s != NO_SOURCE).then_some(s as usize)
}

impl Sources {
    /// `n` cells with no place.
    pub fn none(n: usize) -> Sources {
        Sources(vec![NO_SOURCE; n])
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// The place of cell `i`: None past the line's end or for a cell with no place.
    pub fn get(&self, i: usize) -> Option<usize> {
        self.0.get(i).copied().and_then(unpack_place)
    }

    pub fn set(&mut self, i: usize, s: Option<usize>) {
        self.0[i] = pack_place(s);
    }

    pub fn push(&mut self, s: Option<usize>) {
        self.0.push(pack_place(s));
    }

    pub fn pop(&mut self) -> Option<Option<usize>> {
        self.0.pop().map(unpack_place)
    }

    /// `n` cells, the ones added with no place.
    pub fn resize(&mut self, n: usize) {
        self.0.resize(n, NO_SOURCE);
    }

    /// The place of each cell in order.
    pub fn iter(&self) -> impl Iterator<Item = Option<usize>> + '_ {
        self.0.iter().map(|&s| unpack_place(s))
    }

    /// Whether any cell has a place.
    pub fn any(&self) -> bool {
        self.0.iter().any(|&s| s != NO_SOURCE)
    }

    /// The first place on the line.
    pub fn first(&self) -> Option<usize> {
        self.iter().flatten().next()
    }

    /// Every place moved by `map`; None when one has nowhere to go.
    pub fn map_places(&mut self, map: &impl Fn(usize) -> Option<usize>) -> Option<()> {
        for s in &mut self.0 {
            if *s != NO_SOURCE {
                *s = pack_place(Some(map(*s as usize)?));
            }
        }
        Some(())
    }
}

impl Extend<Option<usize>> for Sources {
    fn extend<I: IntoIterator<Item = Option<usize>>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(pack_place));
    }
}

impl FromIterator<Option<usize>> for Sources {
    fn from_iter<I: IntoIterator<Item = Option<usize>>>(iter: I) -> Sources {
        Sources(iter.into_iter().map(pack_place).collect())
    }
}

/// Document positions that have no cell (a space, a line break, a code, the end of the document), each with the
/// column (0-based) where the formatter's pen stood when it came to it, eight bytes a mark.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks(Vec<(u32, u32)>);

impl Marks {
    pub fn push(&mut self, mark: (usize, usize)) {
        self.0.push((mark.0 as u32, mark.1 as u32));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Each mark as (position, column).
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.0.iter().map(|&(p, c)| (p as usize, c as usize))
    }

    /// Every position moved by `map`; None when one has nowhere to go.
    pub fn map_places(&mut self, map: &impl Fn(usize) -> Option<usize>) -> Option<()> {
        for m in &mut self.0 {
            m.0 = map(m.0 as usize)? as u32;
        }
        Some(())
    }
}

impl Extend<(usize, usize)> for Marks {
    fn extend<I: IntoIterator<Item = (usize, usize)>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(|(p, c)| (p as u32, c as u32)));
    }
}

impl FromIterator<(usize, usize)> for Marks {
    fn from_iter<I: IntoIterator<Item = (usize, usize)>>(iter: I) -> Marks {
        Marks(iter.into_iter().map(|(p, c)| (p as u32, c as u32)).collect())
    }
}

/// One line of a page: its cells, and for each cell the character position in the document it came from (none for
/// cells the formatter added: indentation, the space between words, page numbers).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub sources: Sources,
    /// Document positions that have no cell, each with the column where the formatter's pen stood when it came to
    /// it: the reference's formatter records the same for a position it is asked about (spec 27 5b), so a caret
    /// can stand after the last letter of a line on a space the line does not show, or on an empty last line.
    pub marks: Marks,
}

/// One laid-out page: all its lines (blank ones included, up to the lines per page) and the document position its
/// text starts at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub lines: Vec<Line>,
    pub start: usize,
    /// The print page numbers ([lea]) met on this page, as laid out, with the document place of each (Edit, Goto by
    /// reference page).
    pub print_labels: Vec<(String, usize)>,
    /// The page's own number (its counter and prefix letter), as File Print and File Emboss match typed page
    /// numbers against it ([pntr] makes it roman).
    pub number: crate::range::PageNumber,
    /// The tables that start on this page (spec 35 section 7): the editor's table regions.
    pub tables: Vec<TableRegion>,
    /// The reference points ([rps] ... [rpe]) that end on this page (spec 19 section 10.6), for a table of contents.
    pub references: Vec<ReferencePoint>,
}

/// A deferred note or a placement mark (spec 19 section 10.9): its collection (lower case), its kind (0 a note, 1 a
/// recurring note, 2 a [kdtxp] placement, 3 a placement at [lea]), the piece it starts at and, for a note, the piece
/// after its [kdtxe].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Note {
    name: String,
    kind: u8,
    start: usize,
    end: usize,
}

/// A reference point ([rpsN] ... [rpe]): the document places of its two codes, N, and the braille page number and
/// print page number of its first word and of its last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferencePoint {
    pub start: usize,
    pub end: usize,
    pub value: i64,
    pub first_page: String,
    pub first_print: String,
    pub last_page: String,
    pub last_print: String,
}

/// The table holding document place `pos` among the laid-out pages, when the place is after the table's first
/// cell starts (the standard "in the table" test for the status line and Style Level).
pub fn table_at(pages: &[Page], pos: usize) -> Option<&TableRegion> {
    pages.iter().flat_map(|p| p.tables.iter()).find(|t| t.holds(pos) && t.first_cell_start().is_some_and(|s| pos >= s))
}

/// The table regions of a document from its codes alone (no layout): what Table Properties and the cell moves
/// read when the pages are not at hand.
pub fn table_regions(doc: &Shown) -> Vec<TableRegion> {
    table::regions(pieces(doc, None).iter(), false)
}

/// The font attribute of each character position of the document (bit 1 bold, 2 italic, 4 underline): set by
/// [fts~b], [fts~i] and [fts~u] and cleared by the [fte~] with the same letter (spec 19 section 2). INFERENCE: a
/// bare [fte~] clears all three; a bare [fts~] sets none. File Print draws each run of one attribute in that style.
pub fn text_attributes(doc: &Shown) -> Vec<u8> {
    let chars: Vec<char> = doc.text.chars().collect();
    let mut out = vec![0u8; chars.len()];
    let mut attr = 0u8;
    let mut spans = doc.codes.iter().peekable();
    let mut i = 0;
    while i < chars.len() {
        if let Some(&&(start, end)) = spans.peek().filter(|s| s.0 == i) {
            spans.next();
            let end = end.min(chars.len());
            let code: String = chars[(start + 1).min(end)..end.saturating_sub(1).max(start + 1).min(end)].iter().collect();
            let bit = |rest: &str| match rest.chars().next() {
                Some('b') => 1,
                Some('i') => 2,
                Some('u') => 4,
                _ => 0,
            };
            if let Some(rest) = code.strip_prefix("fts~") {
                attr |= bit(rest);
            } else if let Some(rest) = code.strip_prefix("fte~") {
                let b = bit(rest);
                attr = if b == 0 { 0 } else { attr & !b };
            }
            for a in &mut out[start..end] {
                *a = attr;
            }
            i = end;
            continue;
        }
        out[i] = attr;
        i += 1;
    }
    out
}

/// Whether two pages carry the same page number (number, letter and kind): File Print and File Emboss then offer no
/// page-number range.
pub fn numbers_repeat(pages: &[Page]) -> bool {
    pages.iter().enumerate().any(|(k, p)| pages[..k].iter().any(|q| q.number == p.number))
}

/// A piece of the document: a character with its position (in characters), a space, or a code.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Char(char, usize),
    Space,
    Code(String),
    /// A document position that gets no cell: where the formatter's pen stands there is recorded (Line::marks).
    Mark(usize),
    /// The character U+001D: a hyphenation break may follow the cell before it.
    Break,
}

/// The pieces of a document, packed eight bytes each: a large document has a piece per character, and a layout
/// keeps its pieces for the next edit (reformat), so a piece as an enum of thirty-two bytes would be the largest
/// thing a document holds. The top three bits are the kind; a character piece holds the character and its place,
/// a mark its place, a code the index of its text in the list of code texts.
#[derive(Debug, Clone, Default)]
pub struct Pieces {
    packed: Vec<u64>,
    codes: Vec<String>,
    code_index: std::collections::HashMap<String, u32>,
}

const KIND_CHAR: u64 = 0;
const KIND_SPACE: u64 = 1;
const KIND_CODE: u64 = 2;
const KIND_MARK: u64 = 3;
const KIND_BREAK: u64 = 4;
const PLACE_BITS: u64 = 0xffff_ffff;

impl Pieces {
    fn pack(&mut self, p: Piece) -> u64 {
        match p {
            Piece::Char(c, at) => (KIND_CHAR << 61) | (u64::from(c as u32) << 32) | (at as u64 & PLACE_BITS),
            Piece::Space => KIND_SPACE << 61,
            Piece::Code(code) => {
                let k = match self.code_index.get(&code) {
                    Some(&k) => k,
                    None => {
                        let k = self.codes.len() as u32;
                        self.codes.push(code.clone());
                        self.code_index.insert(code, k);
                        k
                    }
                };
                (KIND_CODE << 61) | u64::from(k)
            }
            Piece::Mark(at) => (KIND_MARK << 61) | (at as u64 & PLACE_BITS),
            Piece::Break => KIND_BREAK << 61,
        }
    }

    fn push(&mut self, p: Piece) {
        let v = self.pack(p);
        self.packed.push(v);
    }

    fn pop(&mut self) {
        self.packed.pop();
    }

    /// Piece `i`, as an enum.
    fn get(&self, i: usize) -> Piece {
        let v = self.packed[i];
        match v >> 61 {
            KIND_CHAR => Piece::Char(char::from_u32(((v >> 32) & 0x1f_ffff) as u32).unwrap_or('\u{fffd}'), (v & PLACE_BITS) as usize),
            KIND_SPACE => Piece::Space,
            KIND_CODE => Piece::Code(self.codes[(v & PLACE_BITS) as usize].clone()),
            KIND_MARK => Piece::Mark((v & PLACE_BITS) as usize),
            _ => Piece::Break,
        }
    }

    /// The place of piece `i` when it has one (a character or a mark).
    fn place(&self, i: usize) -> Option<usize> {
        let v = self.packed[i];
        matches!(v >> 61, KIND_CHAR | KIND_MARK).then(|| (v & PLACE_BITS) as usize)
    }

    pub fn len(&self) -> usize {
        self.packed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packed.is_empty()
    }

    fn iter(&self) -> impl Iterator<Item = Piece> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }

    /// Moves the places of the pieces from `from` on by `delta`.
    fn shift_places(&mut self, from: usize, delta: isize) {
        for v in &mut self.packed[from..] {
            if matches!(*v >> 61, KIND_CHAR | KIND_MARK) {
                let at = ((*v & PLACE_BITS) as isize + delta) as u64;
                *v = (*v & !PLACE_BITS) | (at & PLACE_BITS);
            }
        }
    }

    /// The pieces in `range` replaced by `other`'s.
    fn splice(&mut self, range: std::ops::Range<usize>, other: &Pieces) {
        let packed: Vec<u64> = other.iter().map(|p| self.pack(p)).collect();
        self.packed.splice(range, packed);
    }
}

impl PartialEq for Pieces {
    fn eq(&self, other: &Pieces) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

impl Eq for Pieces {}

/// The document in pieces: its codes (the ranges the document marks as codes; a bracket elsewhere is text), with
/// style codes as their begin or end codes; the editor's line breaks become [<] codes, or nothing after [p] and [l].
/// The text is read once, character by character; no vector of its characters is made.
fn pieces(doc: &Shown, styles: Option<&StyleTable>) -> Pieces {
    let mut out = Pieces::default();
    let total = doc.text.chars().count();
    let mut spans = doc.codes.iter().peekable();
    let mut chars = doc.text.chars().enumerate().peekable();
    let mut after_break_code = false;
    // Inside a table ([htbs] to [htbe]) places are found through the table's cells (spec 27 5b), and the table's
    // regions are read from the marks after its codes.
    let mut in_table = false;
    while let Some(&(i, c)) = chars.peek() {
        if let Some(&&(start, end)) = spans.peek().filter(|s| s.0 == i) {
            spans.next();
            // The code's text: the characters between its brackets.
            let mut code = String::new();
            while let Some(&(k, ch)) = chars.peek() {
                if k >= end {
                    break;
                }
                chars.next();
                if k > start && k + 1 < end {
                    code.push(ch);
                }
            }
            after_break_code = crate::doctext::breaks_line(&code);
            // A code under another of its names in the code list is the same code (spec 19 section 10).
            let code = canonical_code(&code);
            // A code's place is where the pen stands before the code does its work: the standard program records the
            // asked-for place before its code switch (spec 19 section 9). Table codes keep theirs after them.
            let table_code = in_table || code.starts_with("htbs");
            if code.starts_with("htbs") {
                in_table = true;
            }
            if code == "htbe" {
                in_table = false;
            }
            if !table_code {
                out.push(Piece::Mark(start));
            }
            expand(&code, styles, &mut out, 0, start);
            if table_code {
                out.push(Piece::Mark(start));
            }
            continue;
        }
        chars.next();
        if c == '\n' || c == ' ' {
            out.push(Piece::Mark(i));
        }
        match c {
            '\n' => {
                if !after_break_code {
                    out.push(Piece::Code("<".to_string()));
                }
                after_break_code = false;
            }
            ' ' => {
                out.push(Piece::Space);
                after_break_code = false;
            }
            '\u{1d}' => out.push(Piece::Break),
            c => {
                out.push(Piece::Char(c, i));
                after_break_code = false;
            }
        }
    }
    out.push(Piece::Mark(total));
    out
}

/// A code as pieces: a style code becomes its begin or end codes (themselves codes in brackets); the text of a [q~X]
/// code is cells of the word it is in, and that of a [d~X] code cells that end the word (the reference's word
/// reader takes both into the word); anything else stays a code. `depth` stops a style that names itself; `pos` is
/// the code's place in the document, which its cells come from.
fn expand(code: &str, styles: Option<&StyleTable>, out: &mut Pieces, depth: usize, pos: usize) {
    if let Some(text) = code.strip_prefix("q~").or_else(|| code.strip_prefix("d~")) {
        for c in text.chars() {
            out.push(Piece::Char(c, pos));
        }
        if code.starts_with('d') {
            out.push(Piece::Space);
        }
        return;
    }
    let style = code.strip_prefix("es~").map(|n| (n, 0)).or_else(|| code.strip_prefix("ee~").map(|n| (n, 1)));
    if let (Some((name, which)), Some(table), true) = (style, styles, depth < 4) {
        if let Some(codes) = table.lookup(name.as_bytes(), which) {
            let text: String = codes.iter().map(|&b| match b {
                0x1c => '[',
                0x1f => ']',
                b => b as char,
            }).collect();
            let mut rest = text.as_str();
            while let Some(s) = rest.find('[') {
                let Some(e) = rest[s..].find(']') else { break };
                expand(&rest[s + 1..s + e], styles, out, depth + 1, pos);
                rest = &rest[s + e + 1..];
            }
        }
        return;
    }
    out.push(Piece::Code(code.to_string()));
}

/// A code's name and its numbers: "ind5" is ("ind", [5]), "hds1:0" is ("hds", [1, 0]).
/// The names the code list (dolcn.xml) gives a code beside the one this formatter reads, each with that one: the
/// French and Spanish names and the degree sign for [#] (spec 19 section 10).
const CODE_ALIASES: [(&str, &str); 37] = [
    ("pl", "sl"), ("\u{b0}", "#"), ("cd", "fr"), ("rd", "fr"), ("aln", "ind"), ("rtb", "ctb"), ("cbc", "hds"),
    ("ttc", "hds"), ("cbp", "hde"), ("ttt", "hde"), ("htc", "tls"), ("tlc", "tls"), ("htt", "tle"), ("tlp", "tle"),
    ("etb", "stb"), ("ftb", "stb"), ("psac", "ptys"), ("psc", "ptys"), ("psap", "ptye"), ("pst", "ptye"), ("po", "lea"),
    ("md", "rm"), ("fc", "sd"), ("ld", "sd"), ("siimp", "ifprt"), ("sibrl", "ifbrl"), ("mf", "wb"), ("vp", "wb"),
    ("mf-no", "wb-no"), ("vp-no", "wb-no"), ("mf-cb", "wb-cb"), ("vp-cb", "wb-cb"), ("rnc", "kps"), ("rnp", "kpe"),
    ("rnt", "kpe"), ("cu", "pi"), ("im", "pi"),
];

/// A code with its name in the form this formatter reads: the name (up to a digit, ':', ';', '~' or '=', as the
/// standard program's code reader takes it) replaced when it is another name of the same code.
fn canonical_code(code: &str) -> String {
    let end = code.find(|c: char| c.is_ascii_digit() || matches!(c, ':' | ';' | '~' | '=')).unwrap_or(code.len());
    let (name, rest) = code.split_at(end);
    match CODE_ALIASES.iter().find(|(alias, _)| alias.eq_ignore_ascii_case(name)) {
        Some((_, real)) => format!("{real}{rest}"),
        None => code.to_string(),
    }
}

fn split_code(code: &str) -> (&str, Vec<i64>) {
    let end = code.find(|c: char| c.is_ascii_digit() || c == '-' || c == ':').unwrap_or(code.len());
    let (name, rest) = code.split_at(end);
    let numbers = rest
        .split(':')
        .map(|n| n.split(|c: char| !(c.is_ascii_digit() || c == '-')).next().unwrap_or(""))
        .filter_map(|n| n.parse().ok())
        .collect();
    (name, numbers)
}

/// Which fold of `n` (from `k`) line `y` of `l` lines is, for folds of `w` lines with the positions given in `at`
/// (spec 19 section 10.7): one fold in the middle of an odd number of lines, halving, when n is odd, w is 1, l is odd
/// and no position is given; else spread evenly, the remainder going to the last folds (`last`) or the first.
fn fold_index(n: i64, l: i64, y: i64, k: i64, last: bool, w: i64, at: &[i64; 9]) -> Option<usize> {
    if n > 1 && w == 1 && at[0] == 0 && (n + 1) % 2 == 0 && (l + 1) % 2 == 0 {
        if y == (l + 1) / 2 - 1 {
            return Some(((n + 1) / 2 + k) as usize);
        }
        let half = l / 2;
        let (mut y, mut k, mut last) = (y, k, last);
        if half < y {
            y -= half + 1;
            k += 1 + n / 2;
            last = !last;
        }
        return fold_index(n / 2, half, y, k, last, w, at);
    }
    let r = (n + 1) - (l + w) % (n + 1);
    let mut pos = -((w + 2) / 2);
    for c in 0..n {
        if let Some(&p) = at.get((k + c) as usize).filter(|&&p| p != 0) {
            pos = p - 1;
        }
        pos += (l + w) / (n + 1);
        if (last && c >= r) || (!last && c < (n + 1) - r) {
            pos += 1;
        }
        if y == pos {
            return Some((k + c) as usize);
        }
    }
    None
}

/// The length of the word after a [>] or [fr] that is the next item from piece `at` (the piece ending a word): none
/// when the next item is a word, or another code, or no word follows the code (spec 36 section 13).
fn right_word_after(pieces: &Pieces, at: usize) -> Option<usize> {
    let mut i = at;
    while i < pieces.len() && matches!(pieces.get(i), Piece::Space | Piece::Mark(_)) {
        i += 1;
    }
    match pieces.get(i) {
        Piece::Code(c) if c == ">" || c == "fr" => {}
        _ => return None,
    }
    i += 1;
    while i < pieces.len() && matches!(pieces.get(i), Piece::Space | Piece::Mark(_)) {
        i += 1;
    }
    let mut len = 0;
    while i < pieces.len() {
        match pieces.get(i) {
            Piece::Char(..) => len += 1,
            Piece::Break | Piece::Mark(_) => {}
            _ => break,
        }
        i += 1;
    }
    (len > 0).then_some(len)
}

/// The piece after the first [kdtxe] from piece `from` (the end of the document when none): the end of a deferred
/// note (spec 19 section 10.9; no nesting is counted).
fn note_end(pieces: &Pieces, from: usize) -> usize {
    for i in from..pieces.len() {
        if let Piece::Code(c) = pieces.get(i) {
            if c == "kdtxe" {
                return i + 1;
            }
        }
    }
    pieces.len()
}

/// The levels of a major formatting context from piece `from`: 1, or the largest [hlN] (N 1 to 256) before the next
/// [mfce] or the end (spec 19 section 10.8).
fn mfc_levels(pieces: &Pieces, from: usize) -> i64 {
    let mut levels = 1;
    for i in from..pieces.len() {
        if let Piece::Code(c) = pieces.get(i) {
            let (name, n) = split_code(&c);
            if name == "mfce" {
                break;
            }
            if name == "hl" {
                if let Some(&l) = n.first() {
                    levels = levels.max(l.clamp(1, 256));
                }
            }
        }
    }
    levels
}

/// The line fill styles the standard program ships (linefill.xml, spec 19 section 10.10): name (empty for the default),
/// fill cell (a braille pattern is written as the encoding's ASCII cell) and its left, centre and right fields (all three
/// when the style lists none).
const LINE_FILLS: [(&str, char, [bool; 3]); 3] = [
    ("", '=', [true, true, true]),
    ("banaboxtop", '\u{2836}', [true, false, false]),
    ("banaboxbottom", '\u{281b}', [false, false, true]),
];

/// A roman numeral, lower case (the standard program writes it in capitals; the braille and print tables both show
/// it in small letters here, as the digits are); "?" outside 1 to 3999 (spec 19 section 8.1).
fn roman_numeral(n: usize) -> String {
    if !(1..=3999).contains(&n) {
        return "?".into();
    }
    const PARTS: [(usize, &str); 13] = [
        (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"),
        (9, "ix"), (5, "v"), (4, "iv"), (1, "i"),
    ];
    let mut n = n;
    let mut s = String::new();
    for (value, letters) in PARTS {
        while n >= value {
            s.push_str(letters);
            n -= value;
        }
    }
    s
}

/// Where the next word goes (a tab, flush right, [run]): column `col` (1-based; for flush right the last cell before
/// the page number's room), justified 'L' (starting there), 'R' (ending there), 'C' (centred on it) or 'D' (its
/// decimal point on it), with a fill from the pen ('partial' keeps a space at each end of it); `may_wrap` ends the
/// line first when the column is not after the pen and a space. `spacing` ([tas]) puts the word that many cells
/// after the pen instead when the column is behind it and the word still fits; `overwrite` ([taa], [lpr]) puts it at
/// the column whatever is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place {
    col: Option<i64>,
    just: char,
    fill: Option<char>,
    partial: bool,
    may_wrap: bool,
    spacing: Option<usize>,
    overwrite: bool,
}

impl Place {
    fn at(col: i64, just: char) -> Place {
        Place { col: Some(col), just, fill: None, partial: false, may_wrap: true, spacing: None, overwrite: false }
    }

    /// The next word at a tab stop, with the stop's fill (fill kind ' ' none, 'P' partial, any other letter full).
    fn at_stop(stop: &Stop) -> Place {
        let fill = (stop.kind != ' ').then_some(stop.fill);
        Place { fill, partial: stop.kind == 'P', ..Place::at(stop.col, stop.just) }
    }
}

/// A tab stop (spec 19 section 9): its cell (from 1), justification, fill kind and fill character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stop {
    col: i64,
    just: char,
    kind: char,
    fill: char,
}

impl Stop {
    /// The formatter's stops at the start: 1 to 8 at cells 3, 5 ... 17 justified left, 9 at cell 160 justified right,
    /// none with a fill (the state initialiser).
    fn defaults() -> [Stop; 9] {
        std::array::from_fn(|k| {
            if k < 8 {
                Stop { col: k as i64 * 2 + 3, just: 'L', kind: ' ', fill: ' ' }
            } else {
                Stop { col: 160, just: 'R', kind: ' ', fill: ' ' }
            }
        })
    }
}

/// The side room a running head or foot keeps ([svrhsM:N:J], [svrfsM:N:J]): M cells at the left, N at the right, and
/// where the line goes: 'l' after M, 'r' ending N before the end, else centred on the whole line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Side {
    left: usize,
    right: usize,
    align: char,
}

impl Default for Side {
    fn default() -> Side {
        Side { left: 8, right: 8, align: 'c' }
    }
}

impl Side {
    /// From a code's parameters "M:N:J" on lines of `cells` cells: M 8 and N = M when not given; centred, only the
    /// total counts.
    fn parse(params: &str, cells: usize) -> Side {
        let parts: Vec<&str> = params.split(':').collect();
        let number = |k: usize| parts.get(k).and_then(|p| p.trim().parse::<usize>().ok());
        let m = number(0).unwrap_or(8);
        let n = number(1).unwrap_or(m);
        let align = parts.get(2).and_then(|p| p.trim().chars().next()).map(|c| c.to_ascii_lowercase()).unwrap_or('c');
        let most = cells.saturating_sub(1);
        match align {
            'l' | 'r' => {
                let left = m.min(cells / 2);
                Side { left, right: n.min(cells.saturating_sub(left + 1)), align }
            }
            _ => {
                let left = ((m + n) / 2).min(most);
                Side { left, right: ((left + 1 + n) / 2).min(most), align: 'c' }
            }
        }
    }

    /// Where a line of `len` cells starts.
    fn start(self, len: usize, cells: usize) -> usize {
        match self.align {
            'l' => self.left,
            'r' => cells.saturating_sub(self.right + len),
            _ => cells.saturating_sub(len) / 2,
        }
    }

    /// The width a captured head or foot line wraps at.
    fn width(self, cells: usize) -> usize {
        cells.saturating_sub((self.left + self.right).min(cells.saturating_sub(1))).max(1)
    }
}

/// A head or foot line of `text` placed on a line of `cells` cells by `side`.
fn side_line(text: &str, side: Side, cells: usize) -> Line {
    let pad = side.start(text.chars().count(), cells);
    let text = format!("{}{text}", " ".repeat(pad));
    Line { sources: Sources::none(text.chars().count()), text, marks: Marks::default() }
}

/// Words wrapped into lines of at most `width` cells.
fn wrap_words(words: &[String], width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in words {
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= width => {
                l.push(' ');
                l.push_str(w);
            }
            _ => lines.push(w.clone()),
        }
    }
    lines
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Formatter {
    layout: Layout,
    pages: Vec<Page>,
    /// Lines of the page being filled (its number is put on its line when the page is finished).
    page: Vec<Line>,
    /// The page counter: the number of the page being filled; [pgN] sets the next page's; the prefix [pgN~X] sets.
    counter: usize,
    next_counter: Option<usize>,
    prefix: Option<char>,
    /// [pntr] roman page numbers ([pnta] arabic); the number sign and digits of arabic numbers ([svgan], from the
    /// format table's digit line: braille "#" and "jabcdefghi", print none and "0123456789"); the roman indicator
    /// ([svgrn], braille ";", print none; "}" puts ";" only before 5, 10, 50, 100, 400, 500, 605, 650 and 1000) and
    /// whether it is doubled before a numeral of more than one letter; the page's number as it was when the page
    /// opened, and whether it is roman.
    roman: bool,
    number_sign: Option<char>,
    digits: Vec<char>,
    roman_sign: Option<char>,
    roman_double: bool,
    page_text: Option<(String, bool)>,
    /// [svlesN:M:L:P]: the British print page line (N 1), no continuation letters (M 1), the bottom tolerance L and
    /// where the text after the line starts (P 0 the left margin, 1 the runover, 2 the runover when the line had text
    /// at the [lea]).
    leaf: (bool, bool, usize, i64),
    /// An anticipatory head ([svant1]) whose line count changed while the page was filled: the page is laid out again
    /// once with the head's rows at the new count (Formatter::run_from); a second change leaves it as laid.
    head_reserve: Option<usize>,
    head_retried: bool,
    head_overflow: bool,
    /// [vcsD:N] vertical centering (spec 19 section 10): N / D of a page's free lines go above its text, the page laid
    /// out again once with them (center_top) when it closes with lines free; [vce] ends it.
    vcenter: Option<(usize, usize)>,
    center_top: Option<usize>,
    center_done: bool,
    center_overflow: Option<usize>,
    /// Where page numbers go on odd and even pages and the first number shown; [svpnp] and [svpfd] change them from
    /// the next page.
    numbers_odd: Corner,
    numbers_even: Corner,
    first_number_shown: usize,
    next_numbers: Option<(Corner, Corner)>,
    /// Print page numbers: where they go on odd and even pages, the current print page number (the word after
    /// [lea]), how many braille pages it has gone on to (its continuation letter), the text this page shows, the line
    /// of this page that is a print page change line, and a [lea] waiting for its word.
    print_odd: Corner,
    print_even: Corner,
    next_print: Option<(Corner, Corner)>,
    print_number: Option<String>,
    print_since: usize,
    /// Print page numbers met since the last page was finished, with their places (Page::print_labels).
    labels: Vec<(String, usize)>,
    page_print: Option<String>,
    change_line: Option<usize>,
    lea: bool,
    /// The fill of the American print page line [leaF~X] asked for (spec 19 section 8.2): F 'n' none, 'p' partial
    /// (a blank cell at each end), else full; X the fill character (dots 3-6 when not given).
    lea_fill: (char, char),
    next_first_shown: Option<usize>,
    page_start: Option<usize>,
    /// Where the last page finished starts (a page with no text starts there too).
    last_start: usize,
    line: Line,
    /// Where the next text on the line starts.
    start_col: usize,
    left: usize,
    /// Where runover lines start, counted from the left margin (a level's runover may be left of it).
    runover_offset: i64,
    /// The level margins [hi...] set (left, runover, right, their increments per level, the most levels and the
    /// limits; spec 19 7.3) and the current level (1-based).
    levels: Option<[i64; 10]>,
    level: i64,
    right: usize,
    blank_between: usize,
    /// Where the next word goes, set by [tab], [fr] or [run].
    place: Option<Place>,
    heading: Option<(Vec<(String, Vec<Option<usize>>)>, i64, i64)>,
    /// The last thing done was a page break by [pg] (a second one gives a blank page).
    after_page_code: bool,
    /// Between [tls] and [tle] (a head, true) or [rfs] and [rfe] (a foot): the words captured, which are not body text.
    running: Option<(bool, Vec<String>)>,
    /// The running head's lines (at most two) and the running foot (one line), the pages they go on ([svrhp],
    /// [svrfp]: 1 odd, 2 even, 3 both, 0 none) and the side room they keep ([svrhs], [svrfs]).
    head: Vec<String>,
    foot: Option<String>,
    head_pages: i64,
    foot_pages: i64,
    head_side: Side,
    foot_side: Side,
    /// [svrlbN] lines kept blank at the bottom of every page; [svprgN:L:M] where a paragraph starts (cell N counted
    /// from 1, from the left margin when M is not 0) and the lines it skips; [kiN] keep blocks broken at [<] and [hl].
    reserve: usize,
    paragraph: (i64, i64, bool),
    keep_interrupt: bool,
    /// The page being filled is an odd page (the page parity flips at every page).
    odd: bool,
    /// The page has been opened (its first content came), the head lines laid on it, and the foot it carries.
    opened: bool,
    head_rows: usize,
    page_foot: Option<String>,
    /// Marks met while a word was being collected, each with the index in the word of the cell after it: they are
    /// placed with the word (Formatter::word).
    word_marks: Vec<(usize, usize)>,
    /// A code ended the last word: the next one follows it with no space (a line may still end between them).
    touching: bool,
    /// Keep blocks (spec 19 7.4), kept per page: how deep in blocks, the piece the outermost block on this page
    /// started at (and that one just opened), the line a block opened up to which is attached to the last one, and the
    /// second pass of the page that ends it before its block.
    keep_depth: usize,
    keep_start: Option<usize>,
    keep_opened: bool,
    attach_until: Option<usize>,
    retried: bool,
    keep_stop: Option<usize>,
    /// The page just finished filled inside a block that started on it: the piece the block started at.
    keep_overflow: Option<usize>,
    /// Tab stops [stbS:A:P] (stop number, justification, column), the minimum spacing of [tas] ([svmtsN]), the
    /// decimal character of 'D' justification ([svdac~X]), the pen column [lps] saved for [lpr].
    stops: [Stop; 9],
    min_spacing: usize,
    decimal: char,
    saved_col: Option<usize>,
    /// [svlmaN], added to later [ind] and [hi] margins.
    lma: i64,
    /// The word break mode of the word being collected ([wb-no] 1, [wb-cb] 2).
    word_break: u8,
    /// Skipped lines: how many blank lines stand pending at the end of the page (written
    /// them lazily), and lines of the next skip that [skn] cancelled.
    pending_blank: usize,
    skip_debt: i64,
    /// [sdN]: the next page opened must be odd (true) or even.
    wanted_odd: Option<bool>,
    /// [pmN:C]: the next word goes N spaces on in the line if it fits, else starts a paragraph (C its column).
    magazine: Option<(i64, Option<i64>)>,
    /// [linenumC:A] (spec 19 section 10.5): the next word is a line number at cell C (from the right edge when
    /// negative; -1 when not given), aligned A ('R' when not given); the number waiting to be drawn at the line end
    /// (its first cell and its cells), then on the line the next word's first cell went on (with the pen there);
    /// [svpalS:N] the word after it (3 and -3); whether text has been put since the last [l], [p], [pg], [hl], [st]
    /// or paragraph start; the right reserve the numbers keep, the override
    /// margin of [rmN:O] (0), the section of the page the reserve is kept for ([rmN:O] starts one), the reserve each
    /// section needed (at most 40, kept over the layouts of a page) and whether the page is laid out again with a
    /// wider reserve.
    line_number: Option<(i64, char)>,
    number_drawn: Option<(i64, Vec<char>)>,
    number_bound: Option<(i64, Vec<char>, usize)>,
    svpal: (i64, i64),
    line_ended: bool,
    number_room: usize,
    override_margin: usize,
    room_section: usize,
    room_table: Vec<((usize, usize), usize)>,
    room_restart: bool,
    /// The reference point open since its [rps] (the point with its first and last word's page numbers once a word
    /// came), the points ended on the page being filled, and the document place of the last code or mark met.
    ref_open: Option<ReferencePoint>,
    ref_first: bool,
    page_refs: Vec<ReferencePoint>,
    last_place: usize,
    /// [foldsN:W:P1:...~X] (spec 19 section 10.7): N folds (0 none) of W lines each, the fold positions given (0 where
    /// none) and the characters that fill a fold's lines (by fold, the first for the rest; none leaves them blank).
    folds: (usize, usize, [i64; 9], Vec<char>),
    /// The levels of the major formatting context an [mfcs] opens: the largest [hlN] before its [mfce] (1 at least).
    mfc_levels: i64,
    /// Whether an [mfcs] has been met in the document, and the length of the word after a [>] or [fr] that follows
    /// the word being ended (spec 36 section 13): with a right margin of 6 that word's room is its length and one.
    mfc_met: bool,
    next_right_word: Option<usize>,
    /// [wlf~X] (spec 19 section 10.10): the line being filled is a whole-line fill: its style's fill cell, its left,
    /// centre and right fields, and the columns [>] marked on it (two at most).
    fill: Option<(char, [bool; 3], Vec<i64>)>,
    /// Keep-deferred text (spec 19 section 10.9): the notes and placement marks met ([kdtxs] ... [kdtxe] and
    /// [kdtxp]), the note being placed and how (2 at [kdtxp], 3 at a [lea]), the piece the placing code stands at, the
    /// piece to go back to and the [lea] to lay after the notes; the collections placed at the next [lea]
    /// ("name~name~", 159 characters at most); the piece being laid out, the end of the note its [kdtxs] opens (after
    /// its [kdtxe]) and the piece to go on from.
    notes: Vec<Note>,
    placing: Option<(usize, u8, usize, usize, Option<String>)>,
    auto_notes: String,
    piece_at: usize,
    note_end: usize,
    jump: Option<usize>,
    /// [svantN]: 1 or 3 the head as it stands when the page closes; 2 or 3 the print page number likewise.
    anticipatory: i64,
    /// The table being collected (from [htbs] to [htbe], spec 35), the [htbfmtN:FFF] values for the next table
    /// (the options pair and the automatic layout letters), and the regions of the tables that started on the page
    /// being filled.
    table: Option<Box<table::Collector>>,
    htbfmt: (i64, String),
    page_tables: Vec<TableRegion>,
    /// Hyphenation (spec 36): the cells of the word being collected after which a marked break (U+001D) may come;
    /// the [wbN] override of the cells before an automaton break (-1 for the table's); the [wb-cb] continuation
    /// string and the [svcbh] hyphen override; the last word's characters, which [svcbh] reads.
    word_breaks: Vec<usize>,
    wb_min: i64,
    cb_cont: String,
    hyphen_override: Option<char>,
    last_word: Vec<char>,
    /// [ifprt] in a braille document or [ifbrl] in a print document: the next item (a word or a code) is dropped
    /// (pgifmt cases 0x68, 0x69; the document object's +0x94 is 1 for a print document).
    skip_next: bool,
    /// A code that takes the next word (spec 19 section 10.4): 'r' [remw] drops it, 'l' [lec] the print page number
    /// continuation pages show, 'c' [mec] the guide-word continuation text, 'g' [meg] a main entry, 'd' [med] the dash
    /// between guide words.
    next_word: Option<char>,
    /// The guide-word footer: its mode (0 none, 1 carried from a page before, 2 one entry on this page, 3 more), the
    /// first and last entries, the continuation and dash texts, an entry waiting for the next line end, and the print
    /// page continuation number from [lec].
    guide_mode: u8,
    guide_first: String,
    guide_last: String,
    guide_cont: String,
    guide_dash: String,
    guide_pending: Option<String>,
    print_cont: Option<String>,
}

impl Formatter {
    fn new(layout: &Layout) -> Formatter {
        Formatter {
            layout: layout.clone(),
            pages: Vec::new(),
            page: Vec::new(),
            counter: 1,
            next_counter: None,
            prefix: None,
            roman: false,
            number_sign: layout.braille.then_some('#'),
            digits: if layout.braille { "jabcdefghi" } else { "0123456789" }.chars().collect(),
            roman_sign: layout.braille.then_some(';'),
            roman_double: false,
            page_text: None,
            leaf: (false, false, 2, 0),
            head_reserve: None,
            head_retried: false,
            head_overflow: false,
            vcenter: None,
            center_top: None,
            center_done: false,
            center_overflow: None,
            numbers_odd: layout.numbers_odd,
            numbers_even: layout.numbers_even,
            first_number_shown: layout.first_number_shown,
            next_numbers: None,
            print_odd: layout.print_numbers_odd,
            print_even: layout.print_numbers_even,
            next_print: None,
            print_number: None,
            print_since: 0,
            labels: Vec::new(),
            page_print: None,
            change_line: None,
            lea: false,
            lea_fill: ('F', '-'),
            next_first_shown: None,
            page_start: None,
            last_start: 0,
            line: Line::default(),
            start_col: 0,
            left: 0,
            runover_offset: 0,
            levels: None,
            level: 1,
            right: 0,
            blank_between: 0,
            place: None,
            heading: None,
            after_page_code: false,
            running: None,
            head: Vec::new(),
            foot: None,
            head_pages: 3,
            foot_pages: 3,
            head_side: Side::default(),
            foot_side: Side::default(),
            odd: true,
            reserve: 0,
            paragraph: if layout.braille { (3, 0, false) } else { (1, 1, false) },
            keep_interrupt: false,
            opened: false,
            head_rows: 0,
            page_foot: None,
            word_marks: Vec::new(),
            touching: false,
            keep_depth: 0,
            keep_start: None,
            keep_opened: false,
            attach_until: None,
            retried: false,
            keep_stop: None,
            keep_overflow: None,
            stops: Stop::defaults(),
            min_spacing: 1,
            decimal: '.',
            saved_col: None,
            lma: 0,
            word_break: 0,
            pending_blank: 0,
            skip_debt: 0,
            wanted_odd: None,
            magazine: None,
            line_number: None,
            number_drawn: None,
            number_bound: None,
            svpal: (3, -3),
            line_ended: false,
            number_room: 0,
            override_margin: 0,
            room_section: 0,
            room_table: Vec::new(),
            room_restart: false,
            ref_open: None,
            ref_first: false,
            page_refs: Vec::new(),
            last_place: 0,
            folds: (0, 1, [0; 9], Vec::new()),
            mfc_levels: 1,
            mfc_met: false,
            next_right_word: None,
            fill: None,
            notes: Vec::new(),
            placing: None,
            auto_notes: String::new(),
            piece_at: 0,
            note_end: 0,
            jump: None,
            anticipatory: 0,
            table: None,
            htbfmt: (0, String::new()),
            page_tables: Vec::new(),
            word_breaks: Vec::new(),
            wb_min: -1,
            cb_cont: "_&".into(),
            hyphen_override: None,
            last_word: Vec::new(),
            skip_next: false,
            next_word: None,
            guide_mode: 0,
            guide_first: String::new(),
            guide_last: String::new(),
            guide_cont: String::new(),
            guide_dash: String::new(),
            guide_pending: None,
            print_cont: None,
        }
    }

    /// The page geometry a table starting now is laid into (spec 35 2.1, in this formatter's page model): the lines
    /// left on this page below the pen, and the lines of a fresh page under its running head and the blank lines
    /// after it, less the running foot. A table on a full page starts on the next page.
    fn table_geometry(&self) -> table::Geometry {
        let first_depth = self.capacity().saturating_sub(self.page.len());
        let next_odd = !self.odd;
        let on = |pages: i64| pages == 3 || (pages == 1 && next_odd) || (pages == 2 && !next_odd);
        let foot_next = self.foot.is_some() && on(self.foot_pages);
        let head_next = if !self.head.is_empty() && on(self.head_pages) {
            self.head.len() + if self.layout.braille { 0 } else { 2 } + self.blank_between
        } else {
            0
        };
        let other_depth = self.usable().saturating_sub(usize::from(foot_next)).saturating_sub(head_next).max(1);
        let (first_depth, other_depth) = if first_depth == 0 && other_depth > 0 { (other_depth, other_depth) } else { (first_depth, other_depth) };
        table::Geometry { first_depth, other_depth, width: self.layout.cells, first_below_head: self.page.len() > self.head_rows }
    }

    /// [htbs...]: the current line ends, the page is open, and a table object takes every piece up to [htbe].
    fn start_table(&mut self, code: &str) {
        if self.line_has_text() {
            self.end_line(self.left);
        }
        self.open_page();
        let geometry = self.table_geometry();
        let braille = self.layout.braille;
        self.table = Some(Box::new(table::Collector::new(code, &self.htbfmt, braille, geometry)));
    }

    /// A piece while a table is being collected: the table takes it; at [htbe] (its mark, which gives the code's
    /// place) the table is laid out and the mark then stands after it.
    fn table_step(&mut self, p: &Piece) {
        let Some(t) = self.table.as_mut() else { return };
        if t.take(p) {
            return;
        }
        let end = match p {
            Piece::Mark(pos) => *pos + 6,
            _ => t.last_mark().map_or(0, |m| m + 1),
        };
        self.lay_table(end);
        if let Piece::Mark(pos) = p {
            self.mark(*pos);
        }
    }

    /// Lays out the collected table (spec 35 1.4): its page images go on this page and on new pages after it; a
    /// tainted table's text is laid out as ordinary text (the failsafe) and its region keeps the reason.
    fn lay_table(&mut self, end: usize) {
        let Some(t) = self.table.take() else { return };
        let pieces = t.pieces.clone();
        let laid = t.finish(end);
        self.page_tables.push(laid.region);
        if laid.tainted.is_some() {
            let mut word = Vec::new();
            for p in &pieces {
                self.step(p, &mut word);
            }
            if !word.is_empty() {
                self.word(&word);
            }
            self.touching = false;
            return;
        }
        for (k, lines) in laid.images.into_iter().enumerate() {
            if k > 0 {
                self.new_page();
            }
            for line in lines {
                if self.page_start.is_none() {
                    self.page_start = line.sources.first();
                }
                if line.sources.any() {
                    self.pending_blank = 0;
                }
                self.push_line(line);
            }
        }
        self.start_col = self.left;
        self.touching = false;
        self.after_page_code = false;
    }

    /// Skips `count` lines (at most three pages' worth) as a skip code asks, less what [skn] cancelled of them.
    fn skip_lines(&mut self, count: i64) {
        let count = count.clamp(0, 3 * self.layout.lines as i64);
        let n = count - self.skip_debt;
        self.skip_debt = (self.skip_debt - count).max(0);
        for _ in 0..n.max(0) {
            self.push_line(Line::default());
            self.pending_blank += 1;
        }
    }

    /// The cell of the first character of the line above that is not blank: the nearest such line when `nearest`,
    /// else the line just above (0 when it is blank). For the relative form of [hi].
    fn above_cell(&self, nearest: bool) -> i64 {
        let first = |l: &Line| l.text.chars().position(|c| c != ' ').map(|p| p as i64);
        if nearest {
            self.page.iter().rev().find_map(first).unwrap_or(0)
        } else {
            self.page.last().and_then(first).unwrap_or(0)
        }
    }

    /// Records document position `pos` at the pen: after the line's last cell, or where the line's text will start.
    fn mark(&mut self, pos: usize) {
        let used = self.line.text.chars().count();
        let col = if self.line_has_text() { used } else { self.start_col.max(used).max(self.rooms().0) };
        self.line.marks.push((pos, col));
    }

    /// Places the marks of the word being placed whose cell index is `k`, at column `col`.
    fn place_word_marks(&mut self, k: usize, col: usize) {
        let marks = &mut self.word_marks;
        let mut i = 0;
        while i < marks.len() {
            if marks[i].1 == k {
                let (pos, _) = marks.remove(i);
                self.line.marks.push((pos, col));
            } else {
                i += 1;
            }
        }
    }

    /// The left margin a [indP:F] or [inmP:F] sets: cell P counted from 1 (1 when not given), from the current
    /// margin when F is 1 or 3, within the line.
    fn margin(&self, p: Option<i64>, f: Option<i64>) -> usize {
        let mut cell = p.unwrap_or(1) - 1;
        // The [svlma] adjustment goes in below F 2.
        if f.unwrap_or(0) < 2 {
            cell += self.lma;
        }
        if matches!(f, Some(1) | Some(3)) {
            cell += self.left as i64 + 1;
        }
        cell.clamp(0, self.layout.cells.saturating_sub(1) as i64) as usize
    }

    /// Where the line's words wrap: the right margin, or the room a page number on the line keeps.
    /// Where a runover line starts.
    fn runover(&self) -> usize {
        (self.left as i64 + self.runover_offset).clamp(0, self.layout.cells.saturating_sub(1) as i64) as usize
    }

    /// Runs `end` (a [<] or [hl] line end); while [ki] is on, the keep block is closed before it and a new one
    /// opened after it.
    fn interrupt_keep(&mut self, end: impl FnOnce(&mut Formatter)) {
        let interrupt = self.keep_interrupt && self.keep_depth > 0;
        if interrupt {
            self.keep_depth -= 1;
            if self.keep_depth == 0 {
                self.keep_start = None;
            }
        }
        end(self);
        if interrupt {
            let at_top = self.page.len() == self.head_rows && !self.line_has_text();
            if !at_top {
                self.keep_depth += 1;
                if self.keep_depth == 1 {
                    self.keep_opened = true;
                }
            }
        }
    }

    /// [hlN]: level N's margins (the current level when N is not given), from the level margins [hi] set: left
    /// n * I + (L - 1), runover n * J + (V - 1), right n * K + R for n = N - 1, each held by its limit (an upper limit
    /// for a positive increment, a lower one otherwise), then within the line.
    fn set_level(&mut self, level: Option<i64>) {
        let Some([l, v, r, i, j, k, most, nl, ol, pl]) = self.levels else {
            self.left = 0;
            self.runover_offset = 0;
            self.right = 0;
            return;
        };
        let mut n = level.unwrap_or(self.level) - 1;
        if most > 0 {
            n = n.min(most - 1);
        }
        let n = n.clamp(0, 255);
        let limit = |value: i64, inc: i64, limit: i64| match limit {
            lim if lim > 0 && inc > 0 => value.min(lim),
            lim if lim > 0 => value.max(lim),
            _ => value,
        };
        let cells = self.layout.cells.saturating_sub(1) as i64;
        let left = limit(n * i + (l - 1), i, nl - 1).clamp(0, cells);
        let runover = limit(n * j + (v - 1), j, ol - 1).clamp(0, cells);
        let right = limit(n * k + r, k, pl).clamp(0, 159);
        self.left = left as usize;
        self.runover_offset = runover - left;
        self.right = right as usize;
        self.level = n + 1;
    }

    fn width(&self) -> usize {
        self.layout.cells.saturating_sub(self.right.max(self.number_room).max(self.rooms().1)).max(1)
    }

    /// The corner the number of the page being filled goes in (none before the first page shown), by the page
    /// parity.
    fn corner(&self) -> Corner {
        if self.counter < self.first_number_shown {
            return Corner::None;
        }
        if self.odd {
            self.numbers_odd
        } else {
            self.numbers_even
        }
    }

    /// Whether a head or foot set for `pages` (1 odd, 2 even, 3 both) goes on the page being filled.
    fn on_this_page(&self, pages: i64) -> bool {
        pages == 3 || (pages == 1 && self.odd) || (pages == 2 && !self.odd)
    }

    /// Opens the page when its first content comes: the running head goes on its first lines, then the blank lines
    /// after titles (braille 0, print 2) and the line spacing; the running foot is kept for its last line.
    fn open_page(&mut self) {
        if self.opened {
            return;
        }
        self.room_section = 0;
        self.number_room = self.room_lookup(self.override_margin);
        // [sdN] asked for an odd or even page: a page of the wrong parity is left blank (its head and number laid)
        // and the content goes on the next.
        if let Some(odd) = self.wanted_odd.take() {
            if odd != self.odd && self.page.is_empty() {
                self.opened = true;
                self.page_print = self.print_text().filter(|_| self.print_corner() != Corner::None);
                self.finish_page();
            }
        }
        self.opened = true;
        self.page_text = Some((self.page_number_text(), self.roman));
        // [lec]: a page that continues a print page shows the number [lec] gave from then on (spec 19 10.4).
        if self.print_since > 0 && !self.leaf.1 {
            if let Some(n) = self.print_cont.take() {
                self.print_number = Some(n);
            }
        }
        self.page_print = self.print_text().filter(|_| self.print_corner() != Corner::None);
        if self.foot.is_some() && self.on_this_page(self.foot_pages) {
            self.page_foot = self.foot.clone();
        }
        // A page laid out again for its anticipatory head reserves the rows the head had when it closed.
        let rows = self.head_reserve.take().unwrap_or(self.head.len());
        if rows > 0 && self.on_this_page(self.head_pages) {
            let cells = self.layout.cells;
            for k in 0..rows {
                let line = self.head.get(k).map_or_else(Line::default, |h| side_line(h, self.head_side, cells));
                self.page.push(line);
            }
            self.head_rows = rows;
            let after = if self.layout.braille { 0 } else { 2 } + self.blank_between;
            for _ in 0..after.min(self.capacity().saturating_sub(self.page.len() + 1)) {
                self.page.push(Line::default());
            }
        }
        // A page laid out again for vertical centering: its blank lines above the text.
        if let Some(k) = self.center_top.take() {
            for _ in 0..k.min(self.capacity().saturating_sub(self.page.len() + 1)) {
                self.page.push(Line::default());
            }
        }
    }

    /// The page number of the page being filled, as shown: as it was when the page opened (the standard program
    /// writes it then, spec 19 section 8.1), else as it stands.
    fn number_text(&self) -> String {
        match &self.page_text {
            Some((text, _)) => text.clone(),
            None => self.page_number_text(),
        }
    }

    /// The page number as the settings make it now (spec 19 section 8.1): the prefix, then the number sign and the digits,
    /// or the roman indicator (twice before a numeral of more than one letter with [svgrn1]) and the numeral.
    fn page_number_text(&self) -> String {
        let n = self.counter;
        let mut s: String = self.prefix.into_iter().collect();
        if self.roman {
            let numeral = roman_numeral(n);
            match self.roman_sign {
                Some('}') => {
                    if matches!(n, 5 | 10 | 50 | 100 | 400 | 500 | 605 | 650 | 1000) {
                        s.push(';');
                    }
                }
                Some(c) => {
                    if self.roman_double && numeral.chars().count() > 1 {
                        s.push(c);
                    }
                    s.push(c);
                }
                None => {}
            }
            s.push_str(&numeral);
        } else {
            s.extend(self.number_sign);
            for d in n.to_string().chars() {
                let k = d.to_digit(10).unwrap_or(0) as usize;
                s.push(self.digits.get(k).copied().unwrap_or(d));
            }
        }
        s
    }

    /// The lines a page holds. A page number takes no line of its own: it goes on the first or last line, beside the
    /// text (rooms).
    fn capacity(&self) -> usize {
        self.usable().saturating_sub(usize::from(self.page_foot.is_some())).max(1)
    }

    /// The lines of a page in use: all but the [svrlb] reserve at the bottom.
    fn usable(&self) -> usize {
        self.layout.lines.saturating_sub(self.reserve).max(1)
    }

    /// The cells the page number keeps on the line being filled, at its start and at its end: the number's length
    /// and three more on the first line for a number at the top, on the last line for one at the bottom.
    fn rooms(&self) -> (usize, usize) {
        let at = self.page.len();
        let mut rooms = (0, 0);
        let numbers = [(self.corner(), Some(self.number_text())), (self.print_corner(), self.page_print.clone())];
        for (c, text) in numbers {
            let Some(text) = text else { continue };
            if (c.top() && at == 0) || (c.bottom() && at + 1 == self.usable()) {
                let room = text.chars().count() + 3;
                if c.right() {
                    rooms.1 = rooms.1.max(room);
                } else {
                    rooms.0 = rooms.0.max(room);
                }
            }
        }
        rooms
    }

    /// The corner the print page number goes in on the page being filled: none before the first [lea], and none
    /// where the braille page number goes.
    fn print_corner(&self) -> Corner {
        let c = if self.odd { self.print_odd } else { self.print_even };
        if self.print_number.is_none() || c == self.corner() {
            Corner::None
        } else {
            c
        }
    }

    /// The print page number as a page shows it in its corner: with a continuation letter (a, b, ...) on the braille
    /// pages after the one its print page began on.
    fn print_text(&self) -> Option<String> {
        let n = self.print_number.as_ref()?;
        Some(match self.print_since {
            _ if self.leaf.1 => n.clone(),
            0 => n.clone(),
            k => format!("{}{n}", (b'a' + ((k - 1) % 26) as u8) as char),
        })
    }

    /// The word after [lea]: the new print page number. At the top of a page whose print number goes at the top (or
    /// on the last line when the next page's does) it only goes to the corner; else a print page change line is laid
    /// (dots 3-6 across the line, then the number at its end).
    fn print_page(&mut self, cells: &[(char, Option<usize>)]) {
        self.word_marks.clear();
        let text: String = cells.iter().take(36).map(|c| c.0).collect();
        if let Some(place) = cells.iter().find_map(|c| c.1) {
            self.labels.push((text.clone(), place));
        }
        self.print_number = Some(text.clone());
        let had_text = self.line_has_text();
        let next_top = if self.odd { self.print_even } else { self.print_odd }.top();
        // The British line has no corner-only case (spec 19 section 8.2).
        let at_top = !self.leaf.0 && !self.line_has_text() && (!self.opened || self.page.len() == 0);
        if at_top && self.print_corner().top() {
            self.print_since = 0;
            if self.opened {
                self.page_print = self.print_text();
            }
            return;
        }
        // On the last lines of the page with the next page's number at the top: the page is filled out so the
        // change shows in the next page's corner, and the print page begins there.
        // Spec 19 section 8.2: the line the [lea] is on is within the bottom tolerance L ([svles], 2) of the page's last
        // usable line above the foot.
        let at = self.page.len() + usize::from(self.line_has_text());
        let bottom = self.usable().saturating_sub(usize::from(self.page_foot.is_some()));
        if next_top && bottom.saturating_sub(self.leaf.2) <= at && !self.page.is_empty() {
            if self.line_has_text() {
                self.end_line(self.left);
            }
            if !self.page.is_empty() {
                self.finish_page();
            }
            self.print_since = 0;
            self.start_col = self.after_leaf_line(had_text);
            return;
        }
        if self.line_has_text() {
            self.end_line(self.left);
        }
        self.open_page();
        let width = self.layout.cells;
        let len = cells.len().min(width);
        // American: dots 3-6 across the line to the number flush right. British ([svles1]): the number
        // centred on cell (W - 1) / 2 + 2, after dots 5 and 2-5 in the two cells before it.
        let lead = if self.leaf.0 {
            let target = (width as i64 - 1) / 2 + 2;
            let start = (target + 1 - (len as i64 + 1) / 2).clamp(1, (width - len + 1) as i64) as usize - 1;
            let mut lead = " ".repeat(start.saturating_sub(2));
            lead.push_str(&"\"3"[2usize.saturating_sub(start)..]);
            lead
        } else {
            // [leaF~X]: 'N' blank, 'P' a blank cell after the margin and before the number, else the fill throughout.
            let (kind, fill) = self.lea_fill;
            let n = width - len;
            match kind {
                'N' => " ".repeat(n),
                'P' if n >= 2 => format!(" {} ", fill.to_string().repeat(n - 2)),
                _ => fill.to_string().repeat(n),
            }
        };
        let n = lead.chars().count();
        let mut line = Line { text: lead, sources: Sources::none(n), marks: Marks::default() };
        for &(c, s) in cells.iter().take(len) {
            line.text.push(c);
            line.sources.push(s);
        }
        self.change_line = Some(self.page.len());
        self.push_line(line);
        // This page is the one the print page began on; the next continues it.
        self.print_since = 0;
        self.start_col = self.after_leaf_line(had_text);
    }

    /// Where the text after a print page line starts ([svles] P, spec 19 section 8.2): the left margin for
    /// P 0, and for P 2 when the line had no text at the [lea]; else the runover.
    fn after_leaf_line(&self, had_text: bool) -> usize {
        match self.leaf.3 {
            0 => self.left,
            2 if !had_text => self.left,
            _ => self.runover(),
        }
    }

    fn line_has_text(&self) -> bool {
        self.line.sources.any()
    }

    fn page_has_text(&self) -> bool {
        self.line_has_text() || self.page.iter().any(|l| l.sources.any())
    }

    fn put(&mut self, c: char, source: Option<usize>) {
        if source.is_some() {
            // The standard program writes a waiting line number when it puts the next word on a line, and that
            // word counts as text since the last forced line break (spec 19 section 10.5).
            if let Some((start, cells)) = self.number_drawn.take() {
                self.number_bound = Some((start, cells, self.line.text.chars().count()));
            }
            self.line_ended = true;
            if self.page_start.is_none() {
                self.page_start = source;
            }
            self.pending_blank = 0;
        }
        self.line.text.push(c);
        self.line.sources.push(source);
    }

    /// Puts `c` at column `at` of the line: over what is there, or at the end.
    fn put_at(&mut self, at: usize, c: char, source: Option<usize>) {
        let used = self.line.text.chars().count();
        if at < used {
            let mut text: Vec<char> = self.line.text.chars().collect();
            text[at] = c;
            self.line.text = text.into_iter().collect();
            self.line.sources.set(at, source);
            if source.is_some() {
                self.pending_blank = 0;
            }
        } else {
            for _ in used..at {
                self.put(' ', None);
            }
            self.put(c, source);
        }
    }

    /// Ends the line; the next one starts at `next_col`.
    fn end_line(&mut self, next_col: usize) {
        self.saved_col = None;
        self.draw_line_number();
        self.fill_line();
        // A main entry ([meg]) takes effect at the line end: the first of a page is its first and last guide word,
        // a later one its last; the page keeps a foot line for them when there is room (spec 19 10.4).
        if let Some(entry) = self.guide_pending.take() {
            if self.guide_mode < 2 {
                self.guide_first = entry.clone();
                self.guide_mode = 2;
            } else {
                self.guide_mode = 3;
            }
            self.guide_last = entry;
            if self.foot.is_none() {
                self.foot = Some(String::new());
            }
            if self.page_foot.is_none() && self.on_this_page(self.foot_pages) && self.page.len() + 2 < self.usable() {
                self.page_foot = Some(String::new());
            }
        }
        let mut line = std::mem::take(&mut self.line);
        while line.text.ends_with(' ') {
            line.text.pop();
            line.sources.pop();
        }
        self.push_line(line);
        // Line spacing follows every line end, a blank line's too, but never past the end of the page.
        if !self.page.is_empty() {
            let room = self.capacity() - self.page.len();
            for _ in 0..self.blank_between.min(room) {
                self.push_line(Line::default());
            }
        }
        self.start_col = next_col;
        self.after_page_code = false;
    }

    /// [kdtxs~X] (case 0xb3), [kdtxe] (0xb4), [kdtxpN~X] (0xb5), spec 19 section 10.9.
    fn deferred_text(&mut self, code: &str) {
        let (head, name) = code.split_once('~').map_or((code, ""), |(a, b)| (a, b));
        let name = name.to_lowercase();
        let numbered = head[5.min(head.len())..].chars().next().is_some_and(|c| c.is_ascii_digit() || c == '-');
        match &head[..5.min(head.len())] {
            "kdtxs" => {
                if self.placing.is_some() {
                    return;
                }
                // A number makes the note recurring: the standard program tests the code's own number (0xb3) where the
                // manual says the parameter, so any number does.
                let kind = u8::from(numbered);
                let start = self.piece_at + 1;
                let end = self.note_end;
                if !self.notes.iter().any(|n| n.start == start) {
                    self.notes.push(Note { name, kind, start, end });
                }
                let end = self.notes.iter().find(|n| n.start == start).map_or(end, |n| n.end);
                self.jump = Some(end);
            }
            "kdtxe" => {
                let Some((cur, mode, hi, back, lea)) = self.placing.take() else { return };
                let name = self.notes[cur].name.clone();
                let lo = self.notes_floor(hi, &name);
                if let Some(next) = (cur + 1..self.notes.len()).find(|&k| self.note_taken(k, &name, lo, hi, true)) {
                    self.placing = Some((next, mode, hi, back, lea));
                    self.jump = Some(self.notes[next].start);
                    return;
                }
                self.jump = Some(back);
                if let Some(lea) = lea {
                    self.code(&lea);
                }
            }
            "kdtxp" if numbered => {
                // With a number the collection goes on the list placed at [lea] (again at its end: the standard program
                // tests the code's own number, 0xb5, so every number puts it on).
                let entry = format!("{name}~");
                let mut list = self.auto_notes.replace(&entry, "");
                list.push_str(&entry);
                self.auto_notes = list.chars().take(159).collect();
            }
            "kdtxp" => {
                if self.placing.is_some() {
                    return;
                }
                let hi = self.piece_at;
                self.start_placing(&name, hi, 2, None);
            }
            _ => {}
        }
    }

    /// Starts placing the notes of collection `name` deferred since its last placement, from the placing code at piece
    /// `hi`; false when there is none to place.
    fn start_placing(&mut self, name: &str, hi: usize, mode: u8, lea: Option<String>) -> bool {
        let lo = self.notes_floor(hi, name);
        let count = (0..self.notes.len()).filter(|&k| self.note_taken(k, name, lo, hi, false)).count();
        if count == 0 {
            return false;
        }
        let Some(first) = (0..self.notes.len()).find(|&k| self.note_taken(k, name, lo, hi, true)) else { return false };
        if !self.notes.iter().any(|n| n.start == hi) {
            self.notes.push(Note { name: name.to_string(), kind: mode, start: hi, end: hi });
        }
        self.placing = Some((first, mode, hi, hi + 1, lea));
        self.jump = Some(self.notes[first].start);
        true
    }

    /// At [lea]: the first collection on the [kdtxpN] list, when it has notes to place; the [lea] is laid after them.
    fn place_notes_at_lea(&mut self, lea: &str) -> bool {
        let name = self.auto_notes.split('~').next().unwrap_or("").to_string();
        if name.is_empty() {
            return false;
        }
        let hi = self.piece_at;
        self.start_placing(&name, hi, 3, Some(lea.to_string()))
    }

    /// The piece the last placement of collection `name` before piece `hi` stands at (0 when none).
    fn notes_floor(&self, hi: usize, name: &str) -> usize {
        let mut lo = 0;
        for n in &self.notes {
            if (n.kind == 2 || n.kind == 3) && n.name == name {
                if hi <= n.start {
                    break;
                }
                lo = n.start;
            }
        }
        lo
    }

    /// Whether note `k` is placed with collection `name` (every collection when empty) between pieces `lo` and `hi`:
    /// a recurring note always (when `recurring`), a note when it lies between them.
    fn note_taken(&self, k: usize, name: &str, lo: usize, hi: usize, recurring: bool) -> bool {
        let n = &self.notes[k];
        (name.is_empty() || n.name == name)
            && ((n.kind == 1 && recurring) || (n.kind == 0 && lo <= n.start && n.end <= hi))
    }

    /// Lays out a whole-line fill at the line end (spec 19 section 10.10): the text of a right field goes to the right
    /// margin, of a centre field to the middle, and the rest of the line within the margins is the fill cell; a line
    /// with no word is all fill.
    fn fill_line(&mut self) {
        let Some((cell, [left, center, right], marked)) = self.fill.take() else { return };
        let w = self.layout.cells as i64;
        let total = self.width() as i64;
        let has_text = self.line_has_text();
        let mut cells: Vec<(char, Option<usize>)> =
            self.line.text.chars().enumerate().map(|(i, c)| (c, self.line.sources.get(i))).collect();
        let pen = cells.len() as i64;
        let mut len = pen;
        cells.resize(w.max(pen) as usize, (' ', None));
        let fields = if has_text { marked.len() as i64 + 1 } else { 0 };
        let mut stops = [0, pen, pen];
        for (k, &c) in marked.iter().enumerate() {
            stops[k + 1] = c;
        }
        let mut from = if has_text { pen } else { 0 };
        let mut end = total;
        let copy = |cells: &mut Vec<(char, Option<usize>)>, len: &mut i64, src: i64, dst: i64, n: i64| {
            if n > 0 && src >= 0 && dst >= 0 && src + n <= w && dst + n <= w {
                let part: Vec<_> = cells[src as usize..(src + n) as usize].to_vec();
                cells[dst as usize..(dst + n) as usize].copy_from_slice(&part);
                *len = (*len).max(dst + n);
            }
        };
        let fill = |cells: &mut Vec<(char, Option<usize>)>, len: &mut i64, a: i64, b: i64| {
            if a >= 0 && b >= 0 && a < w && b <= w && a < b {
                for k in a..b {
                    cells[k as usize] = (cell, None);
                }
                *len = (*len).max(b);
            }
        };
        if right {
            let k = i64::from(left) + i64::from(center);
            if k < fields {
                from = stops[k as usize];
                end = total - (pen - from);
                copy(&mut cells, &mut len, from, end, pen - from);
            }
        }
        if center {
            let k = i64::from(left);
            if k < fields {
                from = stops[k as usize];
                let n = stops[k as usize + 1] - from;
                let at = (total - n) / 2;
                copy(&mut cells, &mut len, from, at, n);
                fill(&mut cells, &mut len, at + n, end);
                end = at;
            }
        }
        fill(&mut cells, &mut len, if left { from } else { 0 }, end);
        cells.truncate(len.max(0) as usize);
        let mut line = Line { marks: std::mem::take(&mut self.line.marks), ..Line::default() };
        for (c, source) in cells {
            line.text.push(c);
            line.sources.push(source);
        }
        self.line = line;
    }

    /// Takes the word after [linenum] as the number (at most 36 cells and one less than the line) and where it goes:
    /// at the right edge less C when C is negative, keeping room for it and two more cells on the right of the
    /// page section (at most the line less 4); the page is laid out again when the room grows.
    fn take_line_number(&mut self, cells: &[(char, Option<usize>)], col: i64, just: char) {
        let w = self.layout.cells as i64;
        let len = (cells.len() as i64).min(36).min(w - 1).max(0);
        let mut col = col;
        if col < 0 {
            col += w;
            let room = self.room_lookup((len + 2).min(w - 4).max(0) as usize);
            // Laid out again only when the kept room grows (a page state already opened keeps the older reserve).
            let key = (self.pages.len(), self.room_section);
            let known = self.room_table.iter().any(|e| e.0 == key && e.1 >= room);
            if room > self.number_room && self.room_store(room) && !known {
                self.room_restart = true;
            }
            self.number_room = room;
        }
        let start = match just {
            'C' => (col - len + 1) / 2,
            'L' => col - 1,
            _ => col - len + 1,
        };
        self.number_drawn = Some((start, cells.iter().take(len as usize).map(|c| c.0).collect()));
    }

    /// The reserve kept for the page section being filled, or `room` when it is wider.
    fn room_lookup(&self, room: usize) -> usize {
        let key = (self.pages.len(), self.room_section);
        self.room_table.iter().find(|e| e.0 == key).map_or(room, |e| e.1.max(room))
    }

    /// Keeps `room` for the page section being filled; false when 40 sections are kept already.
    fn room_store(&mut self, room: usize) -> bool {
        let key = (self.pages.len(), self.room_section);
        if let Some(e) = self.room_table.iter_mut().find(|e| e.0 == key) {
            e.1 = room;
            return true;
        }
        if self.room_table.len() < 40 {
            self.room_table.push((key, room));
            return true;
        }
        false
    }

    /// Draws the line number the line took: spaces up to its first cell (when the pen had already reached it, the
    /// cells up to there become spaces, as the standard program does), then the number, cut at the line end.
    fn draw_line_number(&mut self) {
        let Some((start, cells, pen)) = self.number_bound.take() else { return };
        if cells.is_empty() || start < 0 {
            return;
        }
        let full = self.layout.cells as i64;
        let to = start.min(full) as usize;
        let used = self.line.text.chars().count();
        if pen < to {
            for _ in used..to {
                self.put(' ', None);
            }
        } else {
            for at in 0..to {
                self.put_at(at, ' ', None);
            }
        }
        for (k, &c) in cells.iter().enumerate() {
            let at = start + k as i64;
            if at < full {
                self.put_at(at as usize, c, None);
            }
        }
    }

    /// When the line the page goes on to is a fold line ([folds]), its W lines are filled with the fold's character
    /// across the page (or left blank) and passed, as the standard program does after each line.
    fn fold_lines(&mut self) {
        let (n, w) = (self.folds.0 as i64, self.folds.1);
        if n == 0 {
            return;
        }
        let l = self.capacity() as i64;
        let y = self.page.len() as i64;
        if y >= l || l / (n + 1) == 0 {
            return;
        }
        let Some(k) = fold_index(n, l, y, 0, true, w as i64, &self.folds.2) else { return };
        if (y + 1) / (l / (n + 1)) == n + 1 {
            return;
        }
        let marker = &self.folds.3;
        let c = marker.get(k).filter(|_| k < 10).or(marker.first()).copied();
        for _ in 0..w {
            if self.page.len() >= self.capacity() {
                break;
            }
            let mut line = Line::default();
            if let Some(c) = c {
                for _ in 0..self.layout.cells {
                    line.text.push(c);
                    line.sources.push(None);
                }
            }
            self.page.push(line);
        }
    }

    fn push_line(&mut self, line: Line) {
        if self.page.len() >= self.capacity() {
            self.finish_page();
        }
        self.open_page();
        // Blank lines are kept at the top of a page too: skips that do not fit on a page go on at the top of the next.
        self.page.push(line);
        self.fold_lines();
        if self.page.len() >= self.capacity() {
            self.finish_page();
        }
    }

    fn finish_page(&mut self) {
        let corner = self.corner();
        let number = self.number_text();
        let mut lines = std::mem::take(&mut self.page);
        let room = self.usable();
        // Vertical centering (pgifmt at the page's close): with lines free above the foot, (usable lines - the
        // lines used - 1) * N / D blank lines go at the top when the page is laid out again; once per page.
        if let Some((num, den)) = self.vcenter.filter(|_| !self.center_done) {
            let used = lines.iter().rposition(|l| !l.text.is_empty() || l.sources.any()).map_or(0, |k| k + 1);
            let avail = room.saturating_sub(usize::from(self.page_foot.is_some()));
            if avail > used + 1 {
                let skip = (room - (used + 1)) * num / den;
                if skip >= 1 {
                    self.center_overflow = Some(skip);
                }
            }
        }
        self.center_done = false;
        lines.resize(self.layout.lines.max(room), Line::default());
        // An anticipatory head ([svant] 1 or 3) is written as it stands when the page closes, over the rows it was
        // given at the opening. When its line count changed, the page is laid out again once with that many rows
        // (spec 19 section 8.3); a second change (error 16) leaves the page as laid,
        // a line that went away blank and one that came not shown.
        if self.anticipatory & 1 != 0 && self.opened && self.on_this_page(self.head_pages) {
            if self.head.len() != self.head_rows && !self.head_retried {
                self.head_overflow = true;
            }
        }
        self.head_retried = false;
        if self.anticipatory & 1 != 0 && self.head_rows > 0 {
            let cells = self.layout.cells;
            for k in 0..self.head_rows.min(lines.len()) {
                lines[k] = match self.head.get(k) {
                    Some(h) => side_line(h, self.head_side, cells),
                    None => Line::default(),
                };
            }
        }
        if self.anticipatory & 2 != 0 && self.opened {
            self.page_print = self.print_text().filter(|_| self.print_corner() != Corner::None);
        }
        // The number goes on the first line (at the top) or the last (at the bottom), at its start or its end; the
        // text on that line was kept clear of it.
        let at = if corner.top() { Some(0) } else if corner.bottom() { Some(room - 1) } else { None };
        if let Some(line) = at.and_then(|k| lines.get_mut(k)) {
            let len = number.chars().count();
            let col = if corner.right() { self.layout.cells.saturating_sub(len) } else { 0 };
            let mut text: Vec<char> = line.text.chars().collect();
            if text.len() < col + len {
                text.resize(col + len, ' ');
                line.sources.resize(col + len);
            }
            for (k, c) in number.chars().enumerate() {
                text[col + k] = c;
                line.sources.set(col + k, None);
            }
            line.text = text.into_iter().collect();
        }
        // The print page number in its corner.
        let print_corner = self.print_corner();
        if let Some(text) = self.page_print.take() {
            let at = if print_corner.top() { Some(0) } else if print_corner.bottom() { Some(room - 1) } else { None };
            if let Some(line) = at.and_then(|k| lines.get_mut(k)) {
                let len = text.chars().count();
                let col = if print_corner.right() { self.layout.cells.saturating_sub(len) } else { 0 };
                let mut cells: Vec<char> = line.text.chars().collect();
                if cells.len() < col + len {
                    cells.resize(col + len, ' ');
                    line.sources.resize(col + len);
                }
                for (k, c) in text.chars().enumerate() {
                    cells[col + k] = c;
                    line.sources.set(col + k, None);
                }
                line.text = cells.into_iter().collect();
            }
        }
        if self.print_number.is_some() && self.opened {
            self.print_since = self.print_since.wrapping_add(1);
        }
        self.change_line = None;
        if let Some((odd, even)) = self.next_print.take() {
            self.print_odd = odd;
            self.print_even = even;
        }
        // The guide-word footer composed (spec 19 10.4): two or more entries on the page give the first, the dash text
        // and the last; one gives it; none gives the last entry before, a space and the continuation text; cut at the
        // width less the foot's side room. The next page carries on in mode 1.
        if self.guide_mode > 0 {
            if let Some(foot) = &mut self.page_foot {
                let text = match self.guide_mode {
                    3 => format!("{}{}{}", self.guide_first, self.guide_dash, self.guide_last),
                    2 => self.guide_first.clone(),
                    _ => format!("{} {}", self.guide_last, self.guide_cont),
                };
                let room = self.layout.cells.saturating_sub(self.foot_side.left + self.foot_side.right);
                *foot = text.chars().take(room).collect();
            }
            self.guide_mode = 1;
        }
        // The running foot on the last line, sharing it with a page number at the bottom.
        if let Some(foot) = self.page_foot.take() {
            let placed = side_line(&foot, self.foot_side, self.layout.cells);
            let line = &mut lines[room - 1];
            let mut text: Vec<char> = line.text.chars().collect();
            let width = placed.text.chars().count();
            if text.len() < width {
                text.resize(width, ' ');
                line.sources.resize(width);
            }
            for (k, c) in placed.text.chars().enumerate() {
                if c != ' ' {
                    text[k] = c;
                    line.sources.set(k, None);
                }
            }
            line.text = text.into_iter().collect();
        }
        self.opened = false;
        self.head_rows = 0;
        self.odd = !self.odd;
        // A page that filled inside a block that started on it is laid out again, once (Formatter::run_from).
        self.keep_overflow = self.keep_start.filter(|_| self.keep_depth > 0 && !self.retried);
        self.keep_depth = 0;
        self.keep_start = None;
        self.attach_until = None;
        self.retried = false;
        let start = self.page_start.take().unwrap_or(self.last_start);
        self.last_start = start;
        let print_labels = std::mem::take(&mut self.labels);
        let roman = self.page_text.take().map_or(self.roman, |(_, roman)| roman);
        let number = crate::range::PageNumber { number: self.counter as u32, letter: self.prefix, roman };
        let tables = std::mem::take(&mut self.page_tables);
        let references = std::mem::take(&mut self.page_refs);
        self.pages.push(Page { lines, start, print_labels, number, tables, references });
        // The next page: its counter, and the number settings changed on this one.
        self.counter = self.next_counter.take().unwrap_or(self.counter + 1);
        if let Some((odd, even)) = self.next_numbers.take() {
            self.numbers_odd = odd;
            self.numbers_even = even;
        }
        if let Some(first) = self.next_first_shown.take() {
            self.first_number_shown = first;
        }
    }

    fn new_page(&mut self) {
        if self.line_has_text() {
            self.end_line(self.left);
        }
        // A page its last line already finished is not finished again (that would be a blank page).
        if !self.page.is_empty() || self.pages.is_empty() {
            self.finish_page();
        }
        self.start_col = self.left;
    }

    /// Places a word where a tab, flush right or [run] put it: the column from the justification, within the line;
    /// the line ends first when the word would be behind the pen (or in a page number's room) and the code may wrap;
    /// the fill runs from the pen to the word.
    fn placed_word(&mut self, cells: &[(char, Option<usize>)], place: Place) {
        let width = self.layout.cells;
        let len = cells.len();
        let mut wrapped = false;
        let col = loop {
            let (room_left, room_right) = self.rooms();
            let target = place.col.unwrap_or((width - room_right.min(width)) as i64);
            let decimal = self.decimal;
            let start = match place.just {
                'R' => target + 1 - len as i64,
                'C' => target + 1 - (len as i64 + 1) / 2,
                'D' => target - cells.iter().position(|c| c.0 == decimal).unwrap_or(len.saturating_sub(1)) as i64,
                _ => target,
            };
            let col = (start.clamp(1, (width as i64 - len as i64 + 1).max(1)) - 1) as usize;
            let used = self.line.text.chars().count();
            let has_text = self.line_has_text();
            let behind = col < used + usize::from(has_text);
            let blocked = col < room_left || col + len > width.saturating_sub(room_right);
            // [taa] and [lpr] put the word at the column whatever is there.
            if place.overwrite {
                break col;
            }
            // [tas]: behind the pen, the word goes its minimum spacing after the pen when it still fits.
            if let Some(s) = place.spacing.filter(|_| behind && has_text) {
                let after = used + s;
                if after + len <= width.saturating_sub(room_right) {
                    break after;
                }
            }
            if place.may_wrap && has_text && (behind || blocked) && !wrapped {
                wrapped = true;
                self.touching = false;
                self.end_line(self.runover());
                continue;
            }
            break if behind { used + usize::from(has_text) } else { col };
        };
        let used = self.line.text.chars().count();
        for at in used..col {
            let edge = place.partial && (at == used || at + 1 == col);
            let c = match place.fill {
                Some(f) if !edge && self.line_has_text() => f,
                _ => ' ',
            };
            self.put(c, None);
        }
        for (k, &(c, s)) in cells.iter().enumerate() {
            let at = if place.overwrite { col + k } else { self.line.text.chars().count() };
            self.place_word_marks(k, at);
            self.put_at(at, c, s);
        }
        let at = if place.overwrite { col + len } else { self.line.text.chars().count() };
        self.place_word_marks(len, at);
        self.touching = false;
    }

    /// Places one word (its cells and their sources), wrapping it to the runover margin when it does not fit.
    /// Lays out a word. After an [mfcs], a word placed with a right margin of 6 that a [>] or [fr] and a word follow
    /// keeps only that word's length and one on the right (spec 36 section 13: the page number of a contents line).
    fn word(&mut self, cells: &[(char, Option<usize>)]) {
        let next = self.next_right_word.take();
        match next {
            Some(len) if self.mfc_met && len > 0 && self.right.max(self.number_room) == 6 => {
                let kept = (self.right, self.number_room);
                self.right = len + 1;
                self.number_room = 0;
                self.lay_word(cells);
                (self.right, self.number_room) = kept;
            }
            _ => self.lay_word(cells),
        }
    }

    fn lay_word(&mut self, cells: &[(char, Option<usize>)]) {
        if std::mem::take(&mut self.skip_next) {
            self.word_marks.clear();
            return;
        }
        // A word a code took (the positioned-word modes 0, 4, 6, 7, 8 of the standard program): not laid out.
        if let Some(kind) = self.next_word.take() {
            let text: String = cells.iter().take(36).map(|c| c.0).collect();
            match kind {
                'l' => self.print_cont = Some(text),
                'c' => self.guide_cont = text,
                'g' => self.guide_pending = Some(text),
                'd' => self.guide_dash = text,
                _ => {}
            }
            self.word_marks.clear();
            return;
        }
        if let Some((_, r)) = &mut self.running {
            r.push(cells.iter().map(|c| c.0).collect());
            self.word_marks.clear();
            return;
        }
        if let Some(h) = &mut self.heading {
            // The heading's words are centred later; places in them with no cell fall back to the cell before.
            h.0.push((cells.iter().map(|c| c.0).collect(), cells.iter().map(|c| c.1).collect()));
            self.word_marks.clear();
            return;
        }
        if self.ref_open.is_some() {
            let page = self.page_number_text();
            let print = self.print_number.clone().unwrap_or_default();
            if let Some(r) = &mut self.ref_open {
                if !std::mem::replace(&mut self.ref_first, true) {
                    r.first_page = page.clone();
                    r.first_print = print.clone();
                }
                r.last_page = page;
                r.last_print = print;
            }
        }
        if let Some((col, just)) = self.line_number.take() {
            self.open_page();
            self.take_line_number(cells, col, just);
            self.word_marks.clear();
            // Text since the last forced line break: the next word follows as after [pm] with the [svpal] values.
            if self.line_ended {
                self.magazine = Some((self.svpal.0, Some(self.svpal.1)));
            }
            return;
        }
        if std::mem::take(&mut self.lea) {
            self.print_page(cells);
            return;
        }
        self.open_page();
        if let Some(place) = self.place.take() {
            self.placed_word(cells, place);
            self.word_break = 0;
            return;
        }
        // [pmN:C] magazine paragraph: N spaces on in the line when the word fits after them, else a paragraph
        // (C: -2 at the left margin, -3 at the runover point, else that cell).
        if let Some((spaces, c)) = self.magazine.take() {
            let used = self.line.text.chars().count();
            let spaces = spaces.max(0) as usize;
            if self.line_has_text() && used + spaces + cells.len() <= self.width() {
                for _ in 0..spaces {
                    self.put(' ', None);
                }
                self.touching = true;
            } else {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let (col, skip, from_margin) = self.paragraph;
                self.skip_lines(skip);
                let col = match c {
                    Some(-2) => self.left,
                    Some(-3) => self.runover(),
                    Some(p) if p > 0 => (p - 1) as usize,
                    _ => (col - 1).max(0) as usize + if from_margin { self.left } else { 0 },
                };
                self.start_col = col.min(self.layout.cells.saturating_sub(1));
            }
        }
        let word_break = std::mem::take(&mut self.word_break);
        let mut cells: Vec<(char, Option<usize>)> = cells.to_vec();
        let mut flags = vec![false; cells.len()];
        for &k in &self.word_breaks {
            if k < flags.len() {
                flags[k] = true;
            }
        }
        self.word_breaks.clear();
        self.last_word = cells.iter().map(|c| c.0).collect();
        // Without the document's table the None table applies: marked breaks only.
        let table = self.layout.hyphen.clone().unwrap_or_else(crate::hyphen::HyphenTable::none);
        // The word placed as the standard program places it (spec 36 section 4): whole when it fits after the pending
        // space; else the first piece up to a break point the hyphenation table finds (a marked break, a dash, an
        // internal hyphen, or an automaton break with a hyphen inserted); else, for a word longer than a whole
        // line, cut at the line end with no mark when at least one cell is free; else the whole word goes to the
        // next line. The remainder goes on from the runover margin, searched and cut again while it is longer than
        // a line. [wb-no] searches for no break; [wb-cb] breaks a computer-braille word after punctuation (or at
        // the room less the continuation string) and writes the continuation string after the first part.
        let n = cells.len();
        let width = self.width();
        let used = self.line.text.chars().count();
        let has_text = self.line_has_text();
        let gap = usize::from(has_text && !self.touching);
        let col = if has_text { used } else { self.start_col.max(used).max(self.rooms().0) };
        let mut placed = 0usize;
        let mut k = 0usize;
        if col + gap + n <= width {
            self.put_piece(&cells, 0, n, &mut k, gap, used, col);
            self.place_word_marks(k, self.line.text.chars().count());
            self.touching = false;
            return;
        }
        if word_break == 2 {
            self.computer_braille_word(cells, gap, used, col);
            return;
        }
        if word_break == 0 {
            let room = width as i64 - col as i64 - gap as i64;
            let before = cells.len();
            // The fourth value is the whole room (spec 36 section 14): a word longer than a whole line with no break
            // point gets a hyphen after its last cell that is not a prefix indicator.
            let bp = crate::hyphen::break_point(&table, crate::hyphen::Word { cells: &mut cells, flags: &mut flags }, 0, room, width as i64, self.wb_min, self.hyphen_override);
            if cells.len() > before {
                self.shift_word_marks(bp.saturating_sub(1));
            }
            if bp > 0 {
                placed = bp.min(width.saturating_sub(col + gap));
                self.put_piece(&cells, 0, placed, &mut k, gap, used, col);
            }
        }
        if placed == 0 && cells.len() > width && col + gap < width {
            placed = width - col - gap;
            self.put_piece(&cells, 0, placed, &mut k, gap, used, col);
        }
        while placed < cells.len() {
            self.touching = false;
            self.end_line(self.runover());
            if cells[placed].0 == ' ' {
                placed += 1;
                k += 1;
                if placed >= cells.len() {
                    break;
                }
            }
            let width = self.width();
            let used = self.line.text.chars().count();
            let col = self.start_col.max(used).max(self.rooms().0);
            let left = cells.len() - placed;
            let mut piece = width.saturating_sub(col).min(left);
            // Every remainder longer than the line is searched again, in [wb-no] too (the standard program tests the
            // mode only before the first piece, spec 36 section 14).
            if width < col + left {
                let before = cells.len();
                let bp = crate::hyphen::break_point(&table, crate::hyphen::Word { cells: &mut cells, flags: &mut flags }, placed, (width as i64 - col as i64).max(0), width as i64, self.wb_min, self.hyphen_override);
                if cells.len() > before {
                    self.shift_word_marks(bp.saturating_sub(1));
                }
                if bp > placed {
                    piece = bp - placed;
                }
            }
            if piece == 0 {
                piece = left.min(1);
            }
            self.put_piece(&cells, placed, piece, &mut k, 0, used, col);
            placed += piece;
        }
        let col = self.line.text.chars().count();
        self.place_word_marks(k, col);
        self.touching = false;
    }

    /// Puts `count` cells of `cells` from `from` on the line: after a space when `gap` is 1, else from column `col`
    /// of an empty line (spaces up to it from the `used` cells).
    fn put_piece(&mut self, cells: &[(char, Option<usize>)], from: usize, count: usize, k: &mut usize, gap: usize, used: usize, col: usize) {
        if self.line_has_text() {
            if gap == 1 {
                self.put(' ', None);
            }
        } else {
            for _ in used..col {
                self.put(' ', None);
            }
        }
        for &(c, s) in &cells[from..from + count] {
            let at = self.line.text.chars().count();
            self.place_word_marks(*k, at);
            self.put(c, s);
            *k += 1;
        }
    }

    /// A hyphen was inserted into the word after cell index `at`: the marks of the cells after it move up one.
    fn shift_word_marks(&mut self, at: usize) {
        for m in &mut self.word_marks {
            if m.1 > at {
                m.1 += 1;
            }
        }
    }

    /// A [wb-cb] word that does not fit (spec 36 section 6): broken after the last punctuation cell past the third
    /// within the room less the continuation string, else at that room; the continuation string follows the first
    /// part; the remainder goes on from the runover margin. INFERENCE: the standard program first looks ahead to the next
    /// computer-braille word and may join or move the whole word (its five cases are not traced, spec 36 unknown
    /// 10); here the word is placed whole when it fits, else split.
    fn computer_braille_word(&mut self, cells: Vec<(char, Option<usize>)>, gap: usize, used: usize, col: usize) {
        let mut rest: &[(char, Option<usize>)] = &cells;
        let mut k = 0usize;
        let (mut gap, mut used, mut col) = (gap, used, col);
        loop {
            let width = self.width();
            let n = rest.len();
            if col + gap + n <= width {
                self.put_piece(rest, 0, n, &mut k, gap, used, col);
                break;
            }
            let cont = self.cb_cont.clone();
            let room = width.saturating_sub(col).saturating_sub(cont.chars().count());
            let mut count = room.min(n);
            let split = crate::hyphen::cb_split(rest, room).unsigned_abs() as usize;
            if split > 0 && split < count {
                count = split;
            }
            if count == 0 && self.line_has_text() {
                self.touching = false;
                self.end_line(self.runover());
                used = self.line.text.chars().count();
                col = self.start_col.max(used).max(self.rooms().0);
                gap = 0;
                continue;
            }
            let count = count.max(1).min(n);
            self.put_piece(rest, 0, count, &mut k, gap, used, col);
            for c in cont.chars() {
                if self.line.text.chars().count() < width {
                    self.put(c, None);
                }
            }
            rest = &rest[count..];
            if rest.is_empty() {
                break;
            }
            self.touching = false;
            self.end_line(self.runover());
            used = self.line.text.chars().count();
            col = self.start_col.max(used).max(self.rooms().0);
            gap = 0;
        }
        let at = self.line.text.chars().count();
        self.place_word_marks(k, at);
        self.touching = false;
    }

    fn code(&mut self, code: &str) {
        if std::mem::take(&mut self.skip_next) {
            return;
        }
        // [ifprt] keeps the next item only in a print document, [ifbrl] only in a braille one.
        if code == "ifprt" || code == "ifbrl" {
            self.skip_next = (code == "ifprt") == self.layout.braille;
            return;
        }
        // [htbfmtN:FFF] the options pair N and the automatic layout letters FFF of the next table (spec 35 1.1);
        // [htbs...] starts a table; [htbe] outside a table is nothing.
        if let Some(rest) = code.strip_prefix("htbfmt") {
            let (n, letters) = rest.split_once(':').unwrap_or((rest, ""));
            self.htbfmt = (n.trim().parse().unwrap_or(0), letters.replace(':', ""));
            return;
        }
        if code.starts_with("htbs") {
            self.start_table(code);
            return;
        }
        if code == "htbe" {
            return;
        }
        // [wbP] the cells before an automaton hyphenation break (P - 1); [wb] alone the table's (spec 36 section 3).
        if let Some(rest) = code.strip_prefix("wb") {
            if rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()) {
                self.wb_min = rest.parse::<i64>().map_or(-1, |p| p - 1);
            }
            return;
        }
        // [svcbhN]: 0 or 2 the last word's cells (up to ten) become the computer-braille continuation string; 1 or
        // 2 its first cell the hyphen for inserted breaks; 3 resets that to the table's.
        if let Some(rest) = code.strip_prefix("svcbh") {
            let n: i64 = rest.trim().parse().unwrap_or(0);
            if n == 0 || n == 2 {
                self.cb_cont = self.last_word.iter().take(10).collect();
            }
            if n == 1 || n == 2 {
                if let Some(&c) = self.last_word.first().filter(|c| (0x20..0x80).contains(&(**c as u32))) {
                    self.hyphen_override = Some(c);
                }
            }
            if n == 3 {
                self.hyphen_override = None;
            }
            return;
        }
        // [fr;F~X] the next word flush right (before a page number's room), with fill F ('p' partial) of the first
        // character of X (the second in a print document).
        if code == "fr" || code.starts_with("fr;") || code.starts_with("fr~") {
            let rest = code[2..].trim_start_matches(';');
            let (kind, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
            let fill = x.and_then(|x| x.chars().nth(if self.layout.braille { 0 } else { 1 }).or_else(|| x.chars().next()));
            self.place = Some(Place {
                col: None,
                just: 'R',
                fill,
                partial: kind.trim().chars().next().is_some_and(|k| k.eq_ignore_ascii_case(&'p')),
                may_wrap: true,
                spacing: None,
                overwrite: false,
            });
            return;
        }
        // [stbS:A:P:F~X] tab stop S (1 to 9, 1 when not given) justified A (L) at cell P (1; -1 the middle of the
        // line), clamped to the line, with fill kind F (none) and fill character X (the second character in a print
        // document, as [fr]); pgifmt case 0x5d.
        if let Some(rest) = code.strip_prefix("stb") {
            let (params, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
            let parts: Vec<&str> = params.split(':').collect();
            let number = |k: usize| parts.get(k).and_then(|p| p.trim().parse::<i64>().ok());
            let letter = |k: usize| parts.get(k).and_then(|p| p.trim().chars().next()).map(|c| c.to_ascii_uppercase());
            let s = number(0).unwrap_or(1).clamp(1, 9) as usize;
            let width = self.layout.cells as i64;
            let col = match number(2).unwrap_or(1) {
                -1 => (width + 1) / 2,
                c => c,
            };
            let fill = x.and_then(|x| {
                let second = if self.layout.braille { None } else { x.chars().nth(1) };
                second.or_else(|| x.chars().next())
            });
            self.stops[s - 1] = Stop {
                col: col.clamp(1, width.max(1)),
                just: letter(1).unwrap_or('L'),
                kind: letter(3).unwrap_or(' '),
                fill: fill.unwrap_or(' '),
            };
            return;
        }
        // [#S:F~X] the next word at stop S (1 to 9), with fill kind F and character X in place of the stop's (case
        // 0x48).
        if let Some(rest) = code.strip_prefix('#') {
            let (params, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
            let parts: Vec<&str> = params.split(':').collect();
            let s = parts.first().and_then(|p| p.trim().parse::<i64>().ok()).unwrap_or(0).clamp(1, 9) as usize;
            let mut stop = self.stops[s - 1];
            if let Some(k) = parts.get(1).and_then(|p| p.trim().chars().next()) {
                stop.kind = k.to_ascii_uppercase();
            }
            if let Some(c) = x.and_then(|x| x.chars().next()) {
                stop.fill = c;
            }
            self.place = Some(Place::at_stop(&stop));
            return;
        }
        // [>] the next word at the nearest stop at or past the cell after the pen (and past a tab already pending),
        // counted with the [svlma] adjustment; with none, flush right at the right margin, no fill (case 0x49).
        // In a centred heading [>] puts the next word at the next tab stop on the heading's line before it is centred
        // (the standard program builds the heading's line with the positioned word, then centres it).
        if code == ">" {
            if let Some(h) = &mut self.heading {
                h.0.push(("	".into(), Vec::new()));
                return;
            }
        }
        if code == ">" && self.fill.is_some() {
            // In a whole-line fill [>] marks the column after the pen (and the space to come) as a field's edge.
            let pen = self.line.text.chars().count() as i64 + i64::from(self.line_has_text() && !self.touching);
            let last = self.layout.cells as i64 - 1;
            if let Some((_, _, stops)) = &mut self.fill {
                if stops.len() < 2 {
                    stops.push(pen.min(last));
                }
            }
            return;
        }
        if code == ">" {
            let pen = self.line.text.chars().count() as i64;
            let mut from = pen + 1 + self.lma;
            if let Some(Place { col: Some(c), may_wrap: true, .. }) = self.place {
                from = from.max(c + self.lma);
            }
            let next = self.stops.iter().filter(|st| st.col >= from).min_by_key(|st| st.col);
            self.place = Some(match next {
                Some(stop) => Place::at_stop(stop),
                None => Place {
                    col: Some(self.layout.cells.saturating_sub(self.right) as i64),
                    ..Place::at(1, 'R')
                },
            });
            return;
        }
        if let Some(x) = code.strip_prefix("svdac~") {
            if let Some(c) = x.chars().next() {
                self.decimal = c;
            }
            return;
        }
        // [svgan~X...] the number sign X ("{" none) and the ten digits after it (pgifmt case 0xa1); [svgrnN~X] the
        // roman indicator X ("{" none) and, N not 0, doubled before a numeral of more than one letter (case 0xa2).
        if let Some(rest) = code.strip_prefix("svgan") {
            if let Some((_, x)) = rest.split_once('~') {
                let mut chars = x.chars();
                if let Some(sign) = chars.next() {
                    self.number_sign = (sign != '{').then_some(sign);
                }
                for (k, c) in chars.take(10).enumerate() {
                    self.digits[k] = c;
                }
            }
            return;
        }
        if let Some(rest) = code.strip_prefix("svgrn") {
            let (params, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
            self.roman_double = params.trim().parse::<i64>().unwrap_or(0) != 0;
            if let Some(sign) = x.and_then(|x| x.chars().next()) {
                self.roman_sign = (sign != '{').then_some(sign);
            }
            return;
        }
        if code.starts_with("kdtx") {
            self.deferred_text(code);
            return;
        }
        // [leaF~X] the next word is the print page number (pgifmt case 99): F the fill kind (a letter, 'F' when not
        // given), X the fill character (dots 3-6 when not given).
        if let Some(rest) = code.strip_prefix("lea") {
            if rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '~' || c == ':') {
                // The notes of the first collection [kdtxpN~X] put on the list go before the print page number.
                if self.placing.is_none() && self.place_notes_at_lea(code) {
                    return;
                }
                let (params, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
                let kind = params.trim_start_matches(':').chars().next().map_or('F', |c| c.to_ascii_uppercase());
                let fill = x.and_then(|x| x.chars().next()).unwrap_or('-');
                self.lea_fill = (kind, fill);
                self.lea = true;
                return;
            }
        }
        // [pvN~X] the page number N (the page being filled when it has not opened, else the next), with X before it
        // ("{" or a space none) when a text is given (case 0x89).
        if let Some(rest) = code.strip_prefix("pv") {
            if rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_digit() || c == '~') {
                let (params, x) = rest.split_once('~').map_or((rest, None), |(a, b)| (a, Some(b)));
                if let Ok(n) = params.trim().parse::<usize>() {
                    if self.opened {
                        self.next_counter = Some(n);
                    } else {
                        self.counter = n;
                    }
                }
                if let Some(x) = x {
                    self.prefix = x.chars().next().filter(|&c| c != ' ' && c != '{');
                }
                return;
            }
        }
        let (name, n) = split_code(code);
        let name = if name.starts_with("tab~") {
            "tab"
        } else if name.starts_with("mfcs~") {
            "mfcs"
        } else if name.starts_with("wlf~") {
            "wlf"
        } else {
            name
        };
        let first = n.first().copied();
        match name {
            // [ctb] every stop at the line end, justified right, no fill (case 0x4c).
            "ctb" => {
                let col = self.layout.cells as i64;
                self.stops = [Stop { col, just: 'R', kind: ' ', fill: ' ' }; 9];
            }
            // [tasP] the next word at cell P, or its minimum spacing after the pen; [taaP] at cell P whatever is
            // there; [svmtsN] that spacing.
            "tas" => self.place = Some(Place { spacing: Some(self.min_spacing), ..Place::at(first.unwrap_or(1), 'L') }),
            "taa" => self.place = Some(Place { may_wrap: false, overwrite: true, ..Place::at(first.unwrap_or(1), 'L') }),
            "svmts" => self.min_spacing = first.unwrap_or(1).max(0) as usize,
            // [lps] saves the pen column; [lpr] moves the pen back there, so what follows is written over what is there
            // (pgifmt cases 0x74, 0x75: the pen, not a positioned word; the saved column is 0 again at every line end).
            "lps" => self.saved_col = Some(self.line.text.chars().count()),
            "lpr" => {
                let col = self.saved_col.unwrap_or(0);
                self.place = Some(Place { may_wrap: false, overwrite: true, ..Place::at(col as i64 + 1, 'L') });
            }
            "svlma" => self.lma = first.unwrap_or(0),
            // [sknL:M] takes back L of the lines the last skip left (M more cancel lines of the next skip).
            "skn" => {
                let l = first.unwrap_or(1).max(0) as usize;
                let m = n.get(1).copied().unwrap_or(0).max(0) as usize;
                let back = l.min(self.pending_blank);
                let more = m.min(self.pending_blank - back);
                for _ in 0..back + more {
                    self.page.pop();
                }
                self.pending_blank -= back + more;
                self.skip_debt += (m - more) as i64;
            }
            // [sdN] the next page is odd (1) or even (0).
            "sd" => self.wanted_odd = Some(first.unwrap_or(1) != 0),
            // [pmN:C]: N 3 when not given (case 0x71).
            "pm" => self.magazine = Some((first.unwrap_or(3), n.get(1).copied())),
            // [linenumC:A] (case 0xaa) and [svpalS:N] (case 0xb7).
            "linenum" => {
                let just = code.split(':').nth(1).and_then(|a| a.chars().next()).filter(|c| c.is_ascii_alphabetic());
                self.line_number = Some((first.unwrap_or(-1), just.map_or('R', |c| c.to_ascii_uppercase())));
            }
            "svpal" => self.svpal = (first.unwrap_or(3), n.get(1).copied().unwrap_or(-3)),
            // [mfcs~X] (case 0xb1) sets the level margins as [hi] would for the levels to its [mfce]: first cell 1,
            // runover two cells past the last level ("exercisenemeth" at most 2 levels, "contents" its own third
            // and seventh values); [mfce] (0xb2) sets them back to [hi1:1] (spec 19 section 10.8).
            "mfcs" => {
                self.mfc_met = true;
                let kind = code.split_once('~').map_or("", |(_, x)| x);
                let mut levels = self.mfc_levels;
                let (third, seventh) = match kind {
                    "contents" => (6, 6),
                    "exercisenemeth" => {
                        levels = levels.min(2);
                        (0, 2)
                    }
                    _ => (0, 6),
                };
                self.code(&format!("hi1:{}:{third}:2:0:0:{seventh}:-1:-1:-1:0", levels * 2 + 1));
            }
            "mfce" => self.code("hi1:1:0:0:0:0:-1:-1:-1:-1:0"),
            // [wlf~X] (case 0xba): the line is filled at its end with style X of the line fill styles (the default
            // style when X is not one of them); [>] marks its fields instead of tabbing.
            "wlf" => {
                let name: String = code.split_once('~').map_or(String::new(), |(_, x)| x.chars().take(19).collect());
                let (_, cell, fields) = LINE_FILLS.iter().find(|f| f.0 == name).unwrap_or(&LINE_FILLS[0]);
                let c = match *cell as u32 {
                    0x2800..=0x28ff => crate::emboss::NA_CELLS[(*cell as u32 & 0x3f) as usize].to_ascii_lowercase() as char,
                    _ => *cell,
                };
                self.fill = Some((c, *fields, Vec::new()));
            }
            // [foldsN:W:P...~X] (case 0xa5): N 2 when not given, 0 to (lines - 4) / 2; W 1, 0 to (lines - 4) / N.
            "folds" => {
                let most = (self.layout.lines as i64 - 4).max(0) / 2;
                let folds = first.unwrap_or(2).clamp(0, most);
                let mut w = self.folds.1 as i64;
                if folds != 0 {
                    w = n.get(1).copied().unwrap_or(1).clamp(0, (self.layout.lines as i64 - 4).max(0) / folds);
                }
                let mut at = [0; 9];
                for (k, &p) in n.iter().skip(2).take(9).enumerate() {
                    at[k] = p;
                }
                let marker = code.split_once('~').map_or(Vec::new(), |(_, x)| x.chars().take(9).collect());
                self.folds = (folds as usize, w as usize, at, marker);
            }
            // [rpsN] (case 0x9b) opens a reference point when none is open; [rpe] (0x9c) keeps it when a word came in
            // it; [svrpt] (0x9d) does nothing here.
            "rps" => {
                if self.ref_open.is_none() {
                    self.ref_open = Some(ReferencePoint { start: self.last_place, value: first.unwrap_or(0), ..Default::default() });
                    self.ref_first = false;
                }
            }
            "rpe" => {
                if let Some(mut r) = self.ref_open.take() {
                    if std::mem::take(&mut self.ref_first) {
                        r.end = self.last_place;
                        self.page_refs.push(r);
                    }
                }
            }
            "svant" => self.anticipatory = first.unwrap_or(0).clamp(0, 3),
            "<" => {
                self.interrupt_keep(|f| {
                    let next = f.left;
                    f.end_line(next);
                });
            }
            "l" => {
                self.line_ended = false;
                if self.line_has_text() {
                    self.end_line(self.left);
                }
            }
            "p" => {
                self.line_ended = false;
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                // The lines a paragraph skips (print 1, braille 0, or [svprg]), at the top of a page too, and its first
                // cell: braille the third, print the first, counted from the edge of the page (or from the left
                // margin when [svprg] says so).
                let (col, skip, from_margin) = self.paragraph;
                self.skip_lines(skip);
                let col = (col - 1).max(0) as usize + if from_margin { self.left } else { 0 };
                self.start_col = col.min(self.layout.cells.saturating_sub(1));
            }
            "cp" => {
                // New page if fewer than N + 1 lines remain, else a new line and L blank lines (braille 1 and 0,
                // print 2 and 1).
                let (need, skip) = if self.layout.braille { (1, 0) } else { (2, 1) };
                let need = first.unwrap_or(need);
                let skip = n.get(1).copied().unwrap_or(skip);
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let remaining = self.capacity().saturating_sub(self.page.len()) as i64;
                if remaining < need + 1 && !self.page.is_empty() && self.keep_depth == 0 {
                    self.finish_page();
                } else if !self.page.is_empty() {
                    self.skip_lines(skip);
                }
            }
            "sk" => {
                // [skN]: [l], then N lines (0 when not given, at most three pages' lines); a negative N goes to the
                // line -N from the page end.
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let n = first.unwrap_or(0);
                let count = if n < 0 { self.capacity() as i64 - self.page.len() as i64 + n } else { n };
                self.skip_lines(count);
            }
            "ind" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                self.left = self.margin(first, n.get(1).copied());
                self.start_col = self.left;
            }
            "inm" => {
                self.left = self.margin(first, n.get(1).copied());
                if !self.line_has_text() {
                    self.start_col = self.left;
                }
            }
            // [ptys] alone is a runover two cells in.
            "ptys" => self.runover_offset = first.unwrap_or(2).max(0),
            "ptye" => self.runover_offset = 0,
            // [hiL:V:R:I:J:K:M:N:O:P:Q:S:T:U] level margins, level 1 at once (spec 19 7.3): L and V the first cells
            // (1-based) of a level-1 first line and runover line, R its right margin, I J K the increments per level,
            // M the most levels, N O P limits; with Q not 0, L and V are added to the cell of the first character of
            // the line above (Q 1: the nearest line with text; else the line just above, 0 when blank) and the
            // [svlma] adjustment is left out; S, T and U replace the [p] settings (start cell, lines skipped, from the
            // margin).
            "hi" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let get = |k: usize, default: i64| n.get(k).copied().unwrap_or(default);
                let relative = get(10, 0) != 0;
                let (l, v) = if relative {
                    let base = self.above_cell(get(10, 0) == 1);
                    (base + get(0, 1) + 1, base + get(1, 1) + 1)
                } else {
                    (get(0, 1) + self.lma, get(1, 1))
                };
                if n.len() > 11 {
                    self.paragraph = (get(11, self.paragraph.0), get(12, self.paragraph.1), get(13, 0) != 0);
                }
                self.levels = Some([
                    l,
                    v,
                    get(2, 0),
                    get(3, 0),
                    get(4, 0),
                    get(5, 0),
                    get(6, -1),
                    get(7, -1),
                    get(8, -1),
                    get(9, -1),
                ]);
                self.set_level(Some(1));
                self.start_col = self.left;
            }
            // [hlN] level N: acts as [l], then the level's margins.
            "hl" => {
                self.line_ended = false;
                self.interrupt_keep(|f| {
                    if f.line_has_text() {
                        f.end_line(f.left);
                    }
                });
                self.set_level(first);
                self.start_col = self.left;
            }
            // [svrlbN] keep N lines blank at the bottom of the page.
            "svrlb" => self.reserve = first.unwrap_or(0).clamp(0, self.layout.lines as i64 - 1) as usize,
            // [svprgN:L:M] where paragraphs start and the lines they skip; alone, the document kind's.
            "svprg" => {
                let default = if self.layout.braille { (3, 0) } else { (1, 1) };
                self.paragraph = (
                    first.unwrap_or(default.0),
                    n.get(1).copied().unwrap_or(default.1),
                    n.get(2).copied().unwrap_or(0) != 0,
                );
            }
            // [slN] N lines, the first ending a line with text; [stL] to line L of the page.
            "sl" | "st" => {
                self.line_ended = false;
                let target = first.unwrap_or(if name == "st" { 1 } else { 0 });
                let mut count = if name == "st" {
                    target.clamp(1, self.layout.lines as i64) - 1 - self.page.len() as i64
                } else {
                    target
                };
                if self.line_has_text() {
                    self.end_line(self.left);
                    count -= 1;
                }
                self.skip_lines(count);
            }
            "ki" => self.keep_interrupt = first.unwrap_or(0) != 0,
            // [rmN:O] (case 0x66): O at least 0 is an override margin (at most the line less 4) the line number
            // reserve does not go below, and starts a section of the page for that reserve.
            "rm" => {
                self.right = first.unwrap_or(0).clamp(0, 159) as usize;
                if let Some(&o) = n.get(1).filter(|&&o| o >= 0) {
                    let o = o.min(self.layout.cells as i64 - 4).max(0) as usize;
                    self.override_margin = o;
                    self.room_section += 1;
                    self.number_room = self.room_lookup(o);
                }
            }
            // [svsblN]: N blank lines after every line (1 when not given), at most (lines - 4) / 2.
            "svsbl" => {
                let most = (self.layout.lines as i64 - 4).max(0) / 2;
                self.blank_between = first.unwrap_or(1).clamp(0, most) as usize;
            }
            "pg" => {
                self.line_ended = false;
                // [pgN~X]: the next page is numbered N, with X before its number.
                if let Some(n) = first.filter(|&n| n > 0) {
                    self.next_counter = Some(n as usize);
                }
                if self.after_page_code && !self.page_has_text() {
                    // Two page codes in a row: a blank page (with its head and number).
                    self.open_page();
                    self.finish_page();
                } else {
                    self.new_page();
                }
                if let Some((_, x)) = code.split_once('~') {
                    self.prefix = x.chars().next().filter(|&c| c != ' ');
                }
                self.after_page_code = true;
            }
            // [svpnpA:B] where page numbers go from the next page, A on odd and B on even pages (0 none, 1 upper left,
            // 2 upper right, 3 lower left, 4 lower right); alone, lower right on both.
            "svpnp" => {
                let (odd, even, p_odd, p_even) = if n.is_empty() {
                    (4, 4, 2, 2)
                } else {
                    let get = |k: usize| n.get(k).copied().unwrap_or(0);
                    (get(0), get(1), get(2), get(3))
                };
                let corner = |v: i64| Corner::from_code(v.clamp(0, 4) as u32);
                self.next_numbers = Some((corner(odd), corner(even)));
                // A print position equal to the braille one on the same pages is off.
                let p_odd = if p_odd == odd { 0 } else { p_odd };
                let p_even = if p_even == even { 0 } else { p_even };
                self.next_print = Some((corner(p_odd), corner(p_even)));
            }
            // [lea] the next word is the print page number.
            // [pntr] roman and [pnta] arabic page numbers, from the next number taken (case 0x76, 0x77).
            "pntr" => self.roman = true,
            "pnta" => self.roman = false,
            // [remw] drops the next word; [lec], [mec], [meg], [med] take it (cases 100, 0x78, 0x82, 0x83, 0xb8).
            "remw" => {
                self.next_word = Some('r');
                self.place = None;
            }
            "lec" | "mec" | "meg" | "med" => {
                self.next_word = Some(match name {
                    "lec" => 'l',
                    "mec" => 'c',
                    "meg" => 'g',
                    _ => 'd',
                });
                self.place = None;
            }
            // [led] ends the print page number (case 0x7f): none is shown from then.
            "led" => self.print_number = None,
            // [vcsD:N] vertical centering, N of D parts of a page's free lines above its text (D 2, N 1, N at most
            // D); [vce] ends it (cases 0xa8, 0xa7).
            "vcs" => {
                let den = n.first().copied().unwrap_or(2).max(1) as usize;
                let num = (n.get(1).copied().unwrap_or(1).max(1) as usize).min(den);
                self.vcenter = Some((num, den));
            }
            "vce" => self.vcenter = None,
            // [svlesN:M:L:P] the print page line (N 0 American, 1 British), continuation letters (M 1 none), the bottom
            // tolerance L and where the text after the line starts (P 0, 1, 2); alone, the file's values 0:0:2:0
            // (case 0x8a: N and M 0..1, L 0..4, P 0..2; a value not given stays).
            "svles" => {
                if n.is_empty() {
                    self.leaf = (false, false, 2, 0);
                } else {
                    let get = |k: usize| n.get(k).copied();
                    if let Some(v) = get(0) {
                        self.leaf.0 = v.clamp(0, 1) == 1;
                    }
                    if let Some(v) = get(1) {
                        self.leaf.1 = v.clamp(0, 1) == 1;
                    }
                    if let Some(v) = get(2) {
                        self.leaf.2 = v.clamp(0, 4) as usize;
                    }
                    if let Some(v) = get(3) {
                        self.leaf.3 = v.clamp(0, 2);
                    }
                }
            }
            // [svpfdN] the first page whose number is shown, from the next page.
            "svpfd" => self.next_first_shown = Some(first.unwrap_or(1).max(1) as usize),
            // [top] a new page once the page has been started (blank lines start it too).
            "top" => {
                if !self.page.is_empty() || self.line_has_text() {
                    self.new_page();
                }
            }
            // [tabP:A:F~X] the next word at cell P (1 when not given; -1 the middle of the line), justified A (L, R,
            // C or D), with fill F ('p' partial) of character X.
            "tab" => {
                let body = &code[3..];
                let (params, x) = body.split_once('~').map_or((body, None), |(a, b)| (a, Some(b)));
                let parts: Vec<&str> = params.split(':').collect();
                let mut col = parts.first().and_then(|p| p.trim().parse::<i64>().ok()).unwrap_or(1);
                if col == -1 {
                    col = (self.layout.cells as i64 + 1) / 2;
                }
                let just = parts.get(1).and_then(|p| p.trim().chars().next()).map_or('L', |c| c.to_ascii_uppercase());
                let kind = parts.get(2).and_then(|p| p.trim().chars().next());
                self.place = Some(Place {
                    fill: x.and_then(|x| x.chars().next()),
                    partial: kind.is_some_and(|k| k.eq_ignore_ascii_case(&'p')),
                    ..Place::at(col, just)
                });
            }
            // [run] the next word at the runover margin, on this line.
            "run" => {
                self.place = Some(Place { may_wrap: false, ..Place::at(self.runover() as i64 + 1, 'L') });
            }
            // [kps] ... [kpe] keep a block together on a page (new lines at both ends); [kbs] ... [kbe] the same within
            // a line (spec 19 7.4). A block at the top of a page's body is not counted.
            "kps" | "kbs" => {
                if name == "kps" && self.line_has_text() {
                    self.end_line(self.left);
                }
                let at_top = self.page.len() == self.head_rows && !self.line_has_text();
                if !at_top {
                    self.keep_depth += 1;
                    if self.keep_depth == 1 {
                        // Opened within the lines the last block keeps after it: kept with that one.
                        let attached = self.attach_until.is_some_and(|u| self.page.len() <= u);
                        self.keep_opened = !attached;
                        self.attach_until = None;
                    }
                }
            }
            // [kpeL:M] (or [kbe], [kpf] closing every block): the block keeps L lines after it on its page, and a
            // block opened within M lines is kept with it.
            "kpe" | "kbe" | "kpf" => {
                if self.keep_depth > 0 {
                    if self.keep_depth == 1 || name == "kpf" {
                        let (mut l, mut m) = (first.unwrap_or(0), n.get(1).copied().unwrap_or(0));
                        if !self.line_has_text() {
                            l -= 1;
                            m -= 1;
                        }
                        let m = m.min(l);
                        let remaining = self.capacity() as i64 - self.page.len() as i64;
                        if remaining < l {
                            if self.line_has_text() {
                                self.end_line(self.left);
                            }
                            if !self.page.is_empty() {
                                self.finish_page();
                            }
                        } else if m >= 1 {
                            self.attach_until = Some(self.page.len() + m as usize);
                        }
                        self.keep_depth = 0;
                    } else {
                        self.keep_depth -= 1;
                    }
                }
                if name != "kbe" && self.line_has_text() {
                    self.end_line(self.left);
                }
            }
            // [scL] skip L lines (1 when not given), only after a line that is not blank (spec 19, Lines).
            // [scL:M:N:P]: L lines (1 when not given) after a line with text; M (L when not given) after a print page
            // change line; N (L when not given) after a running head or at the top of a page that carries one; P (0
            // when not given) after a blank line or at the top of a page.
            "sc" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                let l = first.unwrap_or(1);
                let after_head = n.get(2).copied().unwrap_or(l);
                let after_blank = n.get(3).copied().unwrap_or(0);
                let after_text = self.page.last().is_some_and(|l| l.sources.any());
                let count = if !self.opened {
                    // The top of a page: N when it will carry a head, else P.
                    if !self.head.is_empty() && self.on_this_page(self.head_pages) { after_head } else { after_blank }
                } else if self.change_line.is_some_and(|c| c + 1 == self.page.len()) {
                    n.get(1).copied().unwrap_or(l)
                } else if self.head_rows > 0 && self.page.len() == self.head_rows {
                    after_head
                } else if after_text {
                    l
                } else {
                    after_blank
                };
                self.skip_lines(count);
            }
            // [tls] ... [tle] the running head, [rfs] ... [rfe] the running foot: the line ends, the old one is
            // cleared and the words are captured (spec 19 7.1, 7.2).
            "tls" | "rfs" => {
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                if name == "tls" {
                    self.head.clear();
                } else {
                    self.foot = None;
                }
                self.running = Some((name == "tls", Vec::new()));
            }
            "tle" | "rfe" => {
                if let Some((is_head, words)) = self.running.take() {
                    let cells = self.layout.cells;
                    let lines = self.usable();
                    if is_head {
                        // At most two head lines, and head and foot lines fewer than the page's.
                        let width = self.head_side.width(cells);
                        let feet = usize::from(self.foot.is_some());
                        self.head = wrap_words(&words, width).into_iter().take(2).take(lines.saturating_sub(feet + 1)).collect();
                    } else {
                        let width = self.foot_side.width(cells);
                        self.foot = wrap_words(&words, width).into_iter().next().filter(|_| self.head.len() + 1 < lines);
                        // On this page too, when its text has not come to the foot's line.
                        if self.opened && self.on_this_page(self.foot_pages) && self.page.len() + 2 < lines {
                            self.page_foot = self.foot.clone();
                        }
                    }
                }
            }
            "tld" => self.head.clear(),
            // [rfd] no foot from here, on this page too when its text has not come to the foot's line.
            "rfd" => {
                self.foot = None;
                if self.page.len() + 2 < self.usable() {
                    self.page_foot = None;
                }
            }
            "svrhp" => self.head_pages = first.unwrap_or(3).clamp(0, 3),
            "svrfp" => self.foot_pages = first.unwrap_or(3).clamp(0, 3),
            "svrhs" => self.head_side = Side::parse(&code[5..], self.layout.cells),
            "svrfs" => self.foot_side = Side::parse(&code[5..], self.layout.cells),
            "hds" => {
                let (need, skip) = if self.layout.braille { (1, 0) } else { (2, 1) };
                let need = first.unwrap_or(need);
                let skip = n.get(1).copied().unwrap_or(skip);
                if self.line_has_text() {
                    self.end_line(self.left);
                }
                self.heading = Some((Vec::new(), need, skip));
            }
            "hde" => {
                if let Some((words, need, skip)) = self.heading.take() {
                    self.heading_lines(&words, need, skip);
                }
            }
            _ => {}
        }
    }

    fn heading_lines(&mut self, words: &[(String, Vec<Option<usize>>)], need: i64, skip: i64) {
        self.open_page();
        // On a page number's line a centred line keeps twice the number's room, so it stays centred on the line.
        let (l, r) = self.rooms();
        let width = self.layout.cells.saturating_sub(self.right.max(2 * l.max(r))).max(1);
        let mut lines: Vec<Line> = Vec::new();
        let mut line = Line::default();
        let mut tab = false;
        for (w, s) in words {
            if w == "	" {
                tab = true;
                continue;
            }
            let len = w.chars().count();
            if std::mem::take(&mut tab) && !line.text.is_empty() {
                let pen = line.text.chars().count() as i64;
                let from = pen + 1 + self.lma;
                let stop = self.stops.iter().filter(|st| st.col >= from).min_by_key(|st| st.col).copied();
                let (start, fill) = match stop {
                    Some(st) => (if st.just == 'R' { st.col - len as i64 } else { st.col - 1 }, if st.kind != ' ' { st.fill } else { ' ' }),
                    None => (width as i64 - len as i64, ' '),
                };
                if start >= pen && start as usize + len <= width {
                    for _ in pen..start {
                        line.text.push(fill);
                        line.sources.push(None);
                    }
                    line.text.push_str(w);
                    line.sources.extend(s.iter().copied());
                    continue;
                }
            }
            if !line.text.is_empty() && line.text.chars().count() + 1 + len > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.text.is_empty() {
                line.text.push(' ');
                line.sources.push(None);
            }
            line.text.push_str(w);
            line.sources.extend(s.iter().copied());
        }
        if !line.text.is_empty() {
            lines.push(line);
        }
        // A new page when fewer than need + 1 lines remain; else the skip lines, at the top of a page too.
        let remaining = self.capacity().saturating_sub(self.page.len()) as i64;
        if remaining < need + 1 && !self.page.is_empty() && self.keep_depth == 0 {
            self.finish_page();
        } else {
            for _ in 0..skip.max(0) {
                self.push_line(Line::default());
            }
        }
        for l in lines {
            let len = l.text.chars().count();
            // A centred line is centred on the whole line, rounding down; the right margin only narrows where its
            // words wrap.
            let pad = self.layout.cells.saturating_sub(len) / 2;
            let mut centred = Line { text: " ".repeat(pad), sources: Sources::none(pad), marks: Marks::default() };
            centred.text.push_str(&l.text);
            centred.sources.extend(l.sources.iter());
            if let Some(s) = centred.sources.first() {
                if self.page_start.is_none() {
                    self.page_start = Some(s);
                }
            }
            self.push_line(centred);
        }
        self.start_col = self.left;
    }

    /// Lays out one piece; `word` is the word being collected.
    fn step(&mut self, p: &Piece, word: &mut Vec<(char, Option<usize>)>) {
        if self.table.is_some() {
            self.table_step(p);
            return;
        }
        match p {
            Piece::Char(c, at) => word.push((if *c == '\u{a0}' { ' ' } else { *c }, Some(*at))),
            Piece::Space => {
                if !word.is_empty() {
                    self.word(&std::mem::take(word));
                }
                self.touching = false;
            }
            Piece::Code(code) => {
                // A code in a word ends the word, and what follows it is placed against it, except the codes that
                // belong to a word (grouping, word break, verse and font marks). Codes the formatter does not lay
                // out take no room.
                let in_word = matches!(code.as_str(), ":" | ";")
                    || ["wb", "vss", "vse", "fts~", "fte~"].iter().any(|w| code.starts_with(w));
                if !in_word && !word.is_empty() {
                    self.word(&std::mem::take(word));
                    self.touching = true;
                }
                // The word break mode of the word in progress: [wb-no] none, [wb-cb] computer braille (as [wb-no]
                // on a line narrower than five cells), [wb] normal.
                match code.as_str() {
                    "wb-no" => self.word_break = 1,
                    "wb-cb" => self.word_break = if self.layout.cells < 5 { 1 } else { 2 },
                    c if c.starts_with("wb") => self.word_break = 0,
                    _ => {}
                }
                if is_layout(code) {
                    self.code(code);
                }
            }
            Piece::Mark(pos) => {
                self.last_place = *pos;
                if !word.is_empty() {
                    self.word_marks.push((*pos, word.len()));
                } else if self.heading.is_none() && self.running.is_none() {
                    self.mark(*pos);
                }
            }
            // A marked hyphenation break (U+001D, the old assisted-hyphen code): the word may break after the cell
            // before it (spec 36 section 3).
            Piece::Break => {
                if let Some(k) = word.len().checked_sub(1) {
                    self.word_breaks.push(k);
                }
            }
        }
    }

    /// The formatter as it stands with every document place in it moved by `map`; None when a place has nowhere to
    /// go (it was in the text an edit replaced).
    fn moved(&self, map: &impl Fn(usize) -> Option<usize>) -> Option<Formatter> {
        // A state holding deferred notes keeps piece numbers an edit shifts: it is not reused, the layout goes on afresh.
        if !self.notes.is_empty() || self.placing.is_some() {
            return None;
        }
        // Kept states have no pages, so the copy is small.
        let mut f = self.clone();
        f.pages = Vec::new();
        for line in f.page.iter_mut().chain(std::iter::once(&mut f.line)) {
            move_line(line, map)?;
        }
        if let Some(s) = f.page_start {
            f.page_start = Some(map(s)?);
        }
        f.last_start = map(f.last_start)?;
        f.last_place = map(f.last_place).unwrap_or(f.last_place);
        if let Some(r) = &mut f.ref_open {
            r.start = map(r.start)?;
        }
        for r in &mut f.page_refs {
            r.start = map(r.start)?;
            r.end = map(r.end)?;
        }
        if let Some(h) = &mut f.heading {
            for (_, sources) in &mut h.0 {
                for s in sources.iter_mut().flatten() {
                    *s = map(*s)?;
                }
            }
        }
        for l in &mut f.labels {
            l.1 = map(l.1)?;
        }
        for m in &mut f.word_marks {
            m.0 = map(m.0)?;
        }
        Some(f)
    }

    /// The formatter with no pages, to keep beside the pages it has made.
    fn snapshot(&mut self) -> Formatter {
        let pages = std::mem::take(&mut self.pages);
        let copy = self.clone();
        self.pages = pages;
        copy
    }

    /// Lays out `pieces` from piece `from`, `word` being the word collected so far. After each piece that finished a
    /// page, the state is kept in `checkpoints`; with `join`, a state that matches the old layout's at the same page
    /// past the edit takes the old layout's later pages and stops.
    fn run_from(
        mut self,
        pieces: &Pieces,
        from: usize,
        mut word: Vec<(char, Option<usize>)>,
        checkpoints: &mut Vec<Checkpoint>,
        mut join: Option<Join>,
    ) -> Vec<Page> {
        let mut made = self.pages.len();
        // Where the page being filled started, to lay it out again when it fills inside a keep block.
        let mut page_from = (from, self.snapshot(), word.clone(), made);
        let mut i = from;
        while i < pieces.len() {
            if self.keep_stop == Some(i) {
                // The second pass of a page ends it before its keep block, which goes to the next page (after the word
                // the block's code ended).
                self.keep_stop = None;
                if !word.is_empty() {
                    self.word(&std::mem::take(&mut word));
                }
                self.new_page();
            }
            let piece = pieces.get(i);
            if let Piece::Code(c) = &piece {
                if split_code(c).0.split('~').next() == Some("mfcs") {
                    self.mfc_levels = mfc_levels(pieces, i + 1);
                }
                if c.starts_with("kdtxs") {
                    self.note_end = note_end(pieces, i + 1);
                }
            }
            self.piece_at = i;
            if self.mfc_met && !word.is_empty() && matches!(piece, Piece::Space | Piece::Code(_)) {
                self.next_right_word = right_word_after(pieces, i);
            }
            self.step(&piece, &mut word);
            if std::mem::take(&mut self.keep_opened) {
                self.keep_start = Some(i);
            }
            i += 1;
            if let Some(to) = self.jump.take() {
                // Deferred text: reading goes on elsewhere (the word in progress ends first).
                if !word.is_empty() {
                    self.word(&std::mem::take(&mut word));
                }
                i = to;
            }
            if std::mem::take(&mut self.room_restart) {
                let (at, state, w, n) = page_from.clone();
                let rooms = std::mem::take(&mut self.room_table);
                let mut pages = std::mem::take(&mut self.pages);
                pages.truncate(n);
                self = Formatter { pages, room_table: rooms, ..state };
                word = w;
                i = at;
                made = n;
                continue;
            }
            if self.pages.len() == made {
                continue;
            }
            if let Some(block) = self.keep_overflow.take() {
                let (at, state, w, n) = page_from.clone();
                let mut pages = std::mem::take(&mut self.pages);
                pages.truncate(n);
                self = Formatter { pages, ..state };
                self.retried = true;
                self.keep_stop = Some(block);
                word = w;
                i = at;
                made = n;
                continue;
            }
            if std::mem::take(&mut self.head_overflow) {
                let rows = self.head.len();
                let (at, state, w, n) = page_from.clone();
                let mut pages = std::mem::take(&mut self.pages);
                pages.truncate(n);
                self = Formatter { pages, ..state };
                self.head_reserve = Some(rows);
                self.head_retried = true;
                word = w;
                i = at;
                made = n;
                continue;
            }
            if let Some(skip) = self.center_overflow.take() {
                let (at, state, w, n) = page_from.clone();
                let mut pages = std::mem::take(&mut self.pages);
                pages.truncate(n);
                self = Formatter { pages, ..state };
                self.center_top = Some(skip);
                self.center_done = true;
                word = w;
                i = at;
                made = n;
                continue;
            }
            made = self.pages.len();
            let at = i;
            page_from = (at, self.snapshot(), word.clone(), made);
            checkpoints.push(Checkpoint { piece: at, pages: made, state: self.snapshot(), word: word.clone() });
            if let Some(j) = &mut join {
                if let Some(tail) = j.try_join(at, made, &self.snapshot(), &word) {
                    let mut pages = self.pages;
                    pages.extend(tail.0);
                    checkpoints.extend(tail.1);
                    return pages;
                }
            }
        }
        // The last page closes in `end`: when its anticipatory head changed its line count, it is laid out again.
        let (at, state, w, n) = page_from;
        let (mut pages, head_change, center) = self.end(word);
        if head_change.is_some() || center.is_some() {
            pages.truncate(n);
            let mut again = Formatter { pages, ..state };
            if let Some(rows) = head_change {
                again.head_reserve = Some(rows);
                again.head_retried = true;
            } else if let Some(skip) = center {
                again.center_top = Some(skip);
                again.center_done = true;
            }
            return again.run_from(pieces, at, w, checkpoints, None);
        }
        pages
    }

    /// Lays out the last word and what is still open, and gives the pages, with the head's line count when the last
    /// page's anticipatory head changed it (Formatter::run_from lays that page out again).
    fn end(mut self, word: Vec<(char, Option<usize>)>) -> (Vec<Page>, Option<usize>, Option<usize>) {
        // A table without [htbe] runs to the end of the document (spec 35 1.3).
        if let Some(end) = self.table.as_ref().map(|t| t.last_mark().map_or(0, |m| m + 1)) {
            self.lay_table(end);
        }
        if !word.is_empty() {
            self.word(&word);
        }
        if let Some((words, need, skip)) = self.heading.take() {
            self.heading_lines(&words, need, skip);
        }
        // A line holding only marks is kept too (the end of a document whose last line is empty), but never so as to
        // start a page: marks do not change the pages. On a page not yet started they go on the last line before it.
        if !self.line_has_text() && self.line.text.is_empty() && self.page.is_empty() && !self.pages.is_empty() {
            let marks = std::mem::take(&mut self.line.marks);
            // The last line of the last page that has text (or the page's first line).
            if let Some(page) = self.pages.last_mut() {
                let at = page.lines.iter().rposition(|l| l.sources.any()).unwrap_or(0);
                if let Some(last) = page.lines.get_mut(at) {
                    let col = last.text.chars().count();
                    last.marks.extend(marks.iter().map(|(pos, _)| (pos, col)));
                }
            }
        }
        if self.line_has_text() || !self.line.text.is_empty() || !self.line.marks.is_empty() {
            self.end_line(self.left);
        }
        if !self.page.is_empty() || self.pages.is_empty() {
            self.finish_page();
        }
        let change = std::mem::take(&mut self.head_overflow).then(|| self.head.len());
        let center = self.center_overflow.take();
        (self.pages, change, center)
    }
}

fn is_layout(code: &str) -> bool {
    code.starts_with("htbs")
        || code == "htbe"
        || code.starts_with("htbfmt")
        || code.starts_with("svcbh")
        || (code.starts_with("wb") && code[2..].chars().all(|c| c.is_ascii_digit()))
        || code.starts_with("fr;")
        || code.starts_with("fr~")
        || code.starts_with("tab~")
        || code.starts_with("stb")
        || code.starts_with('#')
        || code.starts_with("svdac~")
        || code.starts_with("svgan")
        || code.starts_with("mfcs~")
        || code.starts_with("wlf~")
        || code.starts_with("kdtx")
        || code.starts_with("svgrn")
        || (code.starts_with("pv") && code[2..].starts_with(|c: char| c.is_ascii_digit() || c == '~'))
        || code == "pv"
        || code == "ifprt"
        || code == "ifbrl"
        || (code.starts_with("lea") && code[3..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '~' || c == ':'))
        || code == ">"
        || matches!(
            split_code(code).0,
            "<" | "l" | "p" | "sk" | "ind" | "inm" | "ptys" | "ptye" | "rm" | "svsbl" | "pg" | "top" | "cp" | "fr" | "hds" | "hde"
                | "kbs" | "kbe" | "kpf" | "tab" | "run"
                | "svpnp" | "svpfd" | "hi" | "hl" | "lea" | "svrlb" | "svprg" | "sl" | "st" | "ki"
                | "tls" | "tle" | "tld" | "rfs" | "rfe" | "rfd" | "svrhp" | "svrfp" | "svrhs" | "svrfs" | "kps" | "kpe" | "sc"
                | "ctb" | "tas" | "taa" | "svmts" | "lps" | "lpr" | "svlma" | "skn" | "sd" | "pm" | "svant" | "pntr" | "pnta" | "svles" | "vcs" | "vce" | "remw" | "lec" | "led" | "mec" | "meg" | "med"
                | "linenum" | "svpal" | "rps" | "rpe" | "folds" | "mfcs" | "mfce" | "wlf"
        )
}

/// Lays out a document (in the editor's form) into pages, with the document's styles.
pub fn format(doc: &Shown, layout: &Layout, styles: Option<&StyleTable>) -> Vec<Page> {
    format_traced(doc, layout, styles).0
}

/// Moves every document place on a line by `map`; None when one has nowhere to go.
fn move_line(line: &mut Line, map: &impl Fn(usize) -> Option<usize>) -> Option<()> {
    line.sources.map_places(map)?;
    line.marks.map_places(map)
}

/// The formatter's state after the piece that finished a page: the piece to go on from, the pages made, the state
/// and the word being collected.
#[derive(Debug, Clone)]
struct Checkpoint {
    piece: usize,
    pages: usize,
    state: Formatter,
    word: Vec<(char, Option<usize>)>,
}

/// What a layout keeps so that it can be laid out again after an edit from the page the edit is on (spec 27 5d: the
/// reference drops its pages from the edit on and lays them out again; the ones before are kept).
#[derive(Debug, Clone, Default)]
pub struct Trail {
    pieces: Pieces,
    checkpoints: Vec<Checkpoint>,
    layout: Option<Layout>,
}

/// The old layout's pages and states after the edit, to take over once the new layout comes to the same state at
/// the same page.
struct Join {
    /// Where the pieces after the edit start, in the new pieces; the new index less the old one.
    same_from: usize,
    shift: isize,
    /// The old pages from `base` on, and the old checkpoints after the restart.
    base: usize,
    pages: Vec<Page>,
    checkpoints: Vec<Checkpoint>,
    total_pages: usize,
    map: Box<dyn Fn(usize) -> Option<usize>>,
}

impl Join {
    /// When the new layout, at piece `at` with `made` pages, stands where the old one stood: the old pages after
    /// `made` and the old checkpoints after it, moved into the new document.
    fn try_join(
        &mut self,
        at: usize,
        made: usize,
        state: &Formatter,
        word: &[(char, Option<usize>)],
    ) -> Option<(Vec<Page>, Vec<Checkpoint>)> {
        // Only past the edit, and not at the old layout's last page (the end of the layout can still change it).
        if at < self.same_from || made >= self.total_pages {
            return None;
        }
        let k = self.checkpoints.iter().position(|c| c.pages == made)?;
        let old = &self.checkpoints[k];
        if old.piece as isize + self.shift != at as isize {
            return None;
        }
        let map = &self.map;
        if old.state.moved(map).as_ref() != Some(state) {
            return None;
        }
        let old_word: Option<Vec<_>> = old.word.iter().map(|&(c, s)| Some((c, match s {
            Some(s) => Some(map(s)?),
            None => None,
        }))).collect();
        if old_word.as_deref() != Some(word) {
            return None;
        }
        let mut pages: Vec<Page> = self.pages.split_off(made - self.base);
        for page in &mut pages {
            page.start = map(page.start)?;
            for line in &mut page.lines {
                move_line(line, map)?;
            }
            for l in &mut page.print_labels {
                l.1 = map(l.1)?;
            }
            for r in &mut page.references {
                r.start = map(r.start)?;
                r.end = map(r.end)?;
            }
        }
        let mut later = self.checkpoints.split_off(k + 1);
        for c in &mut later {
            c.piece = (c.piece as isize + self.shift) as usize;
            c.state = c.state.moved(map)?;
            for w in c.word.iter_mut() {
                if let Some(s) = w.1 {
                    w.1 = Some(map(s)?);
                }
            }
        }
        Some((pages, later))
    }
}

/// Whether document place `p` is inside a code (`codes` in order).
fn in_code(codes: &[(usize, usize)], p: usize) -> bool {
    let k = codes.partition_point(|c| c.0 <= p);
    k > 0 && p < codes[k - 1].1
}

/// The index of the first piece at or after document place `pos`; a piece with no place of its own (a code, a
/// space) counts at the place of the piece before it.
fn first_at(pieces: &Pieces, pos: usize) -> usize {
    let key = |mut i: usize| loop {
        match pieces.place(i) {
            Some(at) => return at,
            None if i == 0 => return 0,
            None => i -= 1,
        }
    };
    let (mut lo, mut hi) = (0, pieces.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if key(mid) < pos {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Lays out a document and keeps what lets it be laid out again quickly after an edit (reformat).
pub fn format_traced(doc: &Shown, layout: &Layout, styles: Option<&StyleTable>) -> (Vec<Page>, Trail) {
    let pieces = pieces(doc, styles);
    let mut checkpoints = Vec::new();
    let pages = Formatter::new(layout).run_from(&pieces, 0, Vec::new(), &mut checkpoints, None);
    (pages, Trail { pieces, checkpoints, layout: Some(layout.clone()) })
}

/// Lays a document out again after an edit replaced its places `from` to `old_to` with what is now `from` to
/// `new_to`: from the last page finished before the edit, the pages before it kept, and stopping at the first page
/// after the edit where the layout stands as it stood before, the old pages after it kept with their places moved.
/// The result is the same as format gives.
pub fn reformat(
    pages: Vec<Page>,
    trail: Trail,
    doc: &Shown,
    layout: &Layout,
    styles: Option<&StyleTable>,
    from: usize,
    old_to: usize,
    new_to: usize,
) -> (Vec<Page>, Trail) {
    if trail.layout.as_ref() != Some(layout) || trail.pieces.is_empty() {
        return format_traced(doc, layout, styles);
    }
    let delta = new_to as isize - old_to as isize;
    // Only the stretch around the edit is cut into pieces again: from the last plain letter before the edit (the
    // same letter in the old text) to the first plain letter at or after its end. A letter's piece does not depend
    // on what came before it, so the pieces before and after that stretch are the old ones (those after it with
    // their places moved).
    let len = doc.text.chars().count();
    let plain = |c: char, p: usize| c != ' ' && c != '\n' && !in_code(&doc.codes, p);
    let lo = from.saturating_sub(64);
    let before = |lo: usize| {
        doc.text.chars().enumerate().skip(lo).take(from - lo).filter(|&(p, c)| plain(c, p)).last().map(|(p, _)| p)
    };
    let start = before(lo).or_else(|| before(0)).unwrap_or(0);
    let end = doc.text.chars().enumerate().skip(new_to).find(|&(p, c)| plain(c, p)).map_or(len, |(p, _)| p);
    let mut old = trail.pieces;
    let cut_from = if start == 0 { 0 } else { first_at(&old, start) };
    let cut_to = if end == len { old.len() } else { first_at(&old, (end as isize - delta) as usize) };
    let mut mid = pieces(&doc.slice(start, end), styles);
    if end < len {
        mid.pop();
    }
    mid.shift_places(0, start as isize);
    let mid_len = mid.len();
    old.splice(cut_from..cut_to, &mid);
    if delta != 0 {
        old.shift_places(cut_from + mid_len, delta);
    }
    let new = old;
    let same_before = cut_from;
    let same_from = cut_from + mid_len;
    let shift = mid_len as isize - (cut_to - cut_from) as isize;
    let map = move |p: usize| -> Option<usize> {
        if p < from {
            Some(p)
        } else if p >= old_to {
            Some((p as isize + delta) as usize)
        } else {
            None
        }
    };
    // The last state kept before the first changed piece, never at the last page (the end of a layout changes it).
    let restart = trail
        .checkpoints
        .iter()
        .rposition(|c| c.piece <= same_before && c.pages < pages.len());
    let mut checkpoints = trail.checkpoints;
    let later = match restart {
        Some(k) => checkpoints.split_off(k + 1),
        None => std::mem::take(&mut checkpoints),
    };
    let total_pages = pages.len();
    let (formatter, start, word, base) = match restart.map(|k| checkpoints[k].clone()) {
        Some(c) => {
            let mut f = c.state;
            f.pages = pages;
            (f, c.piece, c.word, c.pages)
        }
        None => {
            let mut f = Formatter::new(layout);
            f.pages = pages;
            (f, 0, Vec::new(), 0)
        }
    };
    let mut formatter = formatter;
    let old_pages = formatter.pages.split_off(base);
    let join = Join {
        same_from,
        shift,
        base,
        pages: old_pages,
        checkpoints: later,
        total_pages,
        map: Box::new(map),
    };
    let pages = formatter.run_from(&new, start, word, &mut checkpoints, Some(join));
    (pages, Trail { pieces: new, checkpoints, layout: Some(layout.clone()) })
}

/// Edit, Goto: the document place of the cell at page, line and column (1-based), only
/// when that cell is there and came from a place in the document; None otherwise.
pub fn exact_position(pages: &[Page], page: usize, line: usize, column: usize) -> Option<usize> {
    let ln = pages.get(page.checked_sub(1)?)?.lines.get(line.checked_sub(1)?)?;
    ln.sources.get(column.checked_sub(1)?)
}

/// Edit, Goto, "go to the closest location": lines are counted through the document
/// (`lines_per_page` to a page) and tried outward from the one asked for (that line, the one before, the one after,
/// two before, ...); on the first line that has one, the cell with a document place nearest the column (that column,
/// one left, one right, ...). Answers the place and its page, line and column (1-based), or None.
pub fn closest_position(
    pages: &[Page],
    page: usize,
    line: usize,
    column: usize,
    lines_per_page: usize,
) -> Option<(usize, (usize, usize, usize))> {
    let lpp = lines_per_page.max(1);
    let total = pages.len() * lpp;
    let want = (page.max(1) - 1).saturating_mul(lpp).saturating_add(line.max(1) - 1) as i64;
    let col = (column.max(1) - 1) as i64;
    let cell = |g: i64, c: i64| -> Option<usize> {
        if g < 0 || c < 0 || g as usize >= total {
            return None;
        }
        let ln = pages.get(g as usize / lpp)?.lines.get(g as usize % lpp)?;
        ln.sources.get(c as usize)
    };
    let width = |g: i64| -> Option<usize> {
        (g >= 0 && (g as usize) < total).then(|| pages.get(g as usize / lpp).and_then(|p| p.lines.get(g as usize % lpp)))?.map(|l| l.sources.len())
    };
    let mut d = 0i64;
    loop {
        let lines = if d == 0 { vec![want] } else { vec![want - d, want + d] };
        if lines.iter().all(|&g| g < 0 || g as usize >= total) && d > 0 {
            return None;
        }
        for g in lines {
            let Some(w) = width(g) else { continue };
            let mut e = 0i64;
            while col - e >= 0 || col + e < w as i64 {
                for c in if e == 0 { vec![col] } else { vec![col - e, col + e] } {
                    if let Some(place) = cell(g, c) {
                        let g = g as usize;
                        return Some((place, (g / lpp + 1, g % lpp + 1, c as usize + 1)));
                    }
                }
                e += 1;
            }
        }
        d += 1;
    }
}

/// Edit, Goto with "Use reference page number": the place of the first print page number laid out
/// as `number` would be (in a braille document the braille number sign and the letters a to j for the digits, in a
/// print document the digits).
pub fn reference_page_position(pages: &[Page], number: u32, braille: bool) -> Option<usize> {
    let label: String = if braille {
        std::iter::once('#').chain(number.to_string().chars().map(|d| if d == '0' { 'j' } else { (d as u8 + 0x30) as char })).collect()
    } else {
        number.to_string()
    };
    pages.iter().flat_map(|p| &p.print_labels).find(|(l, _)| l.eq_ignore_ascii_case(&label)).map(|(_, place)| *place)
}

/// The character position at page `page`, line `line`, column `column` (all 1-based): the cell there, else the
/// first cell after it on that line, else just after the line's last cell; on a line with no text, the first text
/// after it. None for a page or line the document does not have.
pub fn position_of(pages: &[Page], page: usize, line: usize, column: usize) -> Option<usize> {
    let pg = pages.get(page.checked_sub(1)?)?;
    let ln = pg.lines.get(line.checked_sub(1)?)?;
    if let Some(s) = ln.sources.iter().skip(column.saturating_sub(1)).flatten().next() {
        return Some(s);
    }
    // A place with no cell where the pen stood at or after the column (the first such), else the line's last.
    let col = column.saturating_sub(1);
    let mark = ln.marks.iter().filter(|m| m.1 >= col).min_by_key(|m| (m.1, m.0)).or_else(|| ln.marks.iter().max_by_key(|m| (m.1, m.0)));
    if let Some(m) = mark {
        return Some(m.0);
    }
    if let Some(s) = ln.sources.iter().flatten().last() {
        return Some(s + 1);
    }
    let later = pages[page - 1].lines[line..].iter().chain(pages[page..].iter().flat_map(|p| &p.lines));
    later.flat_map(|l| l.sources.iter().flatten()).next()
}

/// The page, line and column (all 1-based, the line counted on its page) of a character position in the document:
/// the cell that came from that position; for a position that is no cell (a space, a line end, a code, the end), where
/// the formatter's pen stood at it (Line::marks); failing both, the place just after the last cell before it, as a
/// caret at the end of a line stands after its last cell; before any cell, the first cell.
pub fn locate(pages: &[Page], pos: usize) -> (usize, usize, usize) {
    let mut before: Option<(usize, usize, usize)> = None;
    for (p, page) in pages.iter().enumerate() {
        for (l, line) in page.lines.iter().enumerate() {
            // A place with no cell: where the formatter's pen stood (spec 27 5b).
            if let Some((_, col)) = line.marks.iter().find(|m| m.0 == pos) {
                return (p + 1, l + 1, col + 1);
            }
            for (c, s) in line.sources.iter().enumerate() {
                match s {
                    Some(s) if s == pos => return (p + 1, l + 1, c + 1),
                    Some(s) if s < pos => before = Some((p + 1, l + 1, c + 2)),
                    Some(_) => return before.unwrap_or((p + 1, l + 1, c + 1)),
                    None => {}
                }
            }
        }
    }
    before.unwrap_or((1, 1, 1))
}

/// The text of a formatted file (.brf, .prf) from laid-out pages, as the reference program's page writer writes it
/// (spec 04d-B): every line of every page with its trailing spaces left off and a line end after it, and a form feed
/// after every page, the last one included; nothing before the first page or after the last form feed. The line end
/// is CR LF: the reference opens the file in Windows text mode, which writes each line end as CR LF (spec 04d-B
/// item 1, inferred from the open mode). The characters are the cells as the pages hold them; the caller maps them
/// to the chosen encoding.
pub fn formatted_text(pages: &[Page]) -> String {
    let mut out = String::new();
    for page in pages {
        for line in &page.lines {
            out.push_str(line.text.trim_end_matches(' '));
            out.push_str("\r\n");
        }
        out.push('\u{c}');
    }
    out
}

/// The bytes of a formatted text file (.prf) in `code_page`: each line with "\n"
/// and a form feed after every page, a hard space as a space, encoded in the code page ('#' for a character it cannot
/// hold), through a text-mode file stream, which puts a carriage return before every line feed byte. In a one-byte or
/// multi-byte code page that is CR LF per line; in UTF-16 and UTF-32 it is also what the standard files hold (a CR
/// byte before the 0x0A byte of each line end, and of any character holding a 0x0A byte).
pub fn formatted_text_file(pages: &[Page], code_page: &str) -> Vec<u8> {
    let text = formatted_text(pages).replace("\r\n", "\n").replace('\u{a0}', " ");
    let mut out = Vec::new();
    for b in crate::codepages::encode(code_page, &text) {
        if b == b'\n' {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(page: &Page) -> Vec<&str> {
        page.lines.iter().map(|l| l.text.as_str()).collect()
    }

    /// A document whose bracketed runs are codes.
    fn m(text: &str) -> Shown {
        Shown::marked(text)
    }

    #[test]
    fn places_with_no_cell_stand_where_the_pen_was() {
        // A space at the end of a line (the line does not show it) stands after the line's last letter; so does a
        // second space; the next word is on the next line.
        let pages = format(&m("one  two"), &Layout::braille(5, 4), None);
        assert_eq!(texts(&pages[0]), ["one", "two", "", ""]);
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 4), (1, 1, 4));
        assert_eq!(locate(&pages, 5), (1, 2, 1));
        assert_eq!(position_of(&pages, 1, 1, 4), Some(3));
        // The end of a document whose last line is empty stands on that empty line, and back.
        let pages = format(&m("abc\n"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0]), ["abc", "", ""]);
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 4), (1, 2, 1));
        assert_eq!(position_of(&pages, 1, 2, 1), Some(4));
        // A code inside a word stands before the letter after it.
        let pages = format(&m("ab[x]cd"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0])[0], "abcd");
        assert_eq!(locate(&pages, 2), (1, 1, 3));
        assert_eq!(locate(&pages, 7), (1, 1, 5));
        // Marks never add a page: a document ending at a page code keeps its one page.
        let pages = format(&m("a[pg]"), &Layout::braille(10, 3), None);
        assert_eq!(pages.len(), 1);
        assert_eq!(locate(&pages, 5), (1, 1, 2));
    }

    #[test]
    fn words_wrap_at_the_width_and_a_new_line_code_ends_a_line() {
        let pages = format(&m("aaa bbb ccc\nddd"), &Layout::braille(8, 4), None);
        assert_eq!(pages.len(), 1);
        assert_eq!(texts(&pages[0]), ["aaa bbb", "ccc", "ddd", ""]);
    }

    #[test]
    fn a_braille_paragraph_starts_at_the_third_cell_and_runover_follows_the_margin() {
        let pages = format(&m("[p]\naaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "ccc", ""]);
        // Margins count cells from 1: [ind3] is the third cell; [ptys] alone runs over two cells further in.
        let pages = format(&m("[ind3][ptys2]aaa bbb ccc"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0]), ["  aaa bbb", "    ccc", ""]);
        let pages = format(&m("[ind1][ptys]aaa bbb ccc"), &Layout::braille(8, 3), None);
        assert_eq!(texts(&pages[0]), ["aaa bbb", "  ccc", ""]);
        // [ind2:1] is two cells on from the current margin.
        let pages = format(&m("[ind3]a[ind2:1]b"), &Layout::braille(9, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["  a", "    b"]);
    }

    #[test]
    fn pages_break_when_full_and_at_a_page_code_with_the_number_in_its_corner() {
        let mut layout = Layout::braille(10, 3);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::LowerRight;
        // The number shares the last line with text, which keeps three cells clear of it.
        let pages = format(&m("a\nb\nc[pg]d"), &layout, None);
        assert_eq!(pages.len(), 2);
        assert_eq!(texts(&pages[0]), ["a", "b", "c       #a"]);
        assert_eq!(texts(&pages[1]), ["d", "", "        #b"]);
        let pages = format(&m("a\nb\nccc dd"), &layout, None);
        assert_eq!(texts(&pages[0]), ["a", "b", "ccc     #a"]);
        assert_eq!(texts(&pages[1])[0], "dd");
        // At the upper left, text on the first line starts three cells after the number; [pgN~X] numbers the next
        // page N with X before it; a print document's number is in digits.
        layout.numbers_odd = Corner::UpperLeft;
        layout.numbers_even = Corner::UpperLeft;
        let pages = format(&m("a[pg9~p]b"), &layout, None);
        assert_eq!(texts(&pages[0])[0], "#a   a");
        assert_eq!(texts(&pages[1])[0], "p#i   b");
        let mut print = Layout::print(10, 2);
        print.numbers_odd = Corner::LowerRight;
        assert_eq!(texts(&format(&m("a"), &print, None)[0]), ["a", "         1"]);
        // [svpnp] changes where numbers go from the next page.
        let pages = format(&m("a [svpnp0:0]b[pg]c"), &layout, None);
        assert_eq!(texts(&pages[0])[0], "#a   a b");
        assert_eq!(texts(&pages[1])[0], "c");
    }

    #[test]
    fn a_heading_is_centred_and_a_word_longer_than_a_line_is_broken() {
        let pages = format(&m("[hds]ab[hde]abcdefghij"), &Layout::braille(6, 4), None);
        // Longer than the line: a hyphen after the last ordinary cell that leaves room for it (spec 36 section 14).
        assert_eq!(texts(&pages[0]), ["  ab", "abcde-", "fghij", ""]);
    }

    #[test]
    fn styles_expand_to_their_codes() {
        let styles = StyleTable::from_text([("para.", "[p]", ""), ("h1.", "[hds]", "[hde]")]);
        let pages = format(&m("[es~h1.]Title[ee~h1.][es~para.]aa bb[ee~para.]"), &Layout::braille(9, 3), Some(&styles));
        assert_eq!(texts(&pages[0]), ["  Title", "  aa bb", ""]);
    }

    #[test]
    fn a_bracket_cell_is_text_and_a_paragraph_column_ignores_the_margin_and_centring_rounds_down() {
        // Brackets in the text, even ones that look like a code, are text.
        let pages = format(&Shown::plain("d[n by] \"[hds2]\" x"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "d[n by] \"[hds2]\" x");
        let pages = format(&m("[ind4][p]\nab"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "  ab");
        let pages = format(&m("[hds]abc[hde]"), &Layout::braille(6, 2), None);
        assert_eq!(texts(&pages[0])[0], " abc");
        // Centred on the whole line, not inside the right margin.
        let pages = format(&m("[rm4][hds]ab[hde]"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "    ab");
    }

    /// The shipped sample documents (when a folder of them is named in BC_DX_SAMPLES) lay out, as print and as
    /// braille, with every character of their text in one cell, in order, on full pages no wider than the page.
    #[test]
    fn every_sample_document_lays_out_with_all_its_text_in_order() {
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext != "dxp" && ext != "dxb" {
                continue;
            }
            let doc = crate::dxfile::read(&std::fs::read(&p).unwrap()).unwrap();
            let doc = Shown::from_running(&doc.text);
            // Every character of the text, except a running head's or foot's, which is not body text.
            let mut running = false;
            let expected: Vec<usize> = pieces(&doc, None)
                .iter()
                .filter_map(|piece| match piece {
                    Piece::Code(c) if matches!(split_code(&c).0, "tls" | "rfs") => {
                        running = true;
                        None
                    }
                    Piece::Code(c) if matches!(split_code(&c).0, "tle" | "rfe") => {
                        running = false;
                        None
                    }
                    Piece::Char(_, at) if !running => Some(at),
                    _ => None,
                })
                .collect();
            for layout in [Layout::print(80, 50), Layout::braille(40, 25)] {
                let pages = format(&doc, &layout, None);
                let placed: Vec<usize> =
                    pages.iter().flat_map(|pg| &pg.lines).flat_map(|l| l.sources.iter().flatten()).collect();
                if placed != expected {
                    let k = placed.iter().zip(&expected).position(|(a, b)| a != b).unwrap_or(placed.len().min(expected.len()));
                    let at = expected.get(k).or(placed.get(k)).copied().unwrap_or(0);
                    let around: String = doc.text.chars().skip(at.saturating_sub(40)).take(80).collect();
                    panic!(
                        "{}: {} placed, {} expected; first difference at item {k} (placed {:?}, expected {:?}) near {around:?}",
                        p.display(),
                        placed.len(),
                        expected.len(),
                        placed.get(k),
                        expected.get(k)
                    );
                }
                for page in &pages {
                    assert_eq!(page.lines.len(), layout.lines, "{}", p.display());
                    assert!(page.lines.iter().all(|l| l.text.chars().count() <= layout.cells), "{}", p.display());
                }
            }
            count += 1;
        }
        assert!(count > 0);
    }

    #[test]
    fn a_position_is_found_on_its_page_line_and_column() {
        let pages = format(&m("aaa bbb ccc\nddd"), &Layout::braille(8, 2), None);
        assert_eq!(locate(&pages, 0), (1, 1, 1));
        assert_eq!(locate(&pages, 3), (1, 1, 4));
        assert_eq!(locate(&pages, 5), (1, 1, 6));
        assert_eq!(locate(&pages, 8), (1, 2, 1));
        // The line end after "ccc": after its last cell, not at the start of the next page.
        assert_eq!(locate(&pages, 11), (1, 2, 4));
        assert_eq!(locate(&pages, 12), (2, 1, 1));
        assert_eq!(locate(&pages, 99), (2, 1, 4));
        // A code stands where the pen was before it acted: a [p] at the start stands at the first cell.
        let pages = format(&m("[p]\nab"), &Layout::braille(8, 2), None);
        assert_eq!(locate(&pages, 0), (1, 1, 1));
    }

    #[test]
    fn a_formatted_file_has_trimmed_lines_and_a_form_feed_after_every_page() {
        let pages = format(&m("[p]\nab cd[pg]ef"), &Layout::braille(8, 2), None);
        assert_eq!(formatted_text(&pages), "  ab cd\r\n\r\n\u{c}ef\r\n\r\n\u{c}");
        // The file: CR LF in a one-byte code page; in UTF-16 the CR byte goes before the line end's 0x0A byte.
        assert_eq!(formatted_text_file(&pages[1..], "ISO-8859-1"), b"ef\r\n\r\n\x0c".to_vec());
        assert_eq!(formatted_text_file(&pages[1..], "UTF-16LE"), b"e\0f\0\r\n\0\r\n\0\x0c\0".to_vec());
    }

    #[test]
    fn keep_blocks_start_and_end_lines_and_skip_follows_text_only() {
        let pages = format(&m("ab[kps]cd[kpe]ef"), &Layout::braille(20, 5), None);
        assert_eq!(&texts(&pages[0])[..3], ["ab", "cd", "ef"]);
        // [sc1] skips a line after text, none after a blank line or at the top.
        let pages = format(&m("[sc1]ab[sc1]cd[sk1][sc1]ef"), &Layout::braille(20, 8), None);
        assert_eq!(&texts(&pages[0])[..5], ["ab", "", "cd", "", "ef"]);
    }

    /// Edits laid out again from their page give the same pages as laying the whole document out.
    fn check_edits(doc: &Shown, layout: &Layout, edits: usize) {
        let (mut pages, mut trail) = format_traced(doc, layout, None);
        let mut doc = doc.clone();
        let puts = ["x", " ", "\n", "[p]\n", "[pg]", "word ", "[hds]Head[hde]", "", "  ", "[ind4]a b c d e f g"];
        let mut seed: u64 = 12345;
        for k in 0..edits {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let len = doc.text.chars().count();
            let from = (seed >> 33) as usize % (len + 1);
            let take = if k % 3 == 0 { (seed >> 20) as usize % 6 } else { 0 };
            let mut to = (from + take).min(len);
            // Keep the codes whole, as the editor does.
            let mut from = from;
            for &(s, e) in &doc.codes {
                if s < from && e > from {
                    from = s;
                }
                if s < to && e > to {
                    to = e;
                }
            }
            let put = Shown::marked(puts[k % puts.len()]);
            let mut next = doc.slice(0, from);
            next.push(&put);
            next.push(&doc.slice(to, len));
            let new_to = from + put.text.chars().count();
            let (p, t) = reformat(pages, trail, &next, layout, None, from, to, new_to);
            let whole = format(&next, layout, None);
            assert_eq!(t.pieces, pieces(&next, None), "edit {k}: {from}..{to} -> {:?}", put.text);
            assert_eq!(p, whole, "edit {k}: {from}..{to} -> {:?}", put.text);
            pages = p;
            trail = t;
            doc = next;
        }
    }

    #[test]
    fn an_edit_laid_out_from_its_page_gives_the_whole_layout() {
        let text: String = (0..300)
            .map(|i| match i % 17 {
                0 => "[p]\n".to_string(),
                5 => "[hds]Part[hde]".to_string(),
                9 => "[sk1]".to_string(),
                _ => format!("word{} ", i % 7),
            })
            .collect();
        let mut layout = Layout::braille(20, 6);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::UpperLeft;
        check_edits(&m(&text), &layout, 300);
        check_edits(&m(&text), &Layout::print(30, 8), 200);
        let Ok(dir) = std::env::var("BC_DX_SAMPLES") else { return };
        for entry in std::fs::read_dir(dir).unwrap().flatten().take(3) {
            let p = entry.path();
            if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dxb") || e.eq_ignore_ascii_case("dxp")) {
                continue;
            }
            let doc = crate::dxfile::read(&std::fs::read(&p).unwrap()).unwrap();
            check_edits(&Shown::from_running(&doc.text), &Layout::braille(40, 25), 30);
        }
    }

    #[test]
    fn a_word_that_does_not_fit_breaks_after_a_hyphen_and_a_long_word_starts_on_the_line() {
        // With the Dashes and Hyphens table (the templates' default) the word breaks after its hyphen.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/hyphenation/dashes-hyphens.hct");
        let mut layout = Layout::braille(8, 3);
        layout.hyphen = Some(crate::hyphen::HyphenTable::parse(&std::fs::read_to_string(path).unwrap()).unwrap());
        let pages = format(&m("aaa bbb-ccc"), &layout, None);
        assert_eq!(texts(&pages[0])[..2], ["aaa bbb-", "ccc"]);
        // Without a table (None) the word goes whole to the next line; a marked break (U+001D) still breaks it,
        // with a hyphen inserted.
        let pages = format(&m("aaa bbb-ccc"), &Layout::braille(8, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["aaa", "bbb-ccc"]);
        let pages = format(&m("aaa bbb\u{1d}ccc"), &Layout::braille(8, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["aaa bbb-", "ccc"]);
        // Longer than a line: as much as fits after the space with a hyphen, the rest on the next lines the same way
        // (spec 36 section 14).
        let pages = format(&m("ab cdefghijk"), &Layout::braille(6, 3), None);
        assert_eq!(texts(&pages[0])[..3], ["ab cd-", "efghi-", "jk"]);
        // Shorter than a line: to the next line whole.
        let pages = format(&m("abcd efgh"), &Layout::braille(6, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["abcd", "efgh"]);
    }

    #[test]
    fn blank_lines_are_kept_at_the_top_of_a_page() {
        // A blank line that does not fit on a page starts the next one.
        let pages = format(&m("a\nb\n\nc"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[1]), ["", "c"]);
        // A print paragraph skips a line at the top of a page too.
        let pages = format(&m("[p]\na"), &Layout::print(10, 3), None);
        assert_eq!(texts(&pages[0]), ["", "a", ""]);
        // [sc] at the top of a page skips its fourth value, 0 when not given.
        let pages = format(&m("a\nb[sc1]c"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[1]), ["c", ""]);
        let pages = format(&m("a\nb[sc1:0:0:1]c"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[1]), ["", "c"]);
        // Line spacing stops at the end of the page.
        let pages = format(&m("[svsbl]a\nb\nc\nd"), &Layout::braille(10, 6), None);
        assert_eq!(texts(&pages[0]), ["a", "", "b", "", "c", ""]);
        assert_eq!(texts(&pages[1])[0], "d");
    }

    #[test]
    fn code_text_is_laid_out_and_a_code_ends_a_word() {
        // [q~X] is cells of its word, [d~X] cells that end the word.
        let pages = format(&m("aa [q~,']b [d~xx]yy"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "aa ,'b xx yy");
        // The two parts of a word a code ends touch, but a line may end between them.
        let pages = format(&m("abc[x]de"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "abcde");
        let pages = format(&m("abc[x]de"), &Layout::braille(4, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["abc", "de"]);
    }

    #[test]
    fn page_and_line_codes() {
        // [svrlb1] keeps the last line blank; the page number goes on the last line in use.
        let mut layout = Layout::braille(10, 4);
        layout.numbers_odd = Corner::LowerRight;
        layout.numbers_even = Corner::LowerRight;
        let pages = format(&m("[svrlb1]a\nb\nc\nd"), &layout, None);
        assert_eq!(texts(&pages[0]), ["a", "b", "c       #a", ""]);
        assert_eq!(texts(&pages[1])[0], "d");
        // [st3] goes to the third line; [sl2] ends the line and skips one more.
        let pages = format(&m("a[st3]b"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0])[..3], ["a", "", "b"]);
        let pages = format(&m("a[sl2]b"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0])[..3], ["a", "", "b"]);
        // [svprg5:0] paragraphs start at the fifth cell.
        let pages = format(&m("[svprg5:0][p]\nab"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "    ab");
        // [ki1]: a new line in a keep block closes it and opens another, so only the last part moves.
        let pages = format(&m("a\nb\n[kps][ki1]c\nd\ne[kpe]"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0])[..3], ["a", "b", "c"]);
    }

    #[test]
    fn goto_exact_and_closest_as_specified() {
        let line = |s: &[Option<usize>]| Line { sources: s.iter().copied().collect(), ..Line::default() };
        // Two pages of three lines; page 1 line 2 is blank; page 2 line 1 has a page number cell (no place).
        let pages = vec![
            Page { lines: vec![line(&[Some(0), Some(1), Some(2)]), line(&[]), line(&[Some(4), None, Some(6)])], ..Page::default() },
            Page { lines: vec![line(&[None, None]), line(&[Some(8)]), line(&[])], start: 8, ..Page::default() },
        ];
        assert_eq!(exact_position(&pages, 1, 1, 2), Some(1));
        assert_eq!(exact_position(&pages, 1, 3, 2), None);
        assert_eq!(exact_position(&pages, 1, 2, 1), None);
        assert_eq!(exact_position(&pages, 3, 1, 1), None);
        // A cell with no place: the nearest on the line, left first.
        assert_eq!(closest_position(&pages, 1, 3, 2, 3), Some((4, (1, 3, 1))));
        // A blank line: the line before it first.
        assert_eq!(closest_position(&pages, 1, 2, 1, 3), Some((0, (1, 1, 1))));
        // A column past the line's end: back along the line.
        assert_eq!(closest_position(&pages, 2, 2, 9, 3), Some((8, (2, 2, 1))));
        // A page past the end: no line, no place.
        assert_eq!(closest_position(&pages, 9, 1, 1, 3), None);
    }

    #[test]
    fn print_page_numbers_from_lea() {
        let mut layout = Layout::braille(20, 4);
        layout.print_numbers_odd = Corner::UpperRight;
        layout.print_numbers_even = Corner::UpperRight;
        // In the middle of a page: a print page change line; the next braille page shows the number with a letter.
        let pages = format(&m("a [lea]5 b c\nd\ne"), &layout, None);
        assert_eq!(texts(&pages[0]), ["a", &format!("{}5", "-".repeat(19)), "b c", "d"]);
        assert_eq!(texts(&pages[1])[0], format!("e{}a5", " ".repeat(17)));
        // At the top of a page whose print number goes at the top: only in the corner.
        let pages = format(&m("[lea]7 x"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("x{}7", " ".repeat(18)));
    }

    #[test]
    fn tabs_and_flush_right_place_the_next_word() {
        let pages = format(&m("[ind5][tab7]note"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "      note");
        // Flush right with partial leaders: a space, the fill, a space, the word at the end of the line.
        let pages = format(&m("Chapter[fr;p~\".]5"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], format!("Chapter {} 5", "\"".repeat(10)));
        let pages = format(&m("a [fr]b"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "a        b");
        // A tab behind the pen goes to the next line.
        let pages = format(&m("abcdef [tab3]x"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["abcdef", "  x"]);
        // Right justified at a cell.
        let pages = format(&m("a [tab8:R]xyz"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "a    xyz");
    }

    #[test]
    fn tab_stops_and_the_tab_variants() {
        // [#1] goes to stop 1, [>] to the nearest stop past the pen (the default stop 3 at cell 7).
        let pages = format(&m("[stb1:L:5][stb2:L:10]a[#1]b[>]c"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "a   b c");
        // After [ctb] every stop is the line end, justified right; a stop's fill goes from the pen.
        let pages = format(&m("[ctb]a[>]b"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "a        b");
        let pages = format(&m("[stb1:L:8:f~-]a[#1]b"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "a------b");
        // [tas]: at the cell, or the minimum spacing after the pen when the cell is behind it.
        let pages = format(&m("[tas3]ab[tas2]cd"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "  ab cd");
        // [taa] writes over what is there; [lps] and [lpr] go back to the saved column.
        let pages = format(&m("abcdef[taa2]XY"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "aXYdef");
        let pages = format(&m("ab[lps]cd[lpr]XY"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "abXY");
    }

    #[test]
    fn skips_taken_back_margin_adjustment_magazine_paragraph_parity_and_word_break() {
        // [skn2] takes back two of the three lines [sk3] skipped.
        let pages = format(&m("a[sk3][skn2]b"), &Layout::braille(10, 6), None);
        assert_eq!(texts(&pages[0])[..3], ["a", "", "b"]);
        // [svlma] moves later [ind] margins.
        let pages = format(&m("[svlma2][ind1]a"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "  a");
        // A magazine paragraph: the word three spaces on when it fits, else a new paragraph.
        let pages = format(&m("ab[pm3]cd"), &Layout::braille(10, 2), None);
        assert_eq!(texts(&pages[0])[0], "ab   cd");
        let pages = format(&m("abcdef[pm3]ghij"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0])[..2], ["abcdef", "  ghij"]);
        // [sd1] after a page break leaves the even page blank.
        let pages = format(&m("a[pg][sd1]b"), &Layout::braille(10, 2), None);
        assert_eq!(pages.len(), 3);
        assert!(pages[1].lines.iter().all(|l| l.text.trim().is_empty()));
        assert_eq!(texts(&pages[2])[0], "b");
        // [wb-no]: the word moves whole to the next line instead of breaking at its hyphen (with the Dashes and
        // Hyphens table).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/hyphenation/dashes-hyphens.hct");
        let mut layout = Layout::braille(6, 3);
        layout.hyphen = Some(crate::hyphen::HyphenTable::parse(&std::fs::read_to_string(path).unwrap()).unwrap());
        let pages = format(&m("xy ab-cd"), &layout, None);
        assert_eq!(texts(&pages[0])[..2], ["xy ab-", "cd"]);
        let pages = format(&m("xy [wb-no]ab-cd"), &layout, None);
        assert_eq!(texts(&pages[0])[..2], ["xy", "ab-cd"]);
        // An anticipatory head shows as it stands when the page closes.
        let pages = format(&m("[tls]one[tle][svant1]a[tls]two[tle]b"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0])[0], "   two");
    }

    #[test]
    fn a_keep_block_that_would_cross_a_page_starts_the_next_page() {
        let pages = format(&m("a\nb\n[kps]c\nd\ne[kpe]f"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0]), ["a", "b", "", ""]);
        assert_eq!(texts(&pages[1]), ["c", "d", "e", "f"]);
        // [kpe2:2]: a heading kept with the two lines after it.
        let pages = format(&m("a\nb\nc[kps]H[kpe2:2]d\ne"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0]), ["a", "b", "c", ""]);
        assert_eq!(texts(&pages[1])[..3], ["H", "d", "e"]);
        // A block at the top of a page is not moved; one longer than a page flows on.
        let pages = format(&m("[kps]a\nb\nc\nd\ne[kpe]"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0]), ["a", "b", "c"]);
        // A block that fits stays.
        let pages = format(&m("a[kps]b[kpe]c"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0]), ["a", "b", "c", ""]);
    }

    #[test]
    fn level_margins_set_each_level_first_line_and_runover() {
        // A list: level 1 starts at cell 1 and runs over to cell 3, each level two cells further in.
        let pages = format(&m("[hi1:3:0:2:2][hl1]aaa bbb ccc[hl2]aaa bbb ccc[hi]x"), &Layout::braille(9, 8), None);
        assert_eq!(texts(&pages[0])[..6], ["aaa bbb", "  ccc", "  aaa bbb", "    ccc", "x", ""]);
    }

    #[test]
    fn guide_words_go_in_the_foot() {
        // Two entries on page 1: first, dash, last; page 2 with none: the last entry, a space, the continuation.
        let doc = m("[med]- [mec]cont. [meg]apple a\n[meg]bear b\nc\nd\ne\nf\ng");
        // (The words are cut at the width less the foot's side room, 8 and 8.)
        let pages = format(&doc, &Layout::braille(40, 4), None);
        assert_eq!(texts(&pages[0])[3].trim(), "apple-bear");
        assert_eq!(texts(&pages[1])[3].trim(), "bear cont.");
        // [remw] drops the next word.
        let pages = format(&m("a [remw]b c"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&pages[0])[0], "a c");
    }

    #[test]
    fn line_numbers_go_at_the_line_end_with_room_kept_on_the_right() {
        let l = Layout::braille(20, 6);
        // At the last cell, with the number and two more cells kept on the right (width 17).
        let pages = format(&m("[linenum]5 one two three four five six"), &l, None);
        assert_eq!(texts(&pages[0])[..2], ["one two three      5", "four five six"]);
        // A wider number later on the page lays the page out again with its room (16); after text the next word
        // goes on three spaces on ([svpal] 3).
        let pages = format(&m("[linenum]1 aaa bbb ccc ddd eee fff [linenum]12 ggg hhh"), &l, None);
        assert_eq!(texts(&pages[0])[..3], ["aaa bbb ccc ddd    1", "eee fff   ggg     12", "hhh"]);
        // [linenum1:l] at the first cell, over what is there and with no room kept.
        let pages = format(&m("[linenum1:l]7 aaa bbb"), &l, None);
        assert_eq!(texts(&pages[0])[0], "7aa bbb");
        // [svpal1:-2]: when the word does not fit after one space it starts the next line at the margin, with its
        // number.
        let pages = format(&m("[svpal1:-2][linenum]1 aaa bbb ccc ddd [linenum]2 eee"), &l, None);
        assert_eq!(texts(&pages[0])[..2], ["aaa bbb ccc ddd    1", "eee                2"]);
        // [rm0:0] starts a new section of the page with no room kept.
        let pages = format(&m("[linenum]9 aaa[l][rm0:0]bbb ccc ddd eee fff"), &l, None);
        assert_eq!(texts(&pages[0])[1], "bbb ccc ddd eee fff");
    }

    #[test]
    fn a_reference_point_keeps_its_places_and_page_numbers() {
        let doc = m("[rps2]Ch one[rpe] a[pg]b [rps]c[pg]d[rpe] [rps][rpe]");
        let pages = format(&doc, &Layout::braille(20, 4), None);
        let points: Vec<&ReferencePoint> = pages.iter().flat_map(|p| &p.references).collect();
        // The empty point has no word and is not kept; the second runs from page 2 to page 3.
        assert_eq!(points.len(), 2);
        assert_eq!((points[0].start, points[0].end, points[0].value), (0, 12, 2));
        assert_eq!((points[0].first_page.as_str(), points[0].last_page.as_str()), ("#a", "#a"));
        assert_eq!((points[1].start, points[1].end, points[1].value), (25, 36, 0));
        assert_eq!((points[1].first_page.as_str(), points[1].last_page.as_str()), ("#b", "#c"));
    }

    #[test]
    fn folds_leave_their_lines_for_the_fold() {
        let l = Layout::braille(1, 25);
        let blank = |doc: &str| -> Vec<usize> {
            let pages = format(&m(doc), &l, None);
            texts(&pages[0]).iter().enumerate().take(20).filter(|(_, t)| !t.chars().any(|c| c.is_ascii_lowercase())).map(|(k, _)| k).collect()
        };
        let letters = "a b c d e f g h i j k l m n o p q r s t u v w";
        // Two folds of one line in 25: lines 8 and 17 (the remainder to the last fold).
        assert_eq!(blank(&format!("[folds]{letters}")), [7, 16]);
        // Three: the middle line, then the middle of each half.
        assert_eq!(blank(&format!("[folds3]{letters}")), [5, 12, 19]);
        // One, filled with the fold character.
        let pages = format(&m(&format!("[folds1:1~-]{letters}")), &l, None);
        assert_eq!(texts(&pages[0])[12], "-");
        assert_eq!(fold_index(1, 25, 12, 0, true, 1, &[0; 9]), Some(0));
    }

    #[test]
    fn a_major_formatting_context_sets_its_margins_from_its_levels() {
        let same = |a: &str, b: &str| {
            let l = Layout::braille(12, 8);
            assert_eq!(texts(&format(&m(a), &l, None)[0]), texts(&format(&m(b), &l, None)[0]), "{a}");
        };
        // Two levels: as [hi1:5:0:2:0:0:6:-1:-1:-1:0].
        same("[mfcs~list][hl1]aa bb cc dd[hl2]ee ff gg hh[mfce]ii jj kk ll", "[hi1:5:0:2:0:0:6:-1:-1:-1:0][hl1]aa bb cc dd[hl2]ee ff gg hh[hi1:1:0:0:0:0:-1:-1:-1:-1:0]ii jj kk ll");
        same("[mfcs~contents][hl3]aa bb cc dd ee ff[mfce]", "[hi1:7:6:2:0:0:6:-1:-1:-1:0][hl3]aa bb cc dd ee ff");
        same("[mfcs~exercisenemeth][hl4]aa bb cc dd ee ff[mfce]", "[hi1:5:0:2:0:0:2:-1:-1:-1:0][hl4]aa bb cc dd ee ff");
    }

    #[test]
    fn a_whole_line_fill_places_its_fields_and_fills_the_rest() {
        let l = Layout::braille(12, 4);
        let line = |doc: &str| {
            let pages = format(&m(doc), &l, None);
            texts(&pages[0])[0].to_string()
        };
        // The default style (=, all three fields): a line of fill alone, or text then fill.
        assert_eq!(line("[wlf]"), "============");
        assert_eq!(line("[wlf~none]x"), "x===========");
        // The box styles: a left field then the fill (dots 2356), the fill then a right field (dots 1245).
        assert_eq!(line("[wlf~banaboxtop]Note"), "Note77777777");
        assert_eq!(line("[wlf~banaboxbottom]End"), "gggggggggEnd");
        // [>] marks the fields: left, centred, right.
        assert_eq!(line("[wlf]ab[>]cd[>]ef"), "ab===cd===ef");
    }

    #[test]
    fn a_tab_in_a_centred_heading_is_laid_before_the_heading_is_centred() {
        // abc, then de at the stop at cell 5 (the first past the cell after the pen): "abc de" centred on 20 cells.
        let pages = format(&m("[hds]abc[>]de[hde]ef"), &Layout::braille(20, 4), None);
        assert_eq!(texts(&pages[0])[..2], ["       abc de", "ef"]);
        // ab, then cd at the stop at cell 3, right after it; the word after the heading is not moved.
        let pages = format(&m("[hds]ab[>]cd[hde]ef"), &Layout::braille(20, 4), None);
        assert_eq!(texts(&pages[0])[..2], ["        abcd", "ef"]);
    }

    #[test]
    fn deferred_notes_go_where_they_are_placed() {
        let l = Layout::braille(30, 10);
        let lines = |doc: &str| {
            let pages = format(&m(doc), &l, None);
            texts(&pages[0]).iter().take_while(|t| !t.is_empty()).map(|t| t.to_string()).collect::<Vec<_>>()
        };
        // The note is skipped where it is written and laid where [kdtxp] stands.
        assert_eq!(lines("Text one[kdtxs~footnote][l]note A[l][kdtxe] more.[kdtxp~footnote] after."), ["Text one more.", "note A", "after."]);
        // A second placement lays only the notes since the first.
        let doc = "a[kdtxs~footnote][l]n1[l][kdtxe] b[kdtxs~footnote][l]n2[l][kdtxe] c[kdtxp~footnote] d[kdtxs~footnote][l]n3[l][kdtxe] e[kdtxp~footnote] f";
        assert_eq!(lines(doc), ["a b c", "n1", "n2", "d e", "n3", "f"]);
        // [kdtxp1~X]: the notes go before the next print page number.
        let doc = "[kdtxp1~footnote]a[kdtxs~footnote][l]n1[l][kdtxe] b[lea]12 c";
        assert_eq!(lines(doc)[..2], ["a b", "n1"]);
        assert!(lines(doc)[2].ends_with("12"));
    }

    #[test]
    fn a_contents_line_keeps_only_its_page_number_room_on_the_right() {
        let l = Layout::braille(20, 4);
        // 16 cells: past the margin of 6 (14), within the page number's 2 and one (17), only after an [mfcs].
        let pages = format(&m("[mfcs~contents][rm6]aaaa bbbb ccc dd[fr]12"), &l, None);
        assert_eq!(texts(&pages[0])[0], "aaaa bbbb ccc dd  12");
        let pages = format(&m("[rm6]aaaa bbbb ccc dd[fr]12"), &l, None);
        assert_eq!(texts(&pages[0])[..2], ["aaaa bbbb ccc", "dd                12"]);
    }

    #[test]
    fn vertical_centering_puts_part_of_the_free_lines_above() {
        // Ten lines, two used: (10 - 2 - 1) / 2 = 3 blank lines above, a later page not centred after [vce].
        let pages = format(&m("[vcs]a\nb[pg][vce]c"), &Layout::braille(10, 10), None);
        assert_eq!(texts(&pages[0])[..6], ["", "", "", "a", "b", ""]);
        assert_eq!(texts(&pages[1])[0], "c");
        // [vcs3:2]: two thirds above.
        let pages = format(&m("[vcs3:2]a"), &Layout::braille(10, 10), None);
        assert_eq!(texts(&pages[0])[5], "a");
    }

    #[test]
    fn ifprt_and_ifbrl_keep_the_next_item_for_one_kind_of_document() {
        let braille = format(&m("a [ifprt]b [ifbrl]c d"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&braille[0])[0], "a c d");
        let print = format(&m("a [ifprt]b [ifbrl]c d"), &Layout::print(20, 2), None);
        assert_eq!(texts(&print[0])[0], "a b d");
    }

    #[test]
    fn a_code_under_another_name_is_the_same_code() {
        // [ttc] and [ttt] are the French names of [hds] and [hde]; [md] of [rm].
        let a = format(&m("[ttc]Title[ttt]"), &Layout::braille(20, 2), None);
        let b = format(&m("[hds]Title[hde]"), &Layout::braille(20, 2), None);
        assert_eq!(texts(&a[0]), texts(&b[0]));
        assert_eq!(canonical_code("md5"), "rm5");
        assert_eq!(canonical_code("vp-no"), "wb-no");
        assert_eq!(canonical_code("\u{b0}2"), "#2");
        assert_eq!(canonical_code("rm2"), "rm2");
    }

    #[test]
    fn a_table_caption_is_drawn_above_its_rows() {
        // [htbcs] ... [htbce]: the caption's words from the first cell, [l] a new line, then the rows.
        let doc = m("[htbs;r:1:1:b:n:a][htbcs]Ages of[l]people[htbce]Name[>]Age[<]Ann[>]12[htbe]After.");
        let pages = format(&doc, &Layout::print(40, 25), None);
        assert_eq!(&texts(&pages[0])[..6], ["Ages of", "people", "Name  Age", "----  ---", "Ann   12", "After."]);
    }

    #[test]
    fn a_table_is_laid_out_in_columns_with_its_header_underlined() {
        // Two rows, two columns: the header row is underlined, the data row below it; the text after the table
        // follows on the next line. The table's region knows its cells and the status text.
        let doc = m("[htbs;r:1:1:b:n:a]Name[>]Age[<]Ann[>]12[htbe]After.");
        let pages = format(&doc, &Layout::print(40, 25), None);
        assert_eq!(&texts(&pages[0])[..4], ["Name  Age", "----  ---", "Ann   12", "After."]);
        let region = &pages[0].tables[0];
        assert_eq!((region.rows.len(), region.cols, region.kind, region.layout, region.auto), (2, 2, 0, table::COLUMNAR, true));
        let twelve = doc.text.find("12").unwrap();
        assert_eq!(region.place(twelve), (0, Some((1, 1))));
        assert_eq!(region.status(twelve), "Columnar Table (Auto) Row 2/2 Col 2/2");
        assert_eq!(table_at(&pages, twelve).map(|r| r.start), Some(0));
        // The caret finds its cell: the "12" lands on the third line at column 7 (page, line, column from 1).
        assert_eq!(locate(&pages, twelve), (1, 3, 7));
        // The regions from the codes alone agree.
        let regions = table_regions(&doc);
        assert_eq!(regions.len(), 1);
        assert_eq!((regions[0].rows.len(), regions[0].cols), (2, 2));
        // In braille the underline is dots 25 led by dot 5 (a one-cell column shows the dot 5 alone), and an empty
        // body cell gets the guide-dot lead (dot 5, dot 5), which is the width an empty cell measures as.
        let b = m("[htbs]a[>]b[<]c[>][htbe]");
        let pages = format(&b, &Layout::braille(40, 25), None);
        assert_eq!(&texts(&pages[0])[..3], ["a  b", "\"  \"3", "c  \"\""]);
    }

    #[test]
    fn running_heads_and_feet_go_on_the_pages() {
        // A head defined before the page's first text is on that page, centred; the foot reserves the last line.
        // The head wraps inside its side room (8 cells each side when not set).
        let pages = format(&m("[tls]Head Line[tle]ab cd[pg]ef[rfs]Foot[rfe]"), &Layout::braille(40, 5), None);
        assert_eq!(texts(&pages[0]), ["               Head Line", "ab cd", "", "", ""]);
        assert_eq!(texts(&pages[1]), ["               Head Line", "ef", "", "", "                  Foot"]);
        let pages = format(&m("[tls]Head Line[tle]ab"), &Layout::braille(20, 5), None);
        assert_eq!(texts(&pages[0])[..3], ["        Head", "        Line", "ab"]);
        // Head text is not body text; a head after the page has text starts on the next page.
        let pages = format(&m("ab [tls]Head[tle]cd\ne\nf"), &Layout::braille(20, 3), None);
        assert_eq!(texts(&pages[0]), ["ab", "cd", "e"]);
        assert_eq!(texts(&pages[1])[..2], ["        Head", "f"]);
        // [svrhp1] heads on odd pages only; a print page has two blank lines after its head.
        let pages = format(&m("[svrhp1][tls]H[tle]a[pg]b[pg]c"), &Layout::print(10, 5), None);
        assert_eq!(texts(&pages[0])[..4], ["    H", "", "", "a"]);
        assert_eq!(texts(&pages[1])[0], "b");
        assert_eq!(texts(&pages[2])[3], "c");
        // [svrhs0:0:l] puts the head at the left.
        let pages = format(&m("[svrhs0:0:l][tls]H[tle]a"), &Layout::braille(10, 3), None);
        assert_eq!(texts(&pages[0])[0], "H");
    }


    #[test]
    fn roman_page_numbers_and_their_indicators() {
        let mut layout = Layout::braille(20, 2);
        layout.numbers_odd = Corner::UpperRight;
        layout.numbers_even = Corner::UpperRight;
        // [pntr]: the roman indicator and the numeral; [pv] sets the number without a new page.
        let pages = format(&m("[pv4][pntr]a"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("a{};iv", " ".repeat(16)));
        assert_eq!(pages[0].number, crate::range::PageNumber { number: 4, letter: None, roman: true });
        // [svgrn1] doubles the indicator before a numeral of more than one letter; "{" none.
        let pages = format(&m("[svgrn1~;][pv4][pntr]a"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("a{};;iv", " ".repeat(15)));
        let pages = format(&m("[svgrn~{][pntr]a"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("a{}i", " ".repeat(18)));
        // The number is taken when the page opens: [pntr] in the middle of a page shows from the next page.
        let pages = format(&m("a [pntr]b[pg]c"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("a b{}#a", " ".repeat(15)));
        assert_eq!(texts(&pages[1])[0], format!("c{};ii", " ".repeat(16)));
        // [pnta] goes back to arabic; [svgan] sets the sign and digits.
        let pages = format(&m("[pntr]a[pg][pnta]b[pg][svgan~*0123456789]c"), &layout, None);
        assert_eq!(texts(&pages[1])[0], format!("b{}#b", " ".repeat(17)));
        assert_eq!(texts(&pages[2])[0], format!("c{}*3", " ".repeat(17)));
        // Print: small letters, no indicator.
        let mut layout = Layout::print(20, 2);
        layout.numbers_odd = Corner::UpperRight;
        let pages = format(&m("[pv9][pntr]a"), &layout, None);
        assert_eq!(texts(&pages[0])[0], format!("a{}ix", " ".repeat(17)));
    }

    #[test]
    fn print_page_lines_follow_svles() {
        let mut layout = Layout::braille(20, 6);
        layout.print_numbers_odd = Corner::UpperRight;
        layout.print_numbers_even = Corner::UpperRight;
        // Within the bottom tolerance (2) of the page end, with the next corner at the top: the page is filled out.
        let pages = format(&m("a\nb\nc\nd\n[lea]5 e"), &layout, None);
        assert_eq!(texts(&pages[0]), ["a", "b", "c", "d", "", ""]);
        assert_eq!(texts(&pages[1])[0], format!("e{}5", " ".repeat(18)));
        // With tolerance 1 the print page line goes on the page.
        let pages = format(&m("[svles0:0:1]a\nb\nc\nd\n[lea]5 e"), &layout, None);
        assert_eq!(texts(&pages[0])[4], format!("{}5", "-".repeat(19)));
        assert_eq!(texts(&pages[0])[5], "e");
        // British: the number centred after dots 5 and 2-5; no continuation letters with M 1.
        let pages = format(&m("[svles1:1]a [lea]5 b\nc\nd\ne\nf\ng\nh"), &layout, None);
        assert_eq!(texts(&pages[0])[1], format!("{}\"35", " ".repeat(8)));
        assert_eq!(texts(&pages[1])[0], format!("f{}5", " ".repeat(18)));
        // P 1: the text after the line starts at the runover.
        let pages = format(&m("[ind1][ptys3][svles0:0:2:1]a [lea]5 b"), &layout, None);
        assert_eq!(texts(&pages[0])[2], "   b");
    }

    #[test]
    fn the_print_page_line_takes_its_fill_from_lea() {
        let mut layout = Layout::braille(12, 6);
        layout.print_numbers_odd = Corner::UpperRight;
        layout.print_numbers_even = Corner::UpperRight;
        let pages = format(&m("a [lean]5 b"), &layout, None);
        assert_eq!(texts(&pages[0])[1], format!("{}5", " ".repeat(11)));
        let pages = format(&m("a [leap~=]5 b"), &layout, None);
        assert_eq!(texts(&pages[0])[1], format!(" {} 5", "=".repeat(9)));
        let pages = format(&m("a [lea~*]5 b"), &layout, None);
        assert_eq!(texts(&pages[0])[1], format!("{}5", "*".repeat(11)));
        // The British line is laid at the top of a page too.
        let pages = format(&m("[svles1][lea]5 b"), &layout, None);
        // (the number is taken before the page opens, so its corner shows it too).
        assert_eq!(texts(&pages[0])[0], format!("{}\"35{}5", " ".repeat(4), " ".repeat(4)));
    }

    #[test]
    fn an_anticipatory_head_that_changes_its_line_count_lays_the_page_out_again() {
        // The head set in the middle of the page: the page is laid out again with a row for it.
        let pages = format(&m("[svant1]a[tls]H[tle] b"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0]), ["    H", "a", "b", ""]);
        // Without [svant] the head starts on the next page.
        let pages = format(&m("a[tls]H[tle] b[pg]c"), &Layout::braille(10, 4), None);
        assert_eq!(texts(&pages[0]), ["a", "b", "", ""]);
        assert_eq!(texts(&pages[1])[0], "    H");
    }
}
