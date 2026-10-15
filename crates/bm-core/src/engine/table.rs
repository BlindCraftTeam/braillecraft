//! Reading a rule table (.btb).
//!
//! Layout, numbers little-endian, two-byte numbers signed:
//! - a 40-byte title (text up to the first zero byte), 68 bytes not used here;
//! - the signature 0x1A47, the format version (10 to 20), and from version 18 the number of 64-bit words in each
//!   state mask;
//! - four counts: character classes, rule results, conditions, transitions;
//! - per character class: one byte, the class's character-set bits (16-bit before version 11, 32-bit before 13,
//!   then 64-bit), the transition used when no rule matches, the default output byte, the first rule to try;
//! - the class of each of up to 256 character codes;
//! - from version 12: the character sets the rules refer to (an index, a second index, and the bytes);
//! - the rules: the size of the rule pool, then per rule its offset into the pool, its link to the next rule to
//!   try, and its flags (before version 13 two flag bits sit lower and are moved up here); then the pool;
//! - per rule result: its condition and its transition;
//! - per condition and per transition: two state masks;
//! - the tail: the default character, whether 0x17 pairs are plain text, the writer's mode, the output mode, three
//!   values not used here, and what to do with a character the table does not know.

use super::EngineError;

/// One character class.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Class {
    /// The character-set bits of the class: bit n set means the class belongs to set n.
    pub sets: i128,
    /// The transition applied when no rule matches.
    pub transition: i64,
    /// What is written when no rule matches.
    pub default_out: i64,
    /// The first rule to try for this class (1-based; 0 or less: none).
    pub first_rule: i64,
}

/// One rule: where its bytes start in the pool, the link to the next rule, its flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rule {
    pub offset: i64,
    pub link: i64,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleTable {
    pub title: String,
    pub(crate) version: i64,
    pub(crate) width: usize,
    pub(crate) classes: Vec<Class>,
    pub(crate) class_of: [i64; 256],
    pub(crate) set_index: Vec<i64>,
    pub(crate) set_index2: Vec<i64>,
    pub(crate) set_pool: Vec<u8>,
    pub(crate) pool: Vec<u8>,
    pub(crate) rules: Vec<Rule>,
    /// Per rule result: its condition and its transition (both 1-based).
    pub(crate) results: Vec<(i64, i64)>,
    /// Per condition: the mask of state bits it looks at, and the values they must have.
    pub(crate) conditions: Vec<(Vec<i128>, Vec<i128>)>,
    /// Per transition: the mask of state bits it sets, and their new values (bits outside the mask are flipped
    /// where the second mask has them).
    pub(crate) transitions: Vec<(Vec<i128>, Vec<i128>)>,
    pub(crate) tail: [i64; 8],
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], EngineError> {
        let s = self.b.get(self.p..self.p + n).ok_or_else(|| EngineError::BadTable("the rule table ends early".into()))?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<i64, EngineError> {
        Ok(self.take(1)?[0] as i64)
    }
    fn i16(&mut self) -> Result<i64, EngineError> {
        let s = self.take(2)?;
        Ok(i16::from_le_bytes([s[0], s[1]]) as i64)
    }
    fn i32(&mut self) -> Result<i64, EngineError> {
        let s = self.take(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as i64)
    }
    fn u64(&mut self) -> Result<i128, EngineError> {
        let s = self.take(8)?;
        Ok(u64::from_le_bytes(s.try_into().unwrap()) as i128)
    }
}

impl RuleTable {
    pub fn read(data: &[u8]) -> Result<RuleTable, EngineError> {
        let mut r = Reader { b: data, p: 0 };
        let title_bytes = r.take(40)?;
        let end = title_bytes.iter().position(|&b| b == 0).unwrap_or(40);
        let title = super::chars::latin1(&title_bytes[..end]);
        r.take(68)?;
        let sig = r.i16()?;
        if sig != 0x1a47 {
            return Err(EngineError::BadTable(format!("not a rule table (signature {sig:x})")));
        }
        let v = r.i16()?;
        if !(10..=20).contains(&v) {
            return Err(EngineError::BadTable(format!("rule table format version {v} is not known")));
        }
        let width = if v < 18 { 1 } else { r.i16()?.max(0) as usize };
        let (na, nb, nc, nd) = (r.i16()?, r.i16()?, r.i16()?, r.i16()?);
        let mut classes = Vec::new();
        for _ in 0..na.max(0) {
            r.u8()?;
            let sets = if v < 11 {
                r.i16()? as i128
            } else if v < 13 {
                r.i32()? as i128
            } else {
                r.u64()?
            };
            let transition = r.i16()?;
            let default_out = r.u8()?;
            let first_rule = r.i16()?;
            classes.push(Class { sets, transition, default_out, first_rule });
        }
        let nclass = r.i16()?;
        if nclass > 256 {
            return Err(EngineError::BadTable("more than 256 character codes".into()));
        }
        let mut class_of = [0i64; 256];
        for slot in class_of.iter_mut().take(nclass.max(0) as usize) {
            *slot = r.i16()?;
        }
        let (mut set_index, mut set_index2, mut set_pool) = (Vec::new(), Vec::new(), Vec::new());
        if v >= 12 {
            let (pool, n2, n3) = (r.i16()?, r.i16()?, r.i16()?);
            for _ in 0..(n3 + 1).max(0) {
                set_index.push(r.i16()?);
            }
            for _ in 0..n2.max(0) {
                set_index2.push(r.i16()?);
            }
            set_pool = r.take(pool.max(0) as usize)?.to_vec();
        }
        let nrules = r.i16()?.max(0) as usize;
        let size = if v < 13 { r.i16()? } else { r.i32()? };
        let mut offsets = Vec::with_capacity(nrules);
        for _ in 0..nrules {
            offsets.push(if v < 13 { r.i16()? } else { r.i32()? });
        }
        let mut links = Vec::with_capacity(nrules);
        for _ in 0..nrules {
            links.push(r.i16()?);
        }
        let mut rules = Vec::with_capacity(nrules);
        for i in 0..nrules {
            let mut f = r.i16()?;
            if v < 13 {
                // Before version 13 the "no result byte" and "consume two" flags are bits 5 and 4.
                f = (f | if f & 0x20 != 0 { 0x8000 } else { 0 } | if f & 0x10 != 0 { 0x4000 } else { 0 }) & 0xffcf;
            }
            rules.push(Rule { offset: offsets[i], link: links[i], flags: (f & 0xffff_ffff) as u32 });
        }
        let pool = r.take(size.max(0) as usize)?.to_vec();
        let mut results = Vec::new();
        for _ in 0..nb.max(0) {
            results.push((r.i16()?, r.i16()?));
        }
        let mask = |r: &mut Reader| -> Result<Vec<i128>, EngineError> {
            Ok(if v < 11 {
                vec![r.i16()? as i128]
            } else if v < 13 {
                vec![r.i32()? as i128]
            } else if v < 18 {
                vec![r.u64()?]
            } else {
                (0..width).map(|_| r.u64()).collect::<Result<_, _>>()?
            })
        };
        let mut conditions = Vec::new();
        for _ in 0..nc.max(0) {
            conditions.push((mask(&mut r)?, mask(&mut r)?));
        }
        let mut transitions = Vec::new();
        for _ in 0..nd.max(0) {
            transitions.push((mask(&mut r)?, mask(&mut r)?));
        }
        let mut tail = [0i64; 8];
        tail[0] = r.u8()?;
        for t in tail.iter_mut().skip(1) {
            *t = r.i16()?;
        }
        Ok(RuleTable {
            title,
            version: v,
            width,
            classes,
            class_of,
            set_index,
            set_index2,
            set_pool,
            pool,
            rules,
            results,
            conditions,
            transitions,
            tail,
        })
    }

    /// The writer's mode: 2 means the table writes print (its input is braille).
    pub fn writer_mode(&self) -> i64 {
        self.tail[2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_files_that_are_not_rule_tables() {
        assert!(matches!(RuleTable::read(&[0u8; 20]), Err(EngineError::BadTable(_))));
        let mut d = vec![0u8; 112];
        d[108] = 0x47;
        d[109] = 0x1b;
        assert!(matches!(RuleTable::read(&d), Err(EngineError::BadTable(_))));
    }
}
