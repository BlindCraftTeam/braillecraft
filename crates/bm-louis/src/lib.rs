//! BrailleCraft's binding to liblouis 3.39.0 (LGPL 2.1 or later), built from source by build.rs.
//!
//! Translation works in Unicode braille: `translate` returns dot patterns as characters U+2800 to U+283F (U+2800 is
//! the blank cell), and `back_translate` reads them. BrailleCraft turns cells into a braille encoding's characters
//! itself, so the result never depends on the display file a liblouis table happens to include.
//! liblouis keeps global state and is not thread-safe, so every call goes through one lock.

use std::ffi::{c_char, c_int, CStr, CString};
use std::path::Path;
use std::sync::Mutex;

type Widechar = u16;

const DOTS_IO: c_int = 4;
const UC_BRL: c_int = 64;

extern "C" {
    fn lou_version() -> *const c_char;
    fn lou_translateString(
        table_list: *const c_char,
        inbuf: *const Widechar,
        inlen: *mut c_int,
        outbuf: *mut Widechar,
        outlen: *mut c_int,
        typeform: *mut u16,
        spacing: *mut c_char,
        mode: c_int,
    ) -> c_int;
    fn lou_backTranslateString(
        table_list: *const c_char,
        inbuf: *const Widechar,
        inlen: *mut c_int,
        outbuf: *mut Widechar,
        outlen: *mut c_int,
        typeform: *mut u16,
        spacing: *mut c_char,
        mode: c_int,
    ) -> c_int;
}

static LOCK: Mutex<()> = Mutex::new(());

/// The liblouis release built into this program.
pub fn version() -> String {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { CStr::from_ptr(lou_version()) }.to_string_lossy().into_owned()
}

#[cfg(windows)]
extern "C" {
    // The C runtime's own environment, which liblouis reads with getenv.
    fn _putenv_s(name: *const c_char, value: *const c_char) -> c_int;
}

/// Tells liblouis which folder holds its tables, through LOUIS_TABLEPATH (liblouis's supported way; its
/// lou_setDataPath is deprecated). Call once at start, before any translation.
pub fn set_table_folder(dir: &Path) -> Result<(), String> {
    if !dir.join("en-ueb-g2.ctb").is_file() {
        return Err(format!("no liblouis tables in {}", dir.display()));
    }
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("LOUIS_TABLEPATH", dir);
    // On Windows std::env::set_var changes the process environment block, but liblouis reads the C runtime's copy
    // (getenv), which that does not update; set it there too.
    #[cfg(windows)]
    {
        let name = CString::new("LOUIS_TABLEPATH").map_err(|e| e.to_string())?;
        let value = CString::new(dir.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        if unsafe { _putenv_s(name.as_ptr(), value.as_ptr()) } != 0 {
            return Err("could not set LOUIS_TABLEPATH".to_string());
        }
    }
    Ok(())
}

fn run(table: &str, input: &str, back: bool) -> Result<String, String> {
    if input.is_empty() {
        return Ok(String::new());
    }
    let tables = CString::new(table).map_err(|e| e.to_string())?;
    let inbuf: Vec<Widechar> = input.encode_utf16().collect();
    // Contractions shrink text and expansions (capital and number signs, back-translation) grow it; retry larger
    // when liblouis stops before the end of the input.
    let mut capacity = inbuf.len() * 4 + 64;
    for _ in 0..4 {
        let mut inlen = inbuf.len() as c_int;
        let mut outbuf: Vec<Widechar> = vec![0; capacity];
        let mut outlen = capacity as c_int;
        let ok = {
            let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            unsafe {
                let f = if back { lou_backTranslateString } else { lou_translateString };
                f(
                    tables.as_ptr(),
                    inbuf.as_ptr(),
                    &mut inlen,
                    outbuf.as_mut_ptr(),
                    &mut outlen,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    DOTS_IO | UC_BRL,
                )
            }
        };
        if ok == 0 {
            return Err(format!("liblouis could not translate with table {table}"));
        }
        if inlen as usize >= inbuf.len() {
            outbuf.truncate(outlen as usize);
            return Ok(String::from_utf16_lossy(&outbuf));
        }
        capacity *= 4;
    }
    Err("the translation did not fit the output buffer".to_string())
}

/// Translates print text to braille cells (Unicode braille) with the given liblouis table list.
pub fn translate(table: &str, text: &str) -> Result<String, String> {
    run(table, text, false)
}

/// Translates braille cells (Unicode braille) back to print text with the given liblouis table list.
pub fn back_translate(table: &str, braille: &str) -> Result<String, String> {
    run(table, braille, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() {
        set_table_folder(&Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/liblouis/tables")).unwrap();
    }

    #[test]
    fn version_is_the_vendored_release() {
        assert_eq!(version(), "liblouis-3.39.0");
    }

    #[test]
    fn ueb_grade_2_contractions() {
        tables();
        // "the" is one cell (dots 2346); quick = q k; brown = b r ow n; fox is uncontracted.
        assert_eq!(translate("en-ueb-g2.ctb", "the quick brown fox").unwrap(), "⠮⠀⠟⠅⠀⠃⠗⠪⠝⠀⠋⠕⠭");
        // Capital sign (dot 6), the "world" contraction (dots 456 + w), the number sign before 123.
        assert_eq!(translate("en-ueb-g2.ctb", "Hello, World! 123").unwrap(), "⠠⠓⠑⠇⠇⠕⠂⠀⠠⠸⠺⠖⠀⠼⠁⠃⠉");
    }

    #[test]
    fn ueb_grade_2_round_trip() {
        tables();
        for text in ["the quick brown fox", "Hello, World! 123"] {
            let braille = translate("en-ueb-g2.ctb", text).unwrap();
            assert_eq!(back_translate("en-ueb-g2.ctb", &braille).unwrap(), text);
        }
    }

    #[test]
    fn missing_table_folder_is_refused() {
        assert!(set_table_folder(Path::new("/nonexistent")).is_err());
    }
}
