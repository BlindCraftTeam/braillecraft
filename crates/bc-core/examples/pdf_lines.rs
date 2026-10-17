//! Prints the text lines of one page of a PDF after layout analysis (box and text), for checks against the
//! reference's layout. Usage: pdf_lines FILE.pdf PAGE (0-based)

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: pdf_lines FILE.pdf PAGE");
        std::process::exit(2);
    }
    let bytes = std::fs::read(&a[1]).expect("read the file");
    let page: usize = a[2].parse().expect("page number");
    match bc_core::pdf_import::page_lines(&bytes, &Default::default(), page) {
        Ok(lines) => {
            for (r, t) in lines {
                let t: String = t.chars().take(50).collect();
                println!("{:?} {:?} {:?} {:?} {:?}", r.x0, r.y0, r.x1, r.y1, t);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
