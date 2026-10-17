//! Prints what PDF import makes of a file, codes shown in brackets.
//!
//! Usage: pdf_import FILE.pdf [STYLE ...]
//!   STYLE  style names of the document the text goes into (they choose between long and short style names)
//! With PDF_TAGGED=0 in the environment a tagged PDF is read by its page layout, as when the PDF/A choice is off.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 2 {
        eprintln!("usage: pdf_import FILE.pdf [STYLE ...]");
        std::process::exit(2);
    }
    let bytes = std::fs::read(&a[1]).expect("read the file");
    let opts = bc_core::pdf_import::PdfOptions {
        allow_tagged: std::env::var("PDF_TAGGED").map_or(true, |v| v != "0"),
        ..Default::default()
    };
    match bc_core::pdf_import::import(&bytes, &opts, &a[2..]) {
        Ok(text) => println!("{}", text.replace('\u{1c}', "[").replace('\u{1f}', "]")),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
