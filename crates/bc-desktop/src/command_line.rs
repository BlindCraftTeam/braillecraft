//! The command line at start-up (re/spec/26-installer.md section 26.5): each
//! file given is opened; a file given after an option is then printed, embossed or saved as the option says, with
//! the dialog skipped, translated first (a print document for /braille, /simulated,
//! /brf and /ebrl; a braille document for /print and /prf), and closed. The document types the installer registers use
//! this for their Print and Emboss verbs. When every file came with an option, the program ends afterwards without
//! showing the editor. /RemoveEmbosserInfo, /RemovePrinterInfo and /RemovePageInfo write a document file again in
//! its own place; /dumpstyles and /dumptemplate write the document's styles to a file.
//!
//! The run's messages ("Cannot open file ...", a failed translation) are message boxes titled "Error", or lines on
//! the error output with /prompt:continue or /prompt:stop.

use std::path::Path;

use bc_core::cmdline::{CommandLine, Mode, Prompt};
use wxdragon::prelude::*;

use crate::app::{self, DocKind};
use crate::commands;
use crate::documents;

/// Reads the program's arguments.
pub fn read() -> CommandLine {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bc_core::cmdline::parse(&args, bc_core::cmdline::expand_wildcards)
}

/// A message of the run: a box titled "Error" with /prompt:prompt (the default), else a line on the error output.
fn say(prompt: Prompt, text: &str) {
    if prompt.shows_boxes() {
        commands::error_box(text);
    } else {
        eprintln!("{text}");
    }
}

/// The usage, when the first argument asks for it (/help, ?, /?...): a box titled "Note", or the error output with
/// /prompt:continue or /prompt:stop. The program then ends with nothing else done.
pub fn show_usage(cl: &CommandLine) {
    let program = std::env::args().next().unwrap_or_else(|| "braillecraft".into());
    let text = bc_core::cmdline::usage(&program);
    if cl.prompt.shows_boxes() {
        commands::note_box(&text);
    } else {
        eprintln!("{text}");
    }
}

/// Carries out the command line: the settings, then each file in order. Answers whether any file was opened in the
/// editor (a file with no option).
pub fn run(cl: &CommandLine) -> bool {
    for line in &cl.log {
        say(cl.prompt, line);
    }
    // A template that is not found leaves the default as it is, with nothing said.
    if let Some(name) = &cl.template {
        crate::translation::set_run_template(name);
    }
    if cl.reset_save_folder {
        commands::reset_file_folder(true);
    }
    if cl.reset_open_folder {
        commands::reset_file_folder(false);
    }
    let copies = cl.copies.unwrap_or(1);
    let mut opened = false;
    for action in &cl.actions {
        // A remove option rewrites the file itself: no document is opened in the program.
        if let Mode::RemoveInfo(what) = action.mode {
            remove_info(&action.file, what, cl.prompt);
            continue;
        }
        // A file with no option opens as File, Open opens it (the import dialog for a file that is not a document
        // file); a file with an option is imported with the kept import settings and no dialog.
        let Some(key) = commands::open_path_asking(&action.file, action.mode == Mode::Open) else { continue };
        note_memory("open");
        if action.mode == Mode::Open {
            opened = true;
            continue;
        }
        // A file given with an option and imported (not a document file) has the translated line off, whatever
        // Global, Default Views says.
        if !app::with(|a| a.index_of(key).map(|i| a.docs[i].document_file)).unwrap_or(true) {
            translated_line_off(key);
        }
        if carry_out(key, &action.mode, cl, copies) {
            opened = true;
        }
    }
    opened
}

/// /RemoveEmbosserInfo, /RemovePrinterInfo, /RemovePageInfo: the document file read, the part taken out and the file
/// written again in its place; nothing else in it changes. /pagesize is not applied to it (STILL UNKNOWN whether the
/// page size changes saved bytes in a remove run; BrailleCraft changes none). A file that is not a document file is
/// left as it is with "Cannot open file ..." (BrailleCraft's own choice: STILL UNKNOWN whether such a file is
/// imported and the document written over it).
fn remove_info(path: &Path, what: bc_core::cmdline::Remove, prompt: Prompt) {
    let doc = std::fs::read(path).ok().and_then(|b| bc_core::dxfile::read(&b).ok());
    let Some(mut doc) = doc else {
        say(prompt, &format!("Cannot open file {}", path.display()));
        return;
    };
    bc_core::dxfile::remove_info(&mut doc, what);
    if let Err(e) = std::fs::write(path, bc_core::dxfile::write(&doc)) {
        say(prompt, &format!("{}: {e}", path.display()));
    }
}

/// The translated line of the document with this key turned off.
fn translated_line_off(key: u64) {
    app::with(|a| {
        if let Some(i) = a.index_of(key) {
            a.docs[i].view.translated = false;
            a.docs[i].line.show(false);
        }
    });
}

/// Text written as a text file is on Windows: each line end CR LF.
fn text_file(bytes: Vec<u8>) -> Vec<u8> {
    if !cfg!(windows) {
        return bytes;
    }
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 16);
    for b in bytes {
        if b == b'\n' {
            out.push(b'\r');
        }
        out.push(b);
    }
    out
}

/// /dumpstyles (plain) and /dumptemplate (the XML dump, `xml`): the document's styles in its style-name list order.
/// A file that cannot be written is left with nothing said (STILL UNKNOWN whether a message is shown).
fn dump_styles(key: u64, path: &Path, xml: bool) {
    let styles = app::with(|a| {
        a.index_of(key).map(|i| {
            let d = &a.docs[i];
            let styles: Vec<(String, String, String)> = d.styles.iter().map(|s| (s.name.clone(), s.begin.clone(), s.end.clone())).collect();
            let kept = d.file_styles.len() == styles.len() && d.file_styles.iter().zip(&styles).all(|(f, s)| f.0 == s.0);
            bc_core::dxfile::name_list_order(&styles, kept)
        })
    });
    if let Some(styles) = styles {
        let _ = std::fs::write(path, text_file(bc_core::templates::style_dump(&styles, xml, false)));
    }
}

/// One file with a mode: translated first where its mode needs it, laid out with /pagesize, then printed,
/// embossed or saved; every document it made is closed afterwards. Answers whether a document was left open in the
/// editor (a failed translation with /prompt:prompt or /prompt:stop leaves the opened document open).
fn carry_out(key: u64, mode: &Mode, cl: &CommandLine, copies: u32) -> bool {
    let braille = app::with(|a| a.index_of(key).map(|i| a.docs[i].kind == DocKind::Braille)).unwrap_or(false);
    let translate = match mode {
        Mode::Braille(_) | Mode::Simulated | Mode::Brf(_) | Mode::Ebrl(_) => !braille,
        Mode::Print | Mode::Prf(_) => braille,
        _ => false,
    };
    let key = if translate {
        let made = match commands::translate_input(key) {
            Some(input) => {
                let made = commands::translate_from(input);
                note_memory("translation");
                made
            }
            None => Err(String::new()),
        };
        match made {
            Ok(k) => {
                // The opened document is closed once its translation is made; the translation stands in its place,
                // with its translated line off.
                documents::destroy(key);
                bc_core::memory::release();
                note_memory("close of the opened document");
                translated_line_off(k);
                k
            }
            Err(e) => {
                if !e.is_empty() {
                    say(cl.prompt, &e);
                }
                // The run goes on with the next file in every case; only /prompt:continue closes the opened document.
                let (close, _) = bc_core::cmdline::after_failed_translation(cl.prompt);
                if close {
                    documents::destroy(key);
                    bc_core::memory::release();
                }
                return !close;
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
            // document neither embossed nor printed.
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
        // eBraille with the document's metadata: no quality notice and no metadata dialog.
        Mode::Ebrl(path) => {
            crate::ebraille::write_package(key, path);
        }
        Mode::DumpStyles(path) => dump_styles(key, path, false),
        Mode::DumpStylesXml(path) => dump_styles(key, path, true),
        Mode::Open | Mode::RemoveInfo(_) => {}
    }
    note_memory("output");
    documents::destroy(key);
    false
}

/// The batch steps' memory notes (BRAILLECRAFT_MEMORY set; plan line L1).
fn note_memory(step: &str) {
    bc_core::memory::note(step);
}
