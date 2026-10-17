//! Table, Create and Table, Table Properties as the original (spec 33): the table start code [htbs;...] read and
//! written, a new table's codes, and the test that refuses a selection holding a table code. Places are running-text
//! places (codes marked 0x1C ... 0x1F).

use crate::doctext::{CLOSE, OPEN};

/// The Table type list (strings 2063-2070) with the type and sub-kind letters each writes (spec 33 3.1).
pub const TYPES: [(&str, char, char); 8] = [
    ("Unrelated Columns", 'u', 'a'),
    ("Matrix", 'm', 'a'),
    ("Table: [Automatic]", 'r', 'a'),
    ("Table: Columnar", 'r', 'c'),
    ("Table: Stairstep", 'r', 's'),
    ("Table: Listed", 'r', 'l'),
    ("Table: Linear", 'r', 'p'),
    ("Table: Two-Page", 'r', '2'),
];

/// The X/Y (Row/Column) Swap list (strings 2059-2061) with the letter each writes (spec 33 3.2).
pub const SWAPS: [(&str, char); 3] = [("Do Not Swap", 'n'), ("Force Swap", 'y'), ("Swap if More Compact", 'a')];

/// The message of a selection holding a table start or end code (a literal in the original).
pub const SPANS_BOUNDARY: &str = "You have a selection that spans across a table boundary.  Please place the cursor in a table to set table properties or outside of all tables to create a new table.";

/// A table as the dialog shows it (spec 33 1.1): total rows and columns, the type letter, header rows and columns,
/// the hidden fourth letter, the swap letter and the sub-kind letter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    pub rows: u32,
    pub cols: u32,
    pub kind: char,
    pub header_rows: i64,
    pub header_cols: i64,
    pub hidden: char,
    pub swap: char,
    pub sub: char,
}

impl Default for Table {
    /// A new table: 3 rows, 4 columns, Automatic, one header row and column, no swap.
    fn default() -> Table {
        Table { rows: 3, cols: 4, kind: 'r', header_rows: 1, header_cols: 1, hidden: 'b', swap: 'n', sub: 'a' }
    }
}

impl Table {
    /// The start code's name and parameters read as the original reads them, by the pattern
    /// htbs;%c:%d:%d:%c:%c:%c, a field not read taking its default (spec 33 4).
    pub fn parse(code: &str) -> Table {
        let mut t = Table::default();
        let c: Vec<char> = code.chars().collect();
        let mut i = 0;
        let mut n = 0;
        let lit = |i: &mut usize, s: &str| {
            let ok = s.chars().enumerate().all(|(k, x)| c.get(*i + k) == Some(&x));
            if ok {
                *i += s.chars().count();
            }
            ok
        };
        let num = |i: &mut usize| -> Option<i64> {
            while c.get(*i).is_some_and(|x| x.is_whitespace()) {
                *i += 1;
            }
            let start = *i;
            if matches!(c.get(*i), Some('-' | '+')) {
                *i += 1;
            }
            let digits = *i;
            while c.get(*i).is_some_and(|x| x.is_ascii_digit()) {
                *i += 1;
            }
            if *i == digits {
                *i = start;
                return None;
            }
            c[start..*i].iter().collect::<String>().parse().ok()
        };
        let mut letters = [t.kind, t.hidden, t.swap, t.sub];
        let mut numbers = [0i64; 2];
        'read: {
            if !lit(&mut i, "htbs;") {
                break 'read;
            }
            // Fields in order: type, header rows, header columns, hidden, swap, sub-kind.
            for f in 0..6 {
                if f > 0 && !lit(&mut i, ":") {
                    break 'read;
                }
                match f {
                    1 | 2 => match num(&mut i) {
                        Some(v) => numbers[f - 1] = v,
                        None => break 'read,
                    },
                    _ => match c.get(i) {
                        Some(&x) => {
                            letters[[0, 0, 0, 1, 2, 3][f]] = x;
                            i += 1;
                        }
                        None => break 'read,
                    },
                }
                n += 1;
            }
        }
        t.kind = if n >= 1 { letters[0] } else { 'r' };
        let plain = matches!(t.kind, 'u' | 'U' | 'm' | 'M');
        t.header_rows = if n >= 2 { numbers[0] } else if plain { 0 } else { 1 };
        t.header_cols = if n >= 3 { numbers[1] } else if plain { 0 } else { 1 };
        t.hidden = if n >= 4 { letters[1] } else { 'b' };
        t.swap = if n >= 5 { letters[2] } else { 'n' };
        t.sub = if n >= 6 { letters[3] } else { 'a' };
        t
    }

    /// The type list item shown for this table (spec 33 3.1).
    pub fn type_index(&self) -> usize {
        match self.kind {
            'm' | 'M' => 1,
            'u' | 'U' => 0,
            _ => match self.sub {
                'c' | 'C' => 3,
                's' | 'S' => 4,
                'l' | 'L' => 5,
                'p' | 'P' => 6,
                '2' => 7,
                _ => 2,
            },
        }
    }

    /// The swap list item shown for this table (spec 33 3.2).
    pub fn swap_index(&self) -> usize {
        match self.swap {
            '1' | 'Y' | 'y' => 1,
            'A' | 'a' => 2,
            _ => 0,
        }
    }

    pub fn set_type(&mut self, index: usize) {
        if let Some(&(_, kind, sub)) = TYPES.get(index) {
            self.kind = kind;
            self.sub = sub;
        }
    }

    /// The swap letter of list item `index`, set as soon as the list is changed (spec 33 3.2).
    pub fn set_swap(&mut self, index: usize) {
        self.swap = match index {
            1 => 'y',
            2 => 'a',
            _ => 'n',
        };
    }

    fn plain(&self) -> bool {
        matches!(self.kind, 'u' | 'U' | 'm' | 'M')
    }

    /// The start code's name and parameters (spec 33 5.2).
    pub fn code(&self) -> String {
        if self.plain() {
            format!("htbs;{}", self.kind)
        } else {
            format!("htbs;{}:{}:{}:{}:{}:{}", self.kind, self.header_rows, self.header_cols, self.hidden, self.swap, self.sub)
        }
    }

    /// A new table (spec 33 5.3): the start code, for a Matrix two cell codes, then the rows (a row code between
    /// them) of empty cells (a cell code between them), then the end code. Answers the running text and the place
    /// after the start code, where the caret goes.
    pub fn new_table(&self) -> (String, usize) {
        let code = |name: &str| format!("{OPEN}{name}{CLOSE}");
        let start = code(&self.code());
        let mut out = start.clone();
        if matches!(self.kind, 'm' | 'M') {
            out.push_str(&code(">"));
            out.push_str(&code(">"));
        }
        for r in 0..self.rows {
            if r > 0 {
                out.push_str(&code("<"));
            }
            for _ in 1..self.cols {
                out.push_str(&code(">"));
            }
        }
        out.push_str(&code("htbe"));
        (out, start.chars().count())
    }
}

/// A number field as the original reads it: its ASCII digits only, anything else skipped ("-3" is 3, empty is 0).
pub fn digits(field: &str) -> i64 {
    field.chars().filter(char::is_ascii_digit).fold(0i64, |n, d| n.saturating_mul(10).saturating_add(i64::from(d as u8 - b'0')))
}

/// The codes of running text `r` from `from` to `to`: (name, place of the 0x1C, place after the 0x1F); a code cut
/// by `from` is not one of them.
fn codes_in(r: &[char], from: usize, to: usize) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    for (p, &c) in r.iter().enumerate().take(to.min(r.len())).skip(from) {
        if c == OPEN {
            open = Some(p);
        } else if c == CLOSE {
            if let Some(o) = open.take() {
                out.push((r[o + 1..p].iter().collect(), o, p + 1));
            }
        }
    }
    out
}

/// True when the selection from `a` to `b` holds a table start code (a name beginning htbs) or end code (htbe):
/// the original then refuses Table, Create and Table Properties (spec 33 2.2).
pub fn selection_has_table_code(running: &str, a: usize, b: usize) -> bool {
    let r: Vec<char> = running.chars().collect();
    codes_in(&r, a.min(b), a.max(b)).iter().any(|(n, _, _)| n.starts_with("htbs") || n == "htbe")
}

/// The table the caret is in: its start code (the place of its 0x1C and after its 0x1F) and its number of rows and
/// columns, from the formatter's table regions (spec 35 section 7): a table runs from its start code to the first
/// end code (tables do not nest), its rows are the rows collected and its columns the most cells in a row (a
/// matrix without its fence cells).
pub fn table_at(running: &str, caret: usize) -> Option<(usize, usize, u32, u32)> {
    let doc = crate::doctext::Shown::from_running(running);
    let regions = crate::format::table_regions(&doc);
    let t = regions.iter().find(|t| t.holds(caret))?;
    let r: Vec<char> = running.chars().collect();
    let after = r[t.start..].iter().position(|&c| c == CLOSE).map_or(t.start + 1, |p| t.start + p + 1);
    Some((t.start, after, t.rows.len() as u32, t.cols as u32))
}

/// Table Properties' change (spec 33 5.4): the start code at `open` (its 0x1C) to `after` (after its 0x1F) gets
/// `table`'s code; the code's marks stay. Answers the new running text and the change in length.
pub fn set_start_code(running: &str, open: usize, after: usize, table: &Table) -> (String, isize) {
    let r: Vec<char> = running.chars().collect();
    let new = table.code();
    let old_len = after - open - 2;
    let mut out: String = r[..open + 1].iter().collect();
    out.push_str(&new);
    out.extend(&r[after - 1..]);
    (out, new.chars().count() as isize - old_len as isize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> String {
        s.replace('[', "\u{1c}").replace(']', "\u{1f}")
    }

    #[test]
    fn start_codes_read_with_the_original_defaults() {
        assert_eq!(Table::parse("htbs"), Table::default());
        let m = Table::parse("htbs;m");
        assert_eq!((m.kind, m.header_rows, m.header_cols, m.type_index()), ('m', 0, 0, 1));
        let c = Table::parse("htbs;r:2:0:b:y:c");
        assert_eq!((c.header_rows, c.header_cols, c.swap_index(), c.type_index()), (2, 0, 1, 3));
    }

    #[test]
    fn a_new_table_has_its_rows_and_cells() {
        let (t, caret) = Table::default().new_table();
        assert_eq!(t, run("[htbs;r:1:1:b:n:a][>][>][>][<][>][>][>][<][>][>][>][htbe]"));
        assert_eq!(caret, 18);
        let mut m = Table { rows: 1, cols: 2, ..Table::default() };
        m.set_type(1);
        assert_eq!(m.new_table().0, run("[htbs;m][>][>][>][htbe]"));
        assert_eq!(digits("-3"), 3);
        assert_eq!(digits(""), 0);
    }

    #[test]
    fn tables_are_found_and_refused_as_the_original() {
        let r = run("x[htbs;r:1:1:b:n:a]a[>]b[<]c[>]d[htbe]y");
        assert!(selection_has_table_code(&r, 0, 19));
        // A code counts only when its end mark is in the selection, as the original walks it.
        assert!(!selection_has_table_code(&r, 0, 3));
        assert!(!selection_has_table_code(&r, 19, 20));
        let (open, after, rows, cols) = table_at(&r, 20).unwrap();
        assert_eq!((open, after, rows, cols), (1, 19, 2, 2));
        assert!(table_at(&r, 0).is_none());
        let (new, delta) = set_start_code(&r, open, after, &Table { kind: 'u', ..Table::default() });
        assert_eq!(new, run("x[htbs;u]a[>]b[<]c[>]d[htbe]y"));
        assert_eq!(delta, 6 - 16);
    }
}
