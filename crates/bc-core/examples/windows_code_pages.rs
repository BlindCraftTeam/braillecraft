//! Writes what Windows itself does with each character in the code pages formatted text and coded text use, so the
//! tables BrailleCraft carries for them can be made from Windows rather than from other tables of the same names.
//! Windows only: `cargo run --example windows_code_pages -- <folder>`.
//!
//! For each code page, `<page>.encode` has one line per UTF-16 unit that the page can write, "XXXX bytes" (hex), as
//! the original program writes a character of formatted text: one character per call, below code page 50000 with no
//! best-fit replacement (a character for which Windows would use the default character cannot be written), from
//! 50000 up checked by converting the bytes back. `<page>.decode` has one line per byte (and, for a page with lead
//! bytes, per lead and trail byte pair) that Windows reads as characters, "bytes XXXX[ XXXX]", read with invalid
//! characters refused.

#[cfg(windows)]
mod windows {
    use std::io::Write;

    #[allow(non_snake_case)]
    #[link(name = "kernel32")]
    extern "system" {
        fn WideCharToMultiByte(
            CodePage: u32,
            dwFlags: u32,
            lpWideCharStr: *const u16,
            cchWideChar: i32,
            lpMultiByteStr: *mut u8,
            cbMultiByte: i32,
            lpDefaultChar: *const u8,
            lpUsedDefaultChar: *mut i32,
        ) -> i32;
        fn MultiByteToWideChar(
            CodePage: u32,
            dwFlags: u32,
            lpMultiByteStr: *const u8,
            cbMultiByte: i32,
            lpWideCharStr: *mut u16,
            cchWideChar: i32,
        ) -> i32;
        fn IsValidCodePage(CodePage: u32) -> i32;
        fn GetCPInfo(CodePage: u32, lpCPInfo: *mut CpInfo) -> i32;
    }

    #[repr(C)]
    struct CpInfo {
        max_char_size: u32,
        default_char: [u8; 2],
        lead_byte: [u8; 12],
    }

    const WC_NO_BEST_FIT_CHARS: u32 = 0x400;
    const MB_ERR_INVALID_CHARS: u32 = 0x8;

    /// Every code page the original program's formatted text writer gives to Windows (its encoding switch).
    pub const PAGES: [u32; 47] = [
        28591, 28592, 28593, 28594, 28595, 28596, 28597, 28598, 28599, 28600, 874, 28603, 28605, 20866, 21866, 437,
        850, 852, 855, 866, 932, 936, 949, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 65000, 20932,
        10000, 10001, 10002, 10003, 10004, 10005, 10006, 10007, 10021, 10008, 10029, 50222,
    ];
    // (65001 UTF-8 is left out: Windows writes it as the standard says.)
    const _MORE: [u32; 3] = [10082, 10079, 10010];

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }

    fn back(page: u32, bytes: &[u8]) -> Option<Vec<u16>> {
        let mut out = [0u16; 16];
        // SAFETY: the byte slice and the output buffer are live, with their lengths passed.
        let n = unsafe { MultiByteToWideChar(page, 0, bytes.as_ptr(), bytes.len() as i32, out.as_mut_ptr(), 16) };
        (n > 0).then(|| out[..n as usize].to_vec())
    }

    fn encode_one(page: u32, unit: u16) -> Option<Vec<u8>> {
        let mut buf = [0u8; 32];
        let mut used = 0i32;
        let below = page < 50000;
        // SAFETY: one live UTF-16 unit in, a live buffer of 32 bytes out, and a live flag when it is asked for.
        let n = unsafe {
            WideCharToMultiByte(
                page,
                if below { WC_NO_BEST_FIT_CHARS } else { 0 },
                &unit,
                1,
                buf.as_mut_ptr(),
                32,
                std::ptr::null(),
                if below { &mut used } else { std::ptr::null_mut() },
            )
        };
        if n <= 0 || (below && used != 0) {
            return None;
        }
        let bytes = buf[..n as usize].to_vec();
        if !below && back(page, &bytes)? != [unit] {
            return None;
        }
        Some(bytes)
    }

    fn decode_one(page: u32, bytes: &[u8]) -> Option<Vec<u16>> {
        let mut out = [0u16; 8];
        // SAFETY: the byte slice and the output buffer are live, with their lengths passed.
        let n = unsafe {
            MultiByteToWideChar(page, MB_ERR_INVALID_CHARS, bytes.as_ptr(), bytes.len() as i32, out.as_mut_ptr(), 8)
        };
        (n > 0).then(|| out[..n as usize].to_vec())
    }

    pub fn run(folder: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(folder)?;
        let mut summary = std::fs::File::create(folder.join("summary.txt"))?;
        for page in PAGES.iter().chain(_MORE.iter()).copied() {
            // SAFETY: plain calls with a code page number and a live CPINFO.
            let mut info = CpInfo { max_char_size: 0, default_char: [0; 2], lead_byte: [0; 12] };
            let valid = unsafe { IsValidCodePage(page) } != 0 && unsafe { GetCPInfo(page, &mut info) } != 0;
            if !valid {
                writeln!(summary, "{page} not available")?;
                continue;
            }
            let mut enc = std::io::BufWriter::new(std::fs::File::create(folder.join(format!("{page}.encode")))?);
            let mut count = 0;
            for unit in 0u16..=0xffff {
                if let Some(b) = encode_one(page, unit) {
                    writeln!(enc, "{unit:04X} {}", hex(&b))?;
                    count += 1;
                }
            }
            let mut dec = std::io::BufWriter::new(std::fs::File::create(folder.join(format!("{page}.decode")))?);
            let leads: Vec<std::ops::RangeInclusive<u8>> =
                info.lead_byte.chunks(2).take_while(|p| p[0] != 0).map(|p| p[0]..=p[1]).collect();
            let is_lead = |b: u8| leads.iter().any(|r| r.contains(&b));
            let mut decoded = 0;
            if page < 50000 {
                for b in 0u8..=0xff {
                    if is_lead(b) {
                        for t in 0u8..=0xff {
                            if let Some(u) = decode_one(page, &[b, t]) {
                                writeln!(dec, "{} {}", hex(&[b, t]), u.iter().map(|x| format!("{x:04X}")).collect::<Vec<_>>().join(" "))?;
                                decoded += 1;
                            }
                        }
                    } else if let Some(u) = decode_one(page, &[b]) {
                        writeln!(dec, "{} {}", hex(&[b]), u.iter().map(|x| format!("{x:04X}")).collect::<Vec<_>>().join(" "))?;
                        decoded += 1;
                    }
                }
            }
            writeln!(summary, "{page} max {} leads {:?} encode {count} decode {decoded}", info.max_char_size, leads)?;
        }
        Ok(())
    }
}

fn main() {
    let folder = std::env::args().nth(1).unwrap_or_else(|| "code-pages".into());
    #[cfg(windows)]
    {
        if let Err(e) = windows::run(std::path::Path::new(&folder)) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("This writes Windows' own code page tables and runs only on Windows ({folder} not written).");
        std::process::exit(1);
    }
}
