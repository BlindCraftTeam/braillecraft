//! Compares BrailleMaker's own table sources with the binary tables they were recreated from, line by line over a
//! corpus, and reports every line that translates differently.
//!
//! Usage: table_equivalence FOLDER RULES SCAN SOURCE.bmt CORPUS
//!   FOLDER   folder holding the binary rule and scan tables and chitab.txt
//!   RULES    the binary rule table's file name, SCAN the scan table's
//!   SOURCE   the .bmt table source
//!   CORPUS   a text file, one test line per line

use bm_core::engine::{Chitab, Translator};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 6 {
        eprintln!("usage: table_equivalence FOLDER RULES SCAN SOURCE.bmt CORPUS");
        std::process::exit(2);
    }
    let (folder, rules, scan, source, corpus) = (&a[1], &a[2], &a[3], &a[4], &a[5]);
    let reference = Translator::open(folder, rules, scan).expect("binary tables");
    let text = std::fs::read_to_string(source).expect("table source");
    let ours = Translator::from_source(&text)
        .unwrap_or_else(|e| panic!("{e}"))
        .with_chitab(Chitab::read_folder(Path::new(folder)));
    let lines = std::fs::read_to_string(corpus).expect("corpus");
    let (mut total, mut differ) = (0usize, 0usize);
    for (n, line) in lines.lines().enumerate() {
        total += 1;
        let r = format!("{:?}", reference.translate(line));
        let o = format!("{:?}", ours.translate(line));
        if r != o {
            differ += 1;
            if differ <= 20 {
                println!("line {}: {:?}\n  reference: {r}\n  ours:      {o}", n + 1, line);
            }
        }
    }
    println!("{source}: {differ} of {total} lines differ");
}
