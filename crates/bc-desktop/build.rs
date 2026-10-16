//! Records when the program was built (seconds since 1970, UTC) for the About box: SOURCE_DATE_EPOCH when the build
//! sets it (reproducible builds), else the time of this build.

fn main() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let epoch = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    });
    println!("cargo:rustc-env=BC_BUILD_EPOCH={epoch}");
}
