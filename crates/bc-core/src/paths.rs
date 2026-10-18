//! Where BrailleCraft's programs find their data and keep their settings, shared by the desktop program and the
//! command-line program.

use std::path::{Path, PathBuf};

/// The user's BrailleCraft settings folder: $XDG_CONFIG_HOME/BrailleCraft or ~/.config/BrailleCraft on Linux,
/// ~/Library/Preferences/BrailleCraft on macOS, %APPDATA%\BrailleCraft on Windows (where the settings themselves are
/// in the registry, the folder holds the files beside them).
pub fn settings_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("BrailleCraft"));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    Some(if cfg!(target_os = "macos") {
        home?.join("Library").join("Preferences").join("BrailleCraft")
    } else {
        match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            Some(c) => PathBuf::from(c).join("BrailleCraft"),
            None => home?.join(".config").join("BrailleCraft"),
        }
    })
}

/// A data folder (tables, templates, embossers, ...) found beside the running program: next to the executable, in
/// the macOS bundle's Resources, under ../share/braillecraft, or, for a program run from its build tree, at
/// `source` (a path relative to the workspace root, found by walking up from the executable to the Cargo.toml that
/// holds the workspace). The folder counts when it holds `marker`.
pub fn resource_dir(installed: &str, source: &str, marker: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    let mut candidates = vec![dir.join(installed), dir.join("../Resources").join(installed), dir.join("../share/braillecraft").join(installed)];
    if let Some(root) = workspace_root(&dir) {
        candidates.push(root.join(source));
    }
    candidates.into_iter().find(|d| d.join(marker).is_file())
}

/// The workspace root above a build folder: the nearest ancestor holding a Cargo.toml with a [workspace] table.
fn workspace_root(from: &Path) -> Option<PathBuf> {
    let mut dir = from.to_path_buf();
    for _ in 0..6 {
        let cargo = dir.join("Cargo.toml");
        if std::fs::read_to_string(&cargo).is_ok_and(|t| t.contains("[workspace]")) {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}
