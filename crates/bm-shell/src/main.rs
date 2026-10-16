//! BrailleCraft desktop shell.
//!
//! The menu bar (menu.rs) follows the menu table item for item; each command is sent to the page (ui/) as a "menu"
//! event and carried out there. The page reports back the open documents (listed at the end of the View menu)
//! and check marks that change with the active document. Translation runs here, through liblouis (bm-louis).

mod menu;

use std::path::PathBuf;

use tauri::Manager;

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Print text to braille cells (Unicode braille, U+2800 to U+283F) with a liblouis table list.
#[tauri::command]
async fn translate(table: String, text: String) -> Result<String, String> {
    bm_louis::translate(&table, &text)
}

/// Braille cells (Unicode braille) back to print text with a liblouis table list.
#[tauri::command]
async fn back_translate(table: String, braille: String) -> Result<String, String> {
    bm_louis::back_translate(&table, &braille)
}

#[tauri::command]
fn louis_version() -> String {
    bm_louis::version()
}

/// The liblouis tables: bundled with the program as the resource folder liblouis/tables; when the program runs
/// from the source tree, the vendored copy.
fn table_folder(app: &tauri::App) -> Option<PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|d| d.join("liblouis").join("tables"));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bm-louis/vendor/liblouis/tables");
    [bundled, Some(source)].into_iter().flatten().find(|d| d.join("en-ueb-g2.ctb").is_file())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            match table_folder(app) {
                Some(dir) => bm_louis::set_table_folder(&dir)?,
                None => eprintln!("BrailleCraft: liblouis tables not found; translation will fail"),
            }
            menu::install(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            menu::set_check,
            menu::set_documents,
            exit_app,
            translate,
            back_translate,
            louis_version
        ])
        .run(tauri::generate_context!())
        .expect("BrailleCraft could not start");
}
