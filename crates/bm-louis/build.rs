//! Builds liblouis 3.39.0 from vendor/liblouis with the cc crate, the way its own Windows makefile does:
//! the eight library sources, liblouis.h made from liblouis.h.in with 16-bit characters (liblouis's default), and a
//! minimal config.h. The tables are not compiled in; the program ships vendor/liblouis/tables and points liblouis at
//! them at run time (set_data_path).

use std::env;
use std::fs;
use std::path::PathBuf;

const SOURCES: &[&str] = &[
    "commonTranslationFunctions.c",
    "compileTranslationTable.c",
    "logging.c",
    "lou_backTranslateString.c",
    "lou_translateString.c",
    "metadata.c",
    "pattern.c",
    "utils.c",
];

fn main() {
    let src = PathBuf::from("vendor/liblouis/liblouis");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let header = fs::read_to_string(src.join("liblouis.h.in"))
        .expect("liblouis.h.in")
        .replace("@WIDECHAR_TYPE@", "unsigned short int");
    fs::write(out.join("liblouis.h"), header).expect("write liblouis.h");
    // TABLESDIR is the built-in table folder on Linux and macOS; BrailleCraft always sets the data path instead.
    fs::write(out.join("config.h"), "#define PACKAGE_VERSION \"liblouis-3.39.0\"\n#define TABLESDIR \"\"\n")
        .expect("write config.h");

    let mut build = cc::Build::new();
    build.include(&out).include(&src).define("_EXPORTING", None).warnings(false);
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // liblouis uses C99 variable-length arrays, which the Microsoft C compiler does not accept; liblouis's own
        // Windows makefile (windows/Makefile.nmake) builds with clang-cl for that reason, and so does this.
        build.compiler("clang-cl");
        build.define("_CRT_SECURE_NO_WARNINGS", None).define("_CRT_NONSTDC_NO_DEPRECATE", None);
    }
    for file in SOURCES {
        build.file(src.join(file));
    }
    build.compile("louis");

    println!("cargo:rerun-if-changed=vendor/liblouis/liblouis");
    println!("cargo:rerun-if-changed=build.rs");
}
