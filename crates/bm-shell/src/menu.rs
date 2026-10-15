//! The menu bar: menus, items, order, access keys and shortcuts from the decoded menu resource and accelerator
//! table, so every command is where a braille transcriber expects it. Licensing and activation items are left
//! out. Every item sends its command id to the page as a "menu" event; the page carries out the command.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// One entry of the menu table.
pub enum E {
    /// Command: id, label, shortcut.
    I(&'static str, &'static str, Option<&'static str>),
    /// Check mark command: id, label, shortcut, checked at start.
    C(&'static str, &'static str, Option<&'static str>, bool),
    /// Edit command that has a native equivalent on Windows and macOS (clipboard and undo).
    N(Native, &'static str, &'static str),
    /// Separator.
    S,
    /// Submenu: label, entries.
    M(&'static str, &'static [E]),
}

#[derive(Clone, Copy)]
pub enum Native {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

const FILE: &[E] = &[
    E::I("file.new", "&New...", Some("CmdOrCtrl+N")),
    E::I("file.open", "&Open...", Some("CmdOrCtrl+O")),
    E::I("file.close", "&Close", Some("CmdOrCtrl+F4")),
    E::S,
    E::I("file.save", "&Save", Some("CmdOrCtrl+S")),
    E::I("file.saveAs", "S&ave as...", Some("F3")),
    E::I("file.saveEbraille", "Save eBraille", None),
    E::S,
    E::I("file.translate", "&Translate", Some("CmdOrCtrl+T")),
    E::S,
    E::I("file.print", "&Print...", Some("CmdOrCtrl+P")),
    E::I("file.emboss", "&Emboss...", Some("CmdOrCtrl+E")),
    E::S,
    E::I("file.exit", "E&xit", Some("Alt+F4")),
];

const EDIT: &[E] = &[
    E::N(Native::Undo, "edit.undo", "Undo"),
    E::N(Native::Redo, "edit.redo", "Redo"),
    E::N(Native::Cut, "edit.cut", "Cu&t"),
    E::N(Native::Copy, "edit.copy", "&Copy"),
    E::N(Native::Paste, "edit.paste", "&Paste"),
    E::I("edit.clear", "C&lear", None),
    E::N(Native::SelectAll, "edit.selectAll", "Select &All"),
    E::S,
    E::I("edit.highlight", "&Highlight", Some("CmdOrCtrl+H")),
    E::S,
    E::I("edit.changeCase", "Change Case", Some("Shift+F3")),
    E::S,
    E::I("edit.insertFile", "Insert File...", None),
    E::S,
    E::I("edit.goto", "&Goto...", Some("CmdOrCtrl+G")),
    E::I("edit.find", "&Find...", Some("CmdOrCtrl+F")),
    E::I("edit.findAgain", "Fi&nd Again", Some("CmdOrCtrl+I")),
    E::I("edit.replace", "&Replace...", Some("F6")),
    E::S,
    E::I("edit.spellCheck", "&Spell Check...", Some("Shift+F11")),
    E::I("edit.findMisspelling", "Find &Misspelling", Some("F11")),
];

const VIEW: &[E] = &[
    E::C("view.fullPage", "&Full-Page", None, false),
    E::S,
    E::C("view.printFont", "&Print Font", Some("CmdOrCtrl+F5"), true),
    E::C("view.brailleFont", "&Braille Font", Some("CmdOrCtrl+F6"), false),
    E::C("view.simBrailleFont", "&SimBraille Font", Some("CmdOrCtrl+F7"), false),
    E::S,
    E::C("view.sixKey", "Six-&Key Entry", Some("F2"), false),
    E::S,
    E::C("view.codes", "&Codes", Some("Alt+F3"), false),
    E::C("view.translatedLine", "&Translated Line", Some("CmdOrCtrl+F8"), false),
    // The open documents are appended after this separator (set_documents).
    E::S,
];

const TRANSLATION_CODES: &[E] = &[
    E::I("code.grade0", "Grade &0", Some("Alt+0")),
    E::I("code.grade1", "Grade &1", Some("Alt+1")),
    E::I("code.grade2", "Grade &2", Some("Alt+2")),
    E::I("code.cbc", "&CBC", Some("Alt+3")),
    E::I("code.spanishText", "&Spanish Text", Some("Alt+4")),
    E::I("code.lifgText", "&LIFG Text", Some("Alt+5")),
];

const CHARACTER_CODES: &[E] = &[
    E::I("code.letterSign", "&Letter Sign", None),
    E::I("code.terminationSign", "&Termination Sign", None),
    E::I("code.hardSpace", "&Hard Space", Some("Alt+F2")),
    E::I("code.group", "&Group", Some("CmdOrCtrl+B")),
];

const LINE_CODES: &[E] = &[
    E::I("code.newLine", "New &Line", Some("CmdOrCtrl+L")),
    E::I("code.skipLine", "S&kip Line", Some("CmdOrCtrl+K")),
    E::I("code.suppressSkippedLine", "Suppress Skipped Line", Some("Alt+K")),
    E::I("code.centering", "&Centering", Some("Shift+F6")),
    E::I("code.flushRight", "&Flush Right", Some("Shift+F7")),
    E::I("code.flushRightLeader", "Flush Right &w/Leader", Some("Alt+F7")),
    E::I("code.tabRunover", "Tab to &Runover Point", None),
    E::I("code.singleSpacing", "&Single Spacing", None),
    E::I("code.doubleSpacing", "&Double Spacing", Some("Shift+F2")),
    E::I("code.newParagraph", "New &Paragraph", Some("CmdOrCtrl+M")),
];

const LEFT_MARGIN: &[E] = &[
    E::I("code.leftMargin.1", "&1", None),
    E::I("code.leftMargin.2", "&2", None),
    E::I("code.leftMargin.3", "&3", Some("F4")),
    E::I("code.leftMargin.4", "&4", None),
    E::I("code.leftMargin.5", "&5", None),
    E::I("code.leftMargin.7", "&7", None),
    E::I("code.leftMargin.9", "&9", None),
    E::I("code.leftMargin.11", "11", None),
    E::I("code.leftMargin.13", "13", None),
];

const RIGHT_MARGIN: &[E] = &[
    E::I("code.rightMargin.0", "&0", None),
    E::I("code.rightMargin.1", "&1", None),
    E::I("code.rightMargin.2", "&2", None),
    E::I("code.rightMargin.4", "&4", None),
    E::I("code.rightMargin.6", "&6", Some("Shift+F4")),
];

const RUNOVER_OFFSET: &[E] = &[
    E::I("code.runover.0", "&0", None),
    E::I("code.runover.1", "&1", None),
    E::I("code.runover.2", "&2", Some("CmdOrCtrl+R")),
    E::I("code.runover.3", "&3", None),
    E::I("code.runover.4", "&4", None),
    E::I("code.runover.5", "&5", None),
    E::I("code.runover.6", "&6", None),
    E::I("code.runover.8", "&8", None),
    E::I("code.runover.10", "10", None),
];

const MATH_CODES: &[E] = &[
    E::I("code.math.technicalContext", "&Technical context", None),
    E::I("code.math.technicalNotation", "Technical &notation", None),
    E::S,
    E::I("code.math.base", "&Base (subscript)", None),
    E::I("code.math.power", "Po&wer (superscript)", None),
    E::I("code.math.directExpression", "Expression for &direct superscript/subscript", None),
    E::I("code.math.subscriptUnder", "S&ubscript (directly under)", None),
    E::I("code.math.superscriptOver", "&Superscript (directly over)", None),
    E::S,
    E::I("code.math.fraction", "&Fraction", None),
    E::I("code.math.squareRoot", "S&quare root", None),
    E::I("code.math.indexedRoot", "&Indexed root", None),
    E::S,
    E::I("code.math.grade1", "&Grade 1", None),
    E::I("code.math.grade1LetterSign", "Grade 1 with &letter sign", None),
    E::I("code.math.boldPassage", "Bold &verbal passage", None),
    E::I("code.math.italicPassage", "Italic verbal &passage", None),
    E::I("code.math.numericEquivalent", "Numeric equiv&alent", None),
    E::I("code.math.omitNumericIndicator", "&Omit numeric indicator", None),
    E::I("code.math.romanNumeral", "&Roman numeral", None),
    E::I("code.math.cancellation", "&Cancellation", None),
    E::S,
    E::I("code.math.horizontalBar", "&Horizontal bar", None),
    E::I("code.math.extendedVerticalBar", "E&xtended vertical bar", None),
    E::I("code.math.baseline", "Baseline / &multipurpose indicator", None),
    E::I("code.math.enclosedList", "&Enclosed list", None),
    E::I("code.math.newLineIndicated", "new line indicated only, e.g., in binomial", None),
];

const PAGE_CODES: &[E] = &[
    E::I("code.newPage", "New &Page", Some("CmdOrCtrl+Enter")),
    E::I("code.newReferencePage", "New &Reference Page", Some("Alt+Enter")),
    E::I("code.blockProtect", "&Block Protect", None),
    E::I("code.verticalCentering", "&Vertical Centering", None),
];

const PICTURE: &[E] = &[
    E::I("layout.picture.add", "&Add...", None),
    E::S,
    E::I("layout.picture.alignLeft", "Align &Left", None),
    E::I("layout.picture.alignCenter", "Align &Center", None),
    E::I("layout.picture.alignRight", "Align &Right", None),
    E::S,
    E::I("layout.picture.dots25", "2.5 mm dot resolution", None),
    E::I("layout.picture.dots20", "2.0 mm dot resolution", None),
    E::I("layout.picture.dots16", "1.6 mm dot resolution", None),
    E::I("layout.picture.dots13", "1.3 mm dot resolution", None),
    E::I("layout.picture.dots10", "1.0 mm dot resolution", None),
];

const LAYOUT: &[E] = &[
    E::I("layout.applyStyle", "Apply &Style...", Some("F8")),
    E::I("layout.applyLastStyle", "Apply Last Style", Some("Shift+F8")),
    E::I("layout.styleLevelUp", "Style Level +", Some("Alt+Right")),
    E::I("layout.styleLevelDown", "Style Level -", Some("Alt+Left")),
    E::S,
    E::I("layout.joinStyles", "&Join Styles", None),
    E::I("layout.joinLines", "J&oin Lines", None),
    E::S,
    E::M("&Translation Codes", TRANSLATION_CODES),
    E::M("&Character Codes", CHARACTER_CODES),
    E::M("&Line Codes", LINE_CODES),
    E::M("L&eft Margin", LEFT_MARGIN),
    E::M("R&ight Margin", RIGHT_MARGIN),
    E::M("&Runover Offset", RUNOVER_OFFSET),
    E::M("&Math Codes", MATH_CODES),
    E::M("Pa&ge Codes", PAGE_CODES),
    E::I("layout.setPageNumber", "Set Page &Number...", None),
    E::I("layout.header", "&Header...", None),
    E::I("layout.footer", "&Footer...", None),
    E::S,
    E::I("layout.generateToc", "Generate Ta&ble of Contents", None),
    E::S,
    E::M("&Picture", PICTURE),
    E::S,
    E::I("layout.codeList", "Co&de List...", Some("F5")),
];

const TABLE: &[E] = &[
    E::I("table.create", "Create...", None),
    E::I("table.properties", "Table Properties...", None),
];

const TRANSLATION_TABLES: &[E] = &[
    E::I("document.tables.western", "Western Hemisphere", None),
    E::I("document.tables.europe", "Europe", None),
    E::I("document.tables.asia", "Asia", None),
    E::I("document.tables.africa", "Africa", None),
    E::I("document.tables.pacific", "Pacific", None),
    E::I("document.tables.all", "All Regions/All Languages", None),
];

const LEARNING_TABLES: &[E] = &[
    E::I("document.learning.uncontracted", "&Uncontracted", None),
    E::I("document.learning.contracted", "&Fully Contracted", None),
];

const HYPHEN_TABLE: &[E] = &[
    E::C("document.hyphen.none", "&None", None, false),
    E::C("document.hyphen.dashes", "&Dashes", None, true),
    E::C("document.hyphen.dashesHyphens", "Dashes and &Hyphens", None, false),
    E::C("document.hyphen.french", "&French", None, false),
    E::C("document.hyphen.french113", "French (Original 11.3 Compatible)", None, false),
];

const DOCUMENT: &[E] = &[
    E::I("document.embosserSetup", "&Embosser Setup...", Some("CmdOrCtrl+F11")),
    E::I("document.printerSetup", "&Printer Setup...", Some("CmdOrCtrl+F12")),
    E::S,
    E::I("document.pageNumbering", "Page &Numbering...", None),
    E::M("&Translation Tables", TRANSLATION_TABLES),
    E::M("Lea&rning Tables", LEARNING_TABLES),
    E::M("&Hyphen Table", HYPHEN_TABLE),
    E::I("document.ebrailleMetadata", "e&Braille Metadata...", None),
    E::S,
    E::I("document.addStyle", "&Add Style...", None),
    E::I("document.modifyStyle", "&Modify Style...", None),
    E::I("document.deleteStyle", "&Delete Style...", None),
    E::S,
    E::I("document.createTemplate", "&Create Template...", None),
    E::I("document.deleteTemplate", "De&lete Template...", None),
];

const GLOBAL: &[E] = &[
    E::I("global.embosserSetup", "&Embosser Setup...", None),
    E::I("global.printerSetup", "&Printer Setup...", None),
    E::S,
    E::I("global.fileLocations", "File &Locations...", None),
    E::I("global.defaultViews", "&Default Views...", None),
    E::I("global.internationalization", "&Internationalization...", None),
    E::I("global.fonts", "&Fonts...", None),
    E::I("global.viewPreferences", "&View Preferences...", None),
    E::I("global.shortcutPreferences", "&Shortcut Preferences...", None),
    E::I("global.notificationSettings", "&Notification Settings...", None),
    E::S,
    E::I("global.importOptions", "Import &Options...", None),
    E::I("global.wordPerfectImporter", "&WordPerfect Importer...", None),
    E::I("global.wordImporter", "Wo&rd Importer...", None),
    E::I("global.pdfImporter", "PDF Importer...", None),
    E::I("global.formattedBrailleImporter", "Formatted &Braille Importer...", None),
];

const HELP: &[E] = &[
    E::I("help.topics", "&Help Topics...", None),
    E::I("help.translate", "&Translate Help...", None),
    E::I("help.favorites", "&Favorite Topics...", None),
    E::I("help.whatsNew", "&New in BrailleMaker 0.1", None),
    E::S,
    E::I("help.checkUpdates", "Check for &Updates...", None),
    E::I("help.about", "&About BrailleMaker...", None),
];

pub const MENUS: &[(&str, &[E])] = &[
    ("&File", FILE),
    ("&Edit", EDIT),
    ("&View", VIEW),
    ("&Layout", LAYOUT),
    ("&Table", TABLE),
    ("&Document", DOCUMENT),
    // The Macro menu is shown only when macros are defined (the main window shows
    // File Edit View Layout Table Document Global Help), so BrailleMaker leaves it out until macros exist.
    ("&Global", GLOBAL),
    ("&Help", HELP),
];

/// Groups of check items where exactly one is checked (shown as a choice).
const EXCLUSIVE: &[&[&str]] = &[
    &["view.printFont", "view.brailleFont", "view.simBrailleFont"],
    &[
        "document.hyphen.none",
        "document.hyphen.dashes",
        "document.hyphen.dashesHyphens",
        "document.hyphen.french",
        "document.hyphen.french113",
    ],
];

/// Live menu state: the check items by id, the View menu, and the open-document items at its end.
#[derive(Default)]
pub struct MenuState {
    checks: HashMap<String, CheckMenuItem<Wry>>,
    view: Option<Submenu<Wry>>,
    documents: Vec<CheckMenuItem<Wry>>,
}

#[derive(Clone, Serialize)]
struct MenuEvent {
    id: String,
    checked: Option<bool>,
}

fn native(app: &AppHandle, kind: Native, id: &str, label: &str) -> tauri::Result<Box<dyn tauri::menu::IsMenuItem<Wry>>> {
    // Linux (GTK) has no working native clipboard or undo items in this build, so there the item is a plain
    // command with no shortcut: the keys keep going straight to the text area, and a click runs the page action.
    if cfg!(target_os = "linux") {
        return Ok(Box::new(MenuItem::with_id(app, id, label, true, None::<&str>)?));
    }
    let item = match kind {
        Native::Undo => PredefinedMenuItem::undo(app, Some(label))?,
        Native::Redo => PredefinedMenuItem::redo(app, Some(label))?,
        Native::Cut => PredefinedMenuItem::cut(app, Some(label))?,
        Native::Copy => PredefinedMenuItem::copy(app, Some(label))?,
        Native::Paste => PredefinedMenuItem::paste(app, Some(label))?,
        Native::SelectAll => PredefinedMenuItem::select_all(app, Some(label))?,
    };
    Ok(Box::new(item))
}

fn fill(app: &AppHandle, menu: &Submenu<Wry>, entries: &[E], checks: &mut HashMap<String, CheckMenuItem<Wry>>) -> tauri::Result<()> {
    for entry in entries {
        match entry {
            E::I(id, label, accel) => menu.append(&MenuItem::with_id(app, *id, *label, true, *accel)?)?,
            E::C(id, label, accel, checked) => {
                let item = CheckMenuItem::with_id(app, *id, *label, true, *checked, *accel)?;
                menu.append(&item)?;
                checks.insert((*id).to_string(), item);
            }
            E::N(kind, id, label) => menu.append(native(app, *kind, id, label)?.as_ref())?,
            E::S => menu.append(&PredefinedMenuItem::separator(app)?)?,
            E::M(label, sub) => {
                let submenu = Submenu::new(app, *label, true)?;
                fill(app, &submenu, sub, checks)?;
                menu.append(&submenu)?;
            }
        }
    }
    Ok(())
}

/// Builds the menu bar, installs it and starts forwarding menu commands to the page.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let bar = Menu::new(app)?;
    let mut state = MenuState::default();

    #[cfg(target_os = "macos")]
    {
        // On macOS the first menu is the application menu.
        let app_menu = Submenu::new(app, "BrailleMaker", true)?;
        app_menu.append(&MenuItem::with_id(app, "help.about", "About BrailleMaker", true, None::<&str>)?)?;
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        app_menu.append(&PredefinedMenuItem::hide(app, None)?)?;
        app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
        app_menu.append(&PredefinedMenuItem::show_all(app, None)?)?;
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        app_menu.append(&MenuItem::with_id(app, "file.exit", "Quit BrailleMaker", true, Some("Cmd+Q"))?)?;
        bar.append(&app_menu)?;
    }

    for (label, entries) in MENUS {
        let submenu = Submenu::new(app, *label, true)?;
        fill(app, &submenu, entries, &mut state.checks)?;
        if *label == "&View" {
            state.view = Some(submenu.clone());
        }
        bar.append(&submenu)?;
    }
    app.set_menu(bar)?;
    app.manage(Mutex::new(state));

    app.on_menu_event(|app, event| {
        let id = event.id().as_ref().to_string();
        let state = app.state::<Mutex<MenuState>>();
        let mut checked = None;
        if let Ok(state) = state.lock() {
            if let Some(item) = state.checks.get(&id) {
                // A choice group keeps exactly one item checked; a plain check item toggles.
                if let Some(group) = EXCLUSIVE.iter().find(|g| g.contains(&id.as_str())) {
                    for other in group.iter() {
                        if let Some(o) = state.checks.get(*other) {
                            let _ = o.set_checked(*other == id);
                        }
                    }
                    checked = Some(true);
                } else {
                    checked = item.is_checked().ok();
                }
            }
        }
        let _ = app.emit("menu", MenuEvent { id, checked });
    });
    Ok(())
}

/// The page sets a check mark (for example when the active document changes and its view settings differ).
#[tauri::command]
pub fn set_check(state: tauri::State<'_, Mutex<MenuState>>, id: String, checked: bool) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let item = state.checks.get(&id).ok_or_else(|| format!("no check item {id}"))?;
    if let Some(group) = EXCLUSIVE.iter().find(|g| g.contains(&id.as_str())) {
        if checked {
            for other in group.iter() {
                if let Some(o) = state.checks.get(*other) {
                    o.set_checked(*other == id).map_err(|e| e.to_string())?;
                }
            }
            return Ok(());
        }
    }
    item.set_checked(checked).map_err(|e| e.to_string())
}

/// The page lists the open documents; they appear at the end of the View menu,
/// numbered, with a check mark on the active one. Choosing one sends "view.document.N".
#[tauri::command]
pub fn set_documents(
    app: AppHandle,
    state: tauri::State<'_, Mutex<MenuState>>,
    names: Vec<String>,
    active: usize,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    let view = state.view.clone().ok_or("no View menu")?;
    for old in state.documents.drain(..) {
        view.remove(&old).map_err(|e| e.to_string())?;
    }
    for (i, name) in names.iter().enumerate() {
        let label = if i < 9 { format!("&{} {}", i + 1, name) } else { name.clone() };
        let item = CheckMenuItem::with_id(&app, format!("view.document.{i}"), label, true, i == active, None::<&str>)
            .map_err(|e| e.to_string())?;
        view.append(&item).map_err(|e| e.to_string())?;
        state.documents.push(item);
    }
    Ok(())
}
