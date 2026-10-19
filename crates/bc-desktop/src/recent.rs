//! The recent documents at the end of the File menu (items 1116 to 1120, after Exit and a separator): the last five
//! document files closed, newest first.
//!
//! A document is put at the front when its window is closed (File, Close and Exit) if it has a document file, and
//! when Save As gives it a new document file name (its old name goes in). It is taken out when that file is opened
//! again and when Save As saves onto its name. A name whose file cannot be opened for reading is not put in; a file
//! that cannot be opened from the menu stays listed until it drops off the end.
//!
//! The list is read at start-up (only once BrailleCraft has ended a run, setting ConfigInUse) and written at Exit,
//! as RecentDocs: on Windows the registry subkey RecentDocs with values "0" to "4", elsewhere a [RecentDocs]
//! section in settings.ini with keys 0 to 4; an empty slot is an empty text.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::settings_store;

/// At most this many documents are listed.
pub const MAX: usize = 5;
/// The menu id of the first entry; the separator before the list is FIRST_ID - 1.
pub const FIRST_ID: i32 = 1116;
const SECTION: &str = "RecentDocs";

/// The list, newest first.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Recent(pub Vec<PathBuf>);

impl Recent {
    /// `path` put at the front, the last entry dropped when the list is full. No entry is taken out for it: the
    /// callers take the name out first where it can be listed.
    pub fn add(&mut self, path: PathBuf) {
        self.0.truncate(MAX - 1);
        self.0.insert(0, path);
    }

    /// Every entry naming `path` taken out.
    pub fn remove(&mut self, path: &Path) {
        self.0.retain(|p| p != path);
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.0.iter().any(|p| p == path)
    }
}

/// The menu label of entry `index` (from 0): its number from 1 and the file name without its folder, every "&"
/// doubled so the menu shows it.
pub fn label(index: usize, path: &Path) -> String {
    let name = path.file_name().map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy());
    format!("{} {}", index + 1, name).replace('&', "&&")
}

thread_local! {
    static LIST: RefCell<Recent> = RefCell::new(Recent::default());
}

/// Reads the list kept by the last run, when there was one that ended (ConfigInUse yes); the entries are taken in
/// order up to the first empty one.
pub fn load() {
    if !settings_store::get(settings_store::ROOT, "ConfigInUse").is_some_and(|v| v.trim().eq_ignore_ascii_case("yes")) {
        return;
    }
    let paths = (0..MAX).map_while(|i| settings_store::get(SECTION, &i.to_string()).filter(|v| !v.is_empty())).map(PathBuf::from);
    LIST.with(|l| l.borrow_mut().0 = paths.collect());
}

/// Writes all five slots (an empty text for each unused one) and ConfigInUse yes, at Exit.
pub fn save() {
    let list = LIST.with(|l| l.borrow().clone());
    for i in 0..MAX {
        let value = list.0.get(i).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        settings_store::set(SECTION, &i.to_string(), &value);
    }
    settings_store::set(settings_store::ROOT, "ConfigInUse", "yes");
}

/// A document file put at the front, when it can be opened for reading.
pub fn add(path: &Path) {
    if !path.as_os_str().is_empty() && std::fs::File::open(path).is_ok() {
        LIST.with(|l| l.borrow_mut().add(path.to_path_buf()));
    }
}

pub fn remove(path: &Path) {
    LIST.with(|l| l.borrow_mut().remove(path));
}

pub fn contains(path: &Path) -> bool {
    LIST.with(|l| l.borrow().contains(path))
}

/// The file of entry `index`.
pub fn entry(index: usize) -> Option<PathBuf> {
    LIST.with(|l| l.borrow().0.get(index).cloned())
}

/// The menu labels, in order.
pub fn labels() -> Vec<String> {
    LIST.with(|l| l.borrow().0.iter().enumerate().map(|(i, p)| label(i, p)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_list_adds_at_the_front_and_drops_the_sixth() {
        let mut r = Recent::default();
        for n in 1..=5 {
            r.add(PathBuf::from(format!("/d/{n}.dxp")));
        }
        assert_eq!(r.0.first(), Some(&PathBuf::from("/d/5.dxp")));
        r.add(PathBuf::from("/d/6.dxp"));
        assert_eq!(r.0.len(), MAX);
        assert_eq!(r.0[0], PathBuf::from("/d/6.dxp"));
        assert!(!r.contains(Path::new("/d/1.dxp")));
        assert_eq!(r.0[4], PathBuf::from("/d/2.dxp"));
    }

    #[test]
    fn recent_list_removes_every_match() {
        let mut r = Recent::default();
        r.add(PathBuf::from("/d/a.dxb"));
        r.add(PathBuf::from("/d/b.dxb"));
        r.add(PathBuf::from("/d/a.dxb"));
        r.remove(Path::new("/d/a.dxb"));
        assert_eq!(r.0, vec![PathBuf::from("/d/b.dxb")]);
    }

    #[test]
    fn recent_menu_label_doubles_ampersands() {
        assert_eq!(label(0, Path::new("/d/Tom & Jerry.dxp")), "1 Tom && Jerry.dxp");
        assert_eq!(label(4, Path::new("book.dxb")), "5 book.dxb");
    }
}
