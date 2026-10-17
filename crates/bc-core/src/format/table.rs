//! The braille layout of tables (spec 35): from [htbs] to [htbe] every word goes to a table object that collects
//! the cells by row, measures them, lays the table into page images of its own in one of five layouts (columnar,
//! two-page, stairstep, listed, linear), and gives the images back for the formatter to copy into the document's
//! pages. Nothing of the layout is written into the document text. A failure anywhere (the exception texts of
//! spec 35 section 8) taints the table: its text is then laid out as ordinary text (the failsafe) and the status
//! line says so.

use super::{Line, Piece};

pub const COLUMNAR: u8 = 1;
pub const TWOPAGE: u8 = 2;
pub const STAIRSTEP: u8 = 3;
pub const LISTED: u8 = 4;
pub const LINEAR: u8 = 5;

type R<T> = Result<T, String>;

/// A cell as collected: its characters with their document places (a line break as '\n' with the place of the code
/// or space that made it), the places inside it with no character (each with the index of the character after it),
/// the place of its first item (-2 for a padding cell) and of the code that closed it, and its last measurement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub chars: Vec<(char, Option<usize>)>,
    pub marks: Vec<(usize, usize)>,
    pub start: isize,
    pub end: usize,
    width: usize,
    lines: usize,
    wrapped: Vec<(char, Option<usize>)>,
}

impl Cell {
    fn empty(&self) -> bool {
        self.start == self.end as isize
    }
}

/// The table's place in the document and what the editor asks of it (spec 35 section 7): its start (the [htbs]
/// code) and end (after [htbe], or the document's end plus one), its kind (0 regular, 1 unrelated columns, 2
/// matrix), the layout chosen (0 none), whether that was automatic, whether the swapped orientation was laid, the
/// taint text when the layout failed, its cells by row in document order (start, end; a padding cell starts at -2),
/// its number of columns, and the fence cells of a matrix.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableRegion {
    pub start: usize,
    pub end: usize,
    pub kind: u8,
    pub layout: u8,
    pub auto: bool,
    pub swapped: bool,
    pub tainted: Option<String>,
    pub rows: Vec<Vec<(isize, usize)>>,
    pub cols: usize,
    pub left_fence: Option<(isize, usize)>,
    pub right_fence: Option<(isize, usize)>,
}

impl TableRegion {
    /// Whether the table holds `offset` (its start code included, its end excluded).
    pub fn holds(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// The place of the first cell's first item (a matrix: of the left fence), as the editor's "after the start" test
    /// reads it: the cell's start, or its end for an empty cell.
    pub fn first_cell_start(&self) -> Option<usize> {
        let (s, e) = self.left_fence.or_else(|| self.rows.first().and_then(|r| r.first().copied()))?;
        Some(if s >= 0 { s as usize } else { e })
    }

    /// The row and column (0-based, document orientation) of the cell holding `offset`: the last cell in document
    /// order whose start is at or before it, padding cells skipped (spec 35 7.2); kind 1 in the left fence, 2 in the
    /// right fence.
    pub fn place(&self, offset: usize) -> (u8, Option<(usize, usize)>) {
        let in_fence = |f: Option<(isize, usize)>| f.is_some_and(|(s, e)| s >= 0 && s as usize <= offset && offset <= e);
        if in_fence(self.left_fence) {
            return (1, None);
        }
        if in_fence(self.right_fence) {
            return (2, None);
        }
        let mut found = None;
        for (r, row) in self.rows.iter().enumerate() {
            for (c, &(s, _)) in row.iter().enumerate() {
                if s >= 0 && s as usize <= offset {
                    found = Some((r, c));
                }
            }
        }
        (0, found)
    }

    /// The status line's text for the caret at `offset` (spec 35 7.5), in place of the current style.
    pub fn status(&self, offset: usize) -> String {
        let (kind, place) = self.place(offset);
        let mut text = match self.kind {
            1 => "Unrelated Columns".to_string(),
            2 => match kind {
                1 => "Matrix Left Fence".to_string(),
                2 => "Matrix Right Fence".to_string(),
                _ => "Matrix".to_string(),
            },
            _ => {
                let name = match self.layout {
                    COLUMNAR => "Columnar Table".to_string(),
                    TWOPAGE => "Two-Page Table".to_string(),
                    STAIRSTEP => "Stairstep Table".to_string(),
                    LISTED => "Listed Table".to_string(),
                    LINEAR => "Linear Table".to_string(),
                    n => format!("TableType{n}"),
                };
                if self.auto {
                    format!("{name} (Auto)")
                } else {
                    name
                }
            }
        };
        if kind == 0 {
            if let Some((r, c)) = place {
                text.push_str(&format!(" Row {}/{} Col {}/{}", r + 1, self.rows.len(), c + 1, self.cols));
            }
        }
        if self.swapped {
            text = format!("Swapped {text}");
        }
        if let Some(t) = &self.tainted {
            text = format!("Failsafe {text} [{t}]");
        }
        text
    }
}

/// The page geometry a table is laid into: the lines left on the page it starts on, the lines of a fresh page, the
/// cells per line, and whether the table starts below the running head of its first page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Geometry {
    pub first_depth: usize,
    pub other_depth: usize,
    pub width: usize,
    pub first_below_head: bool,
}

/// What a laid table gives the formatter: its page images (the first goes on the page the table started on, each
/// later one on a new page), or the taint text, and its region.
pub struct Laid {
    pub images: Vec<Vec<Line>>,
    pub tainted: Option<String>,
    pub region: TableRegion,
}

/// The table format options (spec 35 2.4 and 10): the BANA and RNIB pairs, print and braille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Options {
    underline: char,
    sep_first: char,
    sep_other: char,
    sep_end: Option<char>,
    guide: char,
    gap: usize,
    gap_numeric: usize,
    gap_matrix: usize,
    lead: &'static str,
    fill_after_lead: bool,
    linear_empty: &'static str,
    listed_empty: &'static str,
    listed_sep: &'static str,
    listed_start: usize,
    listed_runover: usize,
    listed_first_start: usize,
    listed_first_runover: usize,
    stair_runover: usize,
    linear_start: usize,
    linear_runover: usize,
    guide_extra: usize,
}

fn options(set: i64, braille: bool) -> Options {
    let rnib = set == 1;
    match (rnib, braille) {
        (false, false) => Options { underline: '-', sep_first: ':', sep_other: ';', sep_end: None, guide: '.', gap: 2, gap_numeric: 2, gap_matrix: 1, lead: "..", fill_after_lead: true, linear_empty: "---", listed_empty: "---", listed_sep: ": ", listed_start: 0, listed_runover: 2, listed_first_start: 4, listed_first_runover: 4, stair_runover: 0, linear_start: 0, linear_runover: 2, guide_extra: 0 },
        (false, true) => Options { underline: '3', sep_first: '3', sep_other: '2', sep_end: None, guide: '"', gap: 2, gap_numeric: 2, gap_matrix: 1, lead: "\"\"", fill_after_lead: true, linear_empty: "\"\"\"", listed_empty: "\"\"\"", listed_sep: "3 ", listed_start: 0, listed_runover: 2, listed_first_start: 4, listed_first_runover: 4, stair_runover: 0, linear_start: 0, linear_runover: 2, guide_extra: 0 },
        (true, false) => Options { underline: '-', sep_first: ':', sep_other: ';', sep_end: Some('.'), guide: '.', gap: 2, gap_numeric: 2, gap_matrix: 1, lead: "--", fill_after_lead: false, linear_empty: "--", listed_empty: "--", listed_sep: ": ", listed_start: 0, listed_runover: 4, listed_first_start: 4, listed_first_runover: 2, stair_runover: 4, linear_start: 2, linear_runover: 0, guide_extra: 1 },
        (true, true) => Options { underline: '3', sep_first: '3', sep_other: '2', sep_end: Some('4'), guide: '\'', gap: 2, gap_numeric: 2, gap_matrix: 1, lead: "--", fill_after_lead: false, linear_empty: "--", listed_empty: "--", listed_sep: "3 ", listed_start: 0, listed_runover: 4, listed_first_start: 4, listed_first_runover: 2, stair_runover: 4, linear_start: 2, linear_runover: 0, guide_extra: 1 },
    }
}

/// A page image of the table: its lines as cells with their document places, the places with no cell (markers),
/// and its depth.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Image {
    cells: Vec<Vec<(char, Option<usize>)>>,
    marks: Vec<Vec<(usize, usize)>>,
    depth: usize,
}

impl Image {
    fn new(depth: usize) -> Image {
        Image { cells: vec![Vec::new(); depth], marks: vec![Vec::new(); depth], depth }
    }

    fn used(&self) -> usize {
        let cells = self.cells.iter().rposition(|l| !l.is_empty()).map_or(0, |k| k + 1);
        let marks = self.marks.iter().rposition(|l| !l.is_empty()).map_or(0, |k| k + 1);
        cells.max(marks)
    }

    fn lines(&self) -> Vec<Line> {
        (0..self.used())
            .map(|k| Line {
                text: self.cells[k].iter().map(|c| c.0).collect(),
                sources: self.cells[k].iter().map(|c| c.1).collect(),
                marks: self.marks[k].clone(),
            })
            .collect()
    }
}

/// The layout letter sets of the start code (spec 35 10): type r u m, easy navigation v h b, layout a c 2 s l p.
fn letter_index(set: &str, given: Option<&str>, default: usize) -> usize {
    let Some(p) = given.map(str::trim).filter(|p| !p.is_empty()) else { return default };
    let c = p.chars().next().unwrap_or(' ');
    if c.is_ascii_alphabetic() {
        if let Some(i) = set.chars().position(|s| s.eq_ignore_ascii_case(&c)) {
            return i;
        }
    }
    if let Some(d) = c.to_digit(10) {
        if let Some(i) = set.chars().position(|s| s.to_digit(10) == Some(d)) {
            return i;
        }
    }
    default
}

/// The table object: the start code's parameters, the cells collected, and the layout state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collector {
    pub start: usize,
    start_set: bool,
    /// Codes waiting for the mark that follows them (their own place in the document); whether the last piece was
    /// a code (the mark after it is that code's); and a mark that followed text or a space, which a line-break
    /// code right after it takes as its place.
    pending_codes: Vec<String>,
    prev_was_code: bool,
    after_mark: Option<usize>,
    kind: u8,
    row_headings_code: i64,
    col_headings_code: i64,
    swap_allowed: u8,
    layout_requested: u8,
    auto_seq: Vec<u8>,
    opt: Options,
    braille: bool,
    width: usize,
    geometry: Geometry,
    rows: Vec<Vec<Cell>>,
    row_done: usize,
    col_done: usize,
    num_cols: usize,
    word: Vec<(char, Option<usize>)>,
    pending_marks: Vec<usize>,
    last_mark: Option<usize>,
    pub pieces: Vec<Piece>,
    end: usize,
    left_fence: Option<Cell>,
    right_fence: Option<Cell>,
    // Layout state (spec 35 2.2).
    swapped: bool,
    layout: u8,
    row_fmt: usize,
    col_fmt: usize,
    col_offsets: Vec<usize>,
    col_len: Vec<usize>,
    col_num: Vec<bool>,
    boundary_cur: usize,
    boundary_save: usize,
    rows_fmt: usize,
    rows_needed: Vec<usize>,
    out_row: usize,
    out_col: usize,
    cur_depth: usize,
    heads: Option<(i64, i64)>,
    images: Vec<Image>,
}

impl Collector {
    /// A table object for the start code `code` ("htbs;r:1:1:b:n:a" or "htbs;u"), with the [htbfmt] values
    /// (the options pair number and the automatic layout letters); its place is the first mark it takes.
    pub fn new(code: &str, htbfmt: &(i64, String), braille: bool, geometry: Geometry) -> Collector {
        let rest = code.strip_prefix("htbs").unwrap_or("");
        let rest = rest.strip_prefix(';').unwrap_or(rest);
        let parts: Vec<&str> = rest.split(':').collect();
        let part = |k: usize| parts.get(k).copied().filter(|p| !p.trim().is_empty());
        let kind = letter_index("rum", part(0), 0) as u8;
        let number = |k: usize| part(k).map(|p| p.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect::<String>().parse::<i64>().unwrap_or(0));
        let default_head = if kind == 0 { -1 } else { 0 };
        let row_headings_code = number(1).unwrap_or(default_head);
        let col_headings_code = number(2).unwrap_or(default_head);
        let swap_allowed = match part(4).and_then(|p| p.trim().chars().next()).unwrap_or('n') {
            '1' | 'Y' | 'y' => 1,
            'A' | 'a' => 2,
            _ => 0,
        };
        let layout_requested = letter_index("ac2slp", part(5), 0) as u8;
        let mut auto_seq: Vec<u8> = Vec::new();
        for c in htbfmt.1.chars() {
            if let Some(i) = "ac2slp".find(c.to_ascii_lowercase()) {
                if i != 0 && auto_seq.len() < 7 {
                    auto_seq.push(i as u8);
                }
            }
        }
        if auto_seq.is_empty() {
            // The built-in default sequence (the bytes at 0159dd24: "cl"): columnar, then listed.
            auto_seq = vec![COLUMNAR, LISTED];
        }
        Collector {
            start: 0,
            start_set: false,
            pending_codes: Vec::new(),
            // The start code precedes the first piece, so the first mark is the start code's own.
            prev_was_code: true,
            after_mark: None,
            kind,
            row_headings_code,
            col_headings_code,
            swap_allowed,
            layout_requested,
            auto_seq,
            opt: options(htbfmt.0, braille),
            braille,
            width: geometry.width,
            geometry,
            rows: Vec::new(),
            row_done: 0,
            col_done: 0,
            num_cols: 0,
            word: Vec::new(),
            pending_marks: Vec::new(),
            last_mark: None,
            pieces: Vec::new(),
            end: 0,
            left_fence: None,
            right_fence: None,
            swapped: false,
            layout: 0,
            row_fmt: 0,
            col_fmt: 0,
            col_offsets: Vec::new(),
            col_len: Vec::new(),
            col_num: Vec::new(),
            boundary_cur: 0,
            boundary_save: 0,
            rows_fmt: 0,
            rows_needed: Vec::new(),
            out_row: 0,
            out_col: 0,
            cur_depth: 0,
            heads: None,
            images: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- collecting (spec 35 section 3)

    /// The last document place taken.
    pub fn last_mark(&self) -> Option<usize> {
        self.last_mark
    }

    /// Takes a piece of the document; false when [htbe] ended the table. A code's place is the mark that follows
    /// it (the document's pieces put a code's mark after it, and a line break's before it), so a code waits for
    /// its mark unless one came just before it.
    pub fn take(&mut self, piece: &Piece) -> bool {
        self.pieces.push(piece.clone());
        let was_code = std::mem::replace(&mut self.prev_was_code, matches!(piece, Piece::Code(_)));
        match piece {
            Piece::Char(c, at) => {
                self.after_mark = None;
                self.word.push((if *c == '\u{a0}' { ' ' } else { *c }, Some(*at)));
            }
            Piece::Space => {
                self.after_mark = None;
                if !self.word.is_empty() {
                    let space = self.last_mark;
                    let mut w = std::mem::take(&mut self.word);
                    w.push((' ', space));
                    self.item(w, None);
                }
            }
            Piece::Mark(pos) => {
                self.last_mark = Some(*pos);
                if !self.start_set {
                    self.start = *pos;
                    self.start_set = true;
                }
                if was_code {
                    // The mark of the code (or codes) just taken.
                    let codes = std::mem::take(&mut self.pending_codes);
                    let mut ended = false;
                    for code in codes {
                        if !self.apply_code(&code, *pos) {
                            ended = true;
                        }
                    }
                    if ended {
                        return false;
                    }
                    self.after_mark = None;
                } else {
                    self.after_mark = Some(*pos);
                }
                // Placed with the character after it.
                self.pending_marks.push(*pos);
            }
            Piece::Code(code) => {
                if !self.word.is_empty() {
                    let w = std::mem::take(&mut self.word);
                    self.item(w, None);
                }
                match self.after_mark.take() {
                    Some(pos) => {
                        if !self.apply_code(code, pos) {
                            return false;
                        }
                    }
                    None => self.pending_codes.push(code.clone()),
                }
            }
        }
        true
    }

    /// A code at document place `at` (spec 35 3.1): [l], [<], [>] and [htbe] act, every other code is dropped.
    fn apply_code(&mut self, code: &str, at: usize) -> bool {
        match code {
            "l" => self.item(Vec::new(), Some(('l', at))),
            "<" => self.item(Vec::new(), Some(('<', at))),
            ">" => self.item(Vec::new(), Some(('>', at))),
            "htbe" => {
                self.item(Vec::new(), Some(('e', at)));
                return false;
            }
            _ => {}
        }
        true
    }

    /// The row and cell the next item goes into exist (spec 35 3.1), then the item is stored: text appended to the
    /// cell, [l] a line break when the cell has text, [<] a new row, [>] the next cell, [htbe] the end.
    fn item(&mut self, text: Vec<(char, Option<usize>)>, code: Option<(char, usize)>) {
        let first = text.first().and_then(|c| c.1).or(code.map(|c| c.1)).unwrap_or(0);
        while self.rows.len() < self.row_done + 1 {
            self.rows.push(Vec::new());
        }
        while self.rows[self.row_done].len() < self.col_done + 1 {
            self.rows[self.row_done].push(Cell { start: first as isize, end: first, ..Cell::default() });
        }
        let (r, c) = (self.row_done, self.col_done);
        let marks = std::mem::take(&mut self.pending_marks);
        let cell = &mut self.rows[r][c];
        cell.end = first;
        let k = cell.chars.len();
        cell.marks.extend(marks.into_iter().map(|m| (m, k)));
        match code {
            None => cell.chars.extend(text),
            Some(('l', at)) => {
                if !cell.chars.is_empty() {
                    cell.chars.push(('\n', Some(at)));
                }
            }
            Some(('<', at)) => {
                cell.end = at;
                if self.num_cols <= c {
                    self.num_cols = c + 1;
                }
                self.row_done += 1;
                self.col_done = 0;
            }
            Some(('>', at)) => {
                cell.end = at;
                self.col_done += 1;
            }
            Some((_, at)) => cell.end = at,
        }
    }

    /// The end of the table (spec 35 3.2): the last cell's end, the matrix fences taken out of row 0, every row
    /// padded to the number of columns with padding cells.
    fn set_end(&mut self, end: usize) {
        self.end = end;
        if !self.word.is_empty() {
            let w = std::mem::take(&mut self.word);
            self.item(w, None);
        }
        if self.num_cols <= self.col_done {
            self.num_cols = self.col_done + 1;
        }
        if let Some(cell) = self.rows.get_mut(self.row_done).and_then(|r| r.get_mut(self.col_done)) {
            if cell.end < 1 {
                cell.end = end;
            }
        }
        if self.kind == 2 {
            if let Some(row) = self.rows.first_mut() {
                if !row.is_empty() {
                    self.left_fence = Some(row.remove(0));
                }
                if !row.is_empty() {
                    let last = row.len() - 1;
                    self.right_fence = Some(row.remove(last));
                }
            }
            self.num_cols = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        }
        for row in &mut self.rows {
            let last_end = row.last().map_or(-1, |c| c.end as isize);
            while row.len() < self.num_cols {
                row.push(Cell { start: -2, end: last_end.max(0) as usize, ..Cell::default() });
            }
        }
    }

    pub fn region(&self, tainted: Option<String>) -> TableRegion {
        TableRegion {
            start: self.start,
            end: self.end,
            kind: self.kind,
            layout: self.layout,
            auto: self.layout_requested == 0,
            swapped: self.swapped,
            tainted,
            rows: self.rows.iter().map(|r| r.iter().map(|c| (c.start, c.end)).collect()).collect(),
            cols: self.num_cols,
            left_fence: self.left_fence.as_ref().map(|c| (c.start, c.end)),
            right_fence: self.right_fence.as_ref().map(|c| (c.start, c.end)),
        }
    }

    /// The table laid out: its page images, or the taint text when the layout failed.
    pub fn finish(mut self, end: usize) -> Laid {
        self.set_end(end);
        let result = self.finalize().and_then(|_| {
            if self.images.is_empty() && !self.populate()? {
                // INFERENCE: the original returns from the emit when populate lays no data row; what then shows is
                // not traced. The text is shown as ordinary text, which loses nothing.
                return Err("populate laid no row".into());
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                let images = self.images.iter().map(Image::lines).collect();
                let region = self.region(None);
                Laid { images, tainted: None, region }
            }
            Err(e) => {
                let region = self.region(Some(e.clone()));
                Laid { images: Vec::new(), tainted: Some(e), region }
            }
        }
    }

    // ---------------------------------------------------------------- orientation and headings

    fn n_rows(&self) -> usize {
        if self.swapped {
            self.num_cols
        } else {
            self.rows.len()
        }
    }

    fn n_cols(&self) -> usize {
        if self.swapped {
            self.rows.len()
        } else {
            self.num_cols
        }
    }

    fn cell(&self, r: usize, c: usize) -> &Cell {
        let (dr, dc) = if self.swapped { (c, r) } else { (r, c) };
        &self.rows[dr][dc]
    }

    fn cell_mut(&mut self, r: usize, c: usize) -> &mut Cell {
        let (dr, dc) = if self.swapped { (c, r) } else { (r, c) };
        &mut self.rows[dr][dc]
    }

    /// The effective header counts (spec 35 2.5), in the layout orientation.
    fn heads(&mut self) -> (i64, i64) {
        let (rh, ch) = match self.heads {
            Some(h) => h,
            None => {
                let rh = if self.row_headings_code >= 0 {
                    self.row_headings_code
                } else {
                    i64::from(self.kind == 0 && self.rows.len() > 1)
                };
                let ch = if self.col_headings_code >= 0 {
                    self.col_headings_code
                } else {
                    i64::from(self.kind == 0 && self.rows.iter().all(|r| r.len() >= 2))
                };
                self.heads = Some((rh, ch));
                (rh, ch)
            }
        };
        if self.swapped {
            (ch, rh)
        } else {
            (rh, ch)
        }
    }

    fn fence_width(&self, c: usize) -> usize {
        if c == 0 {
            if let Some(f) = &self.left_fence {
                return f.chars.len();
            }
        }
        if c + 1 == self.n_cols() {
            if let Some(f) = &self.right_fence {
                return f.chars.len();
            }
        }
        0
    }

    /// Whether a cell's text is numeric (spec 35 2.6).
    fn numeric(&self, text: &[(char, Option<usize>)]) -> bool {
        let mut i = 0;
        if text.get(i).is_some_and(|c| c.0 == '-') {
            i += 1;
        }
        if text.get(i).is_some_and(|c| c.0 == '#') {
            i += 1;
        }
        let n = text.len();
        while i < n {
            let c = text[i].0;
            let ok = if self.braille { ('a'..='j').contains(&c) || c == '.' || c == '1' } else { c.is_ascii_digit() || c == '.' || c == ',' };
            if !ok && !(c == ' ' && (i == 0 || i + 1 == n)) {
                return false;
            }
            i += 1;
        }
        true
    }

    // ---------------------------------------------------------------- measuring (spec 35 5.1, 5.2)

    /// The smallest width in which `text` fits in at most `lines_avail` lines broken at spaces (the recursion of
    /// the original: a break at the last space before the even split point, then at each later space, the
    /// narrowest kept), with the lines used and the text with its breaks as '\n'.
    fn count_columns(&self, text: &[(char, Option<usize>)], start: usize, indent: usize, lines_avail: i64, depth: usize) -> R<(usize, usize, Vec<(char, Option<usize>)>)> {
        if self.layout == 0 && depth > 10 {
            return Err("countStringColumns: table too complex for analysis".into());
        }
        if lines_avail < 1 {
            return Err("countStringColumns: linesAvail <= 0 (invalid value)".into());
        }
        let len = text.len();
        if len == 0 {
            return Ok((self.opt.lead.chars().count(), 1, Vec::new()));
        }
        let mut seg = start;
        let mut lines = 1usize;
        let mut longest = 0usize;
        let mut best = usize::MAX;
        let mut best_str = text.to_vec();
        let mut best_lines = 1usize;
        let mut i = start;
        while i - seg < best && i < len {
            let c = text[i].0;
            if c == '\n' {
                longest = longest.max(i - seg);
                seg = i + 1;
                lines += 1;
            } else if c == ' ' && lines_avail > 1 {
                let split = (len - start) / lines_avail as usize + start;
                let mut brk = i;
                let mut p = split as isize - 1;
                loop {
                    if p <= i as isize {
                        break;
                    }
                    let cc = text[p as usize].0;
                    if cc == ' ' || cc == '\n' {
                        brk = p as usize;
                        break;
                    }
                    p -= 1;
                }
                longest = longest.max(brk - seg);
                let mut try_s = text.to_vec();
                try_s[brk].0 = '\n';
                let (w, l, t) = self.count_columns(&try_s, brk + 1, indent, lines_avail - 1, depth + 1)?;
                let ind = if start == 0 { indent } else { 0 };
                let cand = if w + ind < longest { longest } else { w + ind };
                if cand < best {
                    best = cand;
                    best_str = t;
                    best_lines = lines + l;
                }
                i = brk;
            }
            i += 1;
        }
        longest = longest.max(i - seg);
        if i == len && longest <= best {
            Ok((longest, lines, text.to_vec()))
        } else {
            Ok((best, best_lines, best_str))
        }
    }

    /// Measures cell (r, c) for at most `lines_avail` lines (spec 35 5.1); answers the width without the fence.
    fn measure(&mut self, r: usize, c: usize, lines_avail: i64) -> R<(usize, usize)> {
        let (rh, _) = self.heads();
        let text = self.cell(r, c).chars.clone();
        if r as i64 >= rh && !self.numeric(&text) {
            self.col_num[c] = false;
        }
        let indent = if r == 0 && rh != 0 { 0 } else { 2 };
        let (w, l, wrapped) = self.count_columns(&text, 0, indent, lines_avail, 0)?;
        let fence = self.fence_width(c);
        let cell = self.cell_mut(r, c);
        cell.width = w + fence;
        cell.lines = l;
        cell.wrapped = wrapped;
        Ok((w, l))
    }

    // ---------------------------------------------------------------- columnar and two-page (spec 35 section 5)

    /// Column widths, row heights and column positions from the stored measurements (5.3); answers the index of the
    /// first row that did not fit the page depth (the rows up to it are laid).
    fn pass(&mut self, start_row: usize, widths: &mut [usize; 2]) -> usize {
        let rows = self.n_rows();
        let cols = self.n_cols();
        let (rh, ch) = self.heads();
        for c in 0..cols {
            self.col_len[c] = 0;
        }
        let mut total = 0usize;
        let mut extra = 0usize;
        let mut r = 0usize;
        while r < rows {
            if r as i64 == rh && r < start_row {
                r = start_row;
                if r >= rows {
                    break;
                }
            }
            let mut height = 0usize;
            for c in 0..cols {
                let (w, l) = (self.cell(r, c).width, self.cell(r, c).lines);
                height = height.max(l);
                self.col_len[c] = self.col_len[c].max(w);
                if r as i64 >= rh && c as i64 == ch - 1 {
                    extra = height.saturating_sub(1);
                    height = 0;
                }
            }
            if height == 0 {
                height = 1;
            }
            if r as i64 == rh - 1 {
                height += 1;
            }
            self.rows_needed[r] = extra + height;
            total += self.rows_needed[r];
            if total >= self.cur_depth {
                break;
            }
            r += 1;
        }
        let stop = r.min(rows);
        let mut part = 0usize;
        widths[0] = 0;
        widths[1] = 0;
        for c in 0..cols {
            let split = self.layout_requested == TWOPAGE && part == 0 && ((c == self.boundary_save && c > 0) || widths[0] + self.col_len[c] >= self.width);
            let prev: isize = if split {
                part = 1;
                widths[1] = 0;
                self.boundary_cur = c;
                -1
            } else {
                c as isize - 1
            };
            self.col_offsets[c] = widths[part];
            widths[part] += self.col_len[c];
            if c + 1 < cols {
                let gap = if self.kind == 2 {
                    self.opt.gap_matrix
                } else if c >= 1 && prev >= 0 && self.col_num[c] && self.col_num[prev as usize] {
                    self.opt.gap_numeric
                } else {
                    self.opt.gap
                };
                widths[part] += gap;
            }
        }
        stop
    }

    /// Every cell measured for one line, then the widths pass, in shrink mode re-wrapping the widest cells into one
    /// more line until the table fits the page width (5.4).
    fn shrink(&mut self, start_row: usize, widths: &mut [usize; 2], shrink: bool) -> R<usize> {
        let rows = self.n_rows();
        let cols = self.n_cols();
        if cols == 0 {
            return Ok(0);
        }
        let (rh, ch) = self.heads();
        self.boundary_save = self.boundary_cur;
        for c in 0..cols {
            self.col_num[c] = true;
        }
        let mut total = 0usize;
        let mut extra = 0usize;
        let mut r = 0usize;
        while r < rows {
            if r as i64 == rh && r < start_row {
                r = start_row;
                if r >= rows {
                    break;
                }
            }
            let mut height = 1usize;
            for c in 0..cols {
                let (_, l) = self.measure(r, c, 1)?;
                height = height.max(l);
                if r as i64 >= rh && c as i64 == ch - 1 {
                    extra = height.saturating_sub(1);
                    height = 0;
                }
            }
            if r as i64 == rh - 1 {
                height += 1;
            }
            total += height + extra;
            if total >= self.cur_depth {
                break;
            }
            r += 1;
        }
        let mut n = 0usize;
        if !shrink {
            return Ok(self.pass(start_row, widths));
        }
        let mut limit: isize = -1;
        let mut round = 1;
        while round < 0x15 {
            n = self.pass(start_row, widths);
            if widths[0] < self.width && widths[1] < self.width {
                break;
            }
            let (w, mut row, mut col) = self.widest(n, limit, 0, 0);
            if w < 5 {
                return Ok(n);
            }
            let mut changed = self.rewrap(row, col)?;
            if !changed {
                loop {
                    let (w2, r2, c2) = self.widest(n, limit, row, col);
                    if w2 != w {
                        break;
                    }
                    if self.rewrap(r2, c2)? {
                        changed = true;
                        break;
                    }
                    row = r2;
                    col = c2 + 1;
                }
                if !changed {
                    limit = w;
                }
            }
            round += 1;
        }
        Ok(n)
    }

    /// The widest cell among the rows laid (below `limit`, from (from_row, from_col) on), as the original searches:
    /// header rows and header columns are skipped for the body rows.
    fn widest(&mut self, n: usize, limit: isize, from_row: usize, from_col: usize) -> (isize, usize, usize) {
        let cols = self.n_cols();
        let (rh, ch) = self.heads();
        let mut best: isize = -1;
        let (mut br, mut bc) = (0usize, 0usize);
        let mut state = 0;
        let mut r = 0usize;
        while r < n {
            let mut c = 0usize;
            while c < cols {
                if r as i64 == rh || r + 1 == n {
                    state = 1;
                    if r < from_row {
                        r = from_row;
                    }
                }
                if c as i64 == ch && state == 1 && c < from_col {
                    c = from_col;
                    state = 2;
                    if from_col == cols {
                        break;
                    }
                }
                if r >= n || r >= self.n_rows() {
                    break;
                }
                let w = self.cell(r, c).width as isize;
                if w > best && (limit == -1 || w < limit) {
                    br = r;
                    bc = c;
                    best = w;
                }
                c += 1;
            }
            r += 1;
        }
        if br < from_row || (br == from_row && bc < from_col) {
            (-1, br, bc)
        } else {
            (best, br, bc)
        }
    }

    /// Re-wraps cell (r, c) into one more line; false when its width did not change (the old wrapping kept).
    fn rewrap(&mut self, r: usize, c: usize) -> R<bool> {
        let (old, lines) = (self.cell(r, c).width, self.cell(r, c).lines);
        let (w, _) = self.measure(r, c, lines as i64 + 1)?;
        if w == old {
            self.measure(r, c, lines as i64)?;
        }
        Ok(w != old)
    }

    /// The columnar layout pass (5.5): on the first try (the layout choice) over the whole table at the full page
    /// depth, else from the rows done at the current page depth; answers the rows laid.
    fn format_columnar(&mut self, first: bool) -> R<usize> {
        let mut row = self.row_fmt;
        let mut widths = [0usize; 2];
        let mut shrink = first;
        if first {
            row = 0;
            self.cur_depth = self.geometry.other_depth;
        }
        loop {
            let n = self.shrink(row, &mut widths, shrink)?;
            if widths[0] > self.width || widths[1] > self.width {
                if shrink {
                    return Err(format!("pageWidth ({}) exceeded ({}/{})", self.width, widths[0], widths[1]));
                }
            } else {
                let (rh, _) = self.heads();
                if n as i64 <= rh {
                    return Err(format!("No data rows (tr={n}; rowHeadings={rh})"));
                }
                row += n;
                if !first {
                    if row != self.row_fmt && row >= self.row_fmt {
                        return Ok(row - self.row_fmt);
                    }
                    if shrink {
                        return Err(format!("rowFmt - row = {}", row as isize - self.row_fmt as isize));
                    }
                } else {
                    if widths[1] > 0 {
                        self.layout = TWOPAGE;
                    }
                    if row >= self.n_rows() {
                        if self.layout != TWOPAGE {
                            self.layout = if widths[1] < 1 { COLUMNAR } else { TWOPAGE };
                        }
                        return Ok(row);
                    }
                }
            }
            shrink = true;
        }
    }

    /// Lays the rows that fit on the current page image (5.6); true when more pages follow.
    fn populate_columnar(&mut self) -> R<bool> {
        let right_half = self.layout == TWOPAGE && self.col_fmt > 0;
        if !right_half {
            self.rows_fmt = self.format_columnar(false)?;
        }
        let cols_to = if self.layout == TWOPAGE && self.col_fmt == 0 { self.boundary_cur } else { self.n_cols() };
        let rows = self.n_rows();
        if self.row_fmt == 0 && self.rows_fmt < rows && self.images.len() == 1 && self.geometry.first_depth < self.geometry.other_depth {
            // A table that does not fit the rest of its first page starts on a new page (the original's test
            // recomputes the same rows here and so always moves on).
            let n2 = self.format_columnar(false)?;
            if n2 < rows {
                return Ok(true);
            }
            if self.format_columnar(false)? < 3 {
                return Ok(true);
            }
        }
        let (rh, ch) = self.heads();
        let mut r = 0usize;
        let stopped = loop {
            if self.rows_fmt <= r {
                if self.layout == TWOPAGE && self.col_fmt == 0 {
                    self.col_fmt = self.boundary_cur;
                    return Ok(true);
                }
                self.row_fmt = r;
                self.col_fmt = 0;
                return Ok(self.row_fmt < rows);
            }
            if r as i64 == rh && r < self.row_fmt {
                r = self.row_fmt;
            }
            if r >= rows {
                break false;
            }
            if self.cur_depth < self.out_row + self.rows_needed[r] {
                break true;
            }
            let mut line = self.out_row;
            let from_col = if right_half { self.boundary_save } else { 0 };
            let mut c = from_col;
            while c < cols_to {
                if c as i64 == ch && c < self.col_fmt {
                    c = self.col_fmt;
                    if c >= cols_to {
                        break;
                    }
                }
                let markers = !(r < self.row_fmt || c < self.col_fmt);
                self.place_cell(r, c, &mut line, markers)?;
                c += 1;
            }
            self.out_row += self.rows_needed[r];
            r += 1;
        };
        if self.layout == TWOPAGE && right_half {
            return Err("Two-Page format irregularity".into());
        }
        if !stopped {
            self.row_fmt = rows;
            return Ok(false);
        }
        Ok(self.images.len() == 1 && self.geometry.first_below_head)
    }

    /// Places cell (r, c) of the columnar layout (5.6, the original's cell writer): the underline under the last
    /// header row, the fences of a matrix, the text with its line breaks at the runover, the markers, the guide dots.
    fn place_cell(&mut self, r: usize, c: usize, row_line: &mut usize, _markers: bool) -> R<()> {
        let (rh, ch) = self.heads();
        let cols = self.n_cols();
        let opt = self.opt;
        let text = self.cell(r, c).wrapped.clone();
        let n = text.len();
        let mut first_end = 0usize;
        let mut line;
        if r as i64 == rh - 1 {
            line = self.out_row + self.rows_needed[r] - 1;
            let fill = if n != 0 { opt.underline } else { ' ' };
            let (off, len) = (self.col_offsets[c], self.col_len[c]);
            self.fill(line, off, fill, len)?;
            if n != 0 && self.braille {
                self.putchr(line, off, '"', None)?;
            }
            let mut breaks = 1usize;
            for k in 0..n {
                if text[k].0 == '\n' {
                    breaks += 1;
                }
            }
            line = line.checked_sub(breaks).ok_or("row (-1) < 0")?;
        } else {
            line = *row_line;
        }
        self.out_col = self.col_offsets[c];
        if c == 0 {
            if let Some(f) = self.left_fence.clone() {
                for (ch, src) in f.chars {
                    self.putchr(line, self.out_col, ch, if r == 0 { src } else { None })?;
                    self.out_col += 1;
                }
            }
        }
        if n == 0 {
            let start = self.cell(r, c).start;
            let src = if start >= 0 { Some(start as usize) } else { None };
            self.putchr(self.out_row, self.out_col, ' ', src)?;
        } else {
            let marks = self.cell(r, c).marks.clone();
            let end = self.cell(r, c).end;
            for k in 0..n {
                let (ch, src) = text[k];
                for &(pos, at) in marks.iter().filter(|m| m.1 == k) {
                    self.mark(line, self.out_col, pos)?;
                    let _ = at;
                }
                if ch == '\n' {
                    if let Some(pos) = src {
                        self.mark(line, self.out_col, pos)?;
                    }
                    if line == *row_line {
                        first_end = self.out_col;
                    }
                    line += 1;
                    let runover = if r == 0 && rh != 0 { 0 } else { 2 };
                    self.out_col = self.col_offsets[c] + runover;
                } else {
                    self.putchr(line, self.out_col, ch, src)?;
                    self.out_col += 1;
                }
            }
            for &(pos, _) in marks.iter().filter(|m| m.1 >= n) {
                self.mark(line, self.out_col, pos)?;
            }
            self.mark(line, self.out_col, end)?;
        }
        if c + 1 == cols {
            if let Some(f) = self.right_fence.clone() {
                let fw = f.chars.len();
                while (self.col_offsets[c] + self.col_len[c]).saturating_sub(fw) > self.out_col {
                    self.putchr(line, self.out_col, ' ', None)?;
                    self.out_col += 1;
                }
                for (ch, src) in f.chars {
                    self.putchr(line, self.out_col, ch, if r == 0 { src } else { None })?;
                    self.out_col += 1;
                }
            }
        }
        if r as i64 >= rh && c as i64 == ch - 1 {
            *row_line = line;
        }
        if line == *row_line {
            first_end = self.out_col;
        }
        if self.kind == 0 {
            let mut guide = opt.guide;
            if r as i64 >= rh && c as i64 >= ch && n == 0 {
                // An empty body cell: the guide-dot lead string, then (BANA) the column filled with its last
                // character. A mark keeps the caret's place in the cell.
                let start = self.cell(r, c).start;
                if start >= 0 {
                    self.mark(*row_line, first_end, start as usize)?;
                }
                self.put_str(*row_line, first_end, opt.lead)?;
                if opt.fill_after_lead && !opt.lead.is_empty() {
                    guide = opt.lead.chars().last().unwrap_or(guide);
                    let count = (self.col_offsets[c] + self.col_len[c]) as isize - first_end as isize + opt.guide_extra as isize;
                    if count > 1 {
                        self.fill(*row_line, first_end, guide, count as usize)?;
                    }
                }
            }
            if r as i64 >= rh && c + 1 < cols && (n != 0 || c as i64 >= ch) && *row_line == line {
                let count = self.col_offsets[c + 1] as isize - first_end as isize - 3 + opt.guide_extra as isize;
                if count > 1 {
                    self.fill(*row_line, first_end + 1, guide, count as usize)?;
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- the page images

    fn image(&mut self) -> &mut Image {
        self.images.last_mut().expect("a page image")
    }

    fn new_image(&mut self) {
        let depth = if self.images.is_empty() { self.geometry.first_depth } else { self.geometry.other_depth };
        self.images.push(Image::new(depth));
        self.cur_depth = depth;
    }

    fn putchr(&mut self, row: usize, col: usize, c: char, src: Option<usize>) -> R<()> {
        let width = self.width;
        let img = self.image();
        if row >= img.depth {
            return Err("dxwxFormatHelperPage::putchr() -- row out-of-bounds".into());
        }
        if col >= width {
            return Err("dxwxFormatHelperPage::putchr() -- col out-of-bounds".into());
        }
        let line = &mut img.cells[row];
        while line.len() <= col {
            line.push((' ', None));
        }
        line[col] = (c, src);
        Ok(())
    }

    fn mark(&mut self, row: usize, col: usize, pos: usize) -> R<()> {
        let width = self.width;
        let img = self.image();
        if row >= img.depth {
            return Err("row >= lineCount".into());
        }
        if col > width {
            return Err("col > pgixc.ph".into());
        }
        img.marks[row].push((pos, col));
        Ok(())
    }

    fn fill(&mut self, row: usize, col: usize, c: char, count: usize) -> R<()> {
        for k in 0..count {
            self.putchr(row, col + k, c, None)?;
        }
        Ok(())
    }

    fn put_str(&mut self, row: usize, col: usize, s: &str) -> R<()> {
        for (k, c) in s.chars().enumerate() {
            self.putchr(row, col + k, c, None)?;
        }
        Ok(())
    }

    /// Blanks the image from line `from` down (a partial row taken back).
    fn blank_from(&mut self, from: usize) {
        let img = self.image();
        for k in from..img.depth {
            img.cells[k].clear();
            img.marks[k].clear();
        }
    }

    fn char_at(&mut self, row: usize, col: usize) -> Option<char> {
        self.image().cells.get(row).and_then(|l| l.get(col)).map(|c| c.0)
    }

    // ---------------------------------------------------------------- the layout choice (spec 35 section 4)

    fn clear_vectors(&mut self) {
        let cols = self.n_cols();
        let rows = self.n_rows();
        self.col_offsets = vec![0; cols];
        self.col_len = vec![0; cols];
        self.col_num = vec![true; cols];
        self.rows_needed = vec![0; rows];
        self.boundary_cur = 0;
        self.boundary_save = 0;
    }

    fn discard_pages(&mut self) {
        self.images.clear();
        self.out_row = 0;
        self.out_col = 0;
    }

    fn size(&self) -> usize {
        self.rows_needed.iter().sum()
    }

    /// One attempt in the current orientation: the layout choice and its size.
    fn attempt(&mut self) -> R<(bool, u8, usize)> {
        self.heads = None;
        self.clear_vectors();
        self.set_type()?;
        let size = self.size();
        Ok((size != 0, self.layout, size))
    }

    /// The swap decision and the layout (section 4).
    fn finalize(&mut self) -> R<()> {
        let mut first: Option<(bool, u8, usize)> = None;
        if self.swap_allowed != 1 {
            self.swapped = false;
            first = Some(self.attempt()?);
        }
        if self.swap_allowed != 0 {
            self.discard_pages();
            self.swapped = true;
            self.layout = 0;
            let (ok1, layout1, size1) = self.attempt()?;
            if self.swap_allowed == 2 {
                let (ok0, layout0, size0) = first.unwrap_or((false, 0, 0));
                let keep_swapped = if !ok0 || !ok1 {
                    if !ok1 {
                        false
                    } else if layout1 == layout0 {
                        size1 < size0
                    } else {
                        matches!(layout1, COLUMNAR | TWOPAGE) && !matches!(layout0, COLUMNAR | TWOPAGE)
                    }
                } else {
                    true
                };
                if !keep_swapped {
                    self.discard_pages();
                    self.swapped = false;
                    self.layout = 0;
                    self.attempt()?;
                }
            }
        }
        Ok(())
    }

    /// setType: the layout by what was requested, automatic when none; linear is the fallback of all.
    fn set_type(&mut self) -> R<()> {
        self.row_fmt = 0;
        self.col_fmt = 0;
        if self.layout == 0 {
            let result = match self.layout_requested {
                0 => self.set_type_auto(),
                COLUMNAR | TWOPAGE => self.format_columnar(true).map(|_| {
                    if self.layout == TWOPAGE {
                        self.geometry.other_depth = self.geometry.first_depth;
                    }
                }),
                STAIRSTEP => {
                    self.format_stairstep();
                    Ok(())
                }
                l => {
                    self.layout = l;
                    Ok(())
                }
            };
            // An exception in setType is only logged; the epilogue falls back.
            let _ = result;
        }
        self.row_fmt = 0;
        self.col_fmt = 0;
        if self.layout == 0 {
            self.layout = self.layout_requested;
        }
        if self.layout == 0 {
            self.layout = LINEAR;
        }
        Ok(())
    }

    /// The automatic sequence: each entry tried in turn, a failure clearing the pages for the next.
    fn set_type_auto(&mut self) -> R<()> {
        if self.n_cols() == 0 {
            return Ok(());
        }
        for k in 0..self.auto_seq.len() {
            let entry = self.auto_seq[k];
            self.row_fmt = 0;
            self.col_fmt = 0;
            let ok = match entry {
                COLUMNAR | TWOPAGE => self.format_columnar(true).is_ok(),
                STAIRSTEP => {
                    self.format_stairstep();
                    true
                }
                _ => {
                    self.layout = entry;
                    matches!(self.populate(), Ok(true))
                }
            };
            if ok {
                return Ok(());
            }
            self.discard_pages();
            self.layout = 0;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- populate (spec 35 section 6)

    /// Lays the table into page images (6.1); true when at least one data row was laid.
    fn populate(&mut self) -> R<bool> {
        self.out_row = 0;
        self.discard_pages();
        self.new_image();
        let mut pages = 0;
        loop {
            let more = match self.layout {
                COLUMNAR | TWOPAGE => self.populate_columnar()?,
                STAIRSTEP => self.populate_stairstep()?,
                LISTED => self.populate_listed()?,
                _ => self.populate_linear()?,
            };
            if !more {
                let listed = usize::from(self.layout == LISTED);
                return Ok(listed < self.row_fmt);
            }
            self.new_image();
            pages += 1;
            if pages > 50 {
                return Err("Page count for table exceeds 50".into());
            }
            self.out_row = 0;
        }
    }

    /// Stairstep offsets (6.2).
    fn format_stairstep(&mut self) {
        let cols = self.n_cols();
        let w = self.width;
        let step = if cols * 3 <= 2 * w {
            if cols * 3 <= w {
                2
            } else {
                1
            }
        } else {
            0
        };
        for c in 0..cols {
            self.col_offsets[c] = c * step;
        }
        self.layout = STAIRSTEP;
    }

    fn populate_stairstep(&mut self) -> R<bool> {
        let rows = self.n_rows();
        let cols = self.n_cols();
        if self.out_row > 0 && self.out_row + rows * cols >= self.cur_depth {
            return Ok(true);
        }
        loop {
            if rows <= self.row_fmt {
                return Ok(false);
            }
            let row_start = self.out_row;
            while self.col_fmt < cols {
                let (r, c) = (self.row_fmt, self.col_fmt);
                let start = self.col_offsets[c];
                let runover = start + self.opt.stair_runover;
                let (text, end) = (self.cell(r, c).chars.clone(), self.cell(r, c).end);
                let marks = self.cell(r, c).marks.clone();
                let (fits, _) = self.wrap_cell(&text, &marks, end, start, runover, 0, true)?;
                if !fits {
                    if row_start > 0 {
                        self.blank_from(row_start);
                        self.col_fmt = 0;
                    }
                    return Ok(true);
                }
                self.out_row += 1;
                self.col_fmt += 1;
            }
            self.row_fmt += 1;
            self.col_fmt = 0;
        }
    }

    /// Listed (6.3): each cell as its column heading, the separator and the cell text, wrapped as one.
    fn populate_listed(&mut self) -> R<bool> {
        let (rh, ch) = self.heads();
        if rh != 1 || ch != 1 {
            return Err("Listed format requires 1 header row and 1 header column".into());
        }
        let rows = self.n_rows();
        let cols = self.n_cols();
        if self.out_row > 0 && self.out_row + rows * cols >= self.cur_depth {
            return Ok(true);
        }
        if self.row_fmt == 0 {
            self.row_fmt = 1;
        }
        loop {
            if rows <= self.row_fmt {
                return Ok(false);
            }
            let row_start = self.out_row;
            while self.col_fmt < cols {
                let (r, c) = (self.row_fmt, self.col_fmt);
                let heading = self.cell(0, c).clone();
                let data = self.cell(r, c).clone();
                let first_row = r == 1;
                let mut comp: Vec<(char, Option<usize>)> = heading.chars.iter().map(|&(ch, s)| (ch, if first_row { s } else { None })).collect();
                let mut marks: Vec<(usize, usize)> = if first_row { heading.marks.clone() } else { Vec::new() };
                if !data.chars.is_empty() {
                    for (k, ch) in self.opt.listed_sep.chars().enumerate() {
                        comp.push((ch, if k == 0 && first_row { Some(heading.end) } else { None }));
                    }
                }
                if data.empty() {
                    for (k, ch) in self.opt.listed_empty.chars().enumerate() {
                        comp.push((ch, if k == 0 && data.start >= 0 { Some(data.start as usize) } else { None }));
                    }
                } else {
                    let base = comp.len();
                    marks.extend(data.marks.iter().map(|&(p, k)| (p, k + base)));
                    comp.extend(data.chars.iter().copied());
                }
                let (start, runover) = if c == 0 { (self.opt.listed_first_start, self.opt.listed_first_runover) } else { (self.opt.listed_start, self.opt.listed_runover) };
                let (fits, _) = self.wrap_cell(&comp, &marks, data.end, start, runover, 0, true)?;
                if !fits {
                    if row_start > 0 {
                        self.blank_from(row_start);
                        self.col_fmt = 0;
                    }
                    return Ok(true);
                }
                self.out_row += 1;
                self.col_fmt += 1;
            }
            self.out_row += 1;
            self.row_fmt += 1;
            self.col_fmt = 0;
        }
    }

    /// Linear (6.4): the cells of a row one after another with separators, as one block.
    fn populate_linear(&mut self) -> R<bool> {
        let rows = self.n_rows();
        let cols = self.n_cols();
        let (r0, c0) = (self.row_fmt, self.col_fmt);
        self.out_col = 0;
        loop {
            if rows <= self.row_fmt {
                return Ok(false);
            }
            let row_start = self.out_row;
            while self.col_fmt < cols {
                if !self.linear_cell()? {
                    if self.row_fmt == r0 && self.col_fmt == c0 {
                        return Err("Cannot fit cell on page".into());
                    }
                    if r0 < self.row_fmt {
                        self.blank_from(row_start);
                        self.col_fmt = 0;
                    }
                    return Ok(true);
                }
                self.col_fmt += 1;
            }
            self.row_fmt += 1;
            self.col_fmt = 0;
        }
    }

    fn linear_cell(&mut self) -> R<bool> {
        let cols = self.n_cols();
        let opt = self.opt;
        let (r, c) = (self.row_fmt, self.col_fmt);
        if c == 0 {
            self.out_col = opt.linear_start;
        }
        let (text, end, marks) = {
            let cell = self.cell(r, c);
            (cell.chars.clone(), cell.end, cell.marks.clone())
        };
        let (fits, non_empty) = self.wrap_cell(&text, &marks, end, self.out_col, opt.linear_runover, 1, true)?;
        if !fits {
            return Ok(false);
        }
        if !non_empty {
            let need = if c + 1 == cols { 3 } else { 4 };
            if self.width.saturating_sub(need) <= self.out_col {
                if self.cur_depth <= self.out_row + 1 {
                    return Ok(false);
                }
                self.out_row += 1;
                self.out_col = opt.linear_runover;
            }
            self.put_str(self.out_row, self.out_col, opt.linear_empty)?;
            self.mark(self.out_row, self.out_col, end)?;
            self.out_col += opt.linear_empty.chars().count();
        }
        let sep = if c + 1 == cols {
            opt.sep_end
        } else if c == 0 {
            Some(opt.sep_first)
        } else {
            Some(opt.sep_other)
        };
        if let Some(sep) = sep {
            if self.out_col > 0 && self.char_at(self.out_row, self.out_col - 1) != Some(sep) {
                let src = if non_empty { Some(end) } else { None };
                self.putchr(self.out_row, self.out_col, sep, src)?;
                self.out_col += 1;
            }
        }
        if self.out_col < self.width {
            self.out_col += 1;
        }
        if c + 1 == cols {
            self.out_row += 1;
            self.out_col = opt.linear_start;
        }
        Ok(true)
    }

    /// Wraps one cell's text from `start` with runover lines at `runover` (6.5): lines counted first, then written
    /// with the markers; false when the lines do not fit the page image. Answers (fits, non-empty).
    fn wrap_cell(&mut self, text: &[(char, Option<usize>)], marks: &[(usize, usize)], end: usize, start: usize, runover: usize, reserve: usize, markers: bool) -> R<(bool, bool)> {
        self.out_col = start;
        let len = text.len();
        let w = self.width;
        if len == 0 {
            return Ok((self.out_row < self.cur_depth, false));
        }
        let room_at = |col: usize| if col > w { usize::MAX / 2 } else { w - col };
        // The count.
        let mut pos = 0usize;
        let mut lines = 1usize;
        let mut first = true;
        loop {
            let col = if first { self.out_col } else { runover };
            let mut room = room_at(col);
            if len + reserve - pos <= room {
                break;
            }
            let mut skip = true;
            let mut room2 = room;
            if len - pos <= room {
                room2 = room.saturating_sub(reserve);
                room = room2;
            }
            let mut e = room;
            while e > 0 && text.get(pos + e).is_some_and(|c| c.0 > ' ') {
                e -= 1;
            }
            if e == 0 {
                skip = false;
                if !first || self.out_col <= runover {
                    e = room2;
                }
            }
            let mut f = 0;
            while f < e && text.get(pos + f).is_some_and(|c| c.0 > '\u{1f}') {
                f += 1;
            }
            if f < e {
                e = f;
            }
            if e == 0 && !skip && !first {
                return Err("putCell: cell line count cannot be calculated".into());
            }
            pos += e;
            if skip {
                pos += 1;
            }
            if pos > len {
                break;
            }
            lines += 1;
            first = false;
        }
        if self.out_row + lines >= self.cur_depth {
            return Ok((false, true));
        }
        // The writing.
        pos = 0;
        let mut done = false;
        first = true;
        while !done {
            let mut room = room_at(self.out_col);
            let mut skip = true;
            let mut e;
            if room < len + reserve - pos {
                let mut room2 = room;
                if len - pos <= room {
                    room2 = room.saturating_sub(reserve);
                    room = room2;
                }
                e = room;
                while e > 0 && text.get(pos + e).is_some_and(|c| c.0 > ' ') {
                    e -= 1;
                }
                if e == 0 && (!first || self.out_col <= runover) {
                    e = room2;
                }
            } else {
                done = true;
                skip = false;
                e = len - pos;
            }
            if e == 0 {
                skip = false;
            }
            let mut f = 0;
            while f < e && text.get(pos + f).is_some_and(|c| c.0 > '\u{1f}') {
                f += 1;
            }
            if f < e {
                e = f;
                done = false;
                skip = true;
            }
            if e == 0 && !done && !skip && !first {
                return Err("putCell: cell line count cannot be calculated".into());
            }
            for k in 0..e {
                let (c, src) = text[pos + k];
                for &(m, _) in marks.iter().filter(|m| m.1 == pos + k) {
                    self.mark(self.out_row, self.out_col, m)?;
                }
                self.putchr(self.out_row, self.out_col, c, src)?;
                self.out_col += 1;
            }
            if !done {
                if self.out_col < w && markers {
                    if let Some(src) = text.get(pos + e).and_then(|c| c.1) {
                        self.mark(self.out_row, self.out_col, src)?;
                    }
                }
                pos += e;
                if skip {
                    pos += 1;
                }
                if pos >= len {
                    done = true;
                }
                self.out_row += 1;
                self.out_col = runover;
            }
            first = false;
        }
        for &(m, k) in marks.iter().filter(|m| m.1 >= len) {
            let _ = k;
            self.mark(self.out_row, self.out_col.min(w), m)?;
        }
        if self.out_col < w && markers {
            self.mark(self.out_row, self.out_col, end)?;
        }
        Ok((true, true))
    }
}

/// The table regions of a running text alone (no layout): the cells by row as the editor commands need them
/// (Table Properties, Style Level, the "in a table" tests), from [htbs] to the first [htbe] (the original's one
/// object swallows every word up to it, so tables do not nest).
pub fn regions(pieces: &[Piece], braille: bool) -> Vec<TableRegion> {
    let mut out = Vec::new();
    let mut table: Option<Collector> = None;
    let geometry = Geometry { first_depth: 25, other_depth: 25, width: 40, first_below_head: false };
    let finish = |mut t: Collector, end: usize| {
        t.set_end(end);
        t.region(None)
    };
    for p in pieces {
        match &mut table {
            Some(t) => {
                if !t.take(p) {
                    let t = table.take().unwrap();
                    let end = match p {
                        Piece::Mark(m) => m + 6,
                        _ => t.last_mark().map_or(0, |m| m + 1),
                    };
                    out.push(finish(t, end));
                }
            }
            None => {
                if let Piece::Code(code) = p {
                    if code.starts_with("htbs") {
                        table = Some(Collector::new(code, &(0, String::new()), braille, geometry));
                    }
                }
            }
        }
    }
    if let Some(t) = table {
        let end = t.last_mark().map_or(0, |m| m + 1);
        out.push(finish(t, end));
    }
    out
}
