//! BrailleCraft, the desktop program: a native main window holding the documents as tabs (on macOS each document a
//! window of its own, the windows tabs of one window; tabs.rs), the menu bar, the status line and native dialogs
//! (wxWidgets through wxDragon). Translation runs through the BrailleCraft engine with the
//! tables shipped beside the program (translation.rs).

mod app;
mod autosave;
mod bell;
mod codes;
mod command_line;
mod commands;
mod control;
mod dialog_table;
mod dlg;
mod editor_view;
mod documents;
mod ebraille;
mod embosser;
mod encodings;
mod ftdi;
#[cfg(windows)]
mod jaws_install;
mod menu;
mod menu_table;
mod page_sample;
mod pdf_options;
mod python_options;
mod print;
mod recent;
mod settings_store;
mod speech;
mod tabs;
mod translation;

use std::path::{Path, PathBuf};

use wxdragon::prelude::*;

/// A resource folder: next to the program (installed), in the macOS bundle's Resources, or in the source tree.
fn resource_dir(installed: &str, source: &str, marker: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    [
        dir.join(installed),
        dir.join("../Resources").join(installed),
        dir.join("../share/braillecraft").join(installed),
        Path::new(env!("CARGO_MANIFEST_DIR")).join(source),
    ]
    .into_iter()
    .find(|d| d.join(marker).is_file())
}

/// The braille fonts (ui/fonts) are registered for this program only; they need no installing.
fn load_fonts() -> usize {
    let Some(dir) = resource_dir("fonts", "../../ui/fonts", "encodings.json") else {
        eprintln!("BrailleCraft: braille fonts not found");
        return 0;
    };
    let mut loaded = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ttf")).collect();
        paths.sort();
        for p in paths {
            if add_font(&p) {
                loaded += 1;
            } else {
                eprintln!("BrailleCraft: could not load {}", p.display());
            }
        }
    }
    loaded
}

/// On Linux the wxWidgets build has no private fonts (its CMake default for GTK), so the fonts are given to
/// fontconfig for this process, as wxWidgets itself does when that option is on. It must happen before GTK starts,
/// so that Pango's font map includes them.
#[cfg(target_os = "linux")]
fn add_font(path: &Path) -> bool {
    use std::ffi::{c_int, c_void, CString};
    #[link(name = "fontconfig")]
    extern "C" {
        fn FcConfigGetCurrent() -> *mut c_void;
        fn FcConfigAppFontAddFile(config: *mut c_void, file: *const u8) -> c_int;
    }
    let Ok(c) = CString::new(path.to_string_lossy().as_bytes()) else { return false };
    unsafe { FcConfigAppFontAddFile(FcConfigGetCurrent(), c.as_ptr() as *const u8) != 0 }
}

/// On macOS wxWidgets registers no private fonts: it only accepts a font already in the bundle's Resources/Fonts
/// folder named in Info.plist. The fonts are given to Core Text for this process instead, wherever they are (the
/// application bundle or the plain program folder).
#[cfg(target_os = "macos")]
fn add_font(path: &Path) -> bool {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFURLCreateFromFileSystemRepresentation(allocator: *const c_void, buffer: *const u8, len: isize, is_directory: u8) -> *const c_void;
        fn CFRelease(object: *const c_void);
    }
    #[link(name = "CoreText", kind = "framework")]
    extern "C" {
        fn CTFontManagerRegisterFontsForURL(url: *const c_void, scope: u32, error: *mut *const c_void) -> bool;
    }
    const PROCESS_SCOPE: u32 = 1;
    let bytes = path.as_os_str().as_bytes();
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize, 0);
        if url.is_null() {
            return false;
        }
        let mut error: *const c_void = std::ptr::null();
        let ok = CTFontManagerRegisterFontsForURL(url, PROCESS_SCOPE, &mut error);
        if !error.is_null() {
            CFRelease(error);
        }
        CFRelease(url);
        ok
    }
}

/// Windows: wxWidgets registers the font for this process.
#[cfg(windows)]
fn add_font(path: &Path) -> bool {
    Font::add_private_font(&path.to_string_lossy())
}

/// The status line font: bold, from Global, Fonts.
pub fn apply_status_font(a: &app::App, status: &StaticText) {
    let f = &a.settings.fonts;
    let mut builder = Font::builder().with_point_size(f.status_size).with_weight(FontWeight::Bold);
    if !f.status_face.is_empty() {
        builder = builder.with_face_name(&f.status_face);
    }
    if let Some(font) = builder.build() {
        status.set_font(&font);
    }
}

/// The status line font on every open document's status line.
pub fn set_status_font(a: &app::App) {
    for d in &a.docs {
        apply_status_font(a, &d.status);
        d.page.layout();
    }
}

/// On Linux the accessibility application name (what Orca names its script after) is the program name GLib takes
/// from the name the program was started by, so a start through the braillecraft symlink of the .desktop file would
/// give "braillecraft" and a direct start "braillecraft-desktop". The name is set here to braillecraft-desktop, the
/// executable's name on every system and the name of the JAWS scripts, the NVDA add-on and the Orca script; before
/// GTK starts and again once it has, since GTK sets the name only when none is set.
#[cfg(target_os = "linux")]
fn set_program_name() {
    use std::ffi::{c_char, CString};
    #[link(name = "glib-2.0")]
    extern "C" {
        fn g_set_prgname(name: *const c_char);
    }
    let name = CString::new("braillecraft-desktop").unwrap();
    unsafe { g_set_prgname(name.as_ptr()) }
}

#[cfg(not(target_os = "linux"))]
fn set_program_name() {}

fn main() {
    set_program_name();
    // Linux: before GTK starts (see add_font). Elsewhere: once wxWidgets is running.
    let early = if cfg!(target_os = "linux") { Some(load_fonts()) } else { None };
    let _ = wxdragon::main(move |wx_app| {
        set_program_name();
        let fonts = early.unwrap_or_else(load_fonts);
        match resource_dir("tables", "../../tables", "index.tsv") {
            Some(dir) => {
                if let Err(e) = translation::load(&dir) {
                    eprintln!("BrailleCraft: {e}");
                }
            }
            None => eprintln!("BrailleCraft: translation tables not found; translation will fail"),
        }
        match resource_dir("templates", "../../templates", "index.tsv") {
            Some(dir) => {
                if let Err(e) = translation::load_templates(&dir) {
                    eprintln!("BrailleCraft: {e}");
                }
            }
            None => eprintln!("BrailleCraft: document templates not found; styles will not be used"),
        }
        match resource_dir("embossers", "../../embossers", "embossers.txt") {
            Some(dir) => {
                if let Err(e) = embosser::load(&dir) {
                    eprintln!("BrailleCraft: {e}");
                }
            }
            None => eprintln!("BrailleCraft: embosser models not found; embossing will not be offered"),
        }
        if let Err(e) = dlg::load_resources() {
            eprintln!("BrailleCraft: {e}");
        }
        // The settings kept from earlier runs, before the menus are built from them.
        let mut settings = app::Settings::default();
        settings_store::load(&mut settings);
        // File, the recent documents the last run kept.
        recent::load();
        // Global, Internationalization, Support UI Automation, for the editing windows (spec 15).
        unsafe { wxdragon_sys::wxd_DocEditor_SetSupportUIAutomation(settings.reader.uia) };
        if let Some(region) = settings_store::load_region() {
            translation::set_region(region);
        }

        let frame = Frame::builder().with_title("BrailleCraft").with_size(Size::new(1000, 720)).build();
        let menus = menu::install(&frame);
        // The tab bar is the window's only child, so it fills the window; each document is a page of it. On macOS
        // the frame holds the menu bar and is never shown; each document is a window of its own, one of the window
        // tabs (tabs.rs), and the frame is the top window wxWidgets puts the menu bar up from.
        let tabs = if tabs::WINDOW_TABS {
            tabs::enable(&frame);
            wxdragon::app::set_top_window(&frame);
            tabs::Tabs::none()
        } else {
            tabs::create(&frame)
        };

        app::set(app::App {
            frame,
            tabs,
            last_frame: None,
            tab_bar_shown: false,
            menus,
            docs: Vec::new(),
            active: None,
            next_key: 0,
            untitled: [0, 0],
            settings,
            memory: dlg::Memory::default(),
            busy: false,
            known: Vec::new(),
        });
        // Windows: the JAWS script set shipped beside the program is offered for installing first, on every start,
        // before the command line is acted on (a command line that runs without the editor included).
        #[cfg(windows)]
        if let Some(dir) = resource_dir("jaws", "../../scripts/jaws", "braillecraft-desktop.jss") {
            commands::jaws_start_up(&dir);
        }
        // The command line is read now: --control PORT starts the control channel (control.rs) with the window,
        // as does the Global, MCP Server setting; the files it names are opened once the window is up.
        let cl = command_line::read();
        // /help, ? or /? as the first argument: the usage, and the program ends with no editor, no start-up dialog
        // and no file handled.
        if cl.help {
            command_line::show_usage(&cl);
            app::with(|a| a.frame.destroy());
            return;
        }
        control::start_at_launch(cl.control);

        frame.on_menu(|e| {
            let id = menu::command_id(e.get_id());
            let checked = e.is_checked().unwrap_or_else(|| app::with(|a| a.menus.is_checked(id)));
            commands::run(id, checked);
        });
        frame.on_close(|e| {
            if let WindowEventData::General(ev) = &e {
                if ev.can_veto() {
                    ev.veto();
                }
            }
            wxdragon::call_after(Box::new(commands::exit_program));
        });
        // macOS: the View menu's document list follows a tab dragged since it was last read.
        if tabs::WINDOW_TABS {
            frame.on_menu_opened(|_| {
                app::try_with(|a| {
                    a.reconcile_tabs();
                });
            });
        }

        // Global, Embosser Setup at start-up when no embosser is set up, once the main window is first active: a
        // dialog opened before that is stacked under the main window by the window manager, out of reach.
        // Layout check, the picture of a dialog as a command opens it (BC_LAYOUT_RUN=<command id>): that menu command
        // runs in place of the start-up Embosser Setup, so its dialog is filled as it is in use.
        let run_for_picture: Option<i32> = std::env::var("BC_LAYOUT_RUN").ok().and_then(|c| c.parse().ok());
        // The tab pictures (tabs::picture_run) open no start-up dialog, which would cover the tabs.
        let tabs_picture = std::env::var("BC_LAYOUT_TABS").ok();
        // A command line that runs without the editor (every file came with an option) opens no start-up dialog:
        // no one is there to answer it, and it held the /brf run on a computer with no embosser set up.
        let batch = cl.is_batch();
        if !batch && tabs_picture.is_none() {
            // Taken once, by the first window to become active (documents::run_start_hook).
            documents::set_start_hook(Box::new(move || match run_for_picture {
                Some(cmd) => wxdragon::call_after(Box::new(move || commands::run(cmd, false))),
                // Then the Autosave Recovery Wizard, when the last run left autosaved documents, and the autosave
                // timer (autosave.rs).
                None => wxdragon::call_after(Box::new(|| {
                    commands::embosser_setup_at_start();
                    autosave::start();
                })),
            }));
        }
        if run_for_picture.is_some() {
            // A program started from a build machine's shell is not always made the active one (macOS), so the
            // command also runs two seconds after start-up if the main window has not become active by then.
            let timer = wxdragon::timer::Timer::new(&frame);
            timer.on_tick(move |_| documents::run_start_hook());
            timer.start(2000, true);
            // Kept for the life of the program, which a picture run ends by being stopped.
            std::mem::forget(timer);
        }
        frame.on_activate(move |e| {
            if let WindowEventData::Activate(a) = &e {
                if a.is_active() {
                    documents::run_start_hook();
                }
                a.event.skip(true);
            }
        });

        // Layout check (dlg::layout_check), for the build checks: the dialogs measured as this system lays them out,
        // and the program ends; or one page of the embosser setup sheet left open for a picture.
        if let Ok(path) = std::env::var("BC_LAYOUT_CHECK") {
            dlg::layout_check(&path);
            // The check has nothing left to do once its report is written; the process ends here rather than
            // waiting for the toolkit to close (on Windows the CI step never saw it end).
            std::process::exit(0);
        }
        if let Some(page) = std::env::var("BC_LAYOUT_SHOW").ok().and_then(|p| p.parse().ok()) {
            dlg::layout_show(page);
            return;
        }
        if let Some(path) = tabs_picture {
            if !tabs::WINDOW_TABS {
                frame.show(true);
            }
            tabs::picture_run(path, std::env::var("BC_LAYOUT_TABS_STAGE").unwrap_or_default());
            return;
        }

        // macOS: documents opened from the Finder (double-click, Open With, a drop on the program icon) come as an
        // Apple event, not on the command line; each is opened in the editor.
        #[cfg(target_os = "macos")]
        wx_app.on_open_files(|files| {
            for f in files {
                commands::open_path(Path::new(&f));
            }
        });
        // macOS: the Dock icon clicked; with no document window open, a new document (documents::reopen).
        #[cfg(target_os = "macos")]
        wx_app.on_reopen_app(|| wxdragon::call_after(Box::new(documents::reopen)));
        #[cfg(not(target_os = "macos"))]
        let _ = wx_app;

        // The command line (spec 26 section 26.5): files given are opened, and printed, embossed or saved when an
        // option says so. When every file came with an option, the work is done without the editor and the program
        // ends.
        if cl.is_batch() {
            documents::hide_new_windows();
        }
        command_line::run(&cl);
        if cl.is_batch() {
            app::with(|a| {
                for d in a.docs.drain(..) {
                    d.page.destroy();
                    if let Some(w) = d.window {
                        w.destroy();
                    }
                }
                a.frame.destroy();
            });
            return;
        }
        // With no file opened the program starts with no document: File, New (Control-N) or File, Open makes one.
        // On macOS the program frame is not shown (the documents are windows of their own); with no document open
        // no window becomes active, so the start-up dialog is opened now.
        if tabs::WINDOW_TABS {
            if app::with(|a| a.docs.is_empty()) {
                wxdragon::call_after(Box::new(documents::run_start_hook));
            }
        } else {
            frame.show(true);
        }
        if fonts == 0 {
            commands::message("The braille fonts could not be loaded; Braille Font and SimBraille Font show print characters.");
        }
    });
}
