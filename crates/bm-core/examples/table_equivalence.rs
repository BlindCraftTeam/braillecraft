//! Compares BrailleCraft's own table sources with the binary tables they were recreated from, line by line over a
//! corpus, and reports every line that translates differently.
//!
//! Usage: table_equivalence FOLDER RULES SCAN SOURCE.bmt CORPUS
//!   FOLDER   folder holding the binary rule and scan tables and chitab.txt
//!   RULES    the binary rule table's file name, SCAN the scan table's
//!   SOURCE   the .bmt table source
//!   CORPUS   a text file, one test line per line

use bm_core::engine::Translator;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 {
        eprintln!("usage: table_equivalence FOLDER RULES SCAN SOURCE.bmt CORPUS");
        std::process::exit(2);
    }
    let (folder, rules, scan, source, corpus) = (&a[1], &a[2], &a[3], &a[4], &a[5]);
    let reference = Translator::open(folder, rules, scan).expect("binary tables");
    let text = std::fs::read_to_string(source).expect("table source");
    // Ours: the table source with BrailleCraft's own character table; the reference: the binary tables with theirs.
    let ours = Translator::from_source(&text).unwrap_or_else(|e| panic!("{e}"));
    let lines = std::fs::read_to_string(corpus).expect("corpus");
    // EQ_SIDE=reference or EQ_SIDE=ours translates with one side only (to find which side a slow line belongs to).
    let side = std::env::var("EQ_SIDE").unwrap_or_default();
    let (mut total, mut differ) = (0usize, 0usize);
    for (n, line) in lines.lines().enumerate() {
        total += 1;
        let r = if side == "ours" { String::new() } else { format!("{:?}", reference.translate(line)) };
        let o = if side == "reference" { String::new() } else { format!("{:?}", ours.translate(line)) };
        if !side.is_empty() {
            continue;
        }
        if r != o {
            differ += 1;
            if differ <= 20 {
                println!("line {}: {:?}\n  reference: {r}\n  ours:      {o}", n + 1, line);
            }
        }
    }
    println!("{source}: {differ} of {total} lines differ");
}
