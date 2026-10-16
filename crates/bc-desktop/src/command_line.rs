//! The command line at start-up, as the reference program carries it out (re/spec/26-installer.md section 26.5): each
//! file given is opened; a file given after an option is then printed, embossed or saved as the option says, with
//! the dialog skipped, translated first where the reference translates (a print document for /braille, /simulated
//! and /brf; a braille document for /print and /prf), and closed. The document types the installer registers use
//! this for their Print and Emboss verbs. When every file came with an option, the program ends afterwards without
//! showing the editor.

use bc_core::cmdline::{CommandLine, Mode, Prompt};

use crate::app::{self, DocKind};
use crate::commands;
use crate::documents;

/// Reads the program's arguments.
pub fn read() -> CommandLine {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bc_core::cmdline::parse(&args, bc_core::cmdline::expand_wildcards)
}

/// Carries out the command line: the settings, then each file in order. Answers whether any file was opened in the
/// editor (a file with no option).
pub fn run(cl: &CommandLine) -> bool {
    for line in &cl.log {
        eprintln!("BrailleCraft: {line}");
    }
    if let Some(name) = &cl.template {
        if !crate::translation::set_run_template(name) {
            eprintln!("BrailleCraft: Ignoring command line parameter: /template:{name}");
        }
    }
    if cl.reset_save_folder || cl.reset_open_folder {
        // BrailleCraft's file dialogs start in the system's own folder each time; there is no kept folder to reset.
        eprintln!("BrailleCraft: no file folder is kept, so there is nothing to reset");
    }
    let copies = cl.copies.unwrap_or(1);
    let mut opened = false;
    for action in &cl.actions {
        let Some(key) = commands::open_path(&action.file) else { continue };
        if action.mode == Mode::Open {
            opened = true;
            continue;
        }
        if let Mode::NotAvailable(option) = &action.mode {
            eprintln!("BrailleCraft: {option} is not available in BrailleCraft; {} was left as it is", action.file.display());
            documents::destroy(key);
            continue;
        }
        match carry_out(key, &action.mode, cl, copies) {
            Step::Next => {}
            Step::Stop => break,
        }
    }
    opened
}

enum Step {
    Next,
    Stop,
}

/// One file with a mode: translated first where the reference does, laid out with /pagesize, then printed,
/// embossed or saved; every document it made is closed afterwards.
fn carry_out(key: u64, mode: &Mode, cl: &CommandLine, copies: u32) -> Step {
    let braille = app::with(|a| a.index_of(key).map(|i| a.docs[i].kind == DocKind::Braille)).unwrap_or(false);
    let translate = match mode {
        Mode::Braille(_) | Mode::Simulated | Mode::Brf(_) => !braille,
        Mode::Print | Mode::Prf(_) => braille,
        _ => false,
    };
    let key = if translate {
        documents::activate_key(key);
        let made = commands::translate_active();
        // The opened document is closed either way; the translation stands in its place.
        documents::destroy(key);
        match made {
            Ok(k) => k,
            Err(e) => {
                return match cl.prompt {
                    Prompt::Continue => {
                        eprintln!("BrailleCraft: {e}");
                        Step::Next
                    }
                    Prompt::Stop => Step::Stop,
                    Prompt::Ask => {
                        commands::message(&e);
                        Step::Stop
                    }
                };
            }
        }
    } else {
        key
    };
    documents::activate_key(key);
    if let Some((rows, cols)) = cl.page_size {
        app::with(|a| {
            if let Some(d) = a.doc_mut() {
                d.lines = rows;
                d.cells = cols;
                d.pages = None;
            }
        });
    }
    match mode {
        Mode::Print | Mode::Simulated => commands::print_with(Some(copies)),
        Mode::Braille(setup) => {
            // A setup named on the command line is the document's for this job; one that is not kept leaves the
            // document neither embossed nor printed, as the reference does.
            let usable = match setup {
                Some(name) if crate::embosser::load_setup(name).is_some() => {
                    app::with(|a| {
                        if let Some(d) = a.doc_mut() {
                            d.embosser = name.clone();
                        }
                    });
                    true
                }
                Some(name) => {
                    eprintln!("BrailleCraft: no embosser setup is named {name}");
                    false
                }
                None => true,
            };
            if usable {
                commands::emboss_with(Some(copies));
            }
        }
        Mode::Prf(path) => {
            commands::write_formatted_for(key, path, false);
        }
        Mode::Brf(path) => {
            commands::write_formatted_for(key, path, true);
        }
        Mode::Lbf(path) => {
            commands::write_refreshable_for(key, path);
        }
        Mode::Open | Mode::NotAvailable(_) => {}
    }
    documents::destroy(key);
    Step::Next
}
