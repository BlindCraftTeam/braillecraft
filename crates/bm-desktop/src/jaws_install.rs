//! The JAWS script set (scripts/jaws, shipped beside the program in the folder "jaws") is offered for installing
//! at start-up, on Windows, into every JAWS installation that lacks the current files.
//!
//! What it does, in order:
//! - Finds the JAWS installations: the subkeys of HKEY_LOCAL_MACHINE\Software\Freedom Scientific\JAWS, each named
//!   by its version, version 4.0 and later. Each subkey's value "Target" is the JAWS program folder, which holds
//!   scompile.exe. The settings of a version from 6 on (or with a two-digit number) are in
//!   %APPDATA%\Freedom Scientific\Jaws\<version>; older versions keep them in the program folder.
//! - In each "settings\en?" folder of those (enu and the like), compares every script file with the one shipped:
//!   a file missing there or older than the shipped one means that installation is out of date.
//! - When one is out of date and the question has not been turned off, asks whether to install (the question has
//!   a "Do not ask this again" check box, remembered for this set of script files).
//! - On yes: in each folder, keeps a copy of every file it replaces (name_bak), copies the shipped files, and runs
//!   scompile.exe on each .jss whose .jsb is missing or older than the .jss or the headers and messages it uses,
//!   in that folder, with no window, waiting for it to finish.
//! - Then asks whether to restart BrailleMaker, since JAWS takes up new scripts when the program starts again.
//! Every step is written to install.log in the shipped "jaws" folder, or nowhere when that cannot be written.
#![cfg(windows)]

use std::ffi::OsString;
use std::io::Write;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The question asked before installing (the message of the reference dialog, in BrailleMaker's name).
pub const OFFER_TITLE: &str = "Install JAWS Scripts";
pub const OFFER_TEXT: &str = "You have one or more JAWS® installations without the most up-to-date script files for \
BrailleMaker.  Installing the most recent script files (included with BrailleMaker) will help BrailleMaker to work \
better with JAWS.";
pub const OFFER_QUESTION: &str =
    "Would you like to install the script files?  (Any existing BrailleMaker script files will be backed up.)";
pub const OFFER_DO_NOT_ASK: &str = "Do not ask this again";
/// The question asked after installing.
pub const RESTART_TEXT: &str = "JAWS® will not work with the newly installed script files until BrailleMaker \
restarts.  Would you like to restart BrailleMaker now?";
/// The settings section and the start of the key under which "Do not ask this again" is kept.
pub const SETTINGS_SECTION: &str = "AddIns";

/// The oldest JAWS version the scripts are installed into.
const MIN_VERSION: (u32, u32) = (4, 0);
const JAWS_KEY: &str = "Software\\Freedom Scientific\\JAWS";

/// The answer to the installing question.
pub struct OfferAnswer {
    pub install: bool,
    pub do_not_ask_again: bool,
}

/// One JAWS settings folder ("...\settings\enu") and the compiler of its installation.
struct Target {
    folder: PathBuf,
    compiler: PathBuf,
}

/// The whole check at start-up. `source` is the shipped "jaws" folder. `kept(key)` gives the answer kept when "Do not
/// ask this again" was checked for `key` (Some(true) install, Some(false) do not), and `keep(key, install)` keeps
/// it; a kept answer is used without asking. `offer` shows the installing question and `offer_restart` the restart
/// question. Returns true when the program should restart.
pub fn at_start_up(
    source: &Path,
    kept: impl Fn(&str) -> Option<bool>,
    keep: impl Fn(&str, bool),
    offer: impl FnOnce() -> OfferAnswer,
    offer_restart: impl FnOnce() -> bool,
) -> bool {
    let mut log = Log::open(source);
    log.line("Starting JAWS script check/installation");
    let files = shipped_files(source, &mut log);
    if files.is_empty() {
        return false;
    }
    let key = answer_key(&files);
    let targets = targets(&mut log);
    let out_of_date: Vec<&Target> = targets.iter().filter(|t| !up_to_date(t, &files, &mut log)).collect();
    if out_of_date.is_empty() {
        return false;
    }
    let wanted = match kept(&key) {
        Some(answer) => answer,
        None => {
            let answer = offer();
            if answer.do_not_ask_again {
                keep(&key, answer.install);
            }
            answer.install
        }
    };
    if !wanted {
        return false;
    }
    let mut all = true;
    for t in out_of_date {
        all &= install(t, &files, &mut log);
    }
    all && offer_restart()
}

/// The shipped script files: every .jss with the .jsh, .jsm and other files beside it.
fn shipped_files(source: &Path, log: &mut Log) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(source) {
        Ok(dir) => dir.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect(),
        Err(e) => {
            log.line(&format!("Error file {} is not present ({e})", source.display()));
            return Vec::new();
        }
    };
    files.retain(|p| p.extension().is_some_and(|x| x != "log" && x != "jsb"));
    files.sort();
    for f in &files {
        log.line(&format!("Found file {}", f.display()));
    }
    files
}

/// The key the "Do not ask this again" answer is kept under: "jaws-" and, in eight hexadecimal digits, the newest
/// modification time (seconds since 1970) of the shipped script files, so a new script set asks again.
pub fn answer_key(files: &[PathBuf]) -> String {
    let newest = files
        .iter()
        .filter_map(|f| modified(f))
        .filter_map(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as u32)
        .max()
        .unwrap_or(0);
    format!("jaws-{newest:08x}")
}

fn parse_version(s: &str) -> Option<(u32, u32)> {
    let mut parts = s.splitn(2, '.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts.next().map_or(Some(0), |m| m.trim().parse().ok())?;
    Some((major, minor))
}

/// Every "settings\en?" folder of every JAWS installation from version 4.0 on.
fn targets(log: &mut Log) -> Vec<Target> {
    let mut out = Vec::new();
    let Some(versions) = reg::subkeys(reg::HKEY_LOCAL_MACHINE, JAWS_KEY) else {
        log.line("Cannot read JAWS registry key.");
        return out;
    };
    for (n, version) in versions.iter().enumerate() {
        log.line(&format!("Subkey {n}: \"{version}\""));
        match parse_version(version) {
            Some(v) if v >= MIN_VERSION => {}
            _ => continue,
        }
        let Some(program) = reg::string_value(reg::HKEY_LOCAL_MACHINE, &format!("{JAWS_KEY}\\{version}"), "Target") else {
            log.line(&format!("Cannot read Target from subkey \"{version}\""));
            continue;
        };
        let program = PathBuf::from(program);
        let compiler = program.join("scompile.exe");
        let b = version.as_bytes();
        let user_settings = b.first().is_some_and(|&c| c > b'5') || b.get(1).is_some_and(|&c| c > b'.');
        let root = if user_settings {
            match std::env::var_os("APPDATA") {
                Some(app_data) => PathBuf::from(app_data).join("Freedom Scientific").join("Jaws").join(version),
                None => continue,
            }
        } else {
            program
        };
        let settings = root.join("settings");
        let Ok(dir) = std::fs::read_dir(&settings) else {
            log.line(&format!("No matches found for pattern \"{}\\en?\"", settings.display()));
            continue;
        };
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_ascii_lowercase();
            if entry.path().is_dir() && lower.len() == 3 && lower.starts_with("en") {
                log.line(&format!("Subdirectory \"{}\\{name}\" found", settings.display()));
                out.push(Target { folder: entry.path(), compiler: compiler.clone() });
            }
        }
    }
    out
}

fn modified(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// True when every shipped file is in the folder and not older than the shipped one.
fn up_to_date(t: &Target, files: &[PathBuf], log: &mut Log) -> bool {
    log.line(&format!("Scanning scripts in \"{}\"", t.folder.display()));
    for f in files {
        let dest = t.folder.join(f.file_name().unwrap_or_default());
        match (modified(f), modified(&dest)) {
            (_, None) => {
                log.line(&format!("Destination file \"{}\" not found", dest.display()));
                return false;
            }
            (Some(src), Some(dst)) if dst < src => {
                log.line("A script file is out-of-date");
                return false;
            }
            _ => {}
        }
    }
    true
}

/// Backs up, copies and compiles in one folder; false on the first failure.
fn install(t: &Target, files: &[PathBuf], log: &mut Log) -> bool {
    log.line(&format!("Installing scripts to \"{}\"", t.folder.display()));
    for f in files {
        let name = f.file_name().unwrap_or_default();
        let dest = t.folder.join(name);
        if dest.exists() {
            let mut bak = OsString::from(name);
            bak.push("_bak");
            if let Err(e) = std::fs::copy(&dest, t.folder.join(bak)) {
                log.line(&format!("Backup of \"{}\" failed.  {e}", dest.display()));
                return false;
            }
        }
        if let Err(e) = std::fs::copy(f, &dest) {
            log.line(&format!("CopyFile(\"{}\", \"{}\", FALSE) failed.  {e}", f.display(), dest.display()));
            return false;
        }
    }
    log.line(&format!("Installing scripts to \"{}\": looking at compiler dependencies", t.folder.display()));
    for f in files.iter().filter(|f| f.extension().is_some_and(|x| x == "jss")) {
        let jss = t.folder.join(f.file_name().unwrap_or_default());
        let jsb = jss.with_extension("jsb");
        // The compiled file depends on the script and on every header and message file shipped with it.
        let newest_input = files
            .iter()
            .filter(|d| d.extension().is_some_and(|x| x == "jss" || x == "jsh" || x == "jsm"))
            .filter_map(|d| modified(&t.folder.join(d.file_name().unwrap_or_default())))
            .max();
        match (modified(&jsb), newest_input) {
            (None, _) => log.line(&format!("Target {} does not exist: scompile will be run to create it", jsb.display())),
            (Some(b), Some(i)) if b < i => {
                log.line(&format!("Target {} is out-of-date: scompile will be run to update it", jsb.display()))
            }
            _ => {
                log.line(&format!("Target {} is up-to-date", jsb.display()));
                continue;
            }
        }
        if !compile(&t.compiler, &jss, log) {
            return false;
        }
    }
    log.line(&format!("Installing scripts succeeded to \"{}\"", t.folder.display()));
    true
}

/// Runs "scompile.exe name.jss" in the script's folder, with no window, and waits for it.
fn compile(compiler: &Path, jss: &Path, log: &mut Log) -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let folder = jss.parent().unwrap_or(Path::new("."));
    let name = jss.file_name().unwrap_or_default();
    log.line("Starting process");
    log.line(&format!("  Current dir.: {}", folder.display()));
    log.line(&format!("  Command line: {} {}", compiler.display(), name.to_string_lossy()));
    match std::process::Command::new(compiler).arg(name).current_dir(folder).creation_flags(CREATE_NO_WINDOW).status() {
        Ok(s) => {
            match s.code() {
                Some(c) => log.line(&format!("  Exit status: {c}")),
                None => log.line("  Exit status: Unknown"),
            }
            true
        }
        Err(e) => {
            log.line(&format!("  CreateProcess() failed.  {e}"));
            false
        }
    }
}

/// install.log in the shipped folder.
struct Log(Option<std::fs::File>);

impl Log {
    fn open(source: &Path) -> Log {
        Log(std::fs::OpenOptions::new().create(true).append(true).open(source.join("install.log")).ok())
    }

    fn line(&mut self, text: &str) {
        if let Some(f) = self.0.as_mut() {
            let _ = writeln!(f, "{text}");
        }
    }
}

/// The few registry calls the check needs (advapi32).
mod reg {
    use super::*;
    use std::ffi::c_void;

    pub type Hkey = *mut c_void;
    pub const HKEY_LOCAL_MACHINE: Hkey = 0x8000_0002u32 as usize as Hkey;
    const KEY_READ: u32 = 0x20019;
    /// The 32-bit registry view, where JAWS registers itself (the reference opens its keys with this view).
    const KEY_WOW64_32KEY: u32 = 0x0200;
    const REG_SZ: u32 = 1;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(key: Hkey, sub: *const u16, options: u32, sam: u32, out: *mut Hkey) -> i32;
        fn RegEnumKeyExW(
            key: Hkey,
            index: u32,
            name: *mut u16,
            len: *mut u32,
            reserved: *mut u32,
            class: *mut u16,
            class_len: *mut u32,
            time: *mut c_void,
        ) -> i32;
        fn RegQueryValueExW(key: Hkey, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn open(root: Hkey, path: &str) -> Option<Hkey> {
        let mut key: Hkey = std::ptr::null_mut();
        let path = wide(path);
        (unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ | KEY_WOW64_32KEY, &mut key) } == 0).then_some(key)
    }

    pub fn subkeys(root: Hkey, path: &str) -> Option<Vec<String>> {
        let key = open(root, path)?;
        let mut out = Vec::new();
        let mut i = 0;
        loop {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            let r = unsafe {
                RegEnumKeyExW(key, i, buf.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            if r != 0 {
                break;
            }
            out.push(OsString::from_wide(&buf[..len as usize]).to_string_lossy().into_owned());
            i += 1;
        }
        unsafe { RegCloseKey(key) };
        Some(out)
    }

    pub fn string_value(root: Hkey, path: &str, name: &str) -> Option<String> {
        let key = open(root, path)?;
        let mut buf = [0u16; 512];
        let mut len = (buf.len() * 2) as u32;
        let mut kind = 0u32;
        let r = unsafe {
            RegQueryValueExW(key, wide(name).as_ptr(), std::ptr::null_mut(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut len)
        };
        unsafe { RegCloseKey(key) };
        if r != 0 || kind != REG_SZ {
            return None;
        }
        let n = (len as usize / 2).min(buf.len());
        let s = OsString::from_wide(&buf[..n]).to_string_lossy().trim_end_matches('\0').to_string();
        Some(s)
    }
}
