//! Rule scripts: small state machines that rewrite a byte stream, as the reference program runs them for coded
//! output, imports and exports (spec 20).
//!
//! A script is lines of rules, `match~replacement~state~next state~mark~`. A run walks the input with a current state
//! (1 at the start); at each byte the first rule, in the order below, whose state is 0 or the current state and whose
//! match stands there writes its replacement and moves the state on; a byte no rule takes is copied when the state is
//! above 0 and dropped otherwise.
//!
//! - "|NNN" is the byte NNN: |256 the start of the input, |257 the end, |258 a line end (read from CR, LF, CR LF or
//!   LF CR; written as CR LF). "|" before any other character is that character; "| " starts a comment.
//! - An empty state is 0, every state. A mark "(" keeps the current state, ")" goes back to the one kept last.
//! - A line with no match names a file of further rules, loaded when the run first enters its state (1 to 10), with
//!   that state and next state as the defaults of its rules.
//! - Rules for one first byte are tried in this order: rules for a particular state, larger states first, before
//!   rules for every state; longer matches first; among equal lengths the rule given later first.

use std::path::{Path, PathBuf};

/// A script could not be read; the number is the reference program's error number for the same fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    pub code: u32,
    pub detail: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rule script error {}: {}", self.code, self.detail)
    }
}

fn error(code: u32, detail: &str) -> ScriptError {
    ScriptError { code, detail: detail.to_string() }
}

#[derive(Debug, Clone)]
struct Rule {
    matches: Vec<u8>,
    replacement: Vec<u8>,
    state: i32,
    next: i32,
    mark: u8,
    /// A start-of-input or end-of-input rule.
    special: bool,
}

/// What one line of a script holds.
enum Line {
    Nothing,
    Rule(Rule),
    /// A file of further rules: its name, and the state and next state its rules take when they give none.
    File(String, i32, i32),
}

struct Parsed {
    line: Line,
    line_in: bool,
    line_out: bool,
}

/// One script line (spec 20, "Script file").
fn parse_line(line: &[u8], default_state: i32, default_next: i32) -> Result<Parsed, ScriptError> {
    let nothing = Parsed { line: Line::Nothing, line_in: false, line_out: false };
    if line.is_empty() || line == b" " {
        return Ok(nothing);
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut field = 1;
    let (mut match_len, mut repl_len): (i64, i64) = (-1, -1);
    let (mut state, mut next) = (default_state, default_next);
    let (mut mark, mut field5) = (b' ', b' ');
    let (mut sign, mut num): (i32, i32) = (1, 0);
    let (mut special, mut line_in, mut line_out) = (false, false, false);
    let mut i = 0;
    while i < line.len() {
        let c = line[i];
        if c == b'|' {
            let Some(&c2) = line.get(i + 1) else { break };
            i += 2;
            if c2.is_ascii_digit() {
                let digits = [Some(c2), line.get(i).copied(), line.get(i + 1).copied()];
                i += 2;
                if digits.iter().any(|d| !d.is_some_and(|d| d.is_ascii_digit())) {
                    return Err(error(200, "a | escape needs three digits"));
                }
                let v = digits.iter().fold(0u32, |n, d| n * 10 + u32::from(d.unwrap() - b'0'));
                match v {
                    256 | 257 => {
                        if !buf.is_empty() {
                            return Err(error(0x104, "|256 and |257 may only begin a match"));
                        }
                        special = true;
                        buf.push((v - 256) as u8);
                    }
                    258 => {
                        buf.push(b'\n');
                        match field {
                            1 => line_in = true,
                            2 => line_out = true,
                            _ => {}
                        }
                    }
                    v if v > 258 => return Err(error(0xD2, "a | escape above 258")),
                    v => buf.push(v as u8),
                }
                continue;
            }
            if c2 == b' ' {
                if buf.is_empty() {
                    return Ok(nothing);
                }
                break;
            }
            buf.push(c2);
            continue;
        }
        i += 1;
        if c == b'~' {
            match field {
                1 => match_len = buf.len() as i64,
                2 => repl_len = buf.len() as i64 - match_len,
                3 => state = sign * num,
                4 => next = sign * num,
                5 => mark = field5,
                _ => return Err(error(0xDC, "more than five fields")),
            }
            field += 1;
            field5 = b' ';
            sign = 1;
            num = 0;
        } else if field < 3 {
            buf.push(c);
        } else if field < 5 {
            match c {
                b' ' => {}
                b'-' => sign = -sign,
                b'0'..=b'9' => num = num.saturating_mul(10).saturating_add(i32::from(c - b'0')),
                _ => return Err(error(0xF0, "a state must be a number")),
            }
        } else if c != b' ' {
            field5 = c;
        }
    }
    // The line must not stop inside a number or after a mark with no closing "~".
    if num != 0 || field5 != b' ' {
        return Err(error(0xFA, "an unfinished field at the end of the line"));
    }
    let parsed = if match_len <= 0 {
        if repl_len < 1 {
            return Err(error(100, "a line with no match and no file name"));
        }
        let name: String = buf[..repl_len as usize].iter().map(|&b| b as char).collect();
        Line::File(name, state, next)
    } else {
        let m = match_len as usize;
        let r = repl_len.max(0) as usize;
        Line::Rule(Rule {
            matches: buf[..m].to_vec(),
            replacement: buf[m..(m + r).min(buf.len())].to_vec(),
            state,
            next,
            mark,
            special,
        })
    };
    Ok(Parsed { line: parsed, line_in, line_out })
}

/// The lines of a file as the reference reads them: CR, LF, CR LF and LF CR each end a line.
fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < data.len() {
        let c = data[i];
        if c == b'\n' || c == b'\r' {
            lines.push(&data[start..i]);
            i += 1;
            if i < data.len() && (data[i] == b'\n' || data[i] == b'\r') && data[i] != c {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    if start < data.len() {
        lines.push(&data[start..]);
    }
    lines
}

/// A loaded rule script.
#[derive(Debug, Clone)]
pub struct RuleScript {
    /// Rules by first byte; 256 holds the start-of-input rules, 257 the end-of-input rules.
    lists: Vec<Vec<Rule>>,
    /// Files of further rules waiting for their state (1 to 10).
    waiting: [Option<(String, i32, i32)>; 10],
    line_in: bool,
    line_out: bool,
    folder: Option<PathBuf>,
}

impl RuleScript {
    /// A script from its text; files it names are looked for in `folder`.
    pub fn from_bytes(data: &[u8], folder: Option<&Path>) -> Result<RuleScript, ScriptError> {
        let mut s = RuleScript {
            lists: vec![Vec::new(); 258],
            waiting: Default::default(),
            line_in: false,
            line_out: false,
            folder: folder.map(Path::to_path_buf),
        };
        s.load(data, 0, 0)?;
        Ok(s)
    }

    /// A script from a file.
    pub fn open(path: &Path) -> Result<RuleScript, ScriptError> {
        let data = std::fs::read(path).map_err(|e| error(0x8C, &format!("{}: {e}", path.display())))?;
        RuleScript::from_bytes(&data, path.parent())
    }

    fn load(&mut self, data: &[u8], default_state: i32, default_next: i32) -> Result<(), ScriptError> {
        for (n, line) in split_lines(data).into_iter().enumerate() {
            let line = if n == 0 { line.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(line) } else { line };
            let parsed = parse_line(line, default_state, default_next)?;
            self.line_in |= parsed.line_in;
            self.line_out |= parsed.line_out;
            match parsed.line {
                Line::Nothing => {}
                Line::File(name, state, next) => {
                    let slot = usize::try_from(state - 1).ok().filter(|&s| s < 10);
                    match slot {
                        Some(s) if self.waiting[s].is_none() && name.len() < 50 => {
                            self.waiting[s] = Some((name, state, next));
                        }
                        _ => self.load_file(&name, state, next)?,
                    }
                }
                Line::Rule(rule) => self.insert(rule),
            }
        }
        Ok(())
    }

    fn load_file(&mut self, name: &str, state: i32, next: i32) -> Result<(), ScriptError> {
        let path = match &self.folder {
            Some(f) => f.join(name),
            None => PathBuf::from(name),
        };
        let data = std::fs::read(&path).map_err(|e| error(0x8C, &format!("{}: {e}", path.display())))?;
        self.load(&data, state, next)
    }

    /// Puts a rule in its list, in the reference's order (spec 20, "Rule order").
    fn insert(&mut self, rule: Rule) {
        let first = rule.matches[0] as usize;
        let key = if rule.special && first < 2 { 256 + first } else { first };
        let list = &mut self.lists[key];
        let pos = list
            .iter()
            .position(|r| {
                let pass = (r.state != 0 || rule.state >= 0)
                    && ((r.state < 0 && rule.state == 0)
                        || rule.state < r.state
                        || (rule.state == r.state && rule.matches.len() < r.matches.len()));
                !pass
            })
            .unwrap_or(list.len());
        list.insert(pos, rule);
    }

    /// Loads the file waiting for `state`, the first time the run enters it.
    fn enter(&mut self, state: i32) -> Result<(), ScriptError> {
        if (1..=10).contains(&state) {
            if let Some((name, s, n)) = self.waiting[(state - 1) as usize].take() {
                self.load_file(&name, s, n)?;
            }
        }
        Ok(())
    }

    /// Runs the script over `input` (spec 20, "Run").
    pub fn run(&mut self, input: &[u8]) -> Result<Vec<u8>, ScriptError> {
        const RING: usize = 4096;
        let mut reader = Reader { data: input, at: 0, held: None };
        let mut out = Vec::with_capacity(input.len() + input.len() / 4);
        let mut ring = vec![-1i32; RING];
        for slot in ring.iter_mut() {
            *slot = reader.read(self.line_in);
            if *slot == -1 {
                break;
            }
        }
        let (mut pos, mut state) = (0usize, 1i32);
        let mut kept: Vec<i32> = Vec::new();
        self.enter(state)?;
        if let Some(start) = self.lists[256].first().cloned() {
            write(&mut out, &start.replacement, self.line_out);
            self.after(&start, &mut state, &mut kept)?;
        }
        while ring[pos] >= 0 {
            let c = ring[pos] as usize;
            let hit = self.lists[c]
                .iter()
                .find(|r| {
                    !r.special
                        && (r.state == 0 || r.state == state)
                        && r.matches[1..].iter().enumerate().all(|(k, &b)| ring[(pos + k + 1) % RING] == i32::from(b))
                })
                .cloned();
            match hit {
                None => {
                    if state > 0 {
                        write(&mut out, &[c as u8], self.line_out);
                    }
                    ring[pos] = reader.read(self.line_in);
                    pos = (pos + 1) % RING;
                }
                Some(rule) => {
                    write(&mut out, &rule.replacement, self.line_out);
                    for _ in 0..rule.matches.len() {
                        ring[pos] = reader.read(self.line_in);
                        pos = (pos + 1) % RING;
                    }
                    self.after(&rule, &mut state, &mut kept)?;
                }
            }
        }
        if let Some(end) = self.lists[257].iter().find(|r| r.special && (r.state == 0 || r.state == state)) {
            write(&mut out, &end.replacement, self.line_out);
        }
        Ok(out)
    }

    /// A rule's mark and next state, after it has written its replacement.
    fn after(&mut self, rule: &Rule, state: &mut i32, kept: &mut Vec<i32>) -> Result<(), ScriptError> {
        match rule.mark {
            b'(' if kept.len() < 20 => kept.push(*state),
            b')' => {
                if let Some(s) = kept.pop() {
                    *state = s;
                }
            }
            _ => {}
        }
        if rule.next != 0 && rule.next != *state {
            *state = rule.next;
            self.enter(*state)?;
        }
        Ok(())
    }
}

/// Bytes to the output; with line ends written natively, a line end as CR LF.
fn write(out: &mut Vec<u8>, bytes: &[u8], line_out: bool) {
    for &b in bytes {
        if line_out && b == b'\n' {
            out.extend_from_slice(b"\r\n");
        } else {
            out.push(b);
        }
    }
}

/// The input, a byte at a time (-1 at the end); with line ends read natively, CR, LF, CR LF and LF CR as one 10.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    held: Option<i32>,
}

impl Reader<'_> {
    fn byte(&mut self) -> i32 {
        if let Some(b) = self.held.take() {
            return b;
        }
        match self.data.get(self.at) {
            Some(&b) => {
                self.at += 1;
                i32::from(b)
            }
            None => -1,
        }
    }

    fn read(&mut self, line_in: bool) -> i32 {
        let c = self.byte();
        if line_in && (c == 10 || c == 13) {
            let d = self.byte();
            if (d != 10 && d != 13) || d == c {
                self.held = Some(d);
            }
            return 10;
        }
        c
    }
}

/// Runs scripts one after the other, each over the output of the one before.
pub fn run_all(scripts: &mut [RuleScript], input: &[u8]) -> Result<Vec<u8>, ScriptError> {
    let mut data = input.to_vec();
    for s in scripts.iter_mut() {
        data = s.run(&data)?;
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(script: &str, input: &[u8]) -> Vec<u8> {
        RuleScript::from_bytes(script.as_bytes(), None).unwrap().run(input).unwrap()
    }

    #[test]
    fn rules_replace_and_unmatched_bytes_are_copied() {
        assert_eq!(run("ab~X~\r\n", b"cabab d"), b"cXX d");
    }

    #[test]
    fn longer_matches_and_state_rules_come_first() {
        let script = "a~1~\r\nab~2~\r\nb~3~1~\r\n";
        assert_eq!(run(script, b"abab b"), b"22 3");
        // Among equal lengths the rule given later is tried first.
        assert_eq!(run("a~1~\r\na~2~\r\n", b"a"), b"2");
    }

    #[test]
    fn states_change_and_bytes_are_dropped_below_state_one() {
        // "<" enters state -1, where unmatched bytes are dropped, ">" goes back to 1.
        let script = "<~~~-1~\r\n>~~-1~1~\r\n";
        assert_eq!(run(script, b"a<hidden>b"), b"ab");
    }

    #[test]
    fn start_end_and_line_end_escapes() {
        let script = "| a comment\r\n|256~[~\r\n|257~]~ | the end\r\n|258~/|258~\r\n||~|$~\r\n";
        assert_eq!(run(script, b"x\r\ny|z"), b"[x/\r\ny$z]");
    }

    #[test]
    fn marks_keep_and_restore_a_state() {
        let script = "(~~~5~(~\r\n)~~5~~)~\r\nq~Q~5~\r\n";
        assert_eq!(run(script, b"q(q)q"), b"qQq");
    }

    fn shipped(name: &str) -> RuleScript {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tables/rule-scripts");
        RuleScript::open(&dir.join(name)).unwrap()
    }

    #[test]
    fn coded_braille_from_the_shipped_scripts() {
        let mut scripts = [shipped("coded-braille.rules"), shipped("coded-wrap.rules")];
        let out = run_all(&mut scripts, b"ab|cd\x1cp\x1f\x1cES~X\x1fHello  there\x1e x").unwrap();
        assert_eq!(out, b"|no-par |spacing |compress ab|$cd|p\r\n|es~X Hello |'there|' x\r\n".to_vec());
    }

    /// Every sample document body through both coded outputs gives what the reference port gives, when a folder of
    /// samples is named in BC_DX_SAMPLES and a folder of the port's outputs in BC_RULES_EXPECTED.
    #[test]
    fn sample_bodies_match_the_reference_port() {
        let (Ok(samples), Ok(expected)) = (std::env::var("BC_DX_SAMPLES"), std::env::var("BC_RULES_EXPECTED")) else {
            return;
        };
        let mut count = 0;
        for entry in std::fs::read_dir(samples).unwrap().flatten() {
            let p = entry.path();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if !lower.ends_with(".dxp") && !lower.ends_with(".dxb") {
                continue;
            }
            let d = std::fs::read(&p).unwrap();
            let length = u32::from_be_bytes([d[28], d[29], d[30], d[31]]) as usize;
            let body = &d[234..234 + length];
            for (kind, first) in [("braille", "coded-braille.rules"), ("text", "coded-text.rules")] {
                let mut scripts = [shipped(first), shipped("coded-wrap.rules")];
                let out = run_all(&mut scripts, body).unwrap();
                let want = std::fs::read(Path::new(&expected).join(format!("{name}.{kind}"))).unwrap();
                if out != want {
                    let k = out.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(out.len().min(want.len()));
                    panic!("{name} {kind}: differs at byte {k} (lengths {} and {})", out.len(), want.len());
                }
                count += 1;
            }
        }
        assert!(count > 0);
    }

    #[test]
    fn faults_are_refused() {
        assert_eq!(RuleScript::from_bytes(b"a|12~b~", None).unwrap_err().code, 200);
        assert_eq!(RuleScript::from_bytes(b"a~b~x~", None).unwrap_err().code, 0xF0);
        assert_eq!(RuleScript::from_bytes(b"a~b~1~2~(~~", None).unwrap_err().code, 0xDC);
        assert_eq!(RuleScript::from_bytes(b"a|256~b~", None).unwrap_err().code, 0x104);
    }
}
