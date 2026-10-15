//! The rule engine: reads the scanner's codes through a ring of 64, finds the first rule of the table that matches
//! at the current place (its text, the character sets around it, and the table's state bits), writes the rule's
//! output, takes the characters it matched, and applies the rule's state transition. A place where no rule matches
//! writes the character class's default output.
//!
//! Bytes below 0x20 in a rule's output are actions: copy a character or a word through, skip a space, move a code,
//! push or pop a state bit, expand a style, switch tables (0x15, handled by the caller), and so on.
//!
//! Styles: a code [es~name] or [ee~name] in the input is replaced by the style's begin or end codes from the style
//! table, while rules look ahead, while matched characters are taken, and when a rule asks for it (action 0x1B).
//! While a style's codes run as the input, output is written or held back as the table says.

use std::rc::Rc;

use super::callbacks::StyleTable;
use super::chars::{flags, item, EOF};
use super::table::RuleTable;
use super::writer::Chitab;

pub(crate) struct Engine<'a> {
    pub t: Rc<RuleTable>,
    style: Option<&'a StyleTable>,
    chitab: &'a Chitab,
    /// 0 reading the input; 2 running a style's codes without output; 1 running them with output; -1 output off.
    exp: i64,
    saved_ring: [i64; 64],
    saved_pos: usize,
    exp_str: Vec<u8>,
    exp_i: usize,
    /// The name collected by action 0x14 (what follows "=" or "~").
    pub name: [u8; 32],
    pub name_len: i64,
    pending: i64,
    /// Where the set matcher saw a code start (0x1C), for moving that code.
    mark: i64,
    la: [i64; 70],
    la_n: i64,
    la_src: usize,
    /// Which ring the look-ahead reads: 0 the input ring, 1 the saved input ring.
    la_which: i64,
    la_fix: i64,
    la_fix_last: i64,
    default_char: i64,
    /// 0: a 0x17 pair passes through as text, and bytes below 0x20 in rule output are actions.
    raw17: i64,
    unknown_mode: i64,
    pub inp: Vec<i64>,
    pub ip: usize,
    pub ring: [i64; 64],
    pub pos: usize,
    state: Vec<i128>,
    stack: [u128; 128],
    pub out: Vec<i64>,
    m_consume: i64,
    m_out: Vec<u8>,
    m_rid: i64,
}

fn ring_index(pos: usize, d: i64) -> usize {
    ((pos as i64 + d) & 63) as usize
}

impl<'a> Engine<'a> {
    pub fn new(t: Rc<RuleTable>, text: &[i64], style: Option<&'a StyleTable>, chitab: &'a Chitab) -> Engine<'a> {
        let width = t.width.max(2);
        let mut inp = text.to_vec();
        inp.push(EOF);
        Engine {
            default_char: t.tail[0],
            raw17: t.tail[1],
            unknown_mode: t.tail[7],
            t,
            style,
            chitab,
            exp: 0,
            saved_ring: [EOF; 64],
            saved_pos: 0,
            exp_str: Vec::new(),
            exp_i: 0,
            name: [0; 32],
            name_len: -1,
            pending: -2,
            mark: -2,
            la: [0; 70],
            la_n: 0,
            la_src: 0,
            la_which: 0,
            la_fix: 0,
            la_fix_last: 0,
            inp,
            ip: 0,
            ring: [0; 64],
            pos: 0,
            state: vec![0; width],
            stack: [0; 128],
            out: Vec::new(),
            m_consume: 0,
            m_out: Vec::new(),
            m_rid: 0,
        }
    }

    /// The next character: from the scanner's codes or, while a style's codes run, from them.
    fn getc(&mut self) -> i64 {
        if self.exp >= 1 {
            if let Some(&c) = self.exp_str.get(self.exp_i) {
                self.exp_i += 1;
                return c as i64;
            }
            return EOF;
        }
        if let Some(&c) = self.inp.get(self.ip) {
            self.ip += 1;
            return c;
        }
        EOF
    }

    fn out_on(&self) -> bool {
        0 <= self.exp && self.exp < 2
    }

    fn put(&mut self, c: i64) {
        if self.out_on() {
            self.out.push(c);
        }
    }

    pub fn emit(&mut self, c: i64) {
        self.out.push(c);
    }

    pub fn adv(&mut self) {
        self.ring[self.pos] = self.getc();
        self.pos = (self.pos + 1) & 63;
    }

    pub fn cur(&self, d: i64) -> i64 {
        self.ring[ring_index(self.pos, d)]
    }

    pub fn unknown_mode(&self) -> i64 {
        self.unknown_mode
    }

    // ---------------------------------------------------------------- look-ahead

    fn push_la(&mut self, c: i64) {
        self.la_n += 1;
        self.la[self.la_n as usize] = c;
    }

    fn peek4(&self, src: i64, p: usize) -> [i64; 4] {
        let ring = if src == 0 { &self.ring } else { &self.saved_ring };
        [ring[p], ring[(p + 1) & 63], ring[(p + 2) & 63], ring[(p + 3) & 63]]
    }

    /// Fills the look-ahead to `n` characters, expanding style codes. False when the ring has no more.
    fn fill(&mut self, n: i64) -> bool {
        let mut in_style = false;
        let mut name: Vec<u8> = Vec::new();
        let mut which = 0;
        loop {
            if n <= self.la_n {
                return true;
            }
            let mut src = self.la_which;
            let mut p = (self.la_src + 1) & 63;
            let mut stop = if src == 0 { self.pos } else { self.saved_pos };
            if p == stop {
                return false;
            }
            let mut c = self.peek4(src, p);
            if c[0] == EOF {
                if self.exp < 1 || src == 1 {
                    if self.la_n > 0x3e {
                        return false;
                    }
                    self.push_la(EOF);
                    self.la_src = p;
                    return n <= self.la_n;
                }
                // The style's codes have ended: go on in the saved input ring.
                stop = self.saved_pos;
                src = 1;
                p = stop;
                c = self.peek4(1, p);
                if c[0] == EOF {
                    if self.la_n > 0x3e {
                        return false;
                    }
                    self.push_la(EOF);
                    self.la_src = p;
                    self.la_which = 1;
                    return n <= self.la_n;
                }
            }
            if self.raw17 != 0 {
                if self.la_n > 0x3e {
                    return false;
                }
                self.push_la(c[0]);
                self.la_src = p;
                self.la_which = src;
                continue;
            }
            if c[0] == 0x17 {
                if ((p + 1) & 63) == stop || ((p + 2) & 63) == stop {
                    return false;
                }
                self.la_src = (p + 1) & 63;
                self.la_which = src;
                continue;
            }
            if !in_style {
                if c[0] == 0x1c
                    && (1..6).all(|i| ((p + i) & 63) != stop)
                    && c[1] == 0x65
                    && (c[2] == 0x73 || c[2] == 0x65)
                    && c[3] == 0x7e
                {
                    in_style = true;
                    name.clear();
                    which = if c[2] != 0x73 { 1 } else { 0 };
                    self.la_src = (p + 3) & 63;
                    self.la_which = src;
                } else {
                    if self.la_n > 0x3e {
                        return false;
                    }
                    self.push_la(c[0]);
                    self.la_src = p;
                    self.la_which = src;
                }
            } else {
                self.la_src = p;
                self.la_which = src;
                if c[0] == 0x1f || c[0] == 0x20 || c[0] == 0x60 {
                    let codes = self.style.and_then(|s| s.lookup(&name, which));
                    if let Some(codes) = codes {
                        self.la_fix += self.la_fix_last;
                        self.la_fix_last = (name.len() as i64 + 5) - codes.len() as i64;
                        for &b in &codes {
                            if self.la_n > 0x3e {
                                return n <= self.la_n;
                            }
                            self.push_la(b as i64);
                        }
                    }
                    in_style = false;
                } else if name.len() < 0x1f {
                    name.push(c[0] as u8);
                }
            }
        }
    }

    fn la_at(&mut self, k: i64) -> Option<i64> {
        if k <= self.la_n || self.fill(k) {
            return self.la.get(k as usize).copied();
        }
        None
    }

    /// The class of a look-ahead character; the end of input counts as a space.
    fn cls_of(&self, ch: Option<i64>) -> i64 {
        match ch {
            None => 0,
            Some(c) if (0..256).contains(&c) => self.t.class_of[c as usize],
            Some(EOF) => self.t.class_of[0x20],
            _ => 0,
        }
    }

    /// Whether the character's class belongs to character set `b`.
    fn cls_bit(&self, ch: Option<i64>, b: i64) -> Option<i64> {
        let k = self.cls_of(ch);
        if k < 1 {
            return None;
        }
        if b < 64 {
            let class = item(&self.t.classes, k - 1)?;
            Some(((class.sets >> (b & 0x3f)) & 1) as i64)
        } else {
            Some(0)
        }
    }

    fn set_byte(&self, i: i64) -> i64 {
        item(&self.t.set_pool, i).map_or(0xfc, |&b| b as i64)
    }

    fn pool_byte(&self, i: i64) -> i64 {
        item(&self.t.pool, i).map_or(0, |&b| b as i64)
    }

    /// Matches character set `sid` at look-ahead place `k`: the number of characters matched, or 0.
    fn setmatch(&mut self, k: i64, sid: i64, depth: i64) -> i64 {
        if depth >= 0x46 {
            return 0;
        }
        let (Some(&start), Some(&end)) = (item(&self.t.set_index, sid), item(&self.t.set_index, sid + 1)) else {
            return 0;
        };
        for j in start..end {
            let Some(&first) = item(&self.t.set_index2, j) else { continue };
            let mut c = first;
            let mut kk = k;
            loop {
                let b = self.set_byte(c);
                if b == 0xfc {
                    return kk - k;
                }
                if kk > 0x3f {
                    break;
                }
                if b == 0xff {
                    c += 1;
                    kk += 1;
                } else if b == 0xfd {
                    let ch = self.la_at(kk);
                    match ch {
                        Some(ch) if (ch & 0xff) == self.set_byte(c + 1) && ch != EOF => {
                            if ch == 0x1c {
                                self.mark = kk;
                            }
                        }
                        _ => break,
                    }
                    c += 2;
                    kk += 1;
                } else if b == 0xfe {
                    let nid = if self.t.version < 18 {
                        self.set_byte(c + 1)
                    } else {
                        self.set_byte(c + 1) | self.set_byte(c + 2) << 8
                    };
                    let r = self.setmatch(kk, nid, depth + 1);
                    if r < 1 {
                        break;
                    }
                    c += 2 + if self.t.version > 17 { 1 } else { 0 };
                    kk += r;
                } else {
                    let ch = self.la_at(kk);
                    if ch.is_none() || self.cls_bit(ch, b) != Some(1) {
                        break;
                    }
                    c += 1;
                    kk += 1;
                }
            }
        }
        0
    }

    // ---------------------------------------------------------------- the matcher

    /// Tries the rules from `node` on; true when one matches (its result is in m_consume, m_out, m_rid).
    fn try_rules(&mut self, node: i64) -> bool {
        self.la_n = 0;
        self.la_src = self.pos;
        self.la_which = 0;
        self.la = [0; 70];
        let (mut i, mut level, mut skip) = (node, 1i64, false);
        loop {
            let Some(&rule) = item(&self.t.rules, i - 1) else { return false };
            let mut off = rule.offset;
            let mut link = rule.link;
            let flags = rule.flags;
            let mut k = level;
            let mut reached_ctx = false;
            if flags & 0x3f == 0 {
                reached_ctx = true;
            } else if !skip {
                let n = (flags & 0x3f) as i64;
                let mut good = true;
                loop {
                    let ch = self.la_at(k);
                    let b = self.pool_byte(off - 1);
                    off += 1;
                    if ch != Some(b) {
                        good = false;
                        break;
                    }
                    k += 1;
                    if k >= level + n {
                        break;
                    }
                }
                reached_ctx = good;
            }
            if reached_ctx {
                let kind = flags & 0xc0;
                let mut c: Option<i64> = None;
                if kind == 0xc0 {
                    c = Some(off);
                } else {
                    let ch = self.la_at(k);
                    if self.cls_of(ch) > 0 {
                        let b = self.pool_byte(off - 1);
                        if kind == 0x80 || b == 0xfe {
                            let sid = if kind == 0x80 { b | self.pool_byte(off) << 8 } else { self.pool_byte(off) };
                            self.mark = -2;
                            if self.setmatch(k, sid, 0) > 0 {
                                c = Some(off + 2);
                            }
                        } else if self.cls_bit(ch, b) == Some(1) {
                            if flags & 0x40 != 0 {
                                c = Some(off + 1);
                            } else {
                                let ch2 = self.la_at(k + 1);
                                if self.cls_of(ch2) > 0 && self.cls_bit(ch2, self.pool_byte(off)) == Some(1) {
                                    c = Some(off + 2);
                                }
                            }
                        }
                    }
                }
                if let Some(mut c) = c {
                    let rid;
                    if flags & 0x8000 != 0 {
                        rid = 1;
                    } else {
                        let mut r = self.pool_byte(c - 1);
                        c += 1;
                        if self.t.version > 14 {
                            r += self.pool_byte(c - 1) * 256;
                            c += 1;
                        }
                        rid = r;
                    }
                    let cond = item(&self.t.results, rid - 1).map(|r| r.0);
                    let ok = cond.and_then(|cond| item(&self.t.conditions, cond - 1)).is_some_and(|(m1, m2)| {
                        (0..m1.len()).all(|x| ((m2[x] ^ self.state.get(x).copied().unwrap_or(0)) & m1[x]) == 0)
                    });
                    if ok {
                        let cons = if flags & 0x4000 != 0 {
                            2
                        } else {
                            let v = self.pool_byte(c - 1);
                            c += 1;
                            v
                        };
                        let nout = ((flags & 0x3f00) >> 8) as i64;
                        self.m_consume = cons;
                        self.m_out = (0..nout)
                            .filter_map(|j| {
                                let at = c - 1 + j;
                                if at >= 0 {
                                    self.t.pool.get(at as usize).copied()
                                } else {
                                    None
                                }
                            })
                            .collect();
                        self.m_rid = rid;
                        return true;
                    }
                }
            }
            // On to the next rule.
            if link < 1 {
                if link == 0 {
                    return false;
                }
                if k == level {
                    while level <= -link {
                        i += 1;
                        loop {
                            let Some(r) = item(&self.t.rules, i - 1) else { return false };
                            link = r.link;
                            if link < 0 {
                                break;
                            }
                            i = link;
                            if link == 0 {
                                return false;
                            }
                        }
                    }
                    level = -link;
                    skip = true;
                    i += 1;
                    continue;
                } else if level <= -link {
                    level = -link;
                    i += 1;
                    continue;
                }
                level = -link;
                skip = true;
                i += 1;
                continue;
            }
            let mut nxt = link;
            if k != level {
                nxt = i + 1;
                if link == i + 1 {
                    skip = true;
                } else {
                    level += 1;
                }
            }
            i = nxt;
        }
    }

    fn transition(&mut self, d: i64) {
        let Some((m1, m2)) = item(&self.t.transitions, d - 1) else { return };
        let (m1, m2) = (m1.clone(), m2.clone());
        for x in 0..m1.len() {
            if x >= self.state.len() {
                break;
            }
            let s = self.state[x];
            self.state[x] = ((!m1[x] & (s ^ m2[x])) | (m1[x] & m2[x])) & 0xffff_ffff_ffff_ffff;
        }
    }

    // ---------------------------------------------------------------- actions

    fn action(&mut self, code: i64, args: &[i64]) {
        match code {
            // Copy the current character through.
            1 => {
                let c = self.cur(0);
                if c > 0x1e && c != EOF {
                    self.put(c);
                    self.adv();
                }
            }
            // Copy a word through, keeping what follows "=" or "~" as the name.
            0x14 => {
                self.name_len = -1;
                loop {
                    let c = self.cur(0);
                    if !(c > 0x20 && c != 0x60 && c != EOF) {
                        break;
                    }
                    if c == 0x3d || c == 0x7e {
                        self.name_len = 0;
                    } else if 0 <= self.name_len && self.name_len < 0x1f {
                        self.name[self.name_len as usize] = c as u8;
                        self.name_len += 1;
                    }
                    self.put(c);
                    self.adv();
                }
            }
            // Skip a space (0x18) or any character (0x19).
            0x18 | 0x19 => {
                let c = self.cur(0);
                if code == 0x19 || c == 0x20 || c == 0x1f || c == 0x60 {
                    self.adv();
                }
            }
            0x1a => {
                self.rotate(1);
            }
            // Push a state bit.
            0x0c if !args.is_empty() => {
                let n = args[0];
                if 0 < n && n < 0x81 {
                    let b = (n - 1) as usize;
                    let bit = (self.state.get(b >> 6).copied().unwrap_or(0) >> (b & 63)) & 1;
                    self.stack[b] = (self.stack[b] << 1) | bit as u128;
                }
            }
            // Pop a state bit.
            0x0d if !args.is_empty() => {
                let n = args[0];
                if 0 < n && n < 0x81 {
                    let b = (n - 1) as usize;
                    if let Some(word) = self.state.get_mut(b >> 6) {
                        if self.stack[b] & 1 != 0 {
                            *word |= 1 << (b & 63);
                        } else {
                            *word &= !(1i128 << (b & 63));
                        }
                    }
                    self.stack[b] >>= 1;
                }
            }
            0x16 if args.len() == 2 => self.code_move(args[0], args[1]),
            0x1b if !args.is_empty() => self.style_action(args[0]),
            _ => self.put(code),
        }
    }

    /// Moves a character or a whole code from `n` places ahead to the current place (or swaps two characters).
    fn code_move(&mut self, kind: i64, n: i64) {
        if kind == 0 {
            if self.exp < 1 {
                let c = self.rotate(n);
                if self.exp > -1 {
                    self.emit(c);
                }
            }
        } else if kind == 1 {
            if self.exp < 1 {
                if self.cur(n) == 0x1c {
                    loop {
                        let c = self.rotate(n);
                        self.put(c);
                        if c == 0x1f || c == 0x60 || c == 0x20 || c == EOF {
                            break;
                        }
                    }
                }
            } else {
                self.pending = n;
            }
        } else if kind == 2 && self.exp < 1 {
            let (a, b) = (ring_index(self.pos, n), ring_index(self.pos, n + 1));
            self.ring.swap(a, b);
        }
    }

    /// Saves the input ring and runs `codes` as the input, without output until a rule turns it on.
    fn start_expansion(&mut self, codes: Vec<u8>) {
        self.saved_ring = self.ring;
        self.saved_pos = self.pos;
        self.exp = 2;
        self.pending = -2;
        self.exp_str = codes;
        self.exp_i = 0;
    }

    fn style_action(&mut self, n: i64) {
        if !(0..=1).contains(&n) || self.exp > 0 {
            match n {
                2 if self.exp > 0 => self.exp = 1,
                3 if self.exp > 0 => self.exp = 2,
                4 => {
                    if self.exp < 1 {
                        if self.cur(self.mark) == 0x1c {
                            loop {
                                let c = self.rotate(self.mark);
                                self.put(c);
                                if c == 0x1f || c == 0x60 || c == 0x20 || c == EOF {
                                    break;
                                }
                            }
                        }
                    } else {
                        self.pending = self.mark;
                    }
                }
                5 if self.exp < 1 => self.exp = -1,
                6 if self.exp < 1 => self.exp = 0,
                _ => {}
            }
            return;
        }
        let name = self.name[..self.name_len.max(0) as usize].to_vec();
        if let Some(codes) = self.style.and_then(|s| s.lookup(&name, n)) {
            self.start_expansion(codes);
            for i in 0..64 {
                self.ring[i] = self.getc();
            }
            self.pos = 0;
        }
    }

    /// Takes the character `n` places ahead out of the ring, closing the gap, and reads one more.
    fn rotate(&mut self, n: i64) -> i64 {
        let new = self.getc();
        let mut u = ring_index(self.pos, n);
        let ret = self.ring[u];
        if n == 0 {
            self.ring[self.pos] = new;
            self.pos = (self.pos + 1) & 63;
        } else {
            let mut l8;
            loop {
                l8 = u;
                u = (l8 + 63) & 63;
                self.ring[l8] = self.ring[u];
                if u == self.pos {
                    break;
                }
            }
            self.ring[self.pos] = new;
            self.pos = l8;
        }
        ret
    }

    /// A 0x17 pair: written as it is, whatever the output state.
    fn pair17(&mut self) {
        let c = self.cur(0);
        self.emit(c);
        self.adv();
        let c = self.cur(0);
        self.emit(c);
        self.adv();
    }

    fn is_style_code_here(&self) -> bool {
        self.cur(0) == 0x1c && self.cur(1) == 0x65 && (self.cur(2) == 0x73 || self.cur(2) == 0x65) && self.cur(3) == 0x7e
    }

    /// Takes `count` matched characters. A style code met after the first is expanded and its codes count towards
    /// the number; when the count ends inside them, the rest of them become the input.
    fn consume(&mut self, count: i64) {
        let mut k = 0;
        while k < count {
            if self.raw17 == 0 {
                while self.cur(0) == 0x17 {
                    self.pair17();
                }
                loop {
                    if !(k >= 1 && self.exp < 1 && self.is_style_code_here()) {
                        break;
                    }
                    let which = if self.cur(2) != 0x73 { 1 } else { 0 };
                    for _ in 0..4 {
                        self.adv();
                    }
                    let mut name = Vec::new();
                    while !matches!(self.cur(0), 0x1f | 0x20 | 0x60) && name.len() < 0x1f {
                        name.push(self.cur(0) as u8);
                        self.adv();
                    }
                    self.adv();
                    let Some(codes) = self.style.and_then(|s| s.lookup(&name, which)) else { continue };
                    let len = codes.len() as i64;
                    if count < k + len {
                        let codes_copy = codes.clone();
                        self.start_expansion(codes);
                        // The ring takes the whole string at once.
                        self.exp_i = codes_copy.len();
                        for j in 0..64 {
                            self.ring[j] = codes_copy.get(j).map_or(EOF, |&b| b as i64);
                        }
                        self.pos = ((count - k) & 63) as usize;
                        for j in 0..self.pos {
                            self.ring[j] = EOF;
                        }
                        k = count;
                    } else {
                        k += len;
                    }
                    if k >= count {
                        break;
                    }
                }
                if k >= count {
                    break;
                }
            }
            self.adv();
            k += 1;
        }
    }

    /// The run loop. Returns 0 at the end of the input, or 0x15 when a rule asks for a table switch (the name
    /// collected by action 0x14 is in `name` / `name_len`). `fill_ring` reads the first 64 characters first.
    pub fn run(&mut self) -> i64 {
        for i in 0..64 {
            self.ring[i] = self.getc();
        }
        self.pos = 0;
        loop {
            let mut c = self.cur(0);
            if self.raw17 == 0 {
                while c == 0x17 {
                    self.pair17();
                    c = self.cur(0);
                }
            }
            while c == EOF {
                if self.exp < 1 {
                    return 0;
                }
                // The style's codes are used up: back to the saved input ring.
                self.exp = 0;
                self.ring = self.saved_ring;
                self.pos = self.saved_pos;
                c = self.cur(0);
                if self.raw17 == 0 {
                    while c == 0x17 {
                        self.pair17();
                        c = self.cur(0);
                    }
                }
                if self.pending >= 0 {
                    self.code_move(1, self.pending);
                    self.pending = -2;
                }
            }
            let mut k = if (0..256).contains(&c) { self.t.class_of[c as usize] } else { 0 };
            let rs;
            if k < 1 {
                c = self.default_char;
                k = if (0..256).contains(&c) { self.t.class_of[c as usize] } else { 0 };
                rs = if self.unknown_mode == 1 { item(&self.t.classes, k - 1).map_or(-1, |a| a.first_rule) } else { -1 };
            } else {
                rs = item(&self.t.classes, k - 1).map_or(-1, |a| a.first_rule);
            }
            let trans;
            if rs >= 1 && self.try_rules(rs) {
                self.consume(self.m_consume);
                let outb = std::mem::take(&mut self.m_out);
                let mut j = 0;
                while j < outb.len() {
                    let o = outb[j] as i64;
                    if o < 0x20 && self.raw17 == 0 {
                        if o == 0x15 {
                            return 0x15;
                        }
                        if matches!(o, 0x0c | 0x0d | 0x1b) && j + 1 < outb.len() {
                            self.action(o, &[outb[j + 1] as i64]);
                            j += 2;
                            continue;
                        }
                        if o == 0x16 && j + 2 < outb.len() {
                            self.action(o, &[outb[j + 1] as i64, outb[j + 2] as i64]);
                            j += 3;
                            continue;
                        }
                        if matches!(o, 0x0c | 0x0d | 0x16 | 0x1b) {
                            j += 1;
                            continue;
                        }
                        self.action(o, &[]);
                    } else {
                        self.put(o);
                    }
                    j += 1;
                }
                trans = item(&self.t.results, self.m_rid - 1).map_or(0, |r| r.1);
            } else if self.unknown_mode < 2 || rs >= 0 {
                self.adv();
                let class = item(&self.t.classes, k - 1).cloned();
                let o = class.as_ref().map_or(0, |a| a.default_out);
                if o < 0x20 && self.raw17 == 0 {
                    if o == 0x15 {
                        return 0x15;
                    }
                    if !matches!(o, 0x0c | 0x0d | 0x16 | 0x1b) {
                        self.action(o, &[]);
                    }
                } else {
                    self.put(o);
                }
                trans = class.map_or(0, |a| a.transition);
            } else {
                escape(self);
                let space = self.t.class_of[0x20];
                trans = item(&self.t.classes, space - 1).map_or(0, |a| a.transition);
            }
            self.transition(trans);
        }
    }
}

/// The escape for a character the table does not know: "@$u" (or "@$d" for a code with no Unicode form) and the
/// number written in braille digits or letters, as the table's unknown-character mode says. Takes the character
/// (both bytes of a two-byte code).
fn escape(e: &mut Engine) {
    let m = e.unknown_mode();
    let mut code = e.cur(0);
    let mut cur = code;
    while cur != EOF {
        e.adv();
        if flags(cur) & 0x20 == 0 {
            break;
        }
        cur = e.cur(0);
        code = code * 0x100 + cur;
    }
    if m != 2 {
        e.emit(0x20);
    }
    e.emit(if m == 4 { 0x3b } else { 0x40 });
    e.emit(0x24);
    if code == 0xf52a {
        for &ch in b"unsupported" {
            e.emit(ch as i64);
        }
    } else {
        let mut value = e.chitab.unicode_of(code);
        if value == 0x2a && code != 0x2a {
            e.emit(0x64);
            value = code & 0xffff;
        } else {
            e.emit(0x75);
        }
        if m == 4 {
            e.emit(0x2b);
        } else {
            if m < 4 {
                e.emit(0x22);
            }
            e.emit(0x36);
        }
        let mut unit: i64 = 1;
        while value / unit > 0xf {
            unit <<= 4;
        }
        let mut number = false;
        while unit != 0 {
            let d = value / unit;
            let di = d.clamp(0, 15) as usize;
            if m < 4 {
                if d < 10 {
                    if !number {
                        e.emit(0x23);
                        number = true;
                    }
                } else if number {
                    e.emit(0x3b);
                    number = false;
                }
                e.emit(b"jabcdefghiabcdef"[di] as i64);
            } else if m == 4 {
                e.emit(b"0123456789abcdef"[di] as i64);
            } else {
                e.emit(b"#*<%?:$]\\[abcdef"[di] as i64);
            }
            value -= unit * d;
            unit >>= 4;
        }
    }
    e.emit(0x20);
}
