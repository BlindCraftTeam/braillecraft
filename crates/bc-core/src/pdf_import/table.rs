//! Ledger tables in a scraped page: a port of the reference importer's table finder (table.py). A row is a text line
//! that ends in an amount, with the lines beside it on the same height read as cells (date D, text T, the amount
//! that started it *, other amounts A); rows below it with the same pattern of cells make a table, the line above
//! each column may be its header, and text under a cell that wrapped is added to that cell. The rules and their
//! quirks are kept as the reference has them.

use super::layout::{Layout, Node, Plane, Rect};

/// One cell: the lines in it, and the box around them.
#[derive(Debug, Clone)]
pub struct Cell {
    pub rect: Rect,
    pub lines: Vec<usize>,
}

impl Cell {
    fn new(line: usize, layout: &Layout) -> Cell {
        Cell { rect: layout.lines[line].rect, lines: vec![line] }
    }

    fn add(&mut self, line: usize, layout: &Layout) {
        self.lines.push(line);
        self.rect = self.rect.union(&layout.lines[line].rect);
    }

    pub fn text(&self, layout: &Layout) -> String {
        self.lines.iter().map(|&l| layout.line_text(l)).collect()
    }
}

/// One row (LTDuxTableRow): its cells and the pattern of their kinds.
#[derive(Debug, Clone)]
pub struct Row {
    pub rect: Rect,
    pub cells: Vec<Cell>,
    pub anchor: usize,
    pub line_desc: String,
    last_cell: Option<usize>,
    /// A header row (no column list of its own in the reference).
    pub header: bool,
}

impl Row {
    fn new(anchor: usize) -> Row {
        Row { rect: Rect::EMPTY, cells: Vec::new(), anchor, line_desc: String::new(), last_cell: None, header: false }
    }

    fn add(&mut self, line: usize, layout: &Layout) {
        let text = layout.line_text(line);
        let kind = if line == self.anchor {
            '*'
        } else if is_amount(&text) {
            'A'
        } else if is_date(&text) {
            'D'
        } else {
            match self.last_cell {
                Some(c) if layout.lines[line].rect.is_hoverlap(&self.cells[c].rect) => {
                    // Joined to the last cell, whatever its kind (the reference presumes it is text); the row's
                    // own box is not widened by it, as in the reference.
                    self.cells[c].add(line, layout);
                    return;
                }
                _ => 'T',
            }
        };
        self.cells.push(Cell::new(line, layout));
        self.last_cell = Some(self.cells.len() - 1);
        self.line_desc.push(kind);
        self.rect = self.rect.union(&layout.lines[line].rect);
    }
}

/// A table (LTDuxTable): its rows, header first when it has one.
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: Vec<Row>,
}

/// An amount: `^[+-=]? ?[$]?[0-9,]+[.][0-9][0-9][ \n]+$` (the first class is the range + to =, digits included).
pub fn is_amount(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    let rest = |mut i: usize| -> bool {
        let start = i;
        while i < c.len() && (c[i].is_ascii_digit() || c[i] == ',') {
            i += 1;
        }
        if i == start || c.get(i) != Some(&'.') {
            return false;
        }
        i += 1;
        if !(c.get(i).is_some_and(char::is_ascii_digit) && c.get(i + 1).is_some_and(char::is_ascii_digit)) {
            return false;
        }
        i += 2;
        i < c.len() && c[i..].iter().all(|&x| x == ' ' || x == '\n')
    };
    // The three optional parts, each tried taken and not taken, as the expression backtracks.
    for sign in [true, false] {
        let mut i = 0;
        if sign {
            if c.first().is_some_and(|&x| ('+'..='=').contains(&x)) {
                i += 1;
            } else {
                continue;
            }
        }
        for space in [true, false] {
            let mut j = i;
            if space {
                if c.get(j) == Some(&' ') {
                    j += 1;
                } else {
                    continue;
                }
            }
            for dollar in [true, false] {
                let mut k = j;
                if dollar {
                    if c.get(k) == Some(&'$') {
                        k += 1;
                    } else {
                        continue;
                    }
                }
                if rest(k) {
                    return true;
                }
            }
        }
    }
    false
}

/// A date at the start: `[0-1]?[0-9]/[0-2]?[0-9]`, or a month's short name, spaces and `[0-2]?[0-9]`.
pub fn is_date(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    // [lo-hi]?[0-9] at i: the positions it can end at.
    let opt_digit = |i: usize, hi: char| -> Vec<usize> {
        let mut ends = Vec::new();
        if c.get(i).is_some_and(|&x| ('0'..=hi).contains(&x)) && c.get(i + 1).is_some_and(char::is_ascii_digit) {
            ends.push(i + 2);
        }
        if c.get(i).is_some_and(char::is_ascii_digit) {
            ends.push(i + 1);
        }
        ends
    };
    for e in opt_digit(0, '1') {
        if c.get(e) == Some(&'/') && !opt_digit(e + 1, '2').is_empty() {
            return true;
        }
    }
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    if c.len() >= 3 && MONTHS.contains(&c[..3].iter().collect::<String>().as_str()) {
        let mut i = 3;
        while c.get(i) == Some(&' ') {
            i += 1;
        }
        return i > 3 && !opt_digit(i, '2').is_empty();
    }
    false
}

const PATTERNS: [&str; 12] = ["DDT*A", "DDTT*A", "DT*A", "DTT*A", "T*A", "TT*A", "DDT*", "DDTT*", "DT*", "DTT*", "T*", "TT*"];

/// The first pattern found in a row's description: (start, end) in cells.
fn find_pattern(desc: &str) -> Option<(usize, usize)> {
    PATTERNS.iter().find_map(|p| desc.find(p).map(|f| (f, f + p.len())))
}

/// Finds tables on a scraped page and tags their lines.
pub struct TableFinder<'a> {
    layout: &'a Layout,
    plane: Plane<usize>,
    /// For each line: the table it belongs to (pdfminer's dux_table).
    pub table_of: Vec<Option<usize>>,
    pub tables: Vec<Table>,
}

impl<'a> TableFinder<'a> {
    /// `media_box` is the page's media box as the file gives it; `header` and `footer` the bands left out.
    pub fn new(layout: &'a Layout, media_box: Rect, header: Rect, footer: Rect) -> TableFinder<'a> {
        let mut plane = Plane::new(media_box);
        for line in group_lines(layout) {
            let r = layout.lines[line].rect;
            if r.is_voverlap(&header) || r.is_voverlap(&footer) {
                continue;
            }
            plane.add(line, r);
        }
        TableFinder { layout, plane, table_of: vec![None; layout.lines.len()], tables: Vec::new() }
    }

    fn sorted(&self, r: Rect, key: impl Fn(&Rect) -> f64) -> Vec<usize> {
        let mut v = self.plane.find(&r);
        v.sort_by(|&a, &b| key(&self.layout.lines[a].rect).total_cmp(&key(&self.layout.lines[b].rect)));
        v
    }

    fn items_on_same_line(&self, line: usize) -> Vec<usize> {
        let r = self.layout.lines[line].rect;
        self.sorted(Rect::new(0.0, r.y0, 1000.0, r.y1), |o| 1000.0 * o.x0 - o.y0)
    }

    fn items_below(&self, line: usize) -> Vec<usize> {
        let r = self.layout.lines[line].rect;
        self.sorted(Rect::new(r.x0, 0.0, r.x1, r.y0), |o| -o.y0)
    }

    fn first_item_above(&self, r: Rect) -> Option<usize> {
        self.sorted(Rect::new(r.x0, r.y1, r.x1, 10000.0), |o| o.y0).first().copied()
    }

    fn untagged_amount(&self, line: usize) -> bool {
        self.table_of[line].is_none() && is_amount(&self.layout.line_text(line))
    }

    /// The row a line anchors: the lines on its height, cut to the first pattern that fits (and the cell after it,
    /// as the reference's inclusive end takes one more).
    fn table_row(&self, line: usize) -> Option<Row> {
        let mut row = Row::new(line);
        for n in self.items_on_same_line(line) {
            row.add(n, self.layout);
        }
        let (first, last) = find_pattern(&row.line_desc)?;
        let mut new_row = Row::new(line);
        for (col, cell) in row.cells.iter().enumerate() {
            if col >= first && col <= last {
                for &l in &cell.lines {
                    new_row.add(l, self.layout);
                }
            }
        }
        Some(new_row)
    }

    /// find_ledgers: every untagged amount line, in group order, may start a table.
    pub fn find_ledgers(&mut self) {
        for item in group_lines(self.layout) {
            if !self.untagged_amount(item) {
                continue;
            }
            let Some(original) = self.table_row(item) else { continue };
            let mut rows: Vec<Row> = Vec::new();
            for neighbor in self.items_below(item) {
                if !self.untagged_amount(neighbor) {
                    continue;
                }
                match self.table_row(neighbor) {
                    Some(r) if r.line_desc == original.line_desc => {
                        if rows.is_empty() {
                            rows.push(original.clone());
                        }
                        rows.push(r);
                    }
                    _ => break,
                }
            }
            if rows.is_empty() {
                continue;
            }
            self.add_header_row(&mut rows);
            self.complete_wrapped_text_cells(&mut rows);
            let t = self.tables.len();
            for row in &rows {
                for cell in &row.cells {
                    for &l in &cell.lines {
                        self.table_of[l] = Some(t);
                    }
                }
            }
            self.tables.push(Table { rows });
        }
    }

    /// The line above each column of the first row, when they make a header: none missing, no line twice, all on
    /// one height, and (after the first) none an amount or a date.
    fn add_header_row(&self, rows: &mut Vec<Row>) {
        let mut last: Option<usize> = None;
        let mut header: Option<Row> = None;
        for cell in &rows[0].cells {
            let Some(h) = self.first_item_above(cell.rect) else { return };
            if let Some(l) = last {
                let (lr, hr) = (self.layout.lines[l].rect, self.layout.lines[h].rect);
                let text = self.layout.line_text(h);
                if h == l || lr.voverlap(&hr) != lr.height() || is_amount(&text) || is_date(&text) {
                    return;
                }
            }
            last = Some(h);
            header.get_or_insert_with(|| Row::new(h)).add(h, self.layout);
        }
        if let Some(mut h) = header {
            h.header = true;
            rows.insert(0, h);
        }
    }

    /// Lines under a cell, down to the next row (or one row height below the last), join that cell.
    fn complete_wrapped_text_cells(&self, rows: &mut [Row]) {
        for i in 0..rows.len() {
            if rows[i].header {
                continue;
            }
            let bottom = match rows.get(i + 1) {
                Some(next) => next.rect.y1,
                None => rows[i].rect.y0 - rows[i].rect.height(),
            };
            for j in 0..rows[i].cells.len() {
                let r = rows[i].cells[j].rect;
                for orphan in self.sorted(Rect::new(r.x0, bottom, r.x1, r.y0), |o| -o.y0) {
                    rows[i].cells[j].add(orphan, self.layout);
                }
            }
        }
    }
}

/// The text lines of the page's groups, depth first in order.
pub fn group_lines(layout: &Layout) -> Vec<usize> {
    let mut out = Vec::new();
    for &g in layout.groups.as_deref().unwrap_or(&[]) {
        node_lines(layout, g, &mut out);
    }
    out
}

fn node_lines(layout: &Layout, n: Node, out: &mut Vec<usize>) {
    layout.node_lines(n, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_and_dates() {
        assert!(is_amount("1,234.56\n"));
        assert!(is_amount("$5.00 \n"));
        assert!(is_amount("- $12.30\n"));
        assert!(is_amount("5.00\n"));
        assert!(!is_amount("5.0\n"));
        assert!(!is_amount("Total 5.00\n"));
        assert!(is_date("1/5\n"));
        assert!(is_date("12/31/24 Payment\n"));
        assert!(is_date("Jan  3\n"));
        assert!(!is_date("Janet\n"));
        assert_eq!(find_pattern("DTT*A"), Some((0, 5)));
        assert_eq!(find_pattern("TDT*A"), Some((1, 5)));
        assert_eq!(find_pattern("AA"), None);
    }
}
