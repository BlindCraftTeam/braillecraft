//! Layout, Style Level + and Style Level - (commands 1403 and 1404), as the original does them (spec 34): the
//! hierarchy level code [hlN] inside the paragraph style around the caret is raised or lowered by one; with a
//! selection, every run of [hl..] and [<] codes inside it is renumbered by the same step. Inside a table the keys
//! move the caret to the next or previous cell instead. Places are in running text (codes as 0x1C name 0x1F).

const OPEN: char = '\x1c';
const CLOSE: char = '\x1f';

/// What the command did: the running text after it, where the caret and the selection anchor stand, whether it
/// beeped, and whether the text changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub running: String,
    pub caret: usize,
    pub anchor: usize,
    pub beep: bool,
    pub changed: bool,
}

/// The name of the code at `p` (a 0x1C) up to its 0x1F, with the place after the 0x1F; None when it is not closed.
fn code_at(r: &[char], p: usize) -> Option<(String, usize)> {
    if r.get(p) != Some(&OPEN) {
        return None;
    }
    let end = r[p + 1..].iter().position(|&c| c == CLOSE)? + p + 1;
    Some((r[p + 1..end].iter().collect(), end + 1))
}

fn matches_at(r: &[char], p: usize, pat: &str, fold: bool) -> bool {
    let pat: Vec<char> = pat.chars().collect();
    p + pat.len() <= r.len()
        && pat.iter().zip(&r[p..]).all(|(a, b)| if fold { a.eq_ignore_ascii_case(b) } else { a == b })
}

/// The level number of an [hl..] code name as the search at the caret reads it (spec 34 3.1): all its digits
/// together; none, or 0, gives None.
fn all_digits(name: &str) -> Option<u32> {
    let n: u32 = name.chars().filter_map(|c| c.to_digit(10)).fold(0u32, |a, d| a.saturating_mul(10).saturating_add(d));
    (name.chars().any(|c| c.is_ascii_digit()) && n > 0).then_some(n)
}

/// The level number of an [hl..] code name as the selection pass reads it (spec 34 3.4): the digits directly after
/// "hl"; none, or 0, gives None.
fn leading_digits(name: &str) -> Option<u32> {
    let digits: String = name[2..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok().filter(|n| *n > 0)
}

/// A place `m` after the text `s..e` was replaced by `new_len` characters; a place at `s` itself stays before the
/// new text.
fn shifted(m: usize, s: usize, e: usize, new_len: usize) -> usize {
    if m == s {
        m
    } else if m >= e {
        m + new_len - (e - s)
    } else if m > s {
        s + new_len
    } else {
        m
    }
}

/// The most levels the [hi] in force at `caret` allows, as Style Level reads it (spec 34 3.2, 8.1): the 7th
/// parameter of the last [hi] before the caret, read as an unsigned byte; -1 or not given reads as 255 (no limit),
/// so 0 lets no level through and 256 wraps to 0, as in the original.
pub fn max_level(running: &str, caret: usize) -> u32 {
    let r: Vec<char> = running.chars().collect();
    let mut m: i64 = -1;
    let mut p = 0;
    while p < caret.min(r.len()) {
        if let Some((name, after)) = code_at(&r, p) {
            if name.starts_with("hi") && name.len() == 2 || name.starts_with("hi") && !name.as_bytes()[2].is_ascii_alphabetic() {
                let params: Vec<&str> = name[2..].split(':').collect();
                m = params.get(6).map_or(-1, |v| v.trim().parse::<i64>().unwrap_or(-1));
            }
            p = after;
        } else {
            p += 1;
        }
    }
    if m == -1 {
        255
    } else {
        (m & 0xff) as u32
    }
}

/// The paragraph style open at `page_start` and the place of its start code (the original's page fields, spec 34
/// 8.2): the last [es~NAME.] before the page start with no [ee~NAME.] after it.
pub fn page_style(running: &str, page_start: usize) -> Option<(String, usize)> {
    let r: Vec<char> = running.chars().collect();
    let mut open: Option<(String, usize)> = None;
    let mut p = 0;
    while p < page_start.min(r.len()) {
        if let Some((name, after)) = code_at(&r, p) {
            if let Some(n) = name.strip_prefix("es~").filter(|n| n.ends_with('.')) {
                open = Some((n.to_string(), p));
            } else if let Some(n) = name.strip_prefix("ee~").filter(|n| n.ends_with('.')) {
                if open.as_ref().is_some_and(|(o, _)| o == n) {
                    open = None;
                }
            }
            p = after;
        } else {
            p += 1;
        }
    }
    open
}

/// Style Level with `step` +1 or -1 (spec 34 sections 2 to 4). `has_selection` and `anchor` are the selection's;
/// `page_start` is the first place of the caret's formatted page and `page_style` the paragraph style open at it
/// (page_style); `max` is max_level. None when nothing changed and nothing beeped (never, in fact: the original
/// beeps on every failure), so the outcome always says what to do.
pub fn change(running: &str, anchor: usize, caret: usize, has_selection: bool, step: i32, page_start: usize, page_style: Option<(String, usize)>, max: u32) -> Outcome {
    let mut r: Vec<char> = running.chars().collect();
    let len = r.len();
    let caret = caret.min(len);
    let anchor = if has_selection { anchor.min(len) } else { caret };
    let (search_start, lower) = if has_selection { (caret.min(anchor), 0) } else { (caret, page_start.min(caret)) };
    let s_mark = caret.min(anchor);
    let e_mark = has_selection.then_some(caret.max(anchor));
    let fail = |beep: bool| Outcome { running: running.to_string(), caret, anchor, beep, changed: false };

    // 2.2: backward for the paragraph style start, then forward for its end before the caret.
    let mut p_after: Option<usize> = None;
    let mut from = search_start;
    while from > lower {
        let Some(hit) = (lower..from).rev().find(|&q| matches_at(&r, q, "\x1cES~", true)) else { break };
        if let Some((name, after)) = code_at(&r, hit).filter(|(n, _)| n.starts_with("es~") && n.ends_with('.')) {
            let end_pat = format!("\x1cEE~{}", &name[3..]);
            if (after..caret).any(|q| matches_at(&r, q, &end_pat, true)) {
                return fail(true);
            }
            p_after = Some(after);
            break;
        }
        from = hit;
    }
    let p = match p_after {
        Some(p) => p,
        None => match page_style {
            Some((_, offset)) if !has_selection => match code_at(&r, offset) {
                Some((_, after)) => after,
                None => return fail(true),
            },
            _ => return fail(true),
        },
    };

    // 3.1: the level in force at the selection start and the code to change.
    let (mut level, mut found): (u32, Option<usize>) = (1, None);
    let mut q = p;
    while q < s_mark {
        if matches_at(&r, q, "\x1cHL", true) {
            if let Some((name, after)) = code_at(&r, q) {
                found = Some(q);
                if let Some(n) = all_digits(&name) {
                    level = n;
                }
                q = after;
                continue;
            }
        }
        q += 1;
    }
    let mut q = found.unwrap_or(p);
    while q < s_mark {
        if matches_at(&r, q, "\x1c<\x1f", false) {
            found = Some(q);
            q += 3;
        } else {
            q += 1;
        }
    }
    let valid = |n: i64| n >= 1 && n <= i64::from(max);

    // 3.3: the edit at the caret.
    let (mut c_mark, mut a_mark, mut e_mark) = (caret, anchor, e_mark);
    let mut beep = false;
    let mut changed = false;
    // The selection pass starts after the caret's own edit (an insertion at the selection start included).
    let mut pass_from = s_mark;
    let n = i64::from(level) + i64::from(step);
    if valid(n) {
        let new: Vec<char> = format!("\x1chl{n}\x1f").chars().collect();
        let (s, e) = match found {
            Some(t) => (t, code_at(&r, t).map_or(t, |(_, after)| after)),
            None => (p, p),
        };
        r.splice(s..e, new.iter().copied());
        c_mark = shifted(c_mark, s, e, new.len());
        a_mark = shifted(a_mark, s, e, new.len());
        e_mark = e_mark.map(|m| shifted(m, s, e, new.len()));
        pass_from = shifted(s_mark, s, e, new.len()).max(s + new.len());
        changed = true;
    } else {
        beep = true;
    }

    // 3.4: every run of [hl..] and [<] codes inside the selection.
    if let Some(mut e_pos) = e_mark {
        let mut level = level;
        let mut pos = pass_from.min(e_pos);
        loop {
            let Some(start) = (pos..e_pos).find(|&q| r[q] == OPEN) else { break };
            let (mut run_end, mut accepted) = (start, 0);
            let mut q = start;
            while q < e_pos {
                let Some((name, after)) = code_at(&r, q) else { break };
                if name.starts_with("hl") {
                    if let Some(n) = leading_digits(&name) {
                        level = n;
                    }
                } else if !name.starts_with('<') {
                    q = after;
                    break;
                }
                accepted += 1;
                run_end = after;
                q = after;
            }
            if accepted > 0 {
                let n = i64::from(level) + i64::from(step);
                let new: Vec<char> = if valid(n) {
                    format!("\x1chl{n}\x1f").chars().collect()
                } else {
                    beep = true;
                    "\x1c<\x1f".chars().collect()
                };
                r.splice(start..run_end, new.iter().copied());
                c_mark = shifted(c_mark, start, run_end, new.len());
                a_mark = shifted(a_mark, start, run_end, new.len());
                e_pos = shifted(e_pos, start, run_end, new.len());
                changed = true;
                pos = start + new.len();
            } else {
                pos = q.max(start + 1);
            }
        }
    }
    Outcome { running: r.into_iter().collect(), caret: c_mark, anchor: a_mark, beep, changed }
}

/// Inside a table, Style Level + and - move the caret to the next or previous cell of its row (spec 34 1, table
/// case): next is the cell after, with no move past the row's last cell; previous first goes to the start of the
/// current cell, then to the cell before, with no move before the row's first. None means a beep and no move.
/// INFERENCE: the original moves by the table's layout; until BrailleCraft lays tables out, the cells are the
/// table's [>] codes and the rows its [<] codes (spec 33).
pub fn cell_move(running: &str, caret: usize, forward: bool) -> Option<usize> {
    let (start, _, _, _) = crate::table_cmd::table_at(running, caret)?;
    let r: Vec<char> = running.chars().collect();
    let end = r.len();
    // The places cells start at in the caret's row (after the table start code or a row code, and after each cell
    // code), up to the caret; the first code at or after the caret says whether a next cell follows in the row.
    let mut cells: Vec<usize> = Vec::new();
    let mut next: Option<usize> = None;
    let mut p = start;
    let mut depth = 0usize;
    while p < end.min(r.len()) {
        match code_at(&r, p) {
            Some((name, after)) => {
                if p >= caret {
                    if depth == 0 && name == ">" {
                        next = Some(after);
                    }
                    break;
                }
                if p == start || (depth == 0 && name == "<") {
                    cells = vec![after];
                } else if depth == 0 && name == ">" {
                    cells.push(after);
                } else if name.starts_with("htbs") {
                    depth += 1;
                } else if name == "htbe" {
                    depth = depth.saturating_sub(1);
                }
                p = after;
            }
            None => p += 1,
        }
    }
    let current = cells.iter().rposition(|&c| c <= caret)?;
    if forward {
        next
    } else if caret != cells[current] {
        Some(cells[current])
    } else {
        current.checked_sub(1).map(|k| cells[k])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, caret: usize, step: i32) -> Outcome {
        change(text, caret, caret, false, step, 0, None, max_level(text, caret))
    }

    #[test]
    fn the_level_code_at_the_caret_is_raised_and_lowered() {
        let t = "\x1ces~h.\x1fWords\x1cee~h.\x1f";
        let o = run(t, 10, 1);
        assert_eq!(o.running, "\x1ces~h.\x1f\x1chl2\x1fWords\x1cee~h.\x1f");
        assert_eq!((o.caret, o.beep, o.changed), (15, false, true));
        // Level - from the default level 1 gives 0: a beep and no change.
        let o = run(t, 10, -1);
        assert_eq!((o.running.as_str(), o.beep, o.changed), (t, true, false));
        let t2 = "\x1ces~h.\x1f\x1chl2\x1fWords";
        assert_eq!(run(t2, 14, 1).running, "\x1ces~h.\x1f\x1chl3\x1fWords");
        assert_eq!(run(t2, 14, -1).running, "\x1ces~h.\x1f\x1chl1\x1fWords");
        // A [<] after the last hl code is the code replaced.
        let t3 = "\x1ces~h.\x1f\x1chl2\x1fa\x1c<\x1fb";
        assert_eq!(run(t3, 17, 1).running, "\x1ces~h.\x1f\x1chl2\x1fa\x1chl3\x1fb");
    }

    #[test]
    fn a_closed_style_or_none_beeps() {
        // The style ended before the caret.
        let t = "\x1ces~h.\x1fa\x1cee~h.\x1fb";
        let o = run(t, 16, 1);
        assert_eq!((o.beep, o.changed), (true, false));
        // A character style is not a paragraph style.
        let o = run("\x1ces~b\x1fWords", 8, 1);
        assert_eq!((o.beep, o.changed), (true, false));
        // The style open at the page start is used when the page holds no start.
        let t = "\x1ces~h.\x1fone\x1cpg\x1ftwo";
        let page = page_style(t, 14);
        assert_eq!(page, Some(("h.".into(), 0)));
        let o = change(t, 16, 16, false, 1, 14, page, 255);
        assert_eq!(o.running, "\x1ces~h.\x1f\x1chl2\x1fone\x1cpg\x1ftwo");
    }

    #[test]
    fn the_maximum_comes_from_the_hi_code() {
        let t = "\x1chi1:1:0:0:0:0:2\x1f\x1ces~h.\x1f\x1chl2\x1fx";
        assert_eq!(max_level(t, 30), 2);
        let o = run(t, 30, 1);
        assert_eq!((o.beep, o.changed), (true, false));
        assert_eq!(max_level("\x1chi\x1f", 4), 255);
        assert_eq!(max_level("\x1chi1:1:0:0:0:0:256\x1f", 20), 0);
    }

    #[test]
    fn a_selection_renumbers_every_code_run() {
        let t = "\x1ces~h.\x1f\x1chl1\x1fa\x1chl2\x1f\x1c<\x1fb\x1chl3\x1fc";
        let o = change(t, 7, t.chars().count(), true, 1, 0, None, 3);
        // The level at the selection start (1, no code before it) gives a new [hl2] at the caret's edit; then each
        // run inside moves up; the last leaves the range and becomes [<] with a beep.
        assert_eq!(o.running, "\x1ces~h.\x1f\x1chl2\x1f\x1chl2\x1fa\x1chl3\x1fb\x1c<\x1fc");
        assert!(o.beep && o.changed);
        assert_eq!((o.anchor, o.caret), (7, o.running.chars().count()));
    }

    #[test]
    fn cells_move_within_a_row() {
        let t = "\x1chtbs\x1fa\x1c>\x1fb\x1c<\x1fc\x1c>\x1fd\x1chtbe\x1f";
        // After the table start: 6; cell b starts after [>] at 10; row two at 14; d at 18.
        assert_eq!(cell_move(t, 7, true), Some(10));
        assert_eq!(cell_move(t, 11, true), None);
        assert_eq!(cell_move(t, 11, false), Some(10));
        assert_eq!(cell_move(t, 10, false), Some(6));
        assert_eq!(cell_move(t, 6, false), None);
        assert_eq!(cell_move(t, 15, true), Some(18));
        assert_eq!(cell_move(t, 14, false), None);
    }
}
