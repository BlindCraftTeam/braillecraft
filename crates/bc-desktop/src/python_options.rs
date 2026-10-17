//! Global, Python Options (command 3110): the dialog the original shows from its XRC resources (audit
//! xrc-dialogs, sections 2 and 4), with its settings kept under the section "Python" with the original's keys and
//! defaults: the custom library path (Lib), Include Built-In Library (IncludeBuiltinLib, yes), the logging level
//! (LogLevel, 3 = Info) and the log file (LogFile). BrailleCraft reads PDFs with its own reader and runs no Python,
//! so these settings have no effect on an import; they are kept as the original keeps them.

use wxdragon::prelude::*;

use crate::settings_store::{get, set};

const SECTION: &str = "Python";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PythonSettings {
    pub library: String,
    pub include_builtin: bool,
    pub log_level: u32,
    pub log_file: String,
}

impl Default for PythonSettings {
    fn default() -> Self {
        PythonSettings { library: String::new(), include_builtin: true, log_level: 3, log_file: String::new() }
    }
}

impl PythonSettings {
    pub fn load() -> PythonSettings {
        let mut s = PythonSettings::default();
        if let Some(v) = get(SECTION, "Lib") {
            s.library = v;
        }
        if let Some(v) = get(SECTION, "IncludeBuiltinLib") {
            s.include_builtin = !v.trim().eq_ignore_ascii_case("no");
        }
        if let Some(v) = get(SECTION, "LogLevel").and_then(|v| v.trim().parse::<u32>().ok()) {
            s.log_level = v.min(4);
        }
        if let Some(v) = get(SECTION, "LogFile") {
            s.log_file = v;
        }
        s
    }

    pub fn save(&self) {
        set(SECTION, "Lib", &self.library);
        set(SECTION, "IncludeBuiltinLib", if self.include_builtin { "yes" } else { "no" });
        set(SECTION, "LogLevel", &self.log_level.to_string());
        set(SECTION, "LogFile", &self.log_file);
    }
}

fn named(d: &crate::dlg::Dlg, name: &str) -> Option<Window> {
    d.dialog.find_window_by_name(name)
}

/// Global, Python Options: the dialog filled from the settings, and the settings kept when it closes with OK.
pub fn dialog() {
    let s = PythonSettings::load();
    let d = crate::dlg::Dlg::open_xrc(&crate::app::frame(), 3110);
    if let Some(t) = named(&d, "PYTHON_LIBRARY").and_then(|w| w.as_widget::<TextCtrl>()) {
        t.set_value(&s.library);
    }
    if let Some(c) = named(&d, "INCLUDE_BUILTIN_PYTHON_LIBRARY").and_then(|w| w.as_widget::<CheckBox>()) {
        c.set_value(s.include_builtin);
    }
    if let Some(c) = named(&d, "CHOICE_PDF_LOGGING_LEVEL").and_then(|w| w.as_widget::<Choice>()) {
        c.set_selection(s.log_level);
    }
    if let Some(p) = named(&d, "FILEPICKER_PDF_LOG").and_then(|w| w.as_widget::<FilePickerCtrl>()) {
        p.set_path(&s.log_file);
    }
    if d.show() == 1 {
        let new = PythonSettings {
            library: named(&d, "PYTHON_LIBRARY").and_then(|w| w.as_widget::<TextCtrl>()).map(|t| t.get_value()).unwrap_or_default(),
            include_builtin: named(&d, "INCLUDE_BUILTIN_PYTHON_LIBRARY").and_then(|w| w.as_widget::<CheckBox>()).is_some_and(|c| c.is_checked()),
            log_level: named(&d, "CHOICE_PDF_LOGGING_LEVEL").and_then(|w| w.as_widget::<Choice>()).and_then(|c| c.get_selection()).unwrap_or(s.log_level).min(4),
            log_file: named(&d, "FILEPICKER_PDF_LOG").and_then(|w| w.as_widget::<FilePickerCtrl>()).map(|p| p.get_path()).unwrap_or_default(),
        };
        new.save();
    }
    d.destroy();
}
