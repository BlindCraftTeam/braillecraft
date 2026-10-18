//! Imports a print file, translates it to braille with the English UEB table pair of the default template, and lays
//! the braille out into pages, reporting the time and peak memory of each stage, for the large-document
//! measurements (plan line L1). Usage:
//!
//!   cargo run --release -p bc-core --example translate_file -- <file> [markdown|html|docx|text]
//!
//! The tables, templates and rule scripts are read from the repository's folders.

use bc_core::doctext::Shown;
use bc_core::engine::TableSet;
use bc_core::format::{format, Layout};
use bc_core::import_chain::{self, Files, Recipe};
use bc_core::rule_script::RuleScript;
use std::path::PathBuf;
use std::time::Instant;

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

fn peak_rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
        .unwrap_or(0)
        / 1024
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: translate_file <file> [kind]");
        std::process::exit(2);
    };
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let kind = args.get(2).cloned().unwrap_or(match ext.as_str() {
        "md" | "markdown" => "markdown".into(),
        "htm" | "html" => "html".into(),
        "docx" => "docx".into(),
        _ => "text".into(),
    });
    let recipe = match kind.as_str() {
        "markdown" => Recipe::Markdown(bc_core::word::Options::default(), 0),
        "html" => Recipe::Html(bc_core::word::Options::default(), 0),
        "docx" => Recipe::Docx(bc_core::word::Options::default(), "default".into(), 0),
        _ => Recipe::FormattedText(0),
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let tables = root.join("tables");
    let bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    });
    println!("file: {path} ({} bytes), kind: {kind}; start {} MB", bytes.len(), peak_rss_mb());

    let t = Instant::now();
    let got = import_chain::run(&bytes, &recipe, &Shipped(tables.clone()));
    let running = bc_core::internal::decode(&got.text);
    println!("import: {:.2} s, {} characters, {} codes, peak {} MB{}", t.elapsed().as_secs_f64(), running.chars().count(), got.text.iter().filter(|&&b| b == 0x1c).count(), peak_rss_mb(), if got.errors.is_empty() { String::new() } else { format!(", errors {:?}", got.errors) });

    // The default template: its styles and its table pair.
    let templates = bc_core::templates::Templates::open(root.join("templates")).unwrap_or_else(|e| panic!("templates: {e}"));
    let entry = templates.by_name(bc_core::templates::BUILT_IN_DEFAULT).expect("default template");
    let text = templates.text(entry).expect("template text");
    let styles = bc_core::templates::styles(&text);
    let codes = bc_core::templates::tables(&text).expect("template tables");
    let set = TableSet::open(&tables).unwrap_or_else(|e| panic!("tables: {e:?}"));
    let tr = set.translator(&codes[0]).unwrap_or_else(|e| panic!("translator {}: {e:?}", codes[0]));

    let t = Instant::now();
    let shown = Shown::from_running(&running);
    println!("document model: {:.2} s, peak {} MB", t.elapsed().as_secs_f64(), peak_rss_mb());

    let t = Instant::now();
    let print_pages = format(&shown, &Layout::print(65, 25), Some(&styles));
    println!("print layout: {:.2} s, {} pages, peak {} MB", t.elapsed().as_secs_f64(), print_pages.len(), peak_rss_mb());
    drop(print_pages);

    let t = Instant::now();
    let braille = tr.translate_marked(&shown.to_running(), Some(&styles), None).unwrap_or_else(|e| panic!("translate: {e:?}"));
    println!("translate ({}): {:.2} s, {} characters, peak {} MB", codes[0], t.elapsed().as_secs_f64(), braille.chars().count(), peak_rss_mb());

    let t = Instant::now();
    let braille_shown = Shown::from_running(&braille);
    let pages = format(&braille_shown, &Layout::braille(40, 25), Some(&styles));
    println!("braille layout: {:.2} s, {} pages, peak {} MB", t.elapsed().as_secs_f64(), pages.len(), peak_rss_mb());
    println!("first braille line: {:?}", pages.first().and_then(|p| p.lines.first()).map(|l| l.text.clone()).unwrap_or_default());
}
