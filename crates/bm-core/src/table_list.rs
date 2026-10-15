//! The translation table sets by language (tables/languages.tsv): what Document, Translation Tables lists, by region,
//! and the learning steps Document, Learning Tables offers.
//!
//! Each line is one entry; its number (from 0, in file order) is the entry number. Columns, tab-separated: the shown
//! name; the regions (America, Europe, Asia, Africa, Pacific, Debug, comma-separated); the language code; the
//! print-to-braille rules and scan table; the braille-to-print rules and scan table; the learning steps as
//! TABLE=shown name separated by " | ". "#" starts a comment line.

use std::path::Path;

/// Region bits, in the order of Document, Translation Tables (Western Hemisphere, Europe, Asia, Africa, Pacific).
pub const REGION_NAMES: [&str; 6] = ["America", "Europe", "Asia", "Africa", "Pacific", "Debug"];
const REGION_BITS: [u32; 6] = [1, 2, 4, 8, 0x10, 0x80];
/// All regions (All Regions/All Languages).
pub const ALL_REGIONS: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The rules table that takes the place of the print-to-braille rules table.
    pub table: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSetEntry {
    pub name: String,
    pub regions: u32,
    pub code: String,
    /// Print-to-braille rules and scan table, braille-to-print rules and scan table.
    pub tables: [String; 4],
    pub steps: Vec<Step>,
}

/// A learning course: the steps whose names share a course name, in list order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Course {
    pub name: String,
    /// Indexes into the entry's steps.
    pub steps: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct TableList {
    pub entries: Vec<TableSetEntry>,
}

impl TableList {
    pub fn parse(text: &str) -> Result<TableList, String> {
        let mut entries = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                return Err(format!("languages.tsv line {}: {line}", n + 1));
            }
            let regions = f[1]
                .split(',')
                .filter_map(|r| REGION_NAMES.iter().position(|x| x.eq_ignore_ascii_case(r.trim())))
                .fold(0, |m, i| m | REGION_BITS[i]);
            let steps = f
                .get(7)
                .map(|s| {
                    s.split(" | ")
                        .filter_map(|p| p.split_once('='))
                        .map(|(t, name)| Step { table: t.trim().to_lowercase(), name: name.trim().to_string() })
                        .collect()
                })
                .unwrap_or_default();
            entries.push(TableSetEntry {
                name: f[0].to_string(),
                regions,
                code: f[2].to_string(),
                tables: [f[3], f[4], f[5], f[6]].map(|t| t.trim().to_lowercase()),
                steps,
            });
        }
        Ok(TableList { entries })
    }

    pub fn read(path: impl AsRef<Path>) -> Result<TableList, String> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        TableList::parse(&text)
    }

    /// The region bit of a Translation Tables region item, 0 to 4 (Western Hemisphere to Pacific).
    pub fn region_bit(item: usize) -> u32 {
        REGION_BITS.get(item).copied().unwrap_or(ALL_REGIONS)
    }

    /// The entries listed under Document, Translation Tables: those of the region in force, and any entry an open
    /// document uses (`in_use`), ordered by shown name (character by character); of entries with the same shown name
    /// only the first is listed.
    pub fn listed(&self, region: u32, in_use: impl Fn(usize) -> bool) -> Vec<usize> {
        let mut order: Vec<usize> = Vec::new();
        for i in 0..self.entries.len() {
            if !order.iter().any(|&j| self.entries[j].name == self.entries[i].name) {
                order.push(i);
            }
        }
        order.sort_by(|&a, &b| self.entries[a].name.cmp(&self.entries[b].name));
        order.into_iter().filter(|&i| self.entries[i].regions & region != 0 || in_use(i)).collect()
    }

    /// The entry a document's four tables belong to: first one whose print-to-braille rules table (or one of its
    /// learning steps) and the other three tables all match; else the first whose print-to-braille rules table (or a
    /// step) matches, whose other three tables the document then takes. Returns the entry and the step in use.
    pub fn entry_for(&self, tables: &[String; 4]) -> Option<(usize, Option<usize>)> {
        let lower = tables.clone().map(|t| t.to_lowercase());
        let p2b = |e: &TableSetEntry| -> Option<Option<usize>> {
            if e.tables[0] == lower[0] {
                Some(None)
            } else {
                e.steps.iter().position(|s| s.table == lower[0]).map(Some)
            }
        };
        let exact = self.entries.iter().enumerate().find_map(|(i, e)| {
            let step = p2b(e)?;
            (e.tables[1..] == lower[1..]).then_some((i, step))
        });
        exact.or_else(|| self.entries.iter().enumerate().find_map(|(i, e)| p2b(e).map(|s| (i, s))))
    }

    /// The learning courses of an entry, in list order: each step's course name (see `course_name`), steps with no
    /// course name left out, steps next to each other with the same course name making one course.
    pub fn courses(&self, entry: usize) -> Vec<Course> {
        let mut out: Vec<Course> = Vec::new();
        let mut last = String::new();
        if let Some(e) = self.entries.get(entry) {
            for (i, s) in e.steps.iter().enumerate() {
                let c = course_name(&s.name);
                if c.is_empty() {
                    last = c;
                    continue;
                }
                if c == last {
                    if let Some(course) = out.last_mut() {
                        course.steps.push(i);
                    }
                } else {
                    out.push(Course { name: c.clone(), steps: vec![i] });
                }
                last = c;
            }
        }
        out
    }
}

/// The course a learning step belongs to, from its shown name: with a "#", what comes before it once " Unit" is
/// taken out ("Mangold Method Unit #A.a" is "Mangold Method"); otherwise the name without its closing number (digits,
/// full stops and spaces at the end) and without " Lesson", " Cluster" and " Unit" ("TSBVI Cluster 7" is "TSBVI",
/// "NLS I.M.B.T. Lesson 4.2" is "NLS I.M.B.T."); "Grade" gives no course.
pub fn course_name(step: &str) -> String {
    if step.contains('#') {
        let s = step.replace(" Unit", "");
        return s.split('#').next().unwrap_or("").trim().to_string();
    }
    let s = step.trim_end_matches(|c: char| c == '.' || c == ' ' || c.is_ascii_digit());
    let s = s.replace(" Lesson", "").replace(" Cluster", "").replace(" Unit", "");
    if s == "Grade" {
        return String::new();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> TableList {
        TableList::read(format!("{}/../../tables/languages.tsv", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn reads_every_entry_of_the_shipped_list() {
        let list = shipped();
        assert_eq!(list.entries.len(), 188);
        let ueb = list.entries.iter().find(|e| e.code == "eng-xueb").unwrap();
        assert_eq!(ueb.tables, ["enuncp", "enudxp", "enuncb", "gendxb"].map(String::from));
        assert_eq!(ueb.regions, 0x1f);
    }

    #[test]
    fn regions_filter_and_order_the_list() {
        let list = shipped();
        let america = list.listed(TableList::region_bit(0), |_| false);
        let all = list.listed(ALL_REGIONS, |_| false);
        assert!(america.len() < all.len());
        assert!(america.iter().all(|&i| list.entries[i].regions & 1 != 0));
        let names: Vec<&str> = all.iter().map(|&i| list.entries[i].name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        // An entry in use is listed whatever the region.
        let pacific_only = list.listed(TableList::region_bit(4), |i| i == america[0]);
        assert!(pacific_only.contains(&america[0]));
    }

    #[test]
    fn a_documents_tables_find_their_entry() {
        let list = shipped();
        let ueb = ["enuncp", "enudxp", "enuncb", "gendxb"].map(String::from);
        let (i, step) = list.entry_for(&ueb).unwrap();
        assert_eq!((list.entries[i].code.as_str(), step), ("eng-xueb", None));
        let first_step = list.entries[i].steps[0].table.clone();
        let with_step = [first_step, "enudxp".into(), "enuncb".into(), "gendxb".into()];
        assert_eq!(list.entry_for(&with_step), Some((i, Some(0))));
        assert_eq!(list.entry_for(&["nosuch", "a", "b", "c"].map(String::from)), None);
    }

    #[test]
    fn learning_courses_group_their_steps() {
        assert_eq!(course_name("TSBVI Cluster 7"), "TSBVI");
        assert_eq!(course_name("NLS I.M.B.T. Lesson 4.2"), "NLS I.M.B.T.");
        assert_eq!(course_name("RNZFB STAR Level 1.10"), "RNZFB STAR Level");
        assert_eq!(course_name("Mangold Method Unit #A.a"), "Mangold Method");
        assert_eq!(course_name("Grade 2"), "");
        let list = shipped();
        let bana = list.entries.iter().position(|e| e.steps.iter().any(|s| s.name.starts_with("TSBVI"))).unwrap();
        let names: Vec<String> = list.courses(bana).into_iter().map(|c| c.name).collect();
        assert!(names.contains(&"TSBVI".to_string()), "{names:?}");
    }
}
