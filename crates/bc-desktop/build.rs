//! Records when the program was built (seconds since 1970, UTC) for the About box: SOURCE_DATE_EPOCH when the build
//! sets it (reproducible builds), else the time of this build. On Windows it also embeds the program's manifest
//! (packaging/windows/braillecraft.manifest: Common Controls 6, the DPI awareness and the supported systems the
//! reference's own manifest gives); without one wxWidgets warns at every start and the controls take the old look.

fn main() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let epoch = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    });
    println!("cargo:rustc-env=BC_BUILD_EPOCH={epoch}");

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/braillecraft.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        if let Err(e) = embed_manifest::embed_manifest_file(&manifest) {
            panic!("the Windows manifest {} could not be embedded: {e}", manifest.display());
        }
    }
}
