//! The table compiler: BrailleMaker's own translation tables are written as plain text (.bmt) and compiled here
//! into the binary rule table the engine loads (`table.rs`), plus the scanner settings (`ScanTable`).
//!
//! The source layout, one statement per line, `#` to the end of a line is a comment:
//!
//! ```text
//! table "English UEB grade 2, print to braille"   # the title (39 bytes at most are kept)
//! default-char " "        # what a character with no class is read as
//! raw 0                   # 0: bytes below 0x20 in rule output are actions and 0x17 pairs pass as text
//! writer 3                # writer mode: 3 raw, 2 print output with case handling
//! output 3                # output mode
//! tail-extra 0 0 0        # three values the engine keeps in the table but does not use
//! unknown 1               # what to do with a character the table does not know
//! state in-word numeric   # state bits, numbered in order of appearance (at most 128)
//! class letter "abcdefghijklmnopqrstuvwxyz"
//! class space " "
//! class gap space punct   # a class may also be the union of classes named before it
//! set word-end = space | punct "'"   # alternatives split by |; items: class, "literal", any, @set
//! char "a" default "a" then set in-word   # default output (one byte, or one action), and its state change
//! rule "and" -> "&" when !in-word followed-by @word-end then set in-word
//! rule "1" -> "#a" when !numeric then set numeric
//! scan fold-words 1       # scanner settings, see `apply_scan`
//! ```
//!
//! Rule parts: the match (the rule's character and the characters that must follow it), `->` and the output
//! (strings, with \xNN for any byte, and actions in braces: `{copy}`, `{push name}`, `{pop name}`, `{drop-space}`,
//! `{drop}`, `{delete-next}`, `{word}`, `{switch}`, `{style n}`, `{cmd c}`, `{move k n}`, `{byte n}`), `consume n`
//! (characters taken, default the whole match), `when` state tests (`name` set, `!name` clear), `followed-by` one
//! or two classes, one `@set`, or one `@@set` (the set named by a one-byte id), `then` state changes (`set name`,
//! `clear name`, `toggle name`).
//!
//! How a rule is laid out for the engine: each rule is one node in the chain of its first character, tried in order
//! (longer matches first, then the order of the source). A rule of one character keeps its context in the node
//! itself: one class bit, two class bits (the next two characters), or a set by its id. A rule of several
//! characters puts the characters after the first and its context into one set program (literal characters,
//! class tests, nested sets), so the matcher compares no literal text itself and only follows the chain. Rules
//! that share their conditions and state changes share one result record. The end of the input counts as a space
//! when a class holds the space.

use super::scan_table::ScanTable;
use super::EngineError;
use std::collections::HashMap;

/// The format version written; the engine reads 10 to 20.
const VERSION: i16 = 19;
const MAX_STATES: usize = 128;
const MAX_CLASSES: usize = 64;
const MAX_OUTPUT: usize = 63;
const MAX_MATCH: usize = 63;

/// A compiled table: the binary rule table and the scanner settings.
pub struct Compiled {
    pub table: Vec<u8>,
    pub scan: ScanTable,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Item {
    Class(u8),
    Lit(u8),
    Any,
    Set(u16),
}

#[derive(Clone, Debug)]
struct Rule {
    line: usize,
    matched: Vec<u8>,
    out: Vec<u8>,
    consume: usize,
    when: Vec<(usize, bool)>,
    followed: Vec<Item>,
    /// `followed-by @@name`: the set is named by a one-byte id in the rule itself (sets 0 to 255).
    short_set: bool,
    then: Vec<(usize, Change)>,
}

/// How a rule's context is laid out for the matcher.
enum Context {
    /// Nothing follows the rule's character.
    None,
    /// One class bit tested on the next character.
    OneClass(u8),
    /// Two class bits, on the next character and the one after it.
    TwoClasses(u8, u8),
    /// A set by a two-byte id.
    Set(u16),
    /// A set by a one-byte id.
    ShortSet(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Change {
    Set,
    Clear,
    Toggle,
}

#[derive(Default, Clone)]
struct CharDef {
    classes: u64,
    defined: bool,
    default_out: Option<u8>,
    then: Vec<(usize, Change)>,
    /// Node mode: the first node tried for this character (0: none).
    rules: Option<i64>,
}

/// Node mode: one node of the rule trie, as the table gives it.
struct NodeDef {
    line: usize,
    lit: Vec<u8>,
    ctx: Context,
    result: i64,
    consume: usize,
    out: Vec<u8>,
    link: i64,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Str(Vec<u8>),
    Brace(String),
}

fn err(line: usize, msg: impl Into<String>) -> EngineError {
    EngineError::Source(format!("line {line}: {}", msg.into()))
}

/// The bytes of a text in code page 437.
fn bytes_of(line: usize, s: &str) -> Result<Vec<u8>, EngineError> {
    let codes = super::chars::encode_cp437(s)
        .map_err(|c| err(line, format!("the character {c} has no code page 437 form")))?;
    Ok(codes.into_iter().map(|c| c as u8).collect())
}

/// Splits a line into words, quoted strings (escapes \" \\ \n \xNN) and brace groups; drops a comment.
fn tokens(line: usize, text: &str) -> Result<Vec<Tok>, EngineError> {
    let c: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '#' {
            break;
        } else if ch == '"' {
            let mut bytes = Vec::new();
            let mut plain = String::new();
            i += 1;
            loop {
                let Some(&d) = c.get(i) else { return Err(err(line, "a string has no closing quote")) };
                i += 1;
                if d == '"' {
                    break;
                }
                if d != '\\' {
                    plain.push(d);
                    continue;
                }
                let Some(&e) = c.get(i) else { return Err(err(line, "a string ends after \\")) };
                i += 1;
                match e {
                    '"' | '\\' => plain.push(e),
                    'n' => plain.push('\n'),
                    'x' => {
                        let hex: String = c.iter().skip(i).take(2).collect();
                        let v = u8::from_str_radix(&hex, 16).map_err(|_| err(line, format!("\\x{hex} is not a byte")))?;
                        i += 2;
                        bytes.extend(bytes_of(line, &std::mem::take(&mut plain))?);
                        bytes.push(v);
                    }
                    _ => return Err(err(line, format!("unknown escape \\{e}"))),
                }
            }
            bytes.extend(bytes_of(line, &plain)?);
            out.push(Tok::Str(bytes));
        } else if ch == '{' {
            let start = i + 1;
            let end = c[start..].iter().position(|&d| d == '}').ok_or_else(|| err(line, "an action has no closing brace"))?;
            out.push(Tok::Brace(c[start..start + end].iter().collect()));
            i = start + end + 1;
        } else {
            let start = i;
            while i < c.len() && !c[i].is_whitespace() && c[i] != '"' && c[i] != '{' && c[i] != '#' {
                i += 1;
            }
            out.push(Tok::Word(c[start..i].iter().collect()));
        }
    }
    Ok(out)
}

struct Builder {
    title: String,
    default_char: u8,
    raw: i16,
    writer: i16,
    output: i16,
    extra: [i16; 3],
    unknown: i16,
    states: Vec<String>,
    classes: Vec<(String, Vec<u8>)>,
    sets: Vec<Vec<Vec<Item>>>,
    set_names: HashMap<String, u16>,
    chars: Vec<CharDef>,
    rules: Vec<Rule>,
    /// Node mode: the results (conditions and state changes) by number, and the nodes in order.
    results: Vec<(Vec<(usize, bool)>, Vec<(usize, Change)>)>,
    nodes: Vec<NodeDef>,
    /// A record no character maps to, written last: the engine reads a character with no class through the
    /// default character, and when that one has no class either, through the last record.
    unknown_record: Option<CharDef>,
    scan: ScanTable,
}

fn number(line: usize, t: Option<&Tok>) -> Result<i64, EngineError> {
    match t {
        Some(Tok::Word(w)) => w.parse::<i64>().map_err(|_| err(line, format!("{w} is not a number"))),
        _ => Err(err(line, "a number is missing")),
    }
}

fn string(line: usize, t: Option<&Tok>) -> Result<Vec<u8>, EngineError> {
    match t {
        Some(Tok::Str(s)) => Ok(s.clone()),
        _ => Err(err(line, "a quoted string is missing")),
    }
}

/// A one-byte value: a quoted single character or a number.
fn byte_value(line: usize, t: Option<&Tok>) -> Result<i64, EngineError> {
    match t {
        Some(Tok::Str(s)) if s.len() == 1 => Ok(s[0] as i64),
        Some(Tok::Word(_)) => number(line, t),
        _ => Err(err(line, "a single character or a number is wanted")),
    }
}

/// A class bit written by number: `bit:N` (bits 64 and above never match, as in the engine).
fn raw_bit(line: usize, w: &str) -> Result<u8, EngineError> {
    w[4..].parse::<u8>().map_err(|_| err(line, format!("{w} is not a class bit")))
}

/// The scanner settings defaults: text is read as it is written, a line end counts as a space, runs of spaces
/// become one, codes are written between 0x1C and 0x1F, words are given in small letters with capital signs (byte
/// 1 for a capital, 2 for the end of a capital passage), and more than two capital words in a row are a passage.
pub fn default_scan() -> ScanTable {
    ScanTable {
        binmod: 0,
        trailing_blanks: 1,
        flag: 0x1c,
        fullstop: '.' as i64,
        newline: 1,
        ff_text: Vec::new(),
        par_code: vec!['p' as i64],
        no_par: 0,
        rejoin: 0,
        spaces: 0,
        tabs: 1,
        ninmod: 0,
        fold_controls: 1,
        fold_words: 1,
        cessation: 1,
        map_braces: 0,
        fonts: Vec::new(),
        spare: 0,
        cpe: 0,
        nxsls: 0,
        out_format: 0x1c,
        out_format2: 0x1b,
        out_cap: 1,
        out_capterm: 2,
        icb: 0,
        modes: Vec::new(),
        triple_caps: 2,
        dbl_adj: 0,
        cpe_fts: 0,
        cpe_l: 0,
        dbl_adj_after: 0,
        fold_lo: 0x61,
        fold_hi: 0x7a,
    }
}

impl Builder {
    fn state(&self, line: usize, name: &str) -> Result<usize, EngineError> {
        self.states.iter().position(|s| s == name).ok_or_else(|| err(line, format!("unknown state {name}")))
    }

    fn class(&self, line: usize, name: &str) -> Result<usize, EngineError> {
        self.classes.iter().position(|c| c.0 == name).ok_or_else(|| err(line, format!("unknown class {name}")))
    }

    fn set(&self, line: usize, name: &str) -> Result<u16, EngineError> {
        self.set_names.get(name).copied().ok_or_else(|| err(line, format!("unknown set @{name}")))
    }

    fn changes(&self, line: usize, toks: &[Tok]) -> Result<Vec<(usize, Change)>, EngineError> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < toks.len() {
            let Tok::Word(op) = &toks[i] else { return Err(err(line, "a state change is not set, clear or toggle")) };
            let change = match op.as_str() {
                "set" => Change::Set,
                "clear" => Change::Clear,
                "toggle" => Change::Toggle,
                _ => return Err(err(line, format!("{op} is not set, clear or toggle"))),
            };
            let Some(Tok::Word(name)) = toks.get(i + 1) else { return Err(err(line, format!("{op} needs a state name"))) };
            out.push((self.state(line, name)?, change));
            i += 2;
        }
        Ok(out)
    }

    /// The items of one set alternative.
    fn items(&self, line: usize, toks: &[Tok]) -> Result<Vec<Item>, EngineError> {
        let mut out = Vec::new();
        for t in toks {
            match t {
                Tok::Str(s) => out.extend(s.iter().map(|&b| Item::Lit(b))),
                Tok::Word(w) if w == "any" => out.push(Item::Any),
                Tok::Word(w) if w.starts_with("bit:") => out.push(Item::Class(raw_bit(line, w)?)),
                Tok::Word(w) if w.starts_with('@') => out.push(Item::Set(self.set(line, &w[1..])?)),
                Tok::Word(w) => out.push(Item::Class(self.class(line, w)? as u8)),
                Tok::Brace(_) => return Err(err(line, "an action cannot stand in a set")),
            }
        }
        if out.is_empty() {
            return Err(err(line, "a set alternative is empty"));
        }
        Ok(out)
    }

    fn action(&self, line: usize, text: &str) -> Result<Vec<u8>, EngineError> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let arg = |n: usize| -> Result<&str, EngineError> {
            parts.get(n).copied().ok_or_else(|| err(line, format!("{{{text}}} needs a value")))
        };
        let num = |s: &str| -> Result<u8, EngineError> { s.parse::<u8>().map_err(|_| err(line, format!("{s} is not a number"))) };
        // A state bit as the engine counts it for push and pop: 1 for the first state.
        let bit = |s: &str| -> Result<u8, EngineError> {
            match s.parse::<u8>() {
                Ok(n) => Ok(n),
                Err(_) => Ok(self.state(line, s)? as u8 + 1),
            }
        };
        Ok(match parts.first().copied().unwrap_or("") {
            "copy" => vec![0x01],
            "push" => vec![0x0c, bit(arg(1)?)?],
            "pop" => vec![0x0d, bit(arg(1)?)?],
            "word" => vec![0x14],
            "switch" => vec![0x15],
            "move" => vec![0x16, num(arg(1)?)?, num(arg(2)?)?],
            "drop-space" => vec![0x18],
            "drop" => vec![0x19],
            "delete-next" => vec![0x1a],
            "style" => vec![0x1b, num(arg(1)?)?],
            "byte" => vec![num(arg(1)?)?],
            "cmd" => {
                let c = arg(1)?;
                let b = bytes_of(line, c)?;
                if b.len() != 1 {
                    return Err(err(line, "{cmd} takes one character"));
                }
                vec![0x10, b[0]]
            }
            other => return Err(err(line, format!("unknown action {{{other}}}"))),
        })
    }

    fn apply_scan(&mut self, line: usize, name: &str, rest: &[Tok]) -> Result<(), EngineError> {
        let s = &mut self.scan;
        let v = rest.first();
        match name {
            "trailing-blanks" => s.trailing_blanks = number(line, v)?,
            "flag" => s.flag = byte_value(line, v)?,
            "full-stop" => s.fullstop = byte_value(line, v)?,
            "newline" => s.newline = number(line, v)?,
            "form-feed" => s.ff_text = string(line, v)?.into_iter().map(i64::from).collect(),
            "paragraph" => s.par_code = string(line, v)?.into_iter().map(i64::from).collect(),
            "new-paragraphs" => s.no_par = number(line, v)?,
            "rejoin-hyphens" => s.rejoin = number(line, v)?,
            "spaces" => s.spaces = number(line, v)?,
            "tab-width" => s.tabs = number(line, v)?,
            "fold-codes" => s.fold_controls = number(line, v)?,
            "fold-words" => s.fold_words = number(line, v)?,
            "capital-cessation" => s.cessation = number(line, v)?,
            "map-braces" => {
                s.map_braces = number(line, v)?;
                (s.fold_lo, s.fold_hi) = if s.map_braces == 1 { (0x60, 0x7e) } else { (0x61, 0x7a) };
            }
            "passage-words" => s.triple_caps = number(line, v)?,
            "input-mode" => s.binmod = number(line, v)?,
            "ninmod" => s.ninmod = number(line, v)?,
            "spare" => s.spare = number(line, v)?,
            "nxsls" => s.nxsls = number(line, v)?,
            "address-switching" => s.icb = number(line, v)?,
            "passage-ends" => {
                s.cpe = number(line, v)?;
                (s.cpe_fts, s.cpe_l) = match s.cpe {
                    1 => (1, 0),
                    2 => (0, 1),
                    3 => (1, 1),
                    _ => (0, 0),
                };
            }
            "doubled-adjectives" => {
                s.dbl_adj = number(line, v)?;
                s.dbl_adj_after = if s.dbl_adj == 0 { 0 } else { s.dbl_adj + 1 };
            }
            "font" => {
                let w = string(line, v)?;
                s.fonts.push(super::chars::latin1(&w).split_whitespace().map(String::from).collect());
            }
            "capital-sign" => s.out_cap = byte_value(line, v)?,
            "capital-end" => s.out_capterm = byte_value(line, v)?,
            "format-word" => s.out_format = byte_value(line, v)?,
            "format-word2" => s.out_format2 = byte_value(line, v)?,
            "mode-words" => {
                let mut words = Vec::new();
                for t in rest {
                    let Tok::Str(w) = t else { return Err(err(line, "mode words are quoted strings")) };
                    words.push(super::chars::latin1(w));
                }
                if words.len() != 11 {
                    return Err(err(line, "mode-words takes eleven words"));
                }
                s.modes = words;
            }
            _ => return Err(err(line, format!("unknown scanner setting {name}"))),
        }
        Ok(())
    }

    fn statement(&mut self, line: usize, toks: Vec<Tok>) -> Result<(), EngineError> {
        let Some(Tok::Word(head)) = toks.first() else { return Err(err(line, "a statement starts with a word")) };
        let rest = &toks[1..];
        match head.as_str() {
            "table" => self.title = super::chars::latin1(&string(line, rest.first())?),
            "default-char" => self.default_char = byte_value(line, rest.first())? as u8,
            "raw" => self.raw = number(line, rest.first())? as i16,
            "writer" => self.writer = number(line, rest.first())? as i16,
            "output" => self.output = number(line, rest.first())? as i16,
            "tail-extra" => {
                for k in 0..3 {
                    self.extra[k] = number(line, rest.get(k))? as i16;
                }
            }
            "unknown" => self.unknown = number(line, rest.first())? as i16,
            "state" => {
                for t in rest {
                    let Tok::Word(n) = t else { return Err(err(line, "a state name is a word")) };
                    if self.states.contains(n) {
                        return Err(err(line, format!("state {n} is named twice")));
                    }
                    self.states.push(n.clone());
                }
                if self.states.len() > MAX_STATES {
                    return Err(err(line, format!("more than {MAX_STATES} states")));
                }
            }
            "class" => {
                let Some(Tok::Word(name)) = rest.first() else { return Err(err(line, "a class needs a name")) };
                if self.classes.iter().any(|c| &c.0 == name) {
                    return Err(err(line, format!("class {name} is named twice")));
                }
                let mut members = Vec::new();
                for t in &rest[1..] {
                    match t {
                        Tok::Str(s) => members.extend(s),
                        Tok::Word(w) => members.extend(self.classes[self.class(line, w)?].1.clone()),
                        Tok::Brace(_) => return Err(err(line, "an action cannot stand in a class")),
                    }
                }
                if self.classes.len() >= MAX_CLASSES {
                    return Err(err(line, format!("more than {MAX_CLASSES} classes")));
                }
                let bit = self.classes.len();
                for &b in &members {
                    self.chars[b as usize].classes |= 1 << bit;
                    self.chars[b as usize].defined = true;
                }
                self.classes.push((name.clone(), members));
            }
            "set" => {
                let Some(Tok::Word(name)) = rest.first() else { return Err(err(line, "a set needs a name")) };
                if rest.get(1) != Some(&Tok::Word("=".into())) {
                    return Err(err(line, "a set is written: set name = alternative | alternative"));
                }
                // Every set was given its id, in order, by a first pass over the source, so a set may name a set
                // written after it.
                if self.set_names.get(name) != Some(&(self.sets.len() as u16)) {
                    return Err(err(line, format!("set @{name} is named twice")));
                }
                let mut alts = Vec::new();
                if rest.len() > 2 {
                    for part in rest[2..].split(|t| *t == Tok::Word("|".into())) {
                        if part == [Tok::Word("empty".into())] {
                            // An empty alternative ends the set's search with no match.
                            alts.push(Vec::new());
                        } else {
                            alts.push(self.items(line, part)?);
                        }
                    }
                }
                self.sets.push(alts);
            }
            "char" | "unknown-record" => {
                let unknown = head == "unknown-record";
                let (mut def, mut i) = if unknown {
                    (CharDef::default(), 0)
                } else {
                    let b = string(line, rest.first())?;
                    if b.len() != 1 {
                        return Err(err(line, "char takes one character"));
                    }
                    (self.chars[b[0] as usize].clone(), 1)
                };
                def.defined = true;
                while i < rest.len() {
                    match &rest[i] {
                        Tok::Word(w) if w == "default" => {
                            let o = match rest.get(i + 1) {
                                Some(Tok::Brace(a)) => self.action(line, a)?,
                                other => string(line, other)?,
                            };
                            if o.len() != 1 {
                                return Err(err(line, "a default output is one byte; write a rule for more"));
                            }
                            def.default_out = Some(o[0]);
                            i += 2;
                        }
                        Tok::Word(w) if w == "rules" => {
                            def.rules = Some(number(line, rest.get(i + 1))?);
                            i += 2;
                        }
                        Tok::Word(w) if w == "then" => {
                            def.then = self.changes(line, &rest[i + 1..])?;
                            i = rest.len();
                        }
                        _ => return Err(err(line, "char is followed by default, rules and then")),
                    }
                }
                if unknown {
                    self.unknown_record = Some(def);
                } else {
                    let b = string(line, rest.first())?;
                    self.chars[b[0] as usize] = def;
                }
            }
            "rule" => self.rule(line, rest)?,
            "result" => {
                let n = number(line, rest.first())?;
                if n != self.results.len() as i64 + 1 {
                    return Err(err(line, format!("results are numbered in order; {} comes next", self.results.len() + 1)));
                }
                let then_at = rest.iter().position(|t| *t == Tok::Word("then".into())).unwrap_or(rest.len());
                let mut when = Vec::new();
                if rest.get(1) == Some(&Tok::Word("when".into())) {
                    for t in &rest[2..then_at] {
                        let Tok::Word(w) = t else { return Err(err(line, "when takes state names")) };
                        let (name, value) = match w.strip_prefix('!') {
                            Some(n) => (n, false),
                            None => (w.as_str(), true),
                        };
                        when.push((self.state(line, name)?, value));
                    }
                } else if then_at != 1 {
                    return Err(err(line, "a result is written: result N when ... then ..."));
                }
                let then = if then_at < rest.len() { self.changes(line, &rest[then_at + 1..])? } else { Vec::new() };
                self.results.push((when, then));
            }
            "node" => self.node(line, rest)?,
            "scan" => {
                let Some(Tok::Word(name)) = rest.first() else { return Err(err(line, "scan needs a setting name")) };
                let name = name.clone();
                self.apply_scan(line, &name, &rest[1..])?;
            }
            other => return Err(err(line, format!("unknown statement {other}"))),
        }
        Ok(())
    }

    /// A class named in a node's context: a class name or `bit:N`.
    fn class_bit(&self, line: usize, t: Option<&Tok>) -> Result<u8, EngineError> {
        match t {
            Some(Tok::Word(w)) if w.starts_with("bit:") => raw_bit(line, w),
            Some(Tok::Word(w)) => Ok(self.class(line, w)? as u8),
            _ => Err(err(line, "a class name is missing")),
        }
    }

    /// `node N "literal" <context> result R consume C -> output... next M | level L | end`. The literal is
    /// compared from the node's level; `next` goes on to node M, `level L` to the next comparison level L, `end` stops.
    fn node(&mut self, line: usize, rest: &[Tok]) -> Result<(), EngineError> {
        let n = number(line, rest.first())?;
        if n != self.nodes.len() as i64 + 1 {
            return Err(err(line, format!("nodes are numbered in order; {} comes next", self.nodes.len() + 1)));
        }
        let lit = string(line, rest.get(1))?;
        if lit.len() > MAX_MATCH {
            return Err(err(line, format!("a node compares at most {MAX_MATCH} characters")));
        }
        let word = |i: usize| match rest.get(i) {
            Some(Tok::Word(w)) => w.as_str(),
            _ => "",
        };
        let (ctx, mut i) = match word(2) {
            "none" => (Context::None, 3),
            "class" => (Context::OneClass(self.class_bit(line, rest.get(3))?), 4),
            "classes" => (Context::TwoClasses(self.class_bit(line, rest.get(3))?, self.class_bit(line, rest.get(4))?), 5),
            "set" | "short-set" => {
                let Some(Tok::Word(w)) = rest.get(3) else { return Err(err(line, "a set name is missing")) };
                let name = w.trim_start_matches('@');
                let id = self.set(line, name)?;
                if word(2) == "set" {
                    (Context::Set(id), 4)
                } else {
                    if id > 255 {
                        return Err(err(line, "short-set names sets 0 to 255 only"));
                    }
                    (Context::ShortSet(id as u8), 4)
                }
            }
            other => return Err(err(line, format!("{other} is not none, class, classes, set or short-set"))),
        };
        if word(i) != "result" {
            return Err(err(line, "a node gives its result"));
        }
        let result = number(line, rest.get(i + 1))?;
        if word(i + 2) != "consume" {
            return Err(err(line, "a node gives its consume count"));
        }
        let consume = number(line, rest.get(i + 3))?;
        if !(0..=255).contains(&consume) {
            return Err(err(line, "consume takes 0 to 255"));
        }
        i += 4;
        if word(i) != "->" {
            return Err(err(line, "a node gives its output after ->"));
        }
        i += 1;
        let mut out = Vec::new();
        while i < rest.len() && !matches!(word(i), "next" | "level" | "end") {
            match &rest[i] {
                Tok::Str(s) => out.extend(s),
                Tok::Brace(b) => out.extend(self.action(line, b)?),
                Tok::Word(w) => return Err(err(line, format!("{w} is not part of a node's output"))),
            }
            i += 1;
        }
        if out.len() > MAX_OUTPUT {
            return Err(err(line, format!("a node writes at most {MAX_OUTPUT} bytes")));
        }
        let link = match word(i) {
            "next" => number(line, rest.get(i + 1))?,
            "level" => -number(line, rest.get(i + 1))?,
            "end" => 0,
            _ => return Err(err(line, "a node ends with next M, level L or end")),
        };
        self.nodes.push(NodeDef { line, lit, ctx, result, consume: consume as usize, out, link });
        Ok(())
    }

    fn rule(&mut self, line: usize, rest: &[Tok]) -> Result<(), EngineError> {
        let matched = string(line, rest.first())?;
        if matched.is_empty() || matched.len() > MAX_MATCH {
            return Err(err(line, format!("a rule matches 1 to {MAX_MATCH} characters")));
        }
        if rest.get(1) != Some(&Tok::Word("->".into())) {
            return Err(err(line, "a rule is written: rule \"match\" -> \"output\""));
        }
        let keywords = ["consume", "when", "followed-by", "then"];
        let is_key = |t: &Tok| matches!(t, Tok::Word(w) if keywords.contains(&w.as_str()));
        let mut i = 2;
        let mut out = Vec::new();
        while i < rest.len() && !is_key(&rest[i]) {
            match &rest[i] {
                Tok::Str(s) => out.extend(s),
                Tok::Brace(b) => out.extend(self.action(line, b)?),
                Tok::Word(w) => return Err(err(line, format!("{w} is not part of a rule's output"))),
            }
            i += 1;
        }
        if out.len() > MAX_OUTPUT {
            return Err(err(line, format!("a rule writes at most {MAX_OUTPUT} bytes")));
        }
        let mut rule = Rule {
            line,
            consume: matched.len(),
            matched,
            out,
            when: Vec::new(),
            followed: Vec::new(),
            short_set: false,
            then: Vec::new(),
        };
        while i < rest.len() {
            let Tok::Word(key) = &rest[i] else { return Err(err(line, "a rule part starts with a keyword")) };
            let end = (i + 1..rest.len()).find(|&j| is_key(&rest[j])).unwrap_or(rest.len());
            let part = &rest[i + 1..end];
            match key.as_str() {
                "consume" => {
                    let n = number(line, part.first())?;
                    if !(0..=255).contains(&n) {
                        return Err(err(line, "consume takes 0 to 255"));
                    }
                    rule.consume = n as usize;
                }
                "when" => {
                    for t in part {
                        let Tok::Word(w) = t else { return Err(err(line, "when takes state names")) };
                        let (name, value) = match w.strip_prefix('!') {
                            Some(n) => (n, false),
                            None => (w.as_str(), true),
                        };
                        rule.when.push((self.state(line, name)?, value));
                    }
                }
                "followed-by" => {
                    let set_ref = matches!(part.first(), Some(Tok::Word(w)) if w.starts_with('@'));
                    if part.is_empty() || part.len() > 2 || (set_ref && part.len() != 1) {
                        return Err(err(line, "followed-by takes one or two classes, or one @set"));
                    }
                    if !set_ref && part.iter().any(|t| !matches!(t, Tok::Word(_))) {
                        return Err(err(line, "followed-by takes class names"));
                    }
                    if let Some(Tok::Word(w)) = part.first() {
                        if let Some(name) = w.strip_prefix("@@") {
                            let id = self.set(line, name)?;
                            if id > 255 {
                                return Err(err(line, "@@ names a set by one byte: sets 0 to 255 only"));
                            }
                            rule.short_set = true;
                        }
                    }
                    rule.followed = if rule.short_set {
                        let Some(Tok::Word(w)) = part.first() else { unreachable!() };
                        vec![Item::Set(self.set(line, &w[2..])?)]
                    } else {
                        self.items(line, part)?
                    };
                }
                "then" => rule.then = self.changes(line, part)?,
                _ => unreachable!(),
            }
            i = end;
        }
        self.rules.push(rule);
        Ok(())
    }
}

/// Little-endian writing.
struct Out(Vec<u8>);

impl Out {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn i16(&mut self, v: i64, what: &str) -> Result<(), EngineError> {
        let v = i16::try_from(v).map_err(|_| EngineError::Source(format!("the table is too large ({what})")))?;
        self.0.extend(v.to_le_bytes());
        Ok(())
    }
    fn i32(&mut self, v: i64) {
        self.0.extend((v as i32).to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend(v.to_le_bytes());
    }
}

/// The state masks of a condition (tested bits, their values) or a transition, one 64-bit word per 64 states.
fn masks(width: usize, bits: impl Iterator<Item = (usize, bool, bool)>) -> (Vec<u64>, Vec<u64>) {
    let (mut m1, mut m2) = (vec![0u64; width], vec![0u64; width]);
    for (bit, a, b) in bits {
        if a {
            m1[bit / 64] |= 1 << (bit % 64);
        }
        if b {
            m2[bit / 64] |= 1 << (bit % 64);
        }
    }
    (m1, m2)
}

fn transition_masks(width: usize, changes: &[(usize, Change)]) -> (Vec<u64>, Vec<u64>) {
    masks(
        width,
        changes.iter().map(|&(bit, c)| match c {
            Change::Set => (bit, true, true),
            Change::Clear => (bit, true, false),
            Change::Toggle => (bit, false, true),
        }),
    )
}

/// Index (1-based) of a record in a list of shared records, adding it when new.
fn shared<T: PartialEq>(list: &mut Vec<T>, rec: T) -> i64 {
    match list.iter().position(|r| *r == rec) {
        Some(p) => p as i64 + 1,
        None => {
            list.push(rec);
            list.len() as i64
        }
    }
}

/// Compiles a table source into the binary rule table and the scanner settings.
pub fn compile(source: &str) -> Result<Compiled, EngineError> {
    let mut b = Builder {
        title: String::new(),
        default_char: b' ',
        raw: 0,
        writer: 3,
        output: 3,
        extra: [0; 3],
        unknown: 1,
        states: Vec::new(),
        classes: Vec::new(),
        sets: Vec::new(),
        set_names: HashMap::new(),
        chars: vec![CharDef::default(); 256],
        rules: Vec::new(),
        results: Vec::new(),
        nodes: Vec::new(),
        unknown_record: None,
        scan: default_scan(),
    };
    // First pass: every set gets its id in the order the sets are written.
    for (n, text) in source.lines().enumerate() {
        let toks = tokens(n + 1, text)?;
        if let (Some(Tok::Word(head)), Some(Tok::Word(name))) = (toks.first(), toks.get(1)) {
            if head == "set" {
                if b.set_names.contains_key(name) {
                    return Err(err(n + 1, format!("set @{name} is named twice")));
                }
                let id = b.set_names.len() as u16;
                b.set_names.insert(name.clone(), id);
            }
        }
    }
    for (n, text) in source.lines().enumerate() {
        let toks = tokens(n + 1, text)?;
        if !toks.is_empty() {
            b.statement(n + 1, toks)?;
        }
    }

    // Checks over the whole table.
    for r in &b.rules {
        for (k, &c) in r.matched.iter().enumerate() {
            if !b.chars[c as usize].defined {
                let what = if k == 0 { "starts with" } else { "holds" };
                return Err(err(r.line, format!("the rule {what} the character {:?}, which has no class", c as char)));
            }
        }
    }
    for (i, r) in b.rules.iter().enumerate() {
        let mut when = r.when.clone();
        when.sort();
        for q in &b.rules[..i] {
            let mut qw = q.when.clone();
            qw.sort();
            if q.matched == r.matched && qw == when && q.followed == r.followed && q.short_set == r.short_set {
                return Err(err(r.line, format!("the same rule (match, conditions and context) is on line {}", q.line)));
            }
        }
    }
    if !b.chars[b.default_char as usize].defined && b.unknown_record.is_none() {
        return Err(err(0, format!("the default character {:?} has no class", b.default_char as char)));
    }

    let width = if b.states.len() <= 64 { 1 } else { 2 };
    let mut conditions: Vec<(Vec<u64>, Vec<u64>)> = vec![masks(width, std::iter::empty())];
    let mut transitions: Vec<(Vec<u64>, Vec<u64>)> = vec![masks(width, std::iter::empty())];

    // Each rule's context: in the node for a rule of one character, else one anonymous set.
    let mut sets = b.sets.clone();
    let mut contexts: Vec<Context> = Vec::new();
    for r in &b.rules {
        let single = r.matched.len() == 1;
        let ctx = match (single, r.followed.as_slice()) {
            (true, []) => Context::None,
            (true, [Item::Class(a)]) => Context::OneClass(*a),
            (true, [Item::Class(a), Item::Class(c)]) => Context::TwoClasses(*a, *c),
            (true, [Item::Set(id)]) if r.short_set => Context::ShortSet(*id as u8),
            (true, [Item::Set(id)]) => Context::Set(*id),
            _ => {
                let mut items: Vec<Item> = r.matched[1..].iter().map(|&c| Item::Lit(c)).collect();
                items.extend(r.followed.iter().cloned());
                sets.push(vec![items]);
                Context::Set(sets.len() as u16 - 1)
            }
        };
        contexts.push(ctx);
    }

    // The set pool: per set its alternatives, each a byte program ended by 0xFC.
    let mut set_pool: Vec<u8> = Vec::new();
    let mut set_index: Vec<i64> = Vec::new();
    let mut set_index2: Vec<i64> = Vec::new();
    for alts in &sets {
        set_index.push(set_index2.len() as i64);
        for alt in alts {
            set_index2.push(set_pool.len() as i64);
            for item in alt {
                match item {
                    Item::Class(bit) => set_pool.push(*bit),
                    Item::Lit(c) => set_pool.extend([0xfd, *c]),
                    Item::Any => set_pool.push(0xff),
                    Item::Set(id) => set_pool.extend([0xfe, (*id & 0xff) as u8, (*id >> 8) as u8]),
                }
            }
            set_pool.push(0xfc);
        }
    }
    set_index.push(set_index2.len() as i64);

    if !b.nodes.is_empty() && !b.rules.is_empty() {
        return Err(err(b.rules[0].line, "a table is written with rule statements or with node statements, not both"));
    }
    let mut pool: Vec<u8> = Vec::new();
    let mut nodes: Vec<(i64, i64, i64)> = Vec::new(); // offset, link, flags
    let mut results: Vec<(i64, i64)> = Vec::new();
    let mut first_rule = [0i64; 256];
    if b.nodes.is_empty() {
        // Rule chains: per first character, longer matches first, then source order.
        let mut order: Vec<usize> = (0..b.rules.len()).collect();
        order.sort_by_key(|&i| (b.rules[i].matched[0], std::cmp::Reverse(b.rules[i].matched.len()), i));
        for (pos, &ri) in order.iter().enumerate() {
            let r = &b.rules[ri];
            let node = nodes.len() as i64 + 1;
            if first_rule[r.matched[0] as usize] == 0 {
                first_rule[r.matched[0] as usize] = node;
            }
            let next_same = order.get(pos + 1).is_some_and(|&n| b.rules[n].matched[0] == r.matched[0]);
            let link = if next_same { node + 1 } else { 0 };
            let offset = pool.len() as i64 + 1;
            let mut flags = match contexts[ri] {
                Context::None => 0xc0,
                Context::OneClass(a) => {
                    pool.push(a);
                    0x40
                }
                Context::TwoClasses(a, c) => {
                    pool.extend([a, c]);
                    0x00
                }
                Context::Set(id) => {
                    pool.extend([(id & 0xff) as u8, (id >> 8) as u8]);
                    0x80
                }
                Context::ShortSet(id) => {
                    pool.extend([0xfe, id]);
                    0x00
                }
            };
            let cond = shared(&mut conditions, masks(width, r.when.iter().map(|&(bit, v)| (bit, true, v))));
            let trans = shared(&mut transitions, transition_masks(width, &r.then));
            let rid = shared(&mut results, (cond, trans));
            if rid == 1 {
                // Result 1 needs no id bytes.
                flags |= 0x8000;
            } else {
                pool.extend([(rid & 0xff) as u8, (rid >> 8) as u8]);
            }
            if r.consume == 2 {
                flags |= 0x4000;
            } else {
                pool.push(r.consume as u8);
            }
            pool.extend(&r.out);
            nodes.push((offset, link, flags | ((r.out.len() as i64) << 8)));
        }
    
    } else {
        // Node mode: the trie as written, the results by their numbers.
        for (when, then) in &b.results {
            let cond = shared(&mut conditions, masks(width, when.iter().map(|&(bit, v)| (bit, true, v))));
            let trans = shared(&mut transitions, transition_masks(width, then));
            results.push((cond, trans));
        }
        for nd in &b.nodes {
            if nd.result < 1 || nd.result > results.len() as i64 {
                return Err(err(nd.line, format!("result {} is not written", nd.result)));
            }
            if nd.link > b.nodes.len() as i64 {
                return Err(err(nd.line, format!("node {} is not written", nd.link)));
            }
            let offset = pool.len() as i64 + 1;
            pool.extend(&nd.lit);
            let mut flags = nd.lit.len() as i64
                | match nd.ctx {
                    Context::None => 0xc0,
                    Context::OneClass(a) => {
                        pool.push(a);
                        0x40
                    }
                    Context::TwoClasses(a, c) => {
                        pool.extend([a, c]);
                        0x00
                    }
                    Context::Set(id) => {
                        pool.extend([(id & 0xff) as u8, (id >> 8) as u8]);
                        0x80
                    }
                    Context::ShortSet(id) => {
                        pool.extend([0xfe, id]);
                        0x00
                    }
                };
            if nd.result == 1 {
                flags |= 0x8000;
            } else {
                pool.extend([(nd.result & 0xff) as u8, (nd.result >> 8) as u8]);
            }
            if nd.consume == 2 {
                flags |= 0x4000;
            } else {
                pool.push(nd.consume as u8);
            }
            pool.extend(&nd.out);
            nodes.push((offset, nd.link, flags | ((nd.out.len() as i64) << 8)));
        }
    }

    // The character records: one per character that has a class or a char statement.
    let mut class_of = [0i64; 256];
    let mut records: Vec<(u8, u64, i64, u8, i64)> = Vec::new();
    for c in 0..256usize {
        let d = &b.chars[c];
        if !d.defined {
            continue;
        }
        let trans = shared(&mut transitions, transition_masks(width, &d.then));
        records.push((c as u8, d.classes, trans, d.default_out.unwrap_or(c as u8), d.rules.unwrap_or(first_rule[c])));
        class_of[c] = records.len() as i64;
    }
    if let Some(d) = &b.unknown_record {
        let trans = shared(&mut transitions, transition_masks(width, &d.then));
        records.push((0, 0, trans, d.default_out.unwrap_or(0), d.rules.unwrap_or(0)));
    }

    let mut o = Out(Vec::new());
    let mut title: Vec<u8> = super::chars::encode_cp437(&b.title).unwrap_or_default().into_iter().map(|c| c as u8).collect();
    title.truncate(39);
    title.resize(40, 0);
    o.0.extend(title);
    o.0.extend([0u8; 68]);
    o.i16(0x1a47, "signature")?;
    o.i16(VERSION as i64, "version")?;
    o.i16(width as i64, "state width")?;
    o.i16(records.len() as i64, "characters")?;
    o.i16(results.len() as i64, "rule results")?;
    o.i16(conditions.len() as i64, "conditions")?;
    o.i16(transitions.len() as i64, "state changes")?;
    for &(c, classes, trans, out, first) in &records {
        o.u8(c);
        o.u64(classes);
        o.i16(trans, "state changes")?;
        o.u8(out);
        o.i16(first, "rules")?;
    }
    o.i16(256, "characters")?;
    for &k in &class_of {
        o.i16(k, "characters")?;
    }
    o.i16(set_pool.len() as i64, "set pool")?;
    o.i16(set_index2.len() as i64, "set alternatives")?;
    o.i16(sets.len() as i64, "sets")?;
    for &v in &set_index {
        o.i16(v, "sets")?;
    }
    for &v in &set_index2 {
        o.i16(v, "set pool")?;
    }
    o.0.extend(&set_pool);
    o.i16(nodes.len() as i64, "rules")?;
    o.i32(pool.len() as i64);
    for n in &nodes {
        o.i32(n.0);
    }
    for n in &nodes {
        o.i16(n.1, "rules")?;
    }
    for n in &nodes {
        // The flags are a 16-bit field; bit 15 marks result 1.
        o.0.extend((n.2 as u16).to_le_bytes());
    }
    o.0.extend(&pool);
    for &(cond, trans) in &results {
        o.i16(cond, "conditions")?;
        o.i16(trans, "state changes")?;
    }
    for list in [&conditions, &transitions] {
        for (m1, m2) in list.iter() {
            for &w in m1 {
                o.u64(w);
            }
            for &w in m2 {
                o.u64(w);
            }
        }
    }
    o.u8(b.default_char);
    let tail = [b.raw, b.writer, b.output, b.extra[0], b.extra[1], b.extra[2], b.unknown];
    for v in tail {
        o.i16(v as i64, "tail")?;
    }
    Ok(Compiled { table: o.0, scan: b.scan })
}

#[cfg(test)]
mod tests {
    use super::super::{RuleTable, Translator};
    use super::*;

    /// A small table: letters, space and full stop, digits with a number sign at the start of a number, "and" as
    /// a whole word, "ing" inside a word.
    const SMALL: &str = r##"
table "test table"
default-char " "
state in-word numeric
class letter "abcdefghijklmnopqrstuvwxyz"
class digit "0123456789"
class space " "
class punct ".,"
set word-end = space | punct
char " " then clear in-word clear numeric
char "." then clear in-word clear numeric
char "," then clear in-word clear numeric
rule "a" -> "a" then set in-word clear numeric
rule "and" -> "&" when !in-word followed-by @word-end then set in-word
rule "ing" -> "+" when in-word
rule "1" -> "#a" when !numeric then set numeric
rule "1" -> "a" when numeric
rule "2" -> "#b" when !numeric then set numeric
rule "2" -> "b" when numeric
"##;

    fn small() -> String {
        // Every other letter: written as is, and inside a word from then on.
        let mut s = SMALL.to_string();
        for c in "bcdefghijklmnopqrstuvwxyz".chars() {
            s.push_str(&format!("char \"{c}\" then set in-word clear numeric\n"));
        }
        s
    }

    #[test]
    fn translates_with_a_small_table() {
        let t = Translator::from_source(&small()).unwrap();
        let cases = [
            ("and", "&"),
            ("and so", "& so"),
            ("band", "band"),
            ("hand.", "hand."),
            ("sing", "s+"),
            ("ing", "ing"),
            ("12 21", "#ab #ba"),
            ("andy", "andy"),
            ("and, 1", "&, #a"),
        ];
        for (text, want) in cases {
            assert_eq!(t.translate(text).unwrap(), want, "{text}");
        }
    }

    #[test]
    fn compiled_bytes_load_through_the_reader() {
        let c = compile(&small()).unwrap();
        let t = RuleTable::read(&c.table).unwrap();
        assert_eq!(t.title, "test table");
        assert_eq!(t.version, 19);
        assert_eq!(t.width, 1);
        assert!(t.class_of[b'a' as usize] > 0);
        assert_eq!(t.class_of[b'#' as usize], 0);
        assert_eq!(t.rules.len(), 7);
    }

    /// One table using every part of the layout: more than 64 states (two mask words), a class bit above 32,
    /// default outputs that are actions, a set with literals, classes, "any" and a nested set, every rule context
    /// kind (none, one class, two classes, set by two-byte id, set by one-byte id, several characters through an
    /// anonymous set), shared results, consume counts of 2 and of 0, actions with arguments in output, and every
    /// tail field.
    fn every() -> String {
        let mut s = String::from(
            r#"table "every feature"
default-char "?"
raw 0
writer 3
output 3
tail-extra 7 8 9
unknown 1
state a0 a1 a2 a3 a4 a5 a6 a7 a8 a9 b0 b1 b2 b3 b4 b5 b6 b7 b8 b9 c0 c1 c2 c3 c4 c5 c6 c7 c8 c9 d0 d1 d2 d3 d4
state d5 d6 d7 d8 d9 e0 e1 e2 e3 e4 e5 e6 e7 e8 e9 f0 f1 f2 f3 f4 f5 f6 f7 f8 f9 g0 g1 g2 g3 g4 high
class letter "abcdefghijklmnopqrstuvwxyz"
class space " "
"#,
        );
        // Empty classes up to bit 39, so the digit class is bit 40.
        for k in 2..40 {
            s.push_str(&format!("class k{k:02} \"\"\n"));
        }
        s.push_str(
            r#"class digit "0123456789"
class mark "?*"
set word-end = space
set digits = digit
set ab = "a" letter | any "z" | @digits
char "?" default "?"
char "*" default {copy}
rule "x" -> "X" followed-by digit
rule "y" -> "Y" followed-by letter letter
rule "w" -> "Z" followed-by @ab
rule "v" -> "W" followed-by @@word-end
rule "q" -> "Q"
rule "qu" -> "!" consume 2 then set high
rule "k" -> "K" {push high} {pop high} when high then clear high
rule "zz" -> "" consume 0 when !g4 followed-by digit then set g4
rule "z" -> {cmd c} "z" then clear g4
rule "1" -> "one"
"#,
        );
        s
    }

    #[test]
    fn every_feature_loads_and_runs() {
        let c = compile(&every()).unwrap();
        let t = RuleTable::read(&c.table).unwrap();
        assert_eq!(t.width, 2);
        assert_eq!(t.tail, [b'?' as i64, 0, 3, 3, 7, 8, 9, 1]);
        assert_eq!(t.title, "every feature");
        // The digit class is bit 40.
        assert_eq!((t.classes[(t.class_of[b'5' as usize] - 1) as usize].sets >> 40) & 1, 1);
        // The star writes action 1 (copy the next character through) when no rule matches.
        assert_eq!(t.classes[(t.class_of[b'*' as usize] - 1) as usize].default_out, 1);
        let kinds: Vec<u32> = t.rules.iter().map(|r| r.flags & 0xc0).collect();
        for k in [0x00, 0x40, 0x80, 0xc0] {
            assert!(kinds.contains(&k), "context kind {k:#x}");
        }
        assert!(t.rules.iter().any(|r| r.flags & 0x8000 != 0));
        assert!(t.rules.iter().any(|r| r.flags & 0x4000 != 0));

        let tr = Translator::from_source(&every()).unwrap();
        let cases = [
            ("x1", "Xone"),  // one class bit in the node
            ("xa", "xa"),    // the class does not match: the default output
            ("yab", "Yab"),  // two class bits
            ("ya1", "yaone"),
            ("wab", "Zab"),  // set alternative 1: "a" then a letter
            ("w z", "Z \u{10}cz"),  // set alternative 2: any character then "z"; writer mode 3 passes {cmd c} as it is
            ("w5", "Z5"),    // set alternative 3: the nested set
            ("wq", "wQ"),    // no alternative matches
            ("v ", "W "),    // set by a one-byte id
            ("q", "Q"),
            ("qu", "!"),     // two characters, consume 2, then sets "high"
            ("quk", "!K"),   // "high" is set, so the k rule applies and clears it
            ("k", "k"),      // "high" is not set: the default output
            ("*a", "a"),     // the default output of "*" is the copy action
            ("zz1", "\u{10}cz\u{10}czone"),  // consume 0 leaves the place, the state change stops a loop
        ];
        for (text, want) in cases {
            assert_eq!(tr.translate(text).unwrap(), want, "{text}");
        }
    }

    #[test]
    fn many_states_use_two_mask_words() {
        let names: Vec<String> = (0..70).map(|i| format!("s{i}")).collect();
        let src = format!("state {}\nclass space \" \"\nchar \"a\" then set s69\n", names.join(" "));
        let t = RuleTable::read(&compile(&src).unwrap().table).unwrap();
        assert_eq!(t.width, 2);
    }

    fn refused(src: &str, part: &str) {
        match compile(src) {
            Err(EngineError::Source(m)) => assert!(m.contains(part), "{m}"),
            Err(e) => panic!("{e:?}"),
            Ok(_) => panic!("accepted: {src}"),
        }
    }

    #[test]
    fn refuses_mistakes() {
        refused("class space \" \"\nclass x space nope", "unknown class nope");
        refused("class space \" \"\nchar \"a\" then set nope", "unknown state nope");
        refused("class space \" \"\nchar \"a\"\nrule \"a\" -> \"a\" followed-by @nope", "unknown set @nope");
        let names: Vec<String> = (0..129).map(|i| format!("s{i}")).collect();
        refused(&format!("state {}", names.join(" ")), "more than 128 states");
        refused("class space \" \"\nrule \"q\" -> \"q\"", "starts with the character 'q', which has no class");
        refused(
            "class space \" \"\nchar \"a\"\nrule \"a\" -> \"x\"\nrule \"a\" -> \"y\"",
            "the same rule (match, conditions and context) is on line 3",
        );
        refused("class letter \"a\"", "the default character ' ' has no class");
        refused("bogus 1", "unknown statement bogus");
    }
}
