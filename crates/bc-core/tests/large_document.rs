//! A book-sized document through the import chain, the layout and the translation, with a bound on the memory each
//! pass may take (plan line L1: files can be very large textbooks). The document is made here (about 5 MB of
//! Markdown, some 1,300 pages of print), so no sample is kept in the repository. The bounds are multiples of the
//! text's size, well above what the passes take today and well below what they took before the passes were made
//! to stream (an import of 18 times the text, a layout of 60 times): they catch a pass that goes back to holding
//! the whole document several times over. Memory is read from the kernel, so the test runs on Linux and is
//! skipped elsewhere; it is ignored by default and run in CI on its own (`--ignored --test-threads=1`), since
//! other tests running beside it would add to the process's peak.

use bc_core::doctext::Shown;
use bc_core::format::{format, Layout};
use bc_core::import_chain::{self, Files, Recipe};
use bc_core::rule_script::RuleScript;
use std::path::PathBuf;

struct Shipped(PathBuf);

impl Files for Shipped {
    fn scan_table(&self, name: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.0.join("scan").join(name)).map_err(|e| e.to_string())
    }
    fn rule_script(&self, name: &str) -> Result<RuleScript, String> {
        RuleScript::open(&self.0.join("rule-scripts").join(name)).map_err(|e| e.to_string())
    }
    fn unimap_tables(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        import_chain::UNIMAP_TABLES
            .iter()
            .map(|n| std::fs::read(self.0.join("unimap").join(n)).map(|b| (n.to_string(), b)).map_err(|e| e.to_string()))
            .collect()
    }
    fn word_map(&self, name: &str) -> Result<String, String> {
        std::fs::read(self.0.join("word-styles").join(format!("{name}.map"))).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())
    }
}

/// A book of `chapters` chapters in Markdown: a heading, paragraphs of varied sentences, a list and a quotation per
/// chapter, with the characters, emphasis and numbers a textbook has.
fn book(chapters: usize) -> String {
    let words = ["the", "braille", "reader", "finds", "every", "contraction", "where", "it", "belongs,", "and", "a", "textbook", "of", "this", "size", "keeps", "its", "pages", "in", "order;", "numbers", "like", "1,250", "and", "3.14", "stay", "whole", "while", "quotes", "\"open\"", "and", "close."];
    let mut out = String::with_capacity(chapters * 4200);
    out.push_str("# A Book for the Measurements\n\n");
    for c in 1..=chapters {
        out.push_str(&format!("## Chapter {c}: Where the Pages Go\n\n"));
        for p in 0..12 {
            let mut sentence = 0;
            for w in 0..60 {
                let word = words[(c * 7 + p * 3 + w) % words.len()];
                if w > 0 {
                    out.push(' ');
                }
                if w % 17 == 0 && p % 2 == 0 {
                    out.push_str(&format!("*{word}*"));
                } else {
                    out.push_str(word);
                }
                if w % 15 == 14 {
                    out.push('.');
                    sentence += 1;
                }
            }
            out.push_str(&format!(" That was paragraph {p} with {sentence} sentences.\n\n"));
        }
        out.push_str("- the first point\n- the second point, somewhat longer than the first\n- the third\n\n> A quotation closes the chapter.\n\n");
    }
    out
}

/// The peak resident memory of this process in kilobytes (Linux), or None elsewhere.
fn peak_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
}

#[test]
#[ignore = "book-sized; run on its own: cargo test --release -p bc-core --test large_document -- --ignored --test-threads=1"]
fn a_book_is_imported_laid_out_and_translated_within_bounds() {
    let Some(before) = peak_kb() else {
        eprintln!("no kernel memory figures here; the bounds are checked on Linux");
        return;
    };
    let text = book(1200);
    let size_kb = (text.len() / 1024) as u64;
    assert!(size_kb > 4000, "the book is {size_kb} KB; it was meant to be about 5 MB");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = Shipped(root.join("tables"));

    let got = import_chain::run(text.as_bytes(), &Recipe::Markdown(bc_core::word::Options::default(), 0), &files);
    assert!(got.errors.is_empty(), "import errors: {:?}", got.errors);
    let running = bc_core::internal::decode(&got.text);
    drop(got);
    let after_import = peak_kb().unwrap();
    let import_grew = after_import.saturating_sub(before);
    eprintln!("book {size_kb} KB: the import took the peak up by {} KB ({:.1} times the text)", import_grew, import_grew as f64 / size_kb as f64);
    assert!(import_grew <= 15 * size_kb, "the import took {import_grew} KB of memory for a {size_kb} KB book: over 15 times the text");
    drop(text);

    let shown = Shown::from_running(&running);
    drop(running);
    let templates = bc_core::templates::Templates::open(root.join("templates")).expect("templates");
    let entry = templates.by_name(bc_core::templates::BUILT_IN_DEFAULT).expect("default template");
    let template_text = templates.text(entry).expect("template text");
    let styles = bc_core::templates::styles(&template_text);
    let codes = bc_core::templates::tables(&template_text).expect("template tables");
    let pages = format(&shown, &Layout::print(80, 50), Some(&styles));
    assert!(pages.len() > 1000, "the book laid out as {} pages; it was meant to be more than a thousand", pages.len());
    drop(pages);

    let set = bc_core::engine::TableSet::open(&root.join("tables")).expect("tables");
    let tr = set.translator(&codes[0]).expect("translator");
    let braille = tr.translate_marked(&shown.to_running(), Some(&styles), None).expect("translate");
    drop(shown);
    let braille_shown = Shown::from_running(&braille);
    drop(braille);
    let braille_pages = format(&braille_shown, &Layout::braille(40, 25), Some(&styles));
    assert!(braille_pages.len() > 1000, "the braille laid out as {} pages", braille_pages.len());
    let after_all = peak_kb().unwrap();
    let all_grew = after_all.saturating_sub(before);
    eprintln!("the whole pipeline took the peak up by {} KB ({:.1} times the text)", all_grew, all_grew as f64 / size_kb as f64);
    assert!(all_grew <= 50 * size_kb, "import, layout and translation took {all_grew} KB of memory for a {size_kb} KB book: over 50 times the text");
}
