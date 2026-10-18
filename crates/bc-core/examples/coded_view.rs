//! Imports one file, makes its editor model in the coded view (View, Codes) and reports the time it took, the
//! sizes of what the view holds and the process's peak memory, for the large-document measurements (plan line
//! L1). Usage:
//!
//!   cargo run --release -p bc-core --example coded_view -- <file> [markdown|html|docx|text]

use bc_core::doctext::Shown;
use bc_core::editor::Editor;
use bc_core::format::Layout;
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: coded_view <file> [kind]");
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
    let tables = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tables");
    let bytes = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    });
    let got = import_chain::run(&bytes, &recipe, &Shipped(tables));
    let running = bc_core::internal::decode(&got.text);
    let shown = Shown::from_running(&running);
    drop(running);
    bc_core::memory::release();
    println!("file: {path} ({} bytes): {} characters, {} codes; before the view {} MB resident, {} MB peak", bytes.len(), shown.text.chars().count(), shown.codes.len(), bc_core::memory::resident_kb() / 1024, bc_core::memory::peak_resident_kb() / 1024);
    drop(bytes);

    let t = Instant::now();
    let ed = Editor::new(shown, Layout::print(80, 50), None, true, 65);
    let took = t.elapsed();
    let v = &ed.view;
    println!("coded view: {:.2} s, {} rows; {} MB resident, {} MB peak", took.as_secs_f64(), ed.rows(), bc_core::memory::resident_kb() / 1024, bc_core::memory::peak_resident_kb() / 1024);
    println!(
        "  view holds: text {} MB, lines {} MB, line bytes {} MB, document places of the view {} MB, view places of the document {} MB, codes {} MB",
        v.text.capacity() / 1_000_000,
        v.lines.capacity() * std::mem::size_of::<(usize, usize, usize)>() / 1_000_000,
        v.line_bytes.capacity() * std::mem::size_of::<usize>() / 1_000_000,
        v.doc_of_view.capacity() * std::mem::size_of::<u32>() / 1_000_000,
        v.view_of_doc.capacity() * std::mem::size_of::<u32>() / 1_000_000,
        v.codes.capacity() * std::mem::size_of::<(usize, usize)>() / 1_000_000
    );
    dump(v);
    let t = Instant::now();
    let last = ed.rows().saturating_sub(1);
    let text = ed.row_text(last);
    println!("last row ({last}) in {:.4} s: {:?}", t.elapsed().as_secs_f64(), text.chars().take(60).collect::<String>());
}

/// With BC_VIEW_DUMP set to a path, the view's text and a digest of its maps go there, so two builds of the view
/// can be compared on the same file.
#[allow(dead_code)]
fn dump(v: &bc_core::fview::View) {
    let Some(path) = std::env::var_os("BC_VIEW_DUMP") else { return };
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for &x in &v.doc_of_view {
        (x as u64).hash(&mut h);
    }
    let dov = h.finish();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for &x in &v.view_of_doc {
        (x as u64).hash(&mut h);
    }
    let vod = h.finish();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.codes.hash(&mut h);
    v.lines.hash(&mut h);
    let rest = h.finish();
    let head = format!("len {} lines {} codes {} doc_of_view {dov:x} view_of_doc {vod:x} codes+lines {rest:x}\n", v.len, v.lines.len(), v.codes.len());
    std::fs::write(path, head + &v.text).expect("dump");
}
