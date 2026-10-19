//! Autosave and the Autosave Recovery Wizard (Global, File Locations: Enable autosave, the autosaved file location
//! and the time between saves; AutoSave Enabled, path and Time).
//!
//! At the end of start-up (after the command-line files and the start-up embosser check) the index the last run left
//! (bc_core::autosave) is read; when it names documents, the wizard (dialog 1061) offers them. Then a timer ticks
//! every second: while autosave is enabled and the time has gone by since the last pass (the first pass at the first
//! tick), a pass runs. A pass also runs after File Save, after Save As that leaves the document unchanged, and after
//! every close.
//!
//! A pass writes every changed document again, whether or not it changed since the last pass, into the file of the
//! lowest number no open document uses, then the index (autosave.new, renamed over autosave.txt), then takes away
//! each document's previous file. A document saved since has its file taken away. With no document open every file
//! the index names is taken away, and so it is at Exit; a run that ends any other way leaves them for the wizard.
//! A document closed without saving while others stay open leaves its file, which the index no longer names.
//!
//! Autosave and the wizard run only in the first copy of BrailleCraft started, so two copies never write the same
//! files.

use std::cell::Cell;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use bc_core::autosave::{self as files, Entry, Origin};
use wxdragon::prelude::*;

use crate::{app, commands, settings_store};

/// The autosaved file location until Global, File Locations sets one ("[MYDOCS]" is the Documents folder).
#[cfg(windows)]
pub const DEFAULT_FOLDER: &str = "[MYDOCS]\\BrailleCraft Autosaves";
#[cfg(not(windows))]
pub const DEFAULT_FOLDER: &str = "[MYDOCS]/BrailleCraft Autosaves";

/// Shown the first time in a run that a file cannot be written; autosave goes on trying (the setting is not changed).
const WRITE_FAILED: &str = "BrailleCraft encountered an error while writing autosave information.  Autosave will be disabled.";

thread_local! {
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static LAST: Cell<Option<Instant>> = const { Cell::new(None) };
    static FAILED: Cell<bool> = const { Cell::new(false) };
    static RUNNING: Cell<bool> = const { Cell::new(false) };
}

/// The autosaved file location.
pub fn folder() -> PathBuf {
    PathBuf::from(settings_store::get_folder("AutoSave", "path", DEFAULT_FOLDER))
}

/// The wizard when the last run left autosaved documents, then the timer. In a second copy of BrailleCraft, nothing.
pub fn start() {
    if !only_copy() || !recover() {
        return;
    }
    let timer = wxdragon::timer::Timer::new(&app::frame());
    timer.on_tick(|_| tick());
    timer.start(1000, false);
    // Kept for the life of the program.
    std::mem::forget(timer);
    STARTED.with(|s| s.set(true));
}

fn tick() {
    let enabled = settings_store::get("AutoSave", "Enabled").map_or(true, |v| v.trim().eq_ignore_ascii_case("yes"));
    if !enabled {
        return;
    }
    let seconds = settings_store::get("AutoSave", "Time").and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(300);
    if LAST.with(|l| l.get()).is_some_and(|t| t.elapsed().as_secs() < seconds) {
        return;
    }
    LAST.with(|l| l.set(Some(Instant::now())));
    pass();
}

/// One autosave pass, once the timer has started.
pub fn pass() {
    if !STARTED.with(|s| s.get()) || RUNNING.with(|r| r.replace(true)) {
        return;
    }
    run_pass();
    RUNNING.with(|r| r.set(false));
}

/// At Exit: every file the index names taken away.
pub fn at_exit() {
    if STARTED.with(|s| s.get()) {
        remove_all();
    }
}

fn write_failed() {
    if !FAILED.with(|f| f.replace(true)) {
        commands::message(WRITE_FAILED);
    }
}

/// What the index says of a document: its document file, the file it was imported from, or its untitled number.
fn origin(d: &app::Doc) -> Origin {
    match &d.path {
        Some(p) if d.document_file => Origin::Saved(p.to_string_lossy().into_owned()),
        Some(p) => Origin::Unsaved(p.to_string_lossy().into_owned()),
        None if d.untitled => Origin::Untitled {
            braille: d.kind == app::DocKind::Braille,
            number: d.name.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0),
        },
        None => Origin::Unsaved(d.name.clone()),
    }
}

fn run_pass() {
    let docs: Vec<(u64, bool)> = app::with(|a| a.docs.iter().map(|d| (d.key, d.dirty)).collect());
    if docs.is_empty() {
        remove_all();
        return;
    }
    let dir = folder();
    let new_index = dir.join(files::INDEX_NEW);
    let index = File::create(&new_index).or_else(|_| {
        make_folder(&dir);
        File::create(&new_index)
    });
    let Ok(mut index) = index else {
        write_failed();
        return;
    };
    let mut busy = None;
    let mut written = 0;
    for (key, changed) in docs {
        if !changed {
            // Saved since (or never changed): its file goes.
            let old = app::with(|a| a.index_of(key).map(|i| std::mem::replace(&mut a.docs[i].autosave[0], -1)));
            if let Some(n) = old.filter(|n| *n >= 0) {
                let _ = std::fs::remove_file(dir.join(files::file_name(n as u32)));
            }
            continue;
        }
        busy.get_or_insert_with(|| BusyCursor::new(None));
        // The current file becomes the previous one, and the document takes the lowest number no document uses.
        let Some((number, origin)) = app::with(|a| {
            let i = a.index_of(key)?;
            a.docs[i].autosave[1] = a.docs[i].autosave[0];
            let used: Vec<i32> = a.docs.iter().flat_map(|d| d.autosave).collect();
            let n = files::lowest_free(&used);
            a.docs[i].autosave[0] = n as i32;
            Some((n, origin(&a.docs[i])))
        }) else {
            continue;
        };
        let Some(bytes) = commands::document_bytes(key, 1) else { continue };
        if std::fs::write(dir.join(files::file_name(number)), bytes).is_err() {
            write_failed();
            continue;
        }
        written += 1;
        let _ = index.write_all(Entry { number, origin }.line().as_bytes());
    }
    drop(index);
    let txt = dir.join(files::INDEX);
    if written == 0 {
        let _ = std::fs::remove_file(&new_index);
        let _ = std::fs::remove_file(&txt);
        return;
    }
    let _ = std::fs::remove_file(&txt);
    let _ = std::fs::rename(&new_index, &txt);
    let previous: Vec<i32> = app::with(|a| a.docs.iter_mut().map(|d| std::mem::replace(&mut d.autosave[1], -1)).collect());
    for n in previous.into_iter().filter(|n| *n >= 0) {
        let _ = std::fs::remove_file(dir.join(files::file_name(n as u32)));
    }
}

/// Every autosave file the index names, and the index, taken away.
fn remove_all() {
    let dir = folder();
    let txt = dir.join(files::INDEX);
    let Ok(text) = std::fs::read(&txt) else { return };
    for line in String::from_utf8_lossy(&text).lines() {
        if let Some((n, rest)) = files::leading_number(line) {
            if rest.starts_with(';') && n >= 0 {
                let _ = std::fs::remove_file(dir.join(files::file_name(n as u32)));
            }
        }
    }
    let _ = std::fs::remove_file(&txt);
}

/// The autosave folder made, with the folders above it; on Windows a folder made here is hidden.
fn make_folder(dir: &Path) {
    let made = !dir.exists() && std::fs::create_dir_all(dir).is_ok();
    #[cfg(windows)]
    if made {
        use std::os::windows::ffi::OsStrExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 2;
        #[link(name = "kernel32")]
        extern "system" {
            fn GetFileAttributesW(name: *const u16) -> u32;
            fn SetFileAttributesW(name: *const u16, attributes: u32) -> i32;
        }
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let a = GetFileAttributesW(wide.as_ptr());
            if a != u32::MAX {
                SetFileAttributesW(wide.as_ptr(), a | FILE_ATTRIBUTE_HIDDEN);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = made;
}

/// Whether this is the first copy of BrailleCraft running: it holds a lock on a file in the temporary folder for
/// as long as it runs, which a second copy cannot take.
fn only_copy() -> bool {
    let path = std::env::temp_dir().join("braillecraft-desktop.lock");
    match lock(&path) {
        Some(file) => {
            std::mem::forget(file);
            true
        }
        None => false,
    }
}

#[cfg(windows)]
fn lock(path: &Path) -> Option<File> {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().create(true).truncate(false).write(true).share_mode(0).open(path).ok()
}

#[cfg(unix)]
fn lock(path: &Path) -> Option<File> {
    use std::os::unix::io::AsRawFd;
    extern "C" {
        fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    const LOCK_EX: std::ffi::c_int = 2;
    const LOCK_NB: std::ffi::c_int = 4;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).ok()?;
    (unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0).then_some(file)
}

// ------------------------------------------------------------------ the recovery wizard

/// A file's modification time as bc_core::autosave compares them; None when it cannot be read.
fn stamp(path: &Path) -> Option<u32> {
    let t = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    files::dos_stamp(t.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64)
}

/// The entries of the index the last run left, without the saved documents whose file is at least as new as their
/// autosave file; then the wizard. False when the program is to end (the wizard's Exit).
fn recover() -> bool {
    let dir = folder();
    let Ok(text) = std::fs::read(dir.join(files::INDEX)) else { return true };
    let entries: Vec<Entry> = files::parse_index(&String::from_utf8_lossy(&text))
        .into_iter()
        .filter(|e| match &e.origin {
            Origin::Saved(p) => files::keep_saved(stamp(Path::new(p)), stamp(&dir.join(files::file_name(e.number)))),
            _ => true,
        })
        .collect();
    entries.is_empty() || wizard(&dir, &entries)
}

/// An entry as the wizard lists it: the file name without its folder, or the untitled document's name.
fn label(e: &Entry) -> String {
    match &e.origin {
        Origin::Saved(p) | Origin::Unsaved(p) => file_part(p),
        Origin::Untitled { braille, number } => format!("Untitled {} Document {number}", if *braille { "Braille" } else { "Print" }),
    }
}

/// The name after the last "\" or "/" (the index names may come from either system).
fn file_part(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// Dialog 1061, the documents in the index's order: Open (4, the default) opens every one; Finish (5, and the close
/// box) asks twice and then takes the files away; Exit (6, and Escape) asks and ends the program, the files kept
/// for the next start. Answering No (or Cancel) to a question returns to the wizard.
fn wizard(dir: &Path, entries: &[Entry]) -> bool {
    let labels: Vec<String> = entries.iter().map(label).collect();
    let items: Vec<&str> = labels.iter().map(String::as_str).collect();
    loop {
        let d = commands::open_dialog(1061, None);
        d.set_items(8, &items, Some(0));
        for cid in [4, 5, 6] {
            d.closes_with(cid);
        }
        let dialog = d.dialog;
        dialog.set_escape_id(6);
        dialog.on_close(move |_| dialog.end_modal(5));
        let code = d.show_focused(8);
        d.destroy();
        match code {
            4 => {
                open_all(dir, entries);
                return true;
            }
            5 => {
                if commands::yes_no("You have not recovered all autosaved documents.  Are you sure you want to finish the BrailleCraft recovery wizard?")
                    && commands::yes_no("Finishing the recovery wizard before all documents have been processed is not reversible.  The remaining documents cannot later be recovered.  Proceed?")
                {
                    remove_all();
                    return true;
                }
            }
            _ => {
                if commands::ok_cancel("BrailleCraft will now exit.  The recovery wizard will be displayed again when BrailleCraft is next launched.") {
                    crate::recent::save();
                    commands::end_program();
                    return false;
                }
            }
        }
    }
}

/// Every autosaved document opened, each with what it was: its document file (Save writes there), the file it was
/// imported from (Save asks for a name), or its untitled number; each marked changed.
fn open_all(dir: &Path, entries: &[Entry]) {
    for e in entries {
        let Some(key) = commands::open_dx_document(&dir.join(files::file_name(e.number))) else { continue };
        app::with(|a| {
            if let Origin::Untitled { braille, number } = e.origin {
                // INFERENCE: the untitled count goes past the number, so File, New does not give the same name again.
                let n = &mut a.untitled[usize::from(braille)];
                *n = (*n).max(number);
            }
            let Some(i) = a.index_of(key) else { return };
            let d = &mut a.docs[i];
            match &e.origin {
                Origin::Saved(p) | Origin::Unsaved(p) => {
                    d.document_file = matches!(e.origin, Origin::Saved(_));
                    d.name = file_part(p);
                    d.path = Some(PathBuf::from(p)).filter(|p| p.parent().is_some_and(|f| !f.as_os_str().is_empty()));
                }
                Origin::Untitled { braille, number } => {
                    d.document_file = false;
                    d.path = None;
                    d.untitled = true;
                    d.saved = false;
                    d.name = format!("Untitled {} Document {number}", if *braille { "Braille" } else { "Print" });
                }
            }
            d.dirty = true;
            app::set_tab_title(&d.page, &d.full_name());
            a.set_title();
            a.refresh_menus();
        });
    }
}
