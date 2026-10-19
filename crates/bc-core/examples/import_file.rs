//! Imports one file through its recipe and reports the time it took, the size of the result and the process's peak
//! memory, for the large-document measurements (plan line L1). Usage:
//!
//!   cargo run --release -p bc-core --example import_file -- <file> [markdown|html|docx|brf|text|coded-text|coded-braille] [out]
//!
//! The kind defaults to the file's extension. The rule scripts, scan tables and character maps are read from the
//! repository's tables folder.

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

/// A field of the kernel's status file for this process, in kilobytes (Linux only; 0 elsewhere): VmHWM is the peak
/// resident memory, VmRSS the resident memory now.
fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with(field)).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

fn peak_rss_kb() -> u64 {
    status_kb("VmHWM:")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: import_file <file> [kind]");
        std::process::exit(2);
    };
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let kind = args.get(2).cloned().unwrap_or(match ext.as_str() {
        "md" | "markdown" => "markdown".into(),
        "htm" | "html" => "html".into(),
        "docx" => "docx".into(),
        "brf" => "brf".into(),
        "bru" => "coded-braille".into(),
        _ => "text".into(),
    });
    let recipe = match kind.as_str() {
        "markdown" => Recipe::Markdown(bc_core::word::Options::default(), 0),
        "html" => Recipe::Html(bc_core::word::Options::default(), 0),
        "docx" => Recipe::Docx(bc_core::word::Options::default(), "default".into(), 0),
        "brf" => Recipe::FormattedBraille(bc_core::braide::Settings::default()),
        "coded-braille" => Recipe::CodedBraille,
        "coded-text" => Recipe::CodedText("UTF-8".into(), 0),
        _ => Recipe::FormattedText(0),
    };
    let tables = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables");
    let bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    });
    let before = peak_rss_kb();
    let start = Instant::now();
    // Each step as it finishes: the text size after it, the memory in use and the peak so far.
    let mut last = Instant::now();
    let mut steps = |name: &str, len: usize| {
        println!(
            "  step {name:<24} {:>9} bytes  {:.2} s  live {} MB  peak {} MB",
            len,
            last.elapsed().as_secs_f64(),
            status_kb("VmRSS:") / 1024,
            status_kb("VmHWM:") / 1024
        );
        last = Instant::now();
    };
    let got = import_chain::run_with(&bytes, &recipe, &Shipped(tables), &mut steps);
    let took = start.elapsed();
    let text = bc_core::internal::decode(&got.text);
    println!("file: {path} ({} bytes), kind: {kind}", bytes.len());
    println!("time: {:.2} s", took.as_secs_f64());
    println!("result: {} internal bytes, {} characters, {} codes", got.text.len(), text.chars().count(), got.text.iter().filter(|&&b| b == 0x1c).count());
    println!("peak memory: {} MB (before the import {} MB)", peak_rss_kb() / 1024, before / 1024);
    if !got.errors.is_empty() {
        println!("errors: {}", got.errors.join("; "));
    }
    if got.warnings.long_page || got.warnings.wide_line {
        println!("warnings: long page {}, wide line {}", got.warnings.long_page, got.warnings.wide_line);
    }
    println!("first 300 characters: {:?}", text.chars().take(300).collect::<String>());
    // A third argument names a file the whole internal text is written to (to compare two builds' results).
    if let Some(out) = args.get(3) {
        std::fs::write(out, &got.text).unwrap_or_else(|e| eprintln!("{out}: {e}"));
    }
}
