//! Memory given back to the system after a large pass (plan line L1): opening, translating or laying out a book
//! frees most of what it allocated on the way, but the C allocator keeps freed memory in its arenas, so the
//! program's resident size stays at the pass's peak until it is asked to trim. On Linux with the GNU allocator
//! this asks; elsewhere the allocator returns memory itself (or cannot be asked), and the call does nothing.

/// Gives the allocator's freed memory back to the system where that can be asked for.
pub fn release() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // SAFETY: malloc_trim takes no pointers and only walks the allocator's own arenas.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// The resident memory of this process in kilobytes, from the kernel (Linux only; 0 elsewhere): for measurements
/// and the large-document tests.
pub fn resident_kb() -> u64 {
    status_kb("VmRSS:")
}

/// The peak resident memory of this process in kilobytes, from the kernel (Linux only; 0 elsewhere).
pub fn peak_resident_kb() -> u64 {
    status_kb("VmHWM:")
}

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with(field)).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

/// With BRAILLECRAFT_MEMORY set, the resident and peak memory of the process after `step` go to standard error
/// (Linux figures; zero elsewhere), so a long document can be measured step by step (plan line L1).
pub fn note(step: &str) {
    if std::env::var_os("BRAILLECRAFT_MEMORY").is_some() {
        let (now, peak) = (resident_kb() / 1024, peak_resident_kb() / 1024);
        eprintln!("BrailleCraft memory after {step}: {now} MB resident, {peak} MB peak");
    }
}
