fn main() {
    println!("cargo:rerun-if-changed=windows/apix.rc");
    println!("cargo:rerun-if-changed=../../packaging/windows/apix.ico");
    // The interface texts are compiled in; a changed translation rebuilds the app.
    println!("cargo:rerun-if-changed=locales");

    // The app icon and version info go into the exe as Windows resources,
    // with the version from Cargo.toml (Explorer's Details tab shows it).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let numbers: Vec<&str> = version
            .split(['-', '+'])
            .next()
            .unwrap_or("0.0.0")
            .split('.')
            .collect();
        let commas = format!(
            "{},{},{},0",
            numbers.first().unwrap_or(&"0"),
            numbers.get(1).unwrap_or(&"0"),
            numbers.get(2).unwrap_or(&"0")
        );
        let icon = manifest.join("../../packaging/windows/apix.ico");
        let icon = icon.to_string_lossy().replace('\\', "/");
        let template = std::fs::read_to_string(manifest.join("windows/apix.rc"))
            .expect("windows/apix.rc is readable");
        let rc = template
            .replace("@VERSION_COMMAS@", &commas)
            .replace("@VERSION@", &version)
            .replace("@ICON@", &icon);
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("apix.rc");
        std::fs::write(&out, rc).expect("writing apix.rc");
        embed_resource::compile(&out, embed_resource::NONE)
            .manifest_optional()
            .expect("failed to compile apix.rc");
    }
}
