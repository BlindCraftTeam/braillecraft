//! Compares translation with BrailleCraft's own data (shipped tables, template and table list) against the binary
//! tables with the same template and table list, for text with style and language codes, line by line.
//!
//! Usage: template_equivalence FOLDER TEMPLATE_CODE CORPUS [DOCUMENTS]
//!   FOLDER         folder holding the binary rule and scan tables, chitab.txt and the template file CODE.dxt
//!   TEMPLATE_CODE  the template's short code (enuncp)
//!   CORPUS         a text file, one test line per line
//!   DOCUMENTS      optional folder of print document files (.dxp) whose lines are compared too

use std::path::Path;
use std::rc::Rc;

use bm_core::engine::{LanguageSource, Languages, StyleTable, TableSet, Translator};
use bm_core::table_list::TableList;
use bm_core::templates::{self, Templates};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: template_equivalence FOLDER TEMPLATE_CODE CORPUS [DOCUMENTS]");
        std::process::exit(2);
    }
    let (folder, code, corpus) = (&a[1], &a[2], &a[3]);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let list = TableList::read(repo.join("tables/languages.tsv")).expect("table list");
    let shipped = Templates::open(repo.join("templates")).expect("templates");
    let entry = shipped.entries.iter().find(|e| &e.code == code).expect("template code");
    let text = shipped.text(entry).expect("template");
    let tables = templates::tables(&text).expect("TranslationTables");

    // The reference: binary tables, the template file it came from, the table list over the binary tables.
    let reference = Translator::open(folder, &format!("{}.btb", tables[0]), &format!("{}.sct", tables[1]))
        .expect("binary tables")
        .with_styles(StyleTable::from_template(Path::new(folder).join(format!("{code}.dxt"))).expect("template file"))
        .with_languages(Languages::from_list(&list, LanguageSource::Folder(folder.into())));
    // Ours: the shipped tables, the shipped template and the table list over the shipped tables.
    let set = Rc::new(TableSet::open(repo.join("tables")).expect("table set"));
    let ours = set.translator(&tables[0]).expect("shipped table");
    let styles = templates::styles(&text);
    let languages = Languages::from_list(&list, LanguageSource::Shipped(set.clone()));

    let mut lines: Vec<String> = std::fs::read_to_string(corpus).expect("corpus").lines().map(String::from).collect();
    if let Some(dir) = a.get(4) {
        let mut names: Vec<_> = std::fs::read_dir(dir).expect("documents").flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names.into_iter().filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxp"))) {
            let doc = bm_core::dxfile::read(&std::fs::read(&p).unwrap()).expect("document");
            lines.extend(doc.text.lines().filter(|l| !l.trim().is_empty()).map(String::from));
        }
    }
    let (mut total, mut differ, mut failed) = (0usize, 0usize, 0usize);
    for (n, line) in lines.iter().enumerate() {
        total += 1;
        let r = reference.translate(line);
        let o = ours.translate_with(line, Some(&styles), Some(&languages));
        match (&r, &o) {
            (Ok(r), Ok(o)) if r == o => {}
            (Err(_), Err(_)) => failed += 1,
            _ => {
                differ += 1;
                if differ <= 20 {
                    println!("line {}: {:?}\n  reference: {r:?}\n  ours:      {o:?}", n + 1, line);
                }
            }
        }
    }
    println!("{code}: {differ} of {total} lines differ ({failed} refused by both)");
}
