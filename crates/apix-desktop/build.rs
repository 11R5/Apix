fn main() {
    println!("cargo:rerun-if-changed=windows/apix.rc");
    println!("cargo:rerun-if-changed=../../packaging/windows/apix.ico");

    // The app icon and version info go into the exe as Windows resources.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("windows/apix.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("failed to compile windows/apix.rc");
    }
}
