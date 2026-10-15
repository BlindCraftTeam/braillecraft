//! BrailleMaker, the desktop program: a native main window holding the document windows, the menu bar, the status
//! line and native dialogs (wxWidgets through wxDragon). Translation runs through the BrailleMaker engine with the
//! tables shipped beside the program (translation.rs).

mod app;
mod codes;
mod commands;
mod dialog_table;
mod dlg;
mod documents;
mod encodings;
#[cfg(windows)]
mod jaws_install;
mod menu;
mod menu_table;
mod settings_store;
mod speech;
mod translation;

use std::path::{Path, PathBuf};

use wxdragon::prelude::*;
use wxdragon::widgets::statusbar::FieldStyle;

/// A resource folder: next to the program (installed), in the macOS bundle's Resources, or in the source tree.
fn resource_dir(installed: &str, source: &str, marker: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    [
        dir.join(installed),
        dir.join("../Resources").join(installed),
        dir.join("../share/braillemaker").join(installed),
        Path::new(env!("CARGO_MANIFEST_DIR")).join(source),
    ]
    .into_iter()
    .find(|d| d.join(marker).is_file())
}

/// The braille fonts (ui/fonts) are registered for this program only; they need no installing.
fn load_fonts() -> usize {
    let Some(dir) = resource_dir("fonts", "../../ui/fonts", "encodings.json") else {
        eprintln!("BrailleMaker: braille fonts not found");
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
                eprintln!("BrailleMaker: could not load {}", p.display());
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

/// Windows and macOS: wxWidgets registers the font for this process.
#[cfg(not(target_os = "linux"))]
fn add_font(path: &Path) -> bool {
    Font::add_private_font(&path.to_string_lossy())
}

/// The status line font: bold, from Global, Fonts.
pub fn set_status_font(a: &app::App) {
    let f = &a.settings.fonts;
    let mut builder = Font::builder().with_point_size(f.status_size).with_weight(FontWeight::Bold);
    if !f.status_face.is_empty() {
        builder = builder.with_face_name(&f.status_face);
    }
    if let Some(font) = builder.build() {
        a.status.set_font(&font);
    }
}

fn main() {
    // Linux: before GTK starts (see add_font). Elsewhere: once wxWidgets is running.
    let early = if cfg!(target_os = "linux") { Some(load_fonts()) } else { None };
    let _ = wxdragon::main(move |_| {
        let fonts = early.unwrap_or_else(load_fonts);
        match resource_dir("tables", "../../tables", "index.tsv") {
            Some(dir) => {
                if let Err(e) = translation::load(&dir) {
                    eprintln!("BrailleMaker: {e}");
                }
            }
            None => eprintln!("BrailleMaker: translation tables not found; translation will fail"),
        }
        if let Err(e) = dlg::load_resources() {
            eprintln!("BrailleMaker: {e}");
        }
        // The settings kept from earlier runs, before the menus are built from them.
        let mut settings = app::Settings::default();
        settings_store::load(&mut settings);
        if let Some(region) = settings_store::load_region() {
            translation::set_region(region);
        }

        let frame = MDIParentFrame::builder().with_title("BrailleMaker").with_size(Size::new(1000, 720)).build();
        let menus = menu::install(&frame);
        let status = frame.create_status_bar(4, menu_status_style(), -1, "statusBar");
        status.set_status_widths(&[260, 110, 110, -1]);
        status.set_status_styles(&[FieldStyle::Sunken, FieldStyle::Sunken, FieldStyle::Sunken, FieldStyle::Sunken]);

        app::set(app::App {
            frame,
            status,
            menus,
            docs: Vec::new(),
            active: None,
            next_key: 0,
            untitled: [0, 0],
            settings,
            memory: dlg::Memory::default(),
            last_find: None,
            busy: false,
        });
        app::with(|a| set_status_font(a));
        // Windows: the JAWS script set shipped beside the program is offered for installing first.
        #[cfg(windows)]
        if let Some(dir) = resource_dir("jaws", "../../scripts/jaws", "braillemaker-desktop.jss") {
            commands::jaws_start_up(&dir);
        }

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

        let key = documents::new_document(app::DocKind::Print, None, "");
        app::with(|a| {
            if let Some(i) = a.index_of(key) {
                a.docs[i].dirty = false;
            }
        });
        frame.show(true);
        if fonts == 0 {
            commands::message("The braille fonts could not be loaded; Braille Font and SimBraille Font show print characters.");
        }
    });
}

fn menu_status_style() -> i64 {
    wxdragon::widgets::statusbar::STB_DEFAULT_STYLE
}
